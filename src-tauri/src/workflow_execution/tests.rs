use super::*;
use crate::{
    action_registry::{ErrorCode, RegistryError},
    workflows::{Payload, Step, Store},
};
use serde_json::{Value, json};
use std::sync::{
    Barrier,
    mpsc::{self, Receiver},
};

type Hook = Box<dyn FnMut(&str) + Send>;
#[derive(Default)]
struct Fake {
    calls: Arc<Mutex<Vec<String>>>,
    fail_at: Option<usize>,
    hook: Option<Hook>,
    checks: usize,
    missing_on_check: Option<usize>,
}
impl Fake {
    fn check(&mut self, name: &str) -> crate::action_registry::Result<()> {
        self.checks += 1;
        if let Some(hook) = &mut self.hook {
            hook("check");
        }
        if name == "missing" || self.missing_on_check == Some(self.checks) {
            Err(RegistryError::new(
                ErrorCode::InvalidReference,
                "Mock target is unavailable",
            ))
        } else {
            Ok(())
        }
    }
    fn record(&mut self, name: &str) -> crate::action_registry::Result<Value> {
        let count = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(name.into());
            calls.len()
        };
        if let Some(hook) = &mut self.hook {
            hook(name);
        }
        if self.fail_at == Some(count) {
            Err(RegistryError::new(
                ErrorCode::ExecutionFailed,
                "Mock Windows failure",
            ))
        } else {
            Ok(json!({"accepted":true,"readiness_verified":false}))
        }
    }
}
impl ActionRuntime for Fake {
    fn check_application(&mut self, id: &str) -> crate::action_registry::Result<()> {
        self.check(id)
    }
    fn inspect_path(&mut self, path: &str, _: bool) -> crate::action_registry::Result<bool> {
        self.check(if path.contains("missing") {
            "missing"
        } else {
            path
        })?;
        Ok(path.ends_with(".exe") || path.ends_with(".ps1"))
    }
    fn launch_application(&mut self, id: &str) -> crate::action_registry::Result<Value> {
        self.record(id)
    }
    fn open_path(
        &mut self,
        _: &str,
        _: bool,
        confirmed: bool,
    ) -> crate::action_registry::Result<Value> {
        assert!(
            !confirmed,
            "workflow must never authorize raw file execution"
        );
        self.record("file")
    }
    fn audio(&mut self, _: crate::audio::AudioRequest) -> crate::action_registry::Result<Value> {
        self.record("audio")
    }
    fn settings(&mut self, page: &str) -> crate::action_registry::Result<Value> {
        self.record(page)
    }
    fn lock(&mut self) -> crate::action_registry::Result<Value> {
        self.record("mock-lock")
    }
    fn sleep(&mut self) -> crate::action_registry::Result<Value> {
        self.record("mock-sleep")
    }
}
fn workflow(actions: Vec<(&str, Value)>) -> Workflow {
    Workflow {
        data: Payload {
            id: Uuid::new_v4(),
            name: "Test workflow".into(),
            description: None,
            aliases: vec![],
            format_version: 1,
            steps: actions
                .into_iter()
                .enumerate()
                .map(|(i, (id, parameters))| Step {
                    id: Uuid::new_v4(),
                    action_id: id.into(),
                    action_schema_version: 1,
                    parameters,
                    order: i as u32,
                    label: None,
                })
                .collect(),
        },
        revision: 1,
        created_at: 0,
        updated_at: 0,
    }
}
fn three() -> Workflow {
    workflow(vec![
        ("settings.open", json!({"page":"sound"})),
        ("settings.open", json!({"page":"wifi"})),
        ("settings.open", json!({"page":"display"})),
    ])
}
fn sink() -> (Sink, Receiver<Progress>) {
    let (tx, rx) = mpsc::channel();
    (
        Arc::new(move |event| {
            let _ = tx.send(event);
        }),
        rx,
    )
}
fn sync_run(w: Workflow, fake: &mut Fake) -> (Snapshot, Vec<Progress>) {
    let engine = Engine::default();
    let (sink, events) = sink();
    let run = engine.reserve(w.data.id, sink).unwrap();
    execute(&run, w, ActionRegistry::default(), fake, CONFIRMATION_TTL);
    (run.snapshot(), events.try_iter().collect())
}
fn start(
    engine: &Engine,
    w: Workflow,
    mut fake: Fake,
    ttl: Duration,
) -> (Snapshot, Receiver<Progress>) {
    let (sink, events) = sink();
    let initial = engine
        .launch(w.data.id, sink, move |run| {
            execute(&run, w, ActionRegistry::default(), &mut fake, ttl);
        })
        .unwrap();
    (initial, events)
}
fn until(events: &Receiver<Progress>, kind: EventKind) -> Progress {
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(5))
            .expect("worker event deadline");
        if event.kind == kind {
            return event;
        }
    }
}

