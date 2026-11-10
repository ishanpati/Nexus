use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub const LABEL: &str = "Ctrl+Shift+Space";

#[derive(Clone, Serialize)]
pub struct ShortcutStatus {
    pub registered: bool,
    pub warning: Option<String>,
}

fn shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space)
}

fn registration_status(result: Result<(), String>) -> ShortcutStatus {
    match result {
        Ok(()) => ShortcutStatus {
            registered: true,
            warning: None,
        },
        Err(error) => {
            eprintln!("Could not register {LABEL}: {error}");
            ShortcutStatus {
                registered: false,
                warning: Some(format!(
                    "{LABEL} is unavailable; another app may own it. Launch NEXUS again to reopen, or close the conflicting app and restart NEXUS."
                )),
            }
        }
    }
}

pub fn report_error(app: &AppHandle, message: String) {
    eprintln!("{message}");
    if let Err(error) = app.emit_to("main", "launcher-error", &message) {
        eprintln!("Could not display NEXUS error: {error}");
    }
}

pub fn register(app: &AppHandle) -> ShortcutStatus {
    // Registration errors are recoverable: startup still opens the window and
    // a second executable launch can summon it through the single-instance plugin.
    registration_status(
        app.global_shortcut()
            .on_shortcut(shortcut(), |app, _, event| {
                if !should_toggle(event.state) {
                    return;
                }
                let handle = app.clone();
                if let Err(error) = app.run_on_main_thread(move || {
                    if let Some(window) = handle.get_webview_window("main") {
                        if let Err(error) = crate::launcher_window::toggle(&window) {
                            report_error(&handle, format!("Could not toggle NEXUS: {error}"));
                        }
                    }
                }) {
                    report_error(app, format!("Could not handle {LABEL}: {error}"));
                }
            })
            .map_err(|error| error.to_string()),
    )
}

fn should_toggle(state: ShortcutState) -> bool {
    state == ShortcutState::Pressed
}

pub fn cleanup(app: &AppHandle) {
    // Only unregister our shortcut, and only if registration succeeded.
    if app.global_shortcut().is_registered(shortcut()) {
        if let Err(error) = app.global_shortcut().unregister(shortcut()) {
            eprintln!("Could not unregister {LABEL} during shutdown: {error}");
        }
    }
}

#[tauri::command]
pub fn shortcut_status(status: State<'_, ShortcutStatus>) -> ShortcutStatus {
    status.inner().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_key_press_toggles_not_key_release() {
        assert!(should_toggle(ShortcutState::Pressed));
        assert!(!should_toggle(ShortcutState::Released));
    }

    #[test]
    fn conflict_keeps_app_usable_with_relaunch_instructions() {
        let status = registration_status(Err("already registered".into()));
        assert!(!status.registered);
        let warning = status.warning.unwrap();
        assert!(warning.contains(LABEL));
        assert!(warning.contains("Launch NEXUS again"));
    }

    #[test]
    fn successful_registration_has_no_warning() {
        let status = registration_status(Ok(()));
        assert!(status.registered);
        assert!(status.warning.is_none());
    }
}
