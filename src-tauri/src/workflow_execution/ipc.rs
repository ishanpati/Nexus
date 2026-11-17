use super::*;
use crate::{
    action_registry::native::NativeRuntime,
    audio::AudioService,
    installed_apps::{CatalogCache, launching::LaunchActivity},
    system_actions::SystemActions,
    workflows::Store,
};
use tauri::{Emitter, Manager};

fn main_window(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err(Failure::new(
            "forbidden",
            "Workflow execution is available only to the main window.",
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn start_workflow(
    window: tauri::WebviewWindow,
    workflow_id: Uuid,
    engine: tauri::State<'_, Engine>,
) -> Result<Snapshot> {
    main_window(&window)?;
    let activation_intent = crate::installed_apps::launching::activation_intent();
    let app = window.app_handle().clone();
    let events = app.clone();
    let sink: Sink = Arc::new(move |event| {
        // Delivery is best effort. Status remains queryable even with no webview listener.
        let _ = events.emit_to("main", "workflow-progress", event);
    });
    engine.launch(workflow_id, sink, move |run| {
        let load = || -> Result<Workflow> {
            let path = app
                .path()
                .app_local_data_dir()
                .map_err(|_| {
                    Failure::new(
                        "storage_error",
                        "Cannot resolve the workflow database location.",
                    )
                })?
                .join("workflows.sqlite3");
            // The connection is dropped before any registry reference checks or Windows calls.
            let store = Store::open(&path)?;
            Ok(store.get(workflow_id)?)
        };
        let workflow = match load() {
            Ok(w) => w,
            Err(e) => {
                run.finish(Status::Failed, Some(e), None);
                return;
            }
        };
        let registry = app.state::<ActionRegistry>().for_run();
        let catalog = app.state::<Arc<CatalogCache>>();
        if workflow
            .data
            .steps
            .iter()
            .any(|s| s.action_id == "app.launch")
        {
            if let Err(message) = catalog.list() {
                run.finish(
                    Status::Failed,
                    Some(Failure::new("catalog_unavailable", &message)),
                    None,
                );
                return;
            }
        }
        let audio = app.state::<Arc<AudioService>>();
        let system = app.state::<Arc<SystemActions>>();
        let activity = app.state::<Arc<LaunchActivity>>();
        let cancelled = || run.cancelled() || !activation_intent();
        let hide = || -> std::result::Result<(), String> {
            if let Some(window) = app.get_webview_window("main")
                && window.is_focused().map_err(|e| e.to_string())?
            {
                window.hide().map_err(|e| e.to_string())?;
                window.is_visible().map_err(|e| e.to_string())?;
            }
            Ok(())
        };
        let mut runtime = NativeRuntime {
            catalog: &catalog,
            audio: &audio,
            system: &system,
            activity: &activity,
            launch_hide: Some(&hide),
            launch_cancelled: Some(&cancelled),
        };
        execute(&run, workflow, registry, &mut runtime, CONFIRMATION_TTL);
    })
}

#[tauri::command]
pub fn workflow_run_status(
    window: tauri::WebviewWindow,
    run_id: Uuid,
    engine: tauri::State<'_, Engine>,
) -> Result<Snapshot> {
    main_window(&window)?;
    engine.status(run_id)
}
#[tauri::command]
pub fn list_workflow_runs(
    window: tauri::WebviewWindow,
    engine: tauri::State<'_, Engine>,
) -> Result<Vec<Snapshot>> {
    main_window(&window)?;
    Ok(engine.list())
}
#[tauri::command]
pub fn cancel_workflow_run(
    window: tauri::WebviewWindow,
    run_id: Uuid,
    engine: tauri::State<'_, Engine>,
) -> Result<Snapshot> {
    main_window(&window)?;
    engine.cancel(run_id)
}
#[tauri::command]
pub fn decide_workflow_action(
    window: tauri::WebviewWindow,
    run_id: Uuid,
    step_id: Uuid,
    token: Uuid,
    decision: Decision,
    engine: tauri::State<'_, Engine>,
) -> Result<Snapshot> {
    main_window(&window)?;
    engine.decide(run_id, step_id, token, decision)
}
