use super::*;
use std::cell::RefCell;

#[test]
fn registry_system_adapter_rejects_unconfirmed_terminal_actions_before_native_calls() {
    let service = SystemActions::default();
    for id in ["lock", "sleep"] {
        assert_eq!(
            service
                .execute_registered(serde_json::json!({"id":id,"args":{}}), false)
                .unwrap_err()
                .code,
            ErrorCode::ConfirmationRequired
        );
    }
    assert_eq!(
        service
            .execute_registered(
                serde_json::json!({"id":"open_settings","args":{"page":"ms-settings:privacy"}}),
                false
            )
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
}
#[derive(Default)]
struct Fake {
    calls: RefCell<Vec<String>>,
    fail: bool,
}
impl Fake {
    fn call(&self, name: &str) -> Result<Option<String>> {
        self.calls.borrow_mut().push(name.into());
        if self.fail {
            Err(ActionError::new(ErrorCode::Restricted, "blocked by policy"))
        } else {
            Ok(None)
        }
    }
}
impl Operations for Fake {
    fn settings(&self, page: SettingsPage) -> Result<Option<String>> {
        self.call(page.uri())
    }
    fn lock(&self) -> Result<Option<String>> {
        self.call("mock-lock")
    }
    fn sleep(&self) -> Result<Option<String>> {
        self.call("mock-sleep")
    }
}
fn sleep() -> SystemRequest {
    SystemRequest::Sleep(Empty {})
}
fn ticket(outcome: ActionOutcome) -> String {
    match outcome {
        ActionOutcome::ConfirmationRequired { token, .. } => token,
        _ => panic!("expected confirmation"),
    }
}
#[test]
fn only_allowlisted_pages_reach_dispatch() {
    let service = SystemActions::default();
    let fake = Fake::default();
    for (page, uri) in [
        ("sound", "ms-settings:sound"),
        ("wifi", "ms-settings:network-wifi"),
        ("bluetooth", "ms-settings:bluetooth"),
        ("display", "ms-settings:display"),
    ] {
        let action = parse(serde_json::json!({"id":"open_settings","args":{"page":page}})).unwrap();
        assert!(matches!(
            service.dispatch(action, None, &fake).unwrap(),
            ActionOutcome::Accepted { .. }
        ));
        assert_eq!(fake.calls.borrow().last().unwrap(), uri);
    }
    for value in [
        serde_json::json!({"id":"open_settings","args":{"page":"ms-settings:privacy"}}),
        serde_json::json!({"id":"open_settings","args":{"page":"sound","uri":"cmd.exe"}}),
        serde_json::json!({"id":"sleep","args":{"confirmed":true}}),
        serde_json::json!({"id":"lock","args":{},"shell":"calc"}),
        serde_json::json!({"id":"shutdown","args":{}}),
        serde_json::json!({"id":"restart","args":{}}),
    ] {
        assert_eq!(parse(value).unwrap_err().code, ErrorCode::InvalidInput);
    }
}
#[test]
fn sleep_requires_current_single_use_confirmation_before_any_operation() {
    let service = SystemActions::default();
    let fake = Fake::default();
    assert!(service.dispatch(sleep(), Some("forged"), &fake).is_err());
    let token = ticket(service.dispatch(sleep(), None, &fake).unwrap());
    assert!(fake.calls.borrow().is_empty());
    assert!(service.dispatch(sleep(), Some(&token), &fake).is_ok());
    assert_eq!(&*fake.calls.borrow(), &["mock-sleep"]);
    assert!(service.dispatch(sleep(), Some(&token), &fake).is_err());
    assert_eq!(fake.calls.borrow().len(), 1);
}
#[test]
fn cancelled_replaced_and_expired_tickets_never_sleep() {
    let service = SystemActions::default();
    let fake = Fake::default();
    let now = Instant::now();
    let old = ticket(service.dispatch_at(sleep(), None, &fake, now).unwrap());
    let current = ticket(service.dispatch_at(sleep(), None, &fake, now).unwrap());
    assert!(service.dispatch(sleep(), Some(&old), &fake).is_err());
    service.cancel(&old).unwrap(); // A stale cancellation must not clear the new prompt.
    assert!(
        service
            .dispatch_at(sleep(), Some(&current), &fake, now + CONFIRMATION_TTL)
            .is_err()
    );
    let cancel = ticket(service.dispatch(sleep(), None, &fake).unwrap());
    service.cancel(&cancel).unwrap();
    assert!(service.dispatch(sleep(), Some(&cancel), &fake).is_err());
    assert!(fake.calls.borrow().is_empty());
}
#[test]
fn lock_dispatch_and_failures_are_mocked_never_native() {
    let service = SystemActions::default();
    let fake = Fake::default();
    assert!(
        service
            .dispatch(SystemRequest::Lock(Empty {}), None, &fake)
            .is_ok()
    );
    assert_eq!(&*fake.calls.borrow(), &["mock-lock"]);
    let blocked = Fake {
        fail: true,
        ..Default::default()
    };
    let token = ticket(service.dispatch(sleep(), None, &blocked).unwrap());
    assert_eq!(
        service
            .dispatch(sleep(), Some(&token), &blocked)
            .unwrap_err()
            .code,
        ErrorCode::Restricted
    );
    assert!(service.dispatch(sleep(), Some(&token), &blocked).is_err());
    assert_eq!(blocked.calls.borrow().len(), 1);
}
#[test]
fn prompt_does_not_hide_and_hide_failure_remains_accepted() {
    let prompt = ActionOutcome::ConfirmationRequired {
        token: "x".into(),
        message: "confirm".into(),
    };
    assert!(matches!(
        finish(prompt, || panic!("must stay visible")),
        ActionOutcome::ConfirmationRequired { .. }
    ));
    assert!(matches!(
        finish(
            ActionOutcome::Accepted {
                message: "accepted".into(),
                warning: None
            },
            || Err("failed".into())
        ),
        ActionOutcome::Accepted {
            warning: Some(_),
            ..
        }
    ));
}
