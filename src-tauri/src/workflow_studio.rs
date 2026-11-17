//! One lazily created webview; requests survive listener startup and missed events.
use serde::{Deserialize, Serialize};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager, WebviewWindow};

pub const LABEL: &str = "workflow-studio";
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Route {
    Create {},
    List {},
    Edit { id: uuid::Uuid },
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    Navigate { route: Route },
    Close,
    Quit,
}
#[derive(Clone, Debug, Serialize)]
pub struct Request {
    pub sequence: u64,
    pub intent: Intent,
}
#[derive(Default)]
struct Mailbox {
    sequence: u64,
    pending: Option<Request>,
}
impl Mailbox {
    fn push(&mut self, intent: Intent) -> Result<Request, String> {
        if self.pending.is_some() {
            return Err("Resolve the pending request in Workflow Studio first.".into());
        }
        self.sequence += 1;
        let request = Request {
            sequence: self.sequence,
            intent,
        };
        self.pending = Some(request.clone());
        Ok(request)
    }
    fn take(&mut self, sequence: u64) -> Result<Request, String> {
        if self.pending.as_ref().map(|r| r.sequence) != Some(sequence) {
            return Err("This Studio request is no longer current.".into());
        }
        Ok(self.pending.take().unwrap())
    }
}
#[derive(Default)]
pub struct Studio {
    ready: AtomicBool,
    mailbox: Mutex<Mailbox>,
}
fn studio_only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == LABEL {
        Ok(())
    } else {
        Err("This command belongs to Workflow Studio.".into())
    }
}
fn enqueue(app: &tauri::AppHandle, intent: Intent) -> Result<(), String> {
    app.state::<Studio>()
        .mailbox
        .lock()
        .map_err(|_| "Studio state unavailable")?
        .push(intent)?;
    // Notification is advisory; ready/focus fetches recover the retained request.
    let _ = app.emit_to(LABEL, "studio-request", ());
    Ok(())
}
pub fn request_close(app: &tauri::AppHandle) {
    let _ = enqueue(app, Intent::Close);
}

/// Keep the launcher alive until Studio has resolved any unsaved draft.
pub fn request_quit(app: &tauri::AppHandle) -> Result<(), String> {
    let studio = app
        .get_webview_window(LABEL)
        .ok_or("Studio is unavailable")?;
    studio.unminimize().map_err(|e| e.to_string())?;
    studio.show().map_err(|e| e.to_string())?;
    studio.set_focus().map_err(|e| e.to_string())?;
    enqueue(app, Intent::Quit)
}

#[derive(Debug, PartialEq)]
pub(crate) enum LauncherClose {
    Hide,
    AskStudio,
    Exit,
}
pub(crate) fn launcher_close(active_run: bool, studio_exists: bool) -> LauncherClose {
    if active_run {
        LauncherClose::Hide
    } else if studio_exists {
        LauncherClose::AskStudio
    } else {
        LauncherClose::Exit
    }
}

// Keep sequencing independently testable without opening native windows.
pub(crate) fn activate<T>(
    existing: Option<T>,
    create: impl FnOnce() -> Result<T, String>,
    ready: impl FnOnce() -> Result<(), String>,
    focus: impl FnOnce(&T) -> Result<(), String>,
    deliver: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let window = match existing {
        Some(window) => window,
        None => create()?,
    };
    ready()?;
    focus(&window)?;
    deliver()
}

#[tauri::command]
pub fn studio_ready(
    window: WebviewWindow,
    state: tauri::State<'_, Studio>,
) -> Result<Option<Request>, String> {
    studio_only(&window)?;
    state.ready.store(true, Ordering::SeqCst);
    Ok(state
        .mailbox
        .lock()
        .map_err(|_| "Studio state unavailable")?
        .pending
        .clone())
}
#[tauri::command]
pub fn studio_pending(
    window: WebviewWindow,
    state: tauri::State<'_, Studio>,
) -> Result<Option<Request>, String> {
    studio_only(&window)?;
    Ok(state
        .mailbox
        .lock()
        .map_err(|_| "Studio state unavailable")?
        .pending
        .clone())
}
#[tauri::command]
pub fn studio_resolve(
    window: WebviewWindow,
    state: tauri::State<'_, Studio>,
    sequence: u64,
    accept: bool,
) -> Result<(), String> {
    studio_only(&window)?;
    let mut mailbox = state
        .mailbox
        .lock()
        .map_err(|_| "Studio state unavailable")?;
    let quitting = accept
        && matches!(mailbox.pending.as_ref(), Some(Request { sequence: current, intent: Intent::Quit }) if *current == sequence);
    if quitting
        && window
            .app_handle()
            .state::<crate::workflow_execution::Engine>()
            .has_active()
    {
        return Err(
            "A workflow is running. Cancel quitting and wait for it to finish before exiting."
                .into(),
        );
    }
    if accept
        && matches!(mailbox.pending.as_ref(), Some(Request { sequence: current, intent: Intent::Close }) if *current == sequence)
    {
        // Keep the request available if hiding fails. Never destroy the app/window.
        window.hide().map_err(|e| e.to_string())?;
    }
    mailbox.take(sequence)?;
    drop(mailbox);
    if quitting {
        window.app_handle().exit(0);
    }
    Ok(())
}

