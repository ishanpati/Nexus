//! Local versioned storage; never executes a workflow or an action.
use crate::action_registry::{
    ActionRegistry, ActionRequest, ActionRuntime, ErrorCode, StepPosition,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct Error {
    pub code: String,
    pub message: String,
}
type Result<T> = std::result::Result<T, Error>;
fn error(code: &str, message: impl Into<String>) -> Error {
    Error {
        code: code.into(),
        message: message.into(),
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        error("storage_error", e.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        error("invalid_data", e.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: Uuid,
    pub action_id: String,
    pub action_schema_version: u32,
    pub parameters: serde_json::Value,
    pub order: u32,
    pub label: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub steps: Vec<Step>,
    pub format_version: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    #[serde(flatten)]
    pub data: Payload,
    pub created_at: u64,
    pub updated_at: u64,
    pub revision: u64,
}
#[derive(Serialize)]
pub struct View {
    pub workflow: Workflow,
    pub executable: bool,
    pub issues: Vec<String>,
}
pub struct Store(Connection);
fn key(s: &str) -> String {
    s.trim().to_lowercase()
}
fn timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .map_err(|e| error("clock_error", e.to_string()))
}

impl Store {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(error("storage_error", "Database path must be absolute."));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| error("storage_error", e.to_string()))?;
        }
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 0 {
            let tables: i64 = tx.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |r| r.get(0))?;
            if tables != 0 {
                return Err(error(
                    "migration_error",
                    "Unversioned existing database was preserved; automatic reset is forbidden.",
                ));
            }
            tx.execute_batch("CREATE TABLE workflows(id TEXT PRIMARY KEY, document TEXT NOT NULL); CREATE TABLE workflow_names(name_key TEXT PRIMARY KEY, workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE); PRAGMA user_version=1;")?;
        } else if version != 1 {
            return Err(error(
                "migration_error",
                format!("Unsupported database schema {version}; data was preserved."),
            ));
        }
        tx.commit()?;
        Ok(Self(connection))
    }
    pub fn get(&self, id: Uuid) -> Result<Workflow> {
        let text: Option<String> = self
            .0
            .query_row(
                "SELECT document FROM workflows WHERE id=?",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        serde_json::from_str(&text.ok_or_else(|| error("not_found", "Workflow does not exist."))?)
            .map_err(Into::into)
    }
    pub fn list(&self) -> Result<Vec<Workflow>> {
        let mut statement = self
            .0
            .prepare("SELECT document FROM workflows ORDER BY id")?;
        let rows = statement.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub fn save(
        &mut self,
        mut payload: Payload,
        expected: Option<u64>,
        registry: &ActionRegistry,
    ) -> Result<Workflow> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<String> = tx
            .query_row(
                "SELECT document FROM workflows WHERE id=?",
                [payload.id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let old: Option<Workflow> = old.map(|s| serde_json::from_str(&s)).transpose()?;
        if expected != old.as_ref().map(|w| w.revision) {
            return Err(error(
                "revision_conflict",
                "Workflow changed or was deleted. Reload before saving.",
            ));
        }
        validate(&mut payload, old.as_ref(), registry)?;
        let now = timestamp()?;
        let workflow = Workflow {
            created_at: old.as_ref().map_or(now, |w| w.created_at),
            updated_at: now.max(old.as_ref().map_or(0, |w| w.updated_at)),
            revision: old.as_ref().map_or(1, |w| w.revision + 1),
            data: payload,
        };
        tx.execute("INSERT INTO workflows(id,document) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET document=excluded.document", params![workflow.data.id.to_string(), serde_json::to_string(&workflow)?])?;
        tx.execute(
            "DELETE FROM workflow_names WHERE workflow_id=?",
            [workflow.data.id.to_string()],
        )?;
        for name in std::iter::once(&workflow.data.name).chain(workflow.data.aliases.iter()) {
            if tx
                .query_row(
                    "SELECT workflow_id FROM workflow_names WHERE name_key=?",
                    [key(name)],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .is_some()
            {
                return Err(error(
                    "duplicate_name",
                    "A workflow name or alias already uses this exact match.",
                ));
            }
            tx.execute(
                "INSERT INTO workflow_names(name_key,workflow_id) VALUES(?1,?2)",
                params![key(name), workflow.data.id.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(workflow)
    }
    pub fn delete(&mut self, id: Uuid, expected: u64, confirmed: bool) -> Result<()> {
        if !confirmed {
            return Err(error(
                "confirmation_required",
                "Confirm deletion of this workflow ID and revision.",
            ));
        }
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let document: Option<String> = tx
            .query_row(
                "SELECT document FROM workflows WHERE id=?",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let workflow: Workflow = serde_json::from_str(
            &document.ok_or_else(|| error("not_found", "Workflow does not exist."))?,
        )?;
        if workflow.revision != expected {
            return Err(error(
                "revision_conflict",
                "Workflow changed. Reload before deleting.",
            ));
        }
        tx.execute("DELETE FROM workflows WHERE id=?", [id.to_string()])?;
        tx.commit()?;
        Ok(())
    }
}

pub(crate) fn validate(
    p: &mut Payload,
    old: Option<&Workflow>,
    registry: &ActionRegistry,
) -> Result<()> {
    p.name = p.name.trim().into();
    if p.id.is_nil()
        || p.format_version != 1
        || p.name.is_empty()
        || p.name.chars().count() > 120
        || p.name.chars().any(char::is_control)
        || p.description.as_ref().is_some_and(|d| d.len() > 4096)
        || p.aliases.len() > 20
        || p.steps.is_empty()
        || p.steps.len() > 64
    {
        return Err(error(
            "validation_error",
            "Use a UUID, format version 1, a name of 1–120 characters, up to 20 aliases and 1–64 steps.",
        ));
    }
    let mut names = HashSet::from([key(&p.name)]);
    for alias in &mut p.aliases {
        *alias = alias.trim().into();
        if alias.is_empty()
            || alias.chars().count() > 120
            || alias.chars().any(char::is_control)
            || !names.insert(key(alias))
        {
            return Err(error(
                "validation_error",
                "Aliases must be bounded, nonempty and unique, including the workflow name.",
            ));
        }
    }
    p.steps.sort_by_key(|s| s.order);
    let mut ids = HashSet::new();
    for (index, step) in p.steps.iter().enumerate() {
        if step.id.is_nil()
            || !ids.insert(step.id)
            || step.order != index as u32
            || step.label.as_ref().is_some_and(|s| s.len() > 240)
            || serde_json::to_vec(&step.parameters)?.len() > 65536
        {
            return Err(error(
                "validation_error",
                "Steps need unique UUIDs, contiguous zero-based order and bounded parameters/labels.",
            ));
        }
        if registry
            .descriptor(&step.action_id)
            .is_ok_and(|d| d.terminal)
            && index + 1 != p.steps.len()
        {
            return Err(error(
                "validation_error",
                "Terminal actions must be last, including obsolete versions.",
            ));
        }
        if matches!(step.action_id.as_str(), "file.open" | "folder.open") {
            if let Some(path) = step.parameters.get("path").and_then(|v| v.as_str()) {
                crate::file_actions::validate_path(path)
                    .map_err(|e| error("validation_error", e))?;
            }
        }
        let request = ActionRequest {
            action_id: step.action_id.clone(),
            schema_version: step.action_schema_version,
            parameters: step.parameters.clone(),
        };
        match registry.validate(&request) {
            Ok(action) => {
                if action.terminal && index + 1 != p.steps.len() {
                    return Err(error(
                        "validation_error",
                        "Lock and sleep must be the final step.",
                    ));
                }
            }
            Err(e) => {
                // Preserve an unavailable step only when it is an unchanged action
                // reference/parameter set from the existing workflow. Repairs must validate.
                let retained = matches!(
                    e.code,
                    ErrorCode::UnknownAction
                        | ErrorCode::UnsupportedVersion
                        | ErrorCode::UnsupportedAction
                ) && old.is_some_and(|w| {
                    w.data.steps.iter().any(|s| {
                        s.id == step.id
                            && s.action_id == step.action_id
                            && s.action_schema_version == step.action_schema_version
                            && s.parameters == step.parameters
                    })
                });
                if !retained {
                    return Err(error("validation_error", e.message));
                }
            }
        }
    }
    Ok(())
}
fn view(workflow: Workflow, registry: &ActionRegistry) -> View {
    let mut issues = Vec::new();
    for (index, step) in workflow.data.steps.iter().enumerate() {
        match registry.validate(&ActionRequest {
            action_id: step.action_id.clone(),
            schema_version: step.action_schema_version,
            parameters: step.parameters.clone(),
        }) {
            Err(e) => issues.push(format!("Step {}: {}", index + 1, e.message)),
            Ok(a) if a.terminal && index + 1 != workflow.data.steps.len() => {
                issues.push("Terminal action is not last.".into())
            }
            _ => {}
        }
    }
    if workflow.data.format_version != 1 || workflow.data.steps.is_empty() {
        issues.push("Unsupported workflow format or empty steps.".into());
    }
    View {
        executable: issues.is_empty(),
        workflow,
        issues,
    }
}

// Open lazily on a worker: storage failure cannot prevent launcher startup.
async fn run<T: Send + 'static>(
    app: tauri::AppHandle,
    operation: impl FnOnce(&mut Store, &ActionRegistry, &tauri::AppHandle) -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = app
            .path()
            .app_local_data_dir()
            .map_err(|e| error("storage_error", e.to_string()))?
            .join("workflows.sqlite3");
        let mut store = Store::open(&path)?;
        operation(&mut store, &app.state::<ActionRegistry>(), &app)
    })
    .await
    .map_err(|e| error("storage_error", e.to_string()))?
}
#[tauri::command]
pub async fn list_workflows(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<Vec<View>> {
    repository_caller(&window)?;
    run(app, |s, r, a| {
        Ok(s.list()?
            .into_iter()
            .map(|w| references(view(w, r), a))
            .collect())
    })
    .await
}
#[tauri::command]
pub async fn get_workflow(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: Uuid,
) -> Result<View> {
    repository_caller(&window)?;
    run(app, move |s, r, a| Ok(references(view(s.get(id)?, r), a))).await
}
#[tauri::command]
pub async fn save_workflow(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    mut payload: Payload,
    expected_revision: Option<u64>,
) -> Result<View> {
    repository_caller(&window)?;
    let result = run(app.clone(), move |s, r, a| {
        let old = match s.get(payload.id) {
            Ok(w) => Some(w),
            Err(e) if e.code == "not_found" => None,
            Err(e) => return Err(e),
        };
        if old.as_ref().map(|w| w.revision) != expected_revision {
            return Err(error(
                "revision_conflict",
                "Workflow changed or was deleted. Reload before saving.",
            ));
        }
        validate(&mut payload, old.as_ref(), r)?;
        let catalog = a.state::<std::sync::Arc<crate::installed_apps::CatalogCache>>();
        let audio = a.state::<std::sync::Arc<crate::audio::AudioService>>();
        let system = a.state::<std::sync::Arc<crate::system_actions::SystemActions>>();
        let activity =
            a.state::<std::sync::Arc<crate::installed_apps::launching::LaunchActivity>>();
        check_save_references(
            &payload,
            r,
            &mut crate::action_registry::native::NativeRuntime {
                catalog: &catalog,
                audio: &audio,
                system: &system,
                activity: &activity,
            },
        )?;
        Ok(references(
            view(s.save(payload, expected_revision, r)?, r),
            a,
        ))
    })
    .await?;
    repository_changed(&app);
    Ok(result)
}

/// Read-only registry validators catch disappeared selections before committing.
/// Store::save still performs authoritative schema/revision/name validation.
fn check_save_references(
    payload: &Payload,
    registry: &ActionRegistry,
    runtime: &mut dyn ActionRuntime,
) -> Result<()> {
    let mut steps: Vec<_> = payload.steps.iter().collect();
    steps.sort_by_key(|s| s.order);
    for (index, step) in steps.iter().enumerate() {
        let request = ActionRequest {
            action_id: step.action_id.clone(),
            schema_version: step.action_schema_version,
            parameters: step.parameters.clone(),
        };
        if let Err(e) = registry.validate(&request) {
            // Store owns the existing unchanged obsolete-step retention policy.
            if matches!(
                e.code,
                ErrorCode::UnknownAction
                    | ErrorCode::UnsupportedVersion
                    | ErrorCode::UnsupportedAction
            ) {
                continue;
            }
            return Err(error(
                "validation_error",
                format!("Step {}: {}", index + 1, e.message),
            ));
        }
        registry
            .preflight(
                &request,
                runtime,
                if index + 1 == steps.len() {
                    StepPosition::Final
                } else {
                    StepPosition::Intermediate
                },
            )
            .and_then(|sensitive| {
                if sensitive {
                    Err(crate::action_registry::RegistryError::new(
                        ErrorCode::UnsupportedAction,
                        "Workflows cannot open executable, script, shortcut, macro-capable or unknown file types. Select a document or a catalog application.",
                    ))
                } else {
                    Ok(())
                }
            })
            .map_err(|e| {
                error(
                    "validation_error",
                    format!("Step {}: {}", index + 1, e.message),
                )
            })?;
    }
    Ok(())
}
#[tauri::command]
pub async fn delete_workflow(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: Uuid,
    expected_revision: u64,
    confirmed: bool,
) -> Result<()> {
    repository_caller(&window)?;
    run(app.clone(), move |s, _, _| {
        s.delete(id, expected_revision, confirmed)
    })
    .await?;
    repository_changed(&app);
    Ok(())
}

fn repository_caller(window: &tauri::WebviewWindow) -> Result<()> {
    if matches!(window.label(), "main" | "workflow-studio") {
        Ok(())
    } else {
        Err(error(
            "forbidden",
            "Workflow storage is unavailable to this window.",
        ))
    }
}
fn repository_changed(app: &tauri::AppHandle) {
    for label in ["main", "workflow-studio"] {
        let _ = app.emit_to(label, "workflows-changed", ());
    }
}

fn references(mut result: View, app: &tauri::AppHandle) -> View {
    for step in &result.workflow.data.steps {
        let problem = match step.action_id.as_str() {
            "app.launch" => step
                .parameters
                .get("application_id")
                .and_then(|v| v.as_str())
                .and_then(|id| {
                    app.state::<std::sync::Arc<crate::installed_apps::CatalogCache>>()
                        .lookup(id)
                        .err()
                }),
            "file.open" | "folder.open" => step
                .parameters
                .get("path")
                .and_then(|v| v.as_str())
                .and_then(|path| {
                    crate::file_actions::inspect_selected(path, step.action_id == "folder.open")
                        .and_then(|sensitive| {
                            if sensitive {
                                Err("This file type cannot be opened by workflows. Select a document or a catalog application.".into())
                            } else {
                                Ok(())
                            }
                        })
                        .err()
                }),
            _ => None,
        };
        if let Some(problem) = problem {
            result.issues.push(format!(
                "Step {} reference unavailable: {problem}",
                step.order + 1
            ));
        }
    }
    result.executable = result.issues.is_empty();
    result
}

#[cfg(test)]
mod tests;
