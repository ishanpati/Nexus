use super::*;
use serde_json::json;

#[derive(Default)]
struct Fake {
    calls: Vec<String>,
    fail: bool,
}
impl Fake {
    fn record(&mut self, value: String) -> Result<Value> {
        self.calls.push(value);
        if self.fail {
            Err(RegistryError::new(
                ErrorCode::ExecutionFailed,
                "Mock Windows restriction",
            ))
        } else {
            Ok(json!({ "ok": true }))
        }
    }
}
impl ActionRuntime for Fake {
    fn check_application(&mut self, id: &str) -> Result<()> {
        if id == "catalog:app" {
            Ok(())
        } else {
            Err(RegistryError::new(
                ErrorCode::InvalidReference,
                "App is not in the catalog",
            ))
        }
    }
    fn inspect_path(&mut self, path: &str, directory: bool) -> Result<bool> {
        if directory && path == r"C:\Folder" {
            Ok(false)
        } else if !directory && path == r"C:\safe.txt" {
            Ok(false)
        } else if !directory && path == r"C:\sensitive.exe" {
            Ok(true)
        } else {
            Err(RegistryError::new(
                ErrorCode::InvalidReference,
                "Missing or wrong-kind path",
            ))
        }
    }
    fn launch_application(&mut self, id: &str) -> Result<Value> {
        self.record(format!("app:{id}"))
    }
    fn open_path(&mut self, path: &str, directory: bool, confirmed: bool) -> Result<Value> {
        self.record(format!("path:{path}:{directory}:{confirmed}"))
    }
    fn audio(&mut self, request: crate::audio::AudioRequest) -> Result<Value> {
        self.record(format!("audio:{request:?}"))
    }
    fn settings(&mut self, page: &str) -> Result<Value> {
        self.record(format!("settings:{page}"))
    }
    fn lock(&mut self) -> Result<Value> {
        self.record("mock-lock".into())
    }
    fn sleep(&mut self) -> Result<Value> {
        self.record("mock-sleep".into())
    }
}
fn request(id: &str, parameters: Value) -> ActionRequest {
    ActionRequest {
        action_id: id.into(),
        schema_version: 1,
        parameters,
    }
}
fn token(outcome: DispatchOutcome) -> String {
    match outcome {
        DispatchOutcome::ConfirmationRequired { token, .. } => token,
        _ => panic!("confirmation required"),
    }
}
fn cases() -> Vec<(&'static str, Value)> {
    vec![
        ("app.launch", json!({"application_id":"catalog:app"})),
        ("file.open", json!({"path":r"C:\safe.txt"})),
        ("folder.open", json!({"path":r"C:\Folder"})),
        ("audio.set_volume", json!({"percent":35})),
        ("audio.adjust_volume", json!({"delta":-10})),
        ("audio.set_mute", json!({"muted":true})),
        ("settings.open", json!({"page":"display"})),
        ("system.lock", json!({})),
        ("system.sleep", json!({})),
    ]
}

#[test]
fn builtin_ids_metadata_and_policy_are_stable_and_serializable() {
    let registry = ActionRegistry::default();
    let descriptors = registry.descriptors();
    let ids: HashSet<_> = descriptors.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, cases().iter().map(|(id, _)| *id).collect());
    assert_eq!(ids.len(), 9);
    let value = serde_json::to_value(&descriptors).unwrap();
    for descriptor in value.as_array().unwrap() {
        assert_eq!(descriptor["schema_version"], 1);
        assert_eq!(descriptor["workflow_compatible"], true);
        assert!(descriptor.get("execute").is_none());
        assert!(descriptor.get("validate").is_none());
        assert!(descriptor["icon"].is_string());
    }
    for id in ["system.lock", "system.sleep"] {
        let descriptor = registry.descriptor(id).unwrap();
        assert_eq!(descriptor.confirmation, ConfirmationPolicy::Always);
        assert!(descriptor.terminal);
    }
    assert_eq!(
        registry.descriptor("file.open").unwrap().confirmation,
        ConfirmationPolicy::SensitiveTarget
    );
    assert!(matches!(
        ActionRegistry::new(vec![test_entry(), test_entry()]),
        Err(RegistryError {
            code: ErrorCode::InvalidRegistration,
            ..
        })
    ));
}

