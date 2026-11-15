use super::*;
use serde_json::json;
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(
            std::env::temp_dir()
                .join(format!("nexus-workflows-{}", Uuid::new_v4()))
                .join("test.sqlite3"),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_dir(self.0.parent().unwrap());
    }
}
fn payload(name: &str) -> Payload {
    Payload {
        id: Uuid::new_v4(),
        name: name.into(),
        description: None,
        aliases: vec![],
        format_version: 1,
        steps: vec![Step {
            id: Uuid::new_v4(),
            action_id: "audio.set_volume".into(),
            action_schema_version: 1,
            parameters: json!({"percent":50}),
            order: 0,
            label: None,
        }],
    }
}

struct SaveRuntime {
    missing: bool,
    sensitive: bool,
    checked_apps: usize,
    checked_paths: usize,
}
impl ActionRuntime for SaveRuntime {
    fn check_application(&mut self, _: &str) -> crate::action_registry::Result<()> {
        self.checked_apps += 1;
        if self.missing {
            Err(crate::action_registry::RegistryError::new(
                ErrorCode::InvalidReference,
                "Application disappeared. Refresh applications.",
            ))
        } else {
            Ok(())
        }
    }
    fn inspect_path(&mut self, path: &str, _: bool) -> crate::action_registry::Result<bool> {
        self.checked_paths += 1;
        assert_eq!(path, "C:\\資料 space\\Work folder");
        Ok(self.sensitive)
    }
    fn launch_application(&mut self, _: &str) -> crate::action_registry::Result<serde_json::Value> {
        panic!("Save must not launch")
    }
    fn open_path(
        &mut self,
        _: &str,
        _: bool,
        _: bool,
    ) -> crate::action_registry::Result<serde_json::Value> {
        panic!("Save must not open")
    }
    fn audio(
        &mut self,
        _: crate::audio::AudioRequest,
    ) -> crate::action_registry::Result<serde_json::Value> {
        panic!("Save must not change audio")
    }
    fn settings(&mut self, _: &str) -> crate::action_registry::Result<serde_json::Value> {
        panic!("Save must not change settings")
    }
    fn lock(&mut self) -> crate::action_registry::Result<serde_json::Value> {
        panic!("Save must not lock")
    }
    fn sleep(&mut self) -> crate::action_registry::Result<serde_json::Value> {
        panic!("Save must not sleep")
    }
}
#[test]
fn save_rechecks_disappeared_apps_without_execution_or_commit() {
    let f = Fixture::new();
    let registry = ActionRegistry::default();
    let mut store = Store::open(&f.0).unwrap();
    let mut p = payload("Three-step editor workflow");
    p.steps[0].parameters = json!({"percent":25});
    for (id, parameters) in [
        ("app.launch", json!({"application_id":"cached-id"})),
        ("folder.open", json!({"path":"C:\\資料 space\\Work folder"})),
    ] {
        p.steps.push(Step {
            id: Uuid::new_v4(),
            action_id: id.into(),
            action_schema_version: 1,
            parameters,
            order: p.steps.len() as u32,
            label: None,
        });
    }
    let mut runtime = SaveRuntime {
        missing: true,
        sensitive: false,
        checked_apps: 0,
        checked_paths: 0,
    };
    let error = check_save_references(&p, &registry, &mut runtime).unwrap_err();
    assert!(error.message.contains("Step 2"));
    assert!(error.message.contains("disappeared"));
    assert!(store.list().unwrap().is_empty());
    runtime.missing = false;
    check_save_references(&p, &registry, &mut runtime).unwrap();
    let saved = store.save(p, None, &registry).unwrap();
    assert_eq!(saved.data.steps[0].parameters["percent"], 25);
    assert_eq!(runtime.checked_apps, 2);
    assert_eq!(runtime.checked_paths, 1);
    runtime.sensitive = true;
    let mut unsafe_payload = saved.data.clone();
    unsafe_payload.steps[2].action_id = "file.open".into();
    assert!(
        check_save_references(&unsafe_payload, &registry, &mut runtime)
            .unwrap_err()
            .message
            .contains("cannot open")
    );
    assert_eq!(store.get(saved.data.id).unwrap().revision, 1);
    // Final sensitive actions are only validated: no approval or executor is used.
    let mut p = payload("Final lock");
    p.steps[0].action_id = "system.lock".into();
    p.steps[0].parameters = json!({});
    check_save_references(&p, &registry, &mut runtime).unwrap();
    p.steps.push(Step {
        id: Uuid::new_v4(),
        action_id: "audio.set_volume".into(),
        action_schema_version: 1,
        parameters: json!({"percent":25}),
        order: 1,
        label: None,
    });
    assert!(
        check_save_references(&p, &registry, &mut runtime)
            .unwrap_err()
            .message
            .contains("last")
    );
}
#[test]
fn crud_rename_order_reopen_and_revisions() {
    let f = Fixture::new();
    let r = ActionRegistry::default();
    let mut s = Store::open(&f.0).unwrap();
    let mut p = payload(" First ");
    let id = p.id;
    let first_step = p.steps[0].id;
    let a = s.save(p.clone(), None, &r).unwrap();
    assert_eq!(a.data.name, "First");
    let other = s.save(payload("Other"), None, &r).unwrap();
    p.name = "Renamed".into();
    let mut second = p.steps[0].clone();
    second.id = Uuid::new_v4();
    second.order = 0;
    p.steps[0].order = 1;
    p.steps.push(second.clone());
    let b = s.save(p.clone(), Some(1), &r).unwrap();
    assert_eq!(b.data.id, id);
    assert_eq!(b.data.steps[1].id, first_step);
    assert_eq!(b.created_at, a.created_at);
    assert_eq!(
        s.save(p, Some(1), &r).unwrap_err().code,
        "revision_conflict"
    );
    drop(s);
    let mut s = Store::open(&f.0).unwrap();
    assert_eq!(s.get(id).unwrap().data.steps[0].id, second.id);
    assert!(s.delete(id, 2, false).is_err());
    s.delete(id, 2, true).unwrap();
    assert!(s.get(id).is_err());
    assert_eq!(s.get(other.data.id).unwrap().revision, 1);
}
#[test]
fn concurrent_sessions_reject_stale_save_delete_and_resurrection_after_restart() {
    let f = Fixture::new();
    let registry = ActionRegistry::default();
    let mut first = Store::open(&f.0).unwrap();
    let created = first
        .save(payload("Session workflow"), None, &registry)
        .unwrap();
    let id = created.data.id;
    let step_id = created.data.steps[0].id;
    let mut second = Store::open(&f.0).unwrap();
    let stale = second.get(id).unwrap();
    let mut update = created.data;
    update.name = "Renamed in first session".into();
    update.steps[0].parameters = json!({"percent":30});
    first.save(update, Some(1), &registry).unwrap();
    assert_eq!(
        second
            .save(stale.data.clone(), Some(1), &registry)
            .unwrap_err()
            .code,
        "revision_conflict"
    );
    assert_eq!(
        second.delete(id, 1, true).unwrap_err().code,
        "revision_conflict"
    );
    assert_eq!(
        second.delete(id, 2, false).unwrap_err().code,
        "confirmation_required"
    );
    drop(first);
    drop(second);
    let mut restarted = Store::open(&f.0).unwrap();
    let saved = restarted.get(id).unwrap();
    assert_eq!(saved.data.name, "Renamed in first session");
    assert_eq!(saved.data.steps[0].id, step_id);
    assert_eq!(saved.data.steps[0].parameters["percent"], 30);
    assert_eq!(saved.revision, 2);
    restarted.delete(id, 2, true).unwrap();
    assert_eq!(
        restarted
            .save(saved.data, Some(2), &registry)
            .unwrap_err()
            .code,
        "revision_conflict"
    );
    assert_eq!(restarted.get(id).unwrap_err().code, "not_found");
    assert_eq!(restarted.delete(id, 2, true).unwrap_err().code, "not_found");
    drop(restarted);
    assert!(Store::open(&f.0).unwrap().list().unwrap().is_empty());
}
#[test]
fn duplicate_names_aliases_and_transaction_rollback() {
    let f = Fixture::new();
    let r = ActionRegistry::default();
    let mut s = Store::open(&f.0).unwrap();
    let mut p = payload("One");
    p.aliases = vec!["Alias".into()];
    let a = s.save(p, None, &r).unwrap();
    assert_eq!(
        s.save(payload("ALIAS"), None, &r).unwrap_err().code,
        "duplicate_name"
    );
    let b = s.save(payload("Two"), None, &r).unwrap();
    let mut update = b.data.clone();
    update.name = "one".into();
    assert!(s.save(update, Some(1), &r).is_err());
    assert_eq!(s.get(b.data.id).unwrap().data.name, "Two");
    assert_eq!(s.list().unwrap().len(), 2);
    s.0.execute_batch("CREATE TRIGGER fail_names BEFORE INSERT ON workflow_names BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    let mut update = a.data.clone();
    update.name = "Changed".into();
    assert!(s.save(update, Some(1), &r).is_err());
    assert_eq!(s.get(a.data.id).unwrap().data.name, "One");
}
#[test]
fn invalid_parameters_and_terminal_order_rejected() {
    let f = Fixture::new();
    let r = ActionRegistry::default();
    let mut s = Store::open(&f.0).unwrap();
    for args in [
        json!({}),
        json!({"percent":"50"}),
        json!({"percent":101}),
        json!({"percent":50,"shell":"cmd.exe"}),
    ] {
        let mut p = payload("Bad");
        p.steps[0].parameters = args;
        assert!(s.save(p, None, &r).is_err());
    }
    let mut p = payload("Bad");
    p.steps.push(p.steps[0].clone());
    assert!(s.save(p, None, &r).is_err());
    let mut p = payload("Bad");
    let mut last = p.steps[0].clone();
    last.id = Uuid::new_v4();
    last.order = 1;
    p.steps[0].action_id = "system.sleep".into();
    p.steps[0].parameters = json!({});
    p.steps.push(last);
    assert!(s.save(p, None, &r).is_err());
}
#[test]
fn unavailable_steps_survive_and_can_be_repaired() {
    let f = Fixture::new();
    let r = ActionRegistry::default();
    let mut s = Store::open(&f.0).unwrap();
    let w = s.save(payload("Old"), None, &r).unwrap();
    let mut old = w.clone();
    old.data.steps[0].action_id = "removed.action".into();
    s.0.execute(
        "UPDATE workflows SET document=? WHERE id=?",
        params![
            serde_json::to_string(&old).unwrap(),
            old.data.id.to_string()
        ],
    )
    .unwrap();
    assert!(!view(s.get(old.data.id).unwrap(), &r).executable);
    old.data.name = "Repair later".into();
    let saved = s.save(old.data, Some(1), &r).unwrap();
    assert_eq!(saved.data.steps[0].action_id, "removed.action");
    let mut repaired = saved.data;
    repaired.steps[0].action_id = "audio.set_volume".into();
    assert!(s.save(repaired, Some(2), &r).is_ok());
    let mut new = payload("Unknown");
    new.steps[0].action_id = "removed.action".into();
    assert!(s.save(new, None, &r).is_err());
}
#[test]
fn migrations_preserve_data_and_reject_unknown_database() {
    let f = Fixture::new();
    let mut s = Store::open(&f.0).unwrap();
    s.save(payload("Keep"), None, &ActionRegistry::default())
        .unwrap();
    s.0.execute_batch("PRAGMA user_version=99").unwrap();
    drop(s);
    assert!(Store::open(&f.0).is_err());
    let c = Connection::open(&f.0).unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM workflows", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(c);
    let f = Fixture::new();
    std::fs::create_dir_all(f.0.parent().unwrap()).unwrap();
    let c = Connection::open(&f.0).unwrap();
    c.execute_batch("CREATE TABLE precious(value TEXT); INSERT INTO precious VALUES('keep');")
        .unwrap();
    drop(c);
    assert!(Store::open(&f.0).is_err());
}