#[tauri::command]
pub async fn open_workflow_studio(window: WebviewWindow, route: Route) -> Result<(), String> {
    if window.label() != "main" && window.label() != LABEL {
        return Err("Unsupported Studio caller.".into());
    }
    if matches!(&route, Route::Edit { id } if id.is_nil()) {
        return Err("Invalid workflow ID.".into());
    }
    let app = window.app_handle().clone();
    // Reuse the existing launch guard: serialize opens and suppress premature blur hiding.
    let guard = app
        .state::<std::sync::Arc<crate::installed_apps::launching::LaunchActivity>>()
        .acquire()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let result = activate(
            app.get_webview_window(LABEL),
            || {
                app.state::<Studio>().ready.store(false, Ordering::SeqCst);
                tauri::WebviewWindowBuilder::new(
                    &app,
                    LABEL,
                    tauri::WebviewUrl::App("index.html?workflow-studio".into()),
                )
                .title("Workflow Studio — NEXUS")
                .inner_size(950.0, 700.0)
                .min_inner_size(680.0, 480.0)
                .resizable(true)
                .decorations(true)
                .theme(Some(tauri::Theme::Dark))
                .visible(false)
                .build()
                .map_err(|e| e.to_string())
            },
            || {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
                while !app.state::<Studio>().ready.load(Ordering::SeqCst) {
                    if std::time::Instant::now() >= deadline {
                        return Err("Workflow Studio did not become ready. Try again.".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Ok(())
            },
            |studio| {
                studio.unminimize().map_err(|e| e.to_string())?;
                studio.show().map_err(|e| e.to_string())?;
                studio.set_focus().map_err(|e| e.to_string())?;
                Ok(())
            },
            || enqueue(&app, Intent::Navigate { route }),
        );
        // Only success reaches the launcher's clear/hide lifecycle.
        if result.is_err() && window.label() == "main" {
            // Do not emit launcher-shown: recovery must retain the failed query.
            let _ = window.show();
            let _ = window.set_focus();
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launcher_close_preserves_runs_and_drafts_before_exit() {
        assert_eq!(launcher_close(true, false), LauncherClose::Hide);
        assert_eq!(launcher_close(true, true), LauncherClose::Hide);
        assert_eq!(launcher_close(false, true), LauncherClose::AskStudio);
        assert_eq!(launcher_close(false, false), LauncherClose::Exit);
        let mut mailbox = Mailbox::default();
        let request = mailbox.push(Intent::Quit).unwrap();
        assert!(mailbox.push(Intent::Close).is_err());
        assert!(mailbox.take(request.sequence + 1).is_err());
        assert!(matches!(
            mailbox.take(request.sequence).unwrap().intent,
            Intent::Quit
        ));
    }
    #[test]
    fn creates_once_then_reuses_and_waits_before_focus_and_delivery() {
        use std::cell::RefCell;
        for existing in [None, Some(7)] {
            let calls = RefCell::new(Vec::new());
            activate(
                existing,
                || {
                    calls.borrow_mut().push("create");
                    Ok(7)
                },
                || {
                    calls.borrow_mut().push("ready");
                    Ok(())
                },
                |window| {
                    assert_eq!(*window, 7);
                    calls.borrow_mut().push("focus");
                    Ok(())
                },
                || {
                    calls.borrow_mut().push("deliver");
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                *calls.borrow(),
                if existing.is_some() {
                    vec!["ready", "focus", "deliver"]
                } else {
                    vec!["create", "ready", "focus", "deliver"]
                }
            );
        }
    }
    #[test]
    fn creation_readiness_and_focus_failures_never_deliver_navigation() {
        for failure in ["create", "ready", "focus"] {
            let result = activate(
                None,
                || {
                    if failure == "create" {
                        Err(failure.into())
                    } else {
                        Ok(())
                    }
                },
                || {
                    if failure == "ready" {
                        Err(failure.into())
                    } else {
                        Ok(())
                    }
                },
                |_| {
                    if failure == "focus" {
                        Err(failure.into())
                    } else {
                        Ok(())
                    }
                },
                || panic!("failed activation must not navigate or clear the launcher"),
            );
            assert_eq!(result.unwrap_err(), failure);
        }
    }
    #[test]
    fn requests_survive_until_resolved_and_cannot_overwrite_unsaved_prompt() {
        let mut box_ = Mailbox::default();
        let first = box_
            .push(Intent::Navigate {
                route: Route::Create {},
            })
            .unwrap();
        assert!(box_.push(Intent::Close).is_err());
        assert!(box_.take(first.sequence + 1).is_err());
        assert!(box_.pending.is_some());
        box_.take(first.sequence).unwrap();
        assert!(box_.take(first.sequence).is_err());
        assert!(box_.push(Intent::Close).unwrap().sequence > first.sequence);
    }
    #[test]
    fn routes_are_closed_and_edit_uses_uuid() {
        for bad in [
            r#"{"mode":"shell"}"#,
            r#"{"mode":"edit","id":"my workflow"}"#,
            r#"{"mode":"create","path":"x"}"#,
        ] {
            assert!(serde_json::from_str::<Route>(bad).is_err());
        }
        assert_eq!(
            serde_json::from_str::<Route>(r#"{"mode":"list"}"#).unwrap(),
            Route::List {}
        );
    }
}