#[test]
fn valid_parameters_and_native_adapter_selection_for_every_registration() {
    let registry = ActionRegistry::default();
    let mut runtime = Fake::default();
    for (id, args) in cases() {
        let request = request(id, args);
        registry.validate(&request).unwrap();
        let first = registry
            .dispatch(&request, &mut runtime, StepPosition::Final, None)
            .unwrap();
        if matches!(first, DispatchOutcome::ConfirmationRequired { .. }) {
            let token = token(first);
            registry
                .dispatch(&request, &mut runtime, StepPosition::Final, Some(&token))
                .unwrap();
        }
    }
    assert_eq!(runtime.calls.len(), 9);
    assert_eq!(runtime.calls[0], "app:catalog:app");
    assert_eq!(runtime.calls[1], r"path:C:\safe.txt:false:false");
    assert_eq!(runtime.calls[2], r"path:C:\Folder:true:false");
    assert!(runtime.calls[3].contains("SetVolume(Percent { percent: 35 })"));
    assert!(runtime.calls[4].contains("AdjustVolume(Delta { delta: -10 })"));
    assert!(runtime.calls[5].contains("SetMute(Mute { muted: true })"));
    assert_eq!(
        &runtime.calls[6..],
        &["settings:display", "mock-lock", "mock-sleep"]
    );
}

#[test]
fn missing_wrong_types_unknown_fields_and_out_of_range_are_rejected() {
    let registry = ActionRegistry::default();
    for (id, valid) in cases() {
        for parameters in [
            Value::Null,
            json!([]),
            json!("text"),
            json!({"confirmed":true}),
            json!({"terminal":false}),
            json!({"confirmation":"none"}),
        ] {
            assert_eq!(
                registry
                    .validate(&request(id, parameters))
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidParameters
            );
        }
        for field in valid.as_object().unwrap().keys() {
            for bad in [Value::Null, json!([]), json!({}), json!("")] {
                let mut parameters = valid.clone();
                parameters[field] = bad;
                assert!(
                    registry.validate(&request(id, parameters)).is_err(),
                    "{id}:{field}"
                );
            }
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert_eq!(
                registry
                    .validate(&request(id, missing))
                    .unwrap_err()
                    .parameter
                    .as_deref(),
                Some(field.as_str())
            );
        }
    }
    for value in [
        json!(-1),
        json!(101),
        json!(35.5),
        json!("35"),
        json!(true),
        json!("NaN"),
        json!("Infinity"),
    ] {
        assert!(
            registry
                .validate(&request("audio.set_volume", json!({"percent":value})))
                .is_err()
        );
    }
    for value in [json!(-101), json!(101), json!(0.1), json!("+10")] {
        assert!(
            registry
                .validate(&request("audio.adjust_volume", json!({"delta":value})))
                .is_err()
        );
    }
    assert!(
        registry
            .validate(&request("audio.set_mute", json!({"muted":1})))
            .is_err()
    );
    for page in ["ms-settings:sound", "privacy", "cmd.exe", "display && calc"] {
        assert!(
            registry
                .validate(&request("settings.open", json!({"page":page})))
                .is_err()
        );
    }
    assert!(
        serde_json::from_str::<ActionRequest>(
            r#"{"action_id":"audio.set_volume","schema_version":1,"parameters":{"percent":NaN}}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<ActionRequest>(
            json!({"action_id":"system.sleep","schema_version":1,"parameters":{},"confirmed":true})
        )
        .is_err()
    );
    assert_eq!(
        registry
            .validate(&request("audio.set_volume", json!({"percent":35.0})))
            .unwrap()
            .parameters["percent"],
        35
    );
}