#[test]
fn successful_three_steps_saved_order_and_terminal_snapshot() {
    let mut w = three();
    w.data.steps.swap(0, 2); // saved order field remains authoritative
    let mut fake = Fake::default();
    let (s, events) = sync_run(w, &mut fake);
    assert_eq!(*fake.calls.lock().unwrap(), ["sound", "wifi", "display"]);
    assert_eq!(s.status, Status::Succeeded);
    assert!(
        s.steps
            .iter()
            .all(|s| s.status == Status::Succeeded && s.error.is_none())
    );
    assert_eq!(
        events.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [
            EventKind::RunStarted,
            EventKind::StepStarted,
            EventKind::StepSucceeded,
            EventKind::StepStarted,
            EventKind::StepSucceeded,
            EventKind::StepStarted,
            EventKind::StepSucceeded,
            EventKind::RunSucceeded,
        ]
    );
    assert!(
        events
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    assert!(events.iter().all(|e| e.run_id == s.run_id));
    assert_eq!(events.last().unwrap().sequence, s.sequence);
    assert_eq!(
        s.steps[0].output.as_ref().unwrap()["readiness_verified"],
        false
    );
    let event_json = serde_json::to_string(&events).unwrap();
    assert!(!event_json.contains("parameters"));
}

#[test]
fn failure_stops_later_steps_without_rollback() {
    let mut fake = Fake {
        fail_at: Some(2),
        ..Default::default()
    };
    let (s, events) = sync_run(three(), &mut fake);
    assert_eq!(*fake.calls.lock().unwrap(), ["sound", "wifi"]);
    assert_eq!(s.status, Status::Failed);
    assert_eq!(
        s.steps.iter().map(|s| s.status).collect::<Vec<_>>(),
        [Status::Succeeded, Status::Failed, Status::Cancelled]
    );
    assert_eq!(s.steps[1].error.as_ref().unwrap().code, "execution_failed");
    assert_eq!(events[events.len() - 2].kind, EventKind::StepFailed);
    assert_eq!(events.last().unwrap().kind, EventKind::RunFailed);
}

#[test]
fn entire_plan_is_validated_before_any_side_effect() {
    for (id, args, expected) in [
        ("unknown.action", json!({}), "unknown_action"),
        ("audio.set_volume", json!({}), "invalid_parameters"),
        (
            "audio.set_volume",
            json!({"percent":999}),
            "invalid_parameters",
        ),
        (
            "system.sleep",
            json!({"confirmed":true}),
            "invalid_parameters",
        ),
        ("workflow.run", json!({}), "recursive_workflow"),
        (
            "app.launch",
            json!({"application_id":"missing"}),
            "invalid_reference",
        ),
        (
            "file.open",
            json!({"path":r"C:\missing.txt"}),
            "invalid_reference",
        ),
        (
            "file.open",
            json!({"path":r"C:\script.ps1"}),
            "unsupported_action",
        ),
        (
            "file.open",
            json!({"path":r"C:\raw.exe"}),
            "unsupported_action",
        ),
    ] {
        let w = workflow(vec![("settings.open", json!({"page":"sound"})), (id, args)]);
        let mut fake = Fake::default();
        let (s, _) = sync_run(w, &mut fake);
        assert_eq!(s.status, Status::Failed, "{id}");
        assert_eq!(s.error.unwrap().code, expected, "{id}");
        assert_eq!(s.steps[1].status, Status::Failed);
        assert!(fake.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn malformed_workflows_versions_and_terminal_position_fail_safely() {
    let mut cases = vec![];
    for action in ["system.lock", "system.sleep"] {
        cases.push(workflow(vec![
            (action, json!({})),
            ("settings.open", json!({"page":"sound"})),
        ]));
    }
    let mut w = three();
    w.data.steps[1].id = w.data.steps[0].id;
    cases.push(w);
    let mut w = three();
    w.data.steps[1].order = 0;
    cases.push(w);
    let mut w = three();
    w.data.steps[0].action_schema_version = 99;
    cases.push(w);
    let mut w = three();
    w.data.format_version = 99;
    cases.push(w);
    cases.push(workflow(vec![]));
    for w in cases {
        let mut fake = Fake::default();
        assert_eq!(sync_run(w, &mut fake).0.status, Status::Failed);
        assert!(fake.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn duplicate_reservations_are_atomic_and_terminal_runs_can_restart() {
    let engine = Arc::new(Engine::default());
    let id = Uuid::new_v4();
    let barrier = Arc::new(Barrier::new(16));
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let engine = engine.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                engine
                    .reserve(id, Arc::new(|_| {}))
                    .map(|r| r.snapshot().run_id)
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .all(|e| e.code == "already_running")
    );
    let old = engine.list()[0].run_id;
    engine
        .find(old)
        .unwrap()
        .finish(Status::Cancelled, None, None);
    let new = engine
        .reserve(id, Arc::new(|_| {}))
        .unwrap()
        .snapshot()
        .run_id;
    assert_ne!(old, new);
    assert_eq!(engine.status(old).unwrap().status, Status::Cancelled);
}

#[test]
fn explicit_cancellation_between_steps_keeps_completed_effects() {
    let engine = Arc::new(Engine::default());
    let w = three();
    let run = engine.reserve(w.data.id, Arc::new(|_| {})).unwrap();
    let id = run.snapshot().run_id;
    let other = engine.clone();
    let mut fake = Fake {
        hook: Some(Box::new(move |_| {
            let snapshot = other.cancel(id).unwrap();
            assert!(snapshot.action_in_flight);
            assert_eq!(snapshot.status, Status::Running);
        })),
        ..Default::default()
    };
    execute(
        &run,
        w,
        ActionRegistry::default(),
        &mut fake,
        CONFIRMATION_TTL,
    );
    let s = run.snapshot();
    assert_eq!(s.status, Status::Cancelled);
    assert_eq!(s.steps[0].status, Status::Succeeded);
    assert_eq!(*fake.calls.lock().unwrap(), ["sound"]);
    assert!(!s.action_in_flight);
}

#[test]
fn prerequisites_are_rechecked_and_cancellation_during_checks_prevents_admission() {
    let w = workflow(vec![(
        "app.launch",
        json!({"application_id":"catalog:app"}),
    )]);
    let mut fake = Fake {
        missing_on_check: Some(2),
        ..Default::default()
    };
    let (s, _) = sync_run(w.clone(), &mut fake);
    assert_eq!(s.error.unwrap().code, "invalid_reference");
    assert!(fake.calls.lock().unwrap().is_empty());
    let engine = Arc::new(Engine::default());
    let run = engine.reserve(w.data.id, Arc::new(|_| {})).unwrap();
    let id = run.snapshot().run_id;
    let other = engine.clone();
    let mut count = 0;
    let mut fake = Fake {
        hook: Some(Box::new(move |name| {
            if name == "check" {
                count += 1;
                if count == 2 {
                    other.cancel(id).unwrap();
                }
            }
        })),
        ..Default::default()
    };
    execute(
        &run,
        w,
        ActionRegistry::default(),
        &mut fake,
        CONFIRMATION_TTL,
    );
    assert_eq!(run.snapshot().status, Status::Cancelled);
    assert!(fake.calls.lock().unwrap().is_empty());
}

#[test]
fn sensitive_actions_require_fresh_run_and_step_bound_approval() {
    let engine = Engine::default();
    for action in ["system.lock", "system.sleep"] {
        let w = workflow(vec![(action, json!({}))]);
        let fake = Fake::default();
        let calls = fake.calls.clone();
        let (initial, events) = start(&engine, w, fake, CONFIRMATION_TTL);
        until(&events, EventKind::ConfirmationRequested);
        let s = engine.status(initial.run_id).unwrap();
        let prompt = s.confirmation.unwrap();
        assert_eq!(s.status, Status::ConfirmationRequired);
        assert!(calls.lock().unwrap().is_empty());
        assert_eq!(prompt.action.action_id, action);
        assert!(
            engine
                .decide(
                    initial.run_id,
                    Uuid::new_v4(),
                    prompt.token,
                    Decision::Approve
                )
                .is_err()
        );
        assert!(
            engine
                .decide(
                    initial.run_id,
                    prompt.step_id,
                    Uuid::new_v4(),
                    Decision::Approve
                )
                .is_err()
        );
        let (other, other_events) = start(
            &engine,
            workflow(vec![(action, json!({}))]),
            Fake::default(),
            CONFIRMATION_TTL,
        );
        until(&other_events, EventKind::ConfirmationRequested);
        assert!(
            engine
                .decide(
                    other.run_id,
                    prompt.step_id,
                    prompt.token,
                    Decision::Approve
                )
                .is_err()
        );
        engine.cancel(other.run_id).unwrap();
        until(&other_events, EventKind::RunCancelled);
        engine
            .decide(
                initial.run_id,
                prompt.step_id,
                prompt.token,
                Decision::Approve,
            )
            .unwrap();
        until(&events, EventKind::RunSucceeded);
        assert_eq!(calls.lock().unwrap().len(), 1);
        assert!(
            engine
                .decide(
                    initial.run_id,
                    prompt.step_id,
                    prompt.token,
                    Decision::Approve
                )
                .is_err()
        );
    }
}

#[test]
fn denial_cancellation_and_timeout_never_invoke_sensitive_action() {
    for mode in ["deny", "cancel", "timeout"] {
        let engine = Engine::default();
        let fake = Fake::default();
        let calls = fake.calls.clone();
        let ttl = if mode == "timeout" {
            Duration::from_millis(30)
        } else {
            CONFIRMATION_TTL
        };
        let (initial, events) = start(
            &engine,
            workflow(vec![("system.sleep", json!({}))]),
            fake,
            ttl,
        );
        until(&events, EventKind::ConfirmationRequested);
        let prompt = engine.status(initial.run_id).unwrap().confirmation;
        match mode {
            "deny" => {
                let p = prompt.unwrap();
                engine
                    .decide(initial.run_id, p.step_id, p.token, Decision::Deny)
                    .unwrap();
            }
            "cancel" => {
                engine.cancel(initial.run_id).unwrap();
            }
            _ => {}
        }
        until(&events, EventKind::RunCancelled);
        let s = engine.status(initial.run_id).unwrap();
        assert_eq!(s.status, Status::Cancelled);
        assert!(s.confirmation.is_none());
        assert!(calls.lock().unwrap().is_empty());
        if mode == "timeout" {
            assert_eq!(s.error.unwrap().code, "confirmation_timeout");
        }
    }
}

#[test]
fn dropping_request_and_event_receiver_does_not_cancel_worker() {
    let engine = Engine::default();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let fake = Fake {
        hook: Some(Box::new(move |name| {
            if name == "sound" {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        })),
        ..Default::default()
    };
    let w = three();
    let (initial, events) = start(&engine, w.clone(), fake, CONFIRMATION_TTL);
    let id = initial.run_id;
    drop(initial);
    drop(events); // equivalent to disappearing IPC client/listener
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(engine.has_active());
    assert_eq!(engine.list()[0].workflow_id, w.data.id);
    release_tx.send(()).unwrap();
    let run = engine.find(id).unwrap();
    let state = run.state();
    let (state, timeout) = run
        .changed
        .wait_timeout_while(state, Duration::from_secs(5), |s| {
            !s.snapshot.status.terminal()
        })
        .unwrap();
    assert!(!timeout.timed_out());
    assert_eq!(state.snapshot.status, Status::Succeeded);
    assert_eq!(state.snapshot.steps.len(), 3);
}

#[test]
fn database_snapshot_is_owned_and_connection_dropped_before_execution() {
    let path = std::env::temp_dir().join(format!("nexus-execution-{}.sqlite3", Uuid::new_v4()));
    let registry = ActionRegistry::default();
    let mut store = Store::open(&path).unwrap();
    let original = store.save(three().data, None, &registry).unwrap();
    let loaded = store.get(original.data.id).unwrap();
    drop(store);
    let edited_path = path.clone();
    let mut changed = original.data.clone();
    changed.steps.reverse();
    for (i, step) in changed.steps.iter_mut().enumerate() {
        step.order = i as u32;
    }
    let mut first = true;
    let mut fake = Fake {
        hook: Some(Box::new(move |_| {
            if first {
                first = false;
                let mut store = Store::open(&edited_path).unwrap();
                store
                    .save(changed.clone(), Some(1), &ActionRegistry::default())
                    .unwrap();
            }
        })),
        ..Default::default()
    };
    let (s, _) = sync_run(loaded, &mut fake);
    assert_eq!(s.workflow_revision, Some(1));
    assert_eq!(s.status, Status::Succeeded);
    assert_eq!(*fake.calls.lock().unwrap(), ["sound", "wifi", "display"]);
    assert_eq!(
        Store::open(&path)
            .unwrap()
            .get(original.data.id)
            .unwrap()
            .revision,
        2
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn worker_panic_is_terminal_and_does_not_leave_duplicate_guard_stuck() {
    let engine = Engine::default();
    let w = three();
    let workflow_id = w.data.id;
    let fake = Fake {
        hook: Some(Box::new(|_| panic!("mock executor panic"))),
        ..Default::default()
    };
    let (initial, events) = start(&engine, w, fake, CONFIRMATION_TTL);
    until(&events, EventKind::RunFailed);
    let s = engine.status(initial.run_id).unwrap();
    assert_eq!(s.error.unwrap().code, "worker_panicked");
    assert_eq!(s.steps[0].status, Status::Failed);
    assert!(engine.reserve(workflow_id, Arc::new(|_| {})).is_ok());
}

#[test]
fn active_and_retained_runs_are_bounded_and_active_runs_are_never_evicted() {
    let engine = Engine::default();
    for _ in 0..MAX_ACTIVE {
        engine.reserve(Uuid::new_v4(), Arc::new(|_| {})).unwrap();
    }
    assert_eq!(
        engine
            .reserve(Uuid::new_v4(), Arc::new(|_| {}))
            .err()
            .unwrap()
            .code,
        "busy"
    );
    let keep = engine.list()[0].run_id;
    for s in engine.list().into_iter().skip(1) {
        engine
            .find(s.run_id)
            .unwrap()
            .finish(Status::Cancelled, None, None);
    }
    for _ in 0..MAX_RETAINED + 4 {
        engine
            .reserve(Uuid::new_v4(), Arc::new(|_| {}))
            .unwrap()
            .finish(Status::Succeeded, None, None);
    }
    assert_eq!(engine.list().len(), MAX_RETAINED);
    assert_eq!(engine.status(keep).unwrap().status, Status::Pending);
}

#[test]
fn cancellation_after_approval_during_revalidation_still_prevents_action() {
    let engine = Engine::default();
    // Model a sensitive action whose prerequisite revalidation takes time.
    let descriptor = ActionRegistry::default()
        .descriptor("system.lock")
        .unwrap()
        .clone();
    let registry = ActionRegistry::new(vec![crate::action_registry::Registration {
        descriptor,
        validate: |_, runtime| {
            runtime.check_application("catalog:app")?;
            Ok(false)
        },
        execute: |_, runtime, authorization| {
            assert!(authorization.is_confirmed());
            runtime.lock()
        },
    }])
    .unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut checks = 0;
    let mut fake = Fake {
        hook: Some(Box::new(move |name| {
            if name == "check" {
                checks += 1;
                if checks == 3 {
                    entered_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                }
            }
        })),
        ..Default::default()
    };
    let calls = fake.calls.clone();
    let w = workflow(vec![("system.lock", json!({}))]);
    let (sink, events) = sink();
    let initial = engine
        .launch(w.data.id, sink, move |run| {
            execute(&run, w, registry, &mut fake, CONFIRMATION_TTL)
        })
        .unwrap();
    until(&events, EventKind::ConfirmationRequested);
    let p = engine.status(initial.run_id).unwrap().confirmation.unwrap();
    engine
        .decide(initial.run_id, p.step_id, p.token, Decision::Approve)
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    engine.cancel(initial.run_id).unwrap();
    release_tx.send(()).unwrap();
    until(&events, EventKind::RunCancelled);
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn expired_approval_is_rejected_at_the_last_admission_boundary() {
    let engine = Engine::default();
    let run = engine.reserve(Uuid::new_v4(), Arc::new(|_| {})).unwrap();
    let mut fake = Fake::default();
    let mut runtime = runtime::Guarded {
        native: &mut fake,
        run: &run,
        approved_until: Some(Instant::now()),
    };
    assert_eq!(
        runtime.sleep().unwrap_err().code,
        ErrorCode::ConfirmationRequired
    );
    assert!(!run.snapshot().action_in_flight);
    assert!(fake.calls.lock().unwrap().is_empty());
}

#[test]
fn pending_cancellation_and_load_failure_are_queryable_terminal_states() {
    let engine = Engine::default();
    let w = three();
    let run = engine.reserve(w.data.id, Arc::new(|_| {})).unwrap();
    engine.cancel(run.snapshot().run_id).unwrap();
    let mut fake = Fake::default();
    execute(
        &run,
        w,
        ActionRegistry::default(),
        &mut fake,
        CONFIRMATION_TTL,
    );
    assert_eq!(run.snapshot().status, Status::Cancelled);
    assert!(fake.calls.lock().unwrap().is_empty());
    let (sink, events) = sink();
    let initial = engine
        .launch(Uuid::new_v4(), sink, |run| {
            run.finish(
                Status::Failed,
                Some(Failure::new("not_found", "Workflow does not exist.")),
                None,
            );
        })
        .unwrap();
    let terminal = until(&events, EventKind::RunFailed);
    let snapshot = engine.status(initial.run_id).unwrap();
    assert_eq!(snapshot.sequence, terminal.sequence);
    assert_eq!(snapshot.error.unwrap().code, "not_found");
    assert!(!engine.has_active());
}