#[test]
fn unknown_ids_versions_and_references_fail_before_execution() {
    let registry = ActionRegistry::default();
    let mut runtime = Fake::default();
    for id in [
        "unknown.action",
        "cmd.exe",
        "system.shutdown",
        "audio.set_volume ",
        "",
    ] {
        assert_eq!(
            registry
                .dispatch(
                    &request(id, json!({})),
                    &mut runtime,
                    StepPosition::Final,
                    None
                )
                .unwrap_err()
                .code,
            ErrorCode::UnknownAction
        );
    }
    let mut action = request("system.sleep", json!({}));
    action.schema_version = 2;
    assert_eq!(
        registry.validate(&action).unwrap_err().code,
        ErrorCode::UnsupportedVersion
    );
    for (id, args) in [
        ("app.launch", json!({"application_id":"C:\\arbitrary.exe"})),
        ("file.open", json!({"path":r"C:\Folder"})),
        ("folder.open", json!({"path":r"C:\safe.txt"})),
    ] {
        assert_eq!(
            registry
                .dispatch(&request(id, args), &mut runtime, StepPosition::Final, None)
                .unwrap_err()
                .code,
            ErrorCode::InvalidReference
        );
    }
    assert!(runtime.calls.is_empty());
}

#[test]
fn terminal_confirmation_is_final_only_single_use_and_bound_to_action() {
    let registry = ActionRegistry::default();
    let mut runtime = Fake::default();
    for id in ["system.lock", "system.sleep"] {
        let action = request(id, json!({}));
        assert_eq!(
            registry
                .dispatch(&action, &mut runtime, StepPosition::Intermediate, None)
                .unwrap_err()
                .code,
            ErrorCode::InvalidPosition
        );
        let before = runtime.calls.len();
        let ticket = token(
            registry
                .dispatch(&action, &mut runtime, StepPosition::Final, None)
                .unwrap(),
        );
        assert_eq!(runtime.calls.len(), before);
        let other = request(
            if id == "system.lock" {
                "system.sleep"
            } else {
                "system.lock"
            },
            json!({}),
        );
        assert!(
            registry
                .dispatch(&other, &mut runtime, StepPosition::Final, Some(&ticket))
                .is_err()
        );
        assert!(
            registry
                .dispatch(
                    &action,
                    &mut runtime,
                    StepPosition::Intermediate,
                    Some(&ticket)
                )
                .is_err()
        );
        registry
            .dispatch(&action, &mut runtime, StepPosition::Final, Some(&ticket))
            .unwrap();
        assert!(
            registry
                .dispatch(&action, &mut runtime, StepPosition::Final, Some(&ticket))
                .is_err()
        );
        assert_eq!(runtime.calls.len(), before + 1);
    }
}

#[test]
fn sensitive_paths_cancel_expiry_changed_parameters_and_failure_cannot_bypass_confirmation() {
    let registry = ActionRegistry::default();
    let mut runtime = Fake::default();
    let action = request("file.open", json!({"path":r"C:\sensitive.exe"}));
    let now = Instant::now();
    let ticket = token(
        registry
            .dispatch_at(&action, &mut runtime, StepPosition::Final, None, now)
            .unwrap(),
    );
    assert!(runtime.calls.is_empty());
    assert!(
        registry
            .dispatch_at(
                &action,
                &mut runtime,
                StepPosition::Final,
                Some(&ticket),
                now + Duration::from_secs(60)
            )
            .is_err()
    );
    let ticket = token(
        registry
            .dispatch(&action, &mut runtime, StepPosition::Final, None)
            .unwrap(),
    );
    let other = request("file.open", json!({"path":r"C:\safe.txt"}));
    assert!(
        registry
            .dispatch(&other, &mut runtime, StepPosition::Final, Some(&ticket))
            .is_err()
    );
    registry.cancel_confirmation(&ticket).unwrap();
    assert!(
        registry
            .dispatch(&action, &mut runtime, StepPosition::Final, Some(&ticket))
            .is_err()
    );
    let ticket = token(
        registry
            .dispatch(&action, &mut runtime, StepPosition::Final, None)
            .unwrap(),
    );
    runtime.fail = true;
    assert_eq!(
        registry
            .dispatch(&action, &mut runtime, StepPosition::Final, Some(&ticket))
            .unwrap_err()
            .code,
        ErrorCode::ExecutionFailed
    );
    assert!(
        registry
            .dispatch(&action, &mut runtime, StepPosition::Final, Some(&ticket))
            .is_err()
    );
    assert_eq!(runtime.calls, [r"path:C:\sensitive.exe:false:true"]);
}

fn test_entry() -> Registration {
    Registration {
        descriptor: ActionDescriptor {
            id: "test.echo".into(),
            display_name: "Echo".into(),
            description: "Test-only adapter".into(),
            category: "Tests".into(),
            icon: Icon::File,
            parameters: vec![
                Parameter::required("text", "Text", ParameterType::Text { max_length: 20 }),
                Parameter {
                    key: "optional".into(),
                    label: "Optional".into(),
                    kind: ParameterType::Boolean,
                    required: false,
                    default: None,
                },
                Parameter {
                    key: "factor".into(),
                    label: "Factor".into(),
                    kind: ParameterType::Number {
                        min: 0.0,
                        max: 1.0,
                        integer: false,
                    },
                    required: false,
                    default: Some(json!(0.5)),
                },
            ],
            workflow_compatible: true,
            confirmation: ConfirmationPolicy::None,
            terminal: false,
            schema_version: 3,
        },
        validate: |_, _| Ok(false),
        execute: |args, _, _| Ok(Value::Object(args.clone())),
    }
}

#[test]
fn extension_with_existing_types_optional_fields_defaults_needs_only_registration() {
    let registry = ActionRegistry::new(vec![test_entry()]).unwrap();
    let mut action = request("test.echo", json!({"text":"資料 hello"}));
    action.schema_version = 3;
    let validated = registry.validate(&action).unwrap();
    assert_eq!(validated.parameters["factor"], 0.5);
    assert!(!validated.parameters.contains_key("optional"));
    let outcome = registry
        .dispatch(
            &action,
            &mut Fake::default(),
            StepPosition::Intermediate,
            None,
        )
        .unwrap();
    assert!(
        matches!(outcome, DispatchOutcome::Succeeded { output } if output["text"] == "資料 hello")
    );
    for text in [
        "",
        "a\0b",
        "a\nb",
        "this is much too long for this parameter",
    ] {
        action.parameters = json!({"text":text});
        assert!(registry.validate(&action).is_err());
    }
    let mut invalid_default = test_entry();
    invalid_default.descriptor.parameters[2].default = Some(json!(9));
    assert!(ActionRegistry::new(vec![invalid_default]).is_err());
    let mut hidden = test_entry();
    hidden.descriptor.workflow_compatible = false;
    let registry = ActionRegistry::new(vec![hidden]).unwrap();
    assert!(registry.descriptors().is_empty());
    assert_eq!(
        registry.validate(&action).unwrap_err().code,
        ErrorCode::UnsupportedAction
    );
}

#[test]
fn native_reference_checks_fail_closed_without_launching_anything() {
    let catalog = crate::installed_apps::CatalogCache::default();
    let audio = crate::audio::AudioService::default();
    let system = crate::system_actions::SystemActions::default();
    let activity = std::sync::Arc::new(crate::installed_apps::launching::LaunchActivity::default());
    let mut runtime = native::NativeRuntime {
        catalog: &catalog,
        audio: &audio,
        system: &system,
        activity: &activity,
        launch_hide: None,
        launch_cancelled: None,
    };
    assert!(runtime.check_application("cmd.exe").is_err());
    for path in [
        "relative.txt",
        "https://example.com",
        r"C:\NUL.txt",
        r"C:\missing-nexus-stage-a-9276.txt",
    ] {
        assert!(runtime.inspect_path(path, false).is_err());
    }
    // No native audio, lock or sleep calls occur in this test.
}
