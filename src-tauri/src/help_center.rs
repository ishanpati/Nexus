//! Lazy, read-only Help window. Main owns launch; Help can only acknowledge readiness.
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager, WebviewWindow};

pub const LABEL: &str = "help-center";
#[derive(Default)]
pub struct HelpCenter {
    ready: AtomicBool,
}

fn caller_is(label: &str, expected: &str) -> Result<(), String> {
    if label == expected {
        Ok(())
    } else {
        Err("Unsupported Help Center caller.".into())
    }
}

/// Reserve logical space for borders, title bar and desktop margins at any DPI.
fn dimensions(width: f64, height: f64) -> (f64, f64, f64, f64) {
    let width = (width - 40.0).clamp(1.0, 850.0);
    let height = (height - 80.0).clamp(1.0, 650.0);
    (width, height, width.min(620.0), height.min(440.0))
}

#[tauri::command]
pub fn help_center_ready(
    window: WebviewWindow,
    state: tauri::State<'_, HelpCenter>,
) -> Result<(), String> {
    caller_is(window.label(), LABEL)?;
    state.ready.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn open_help_center(window: WebviewWindow) -> Result<(), String> {
    caller_is(window.label(), "main")?;
    let app = window.app_handle().clone();
    // Shared admission prevents duplicate creation and suppresses blur dismissal
    // until successful native focus or launcher recovery.
    let guard = app
        .state::<std::sync::Arc<crate::installed_apps::launching::LaunchActivity>>()
        .acquire()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let result = crate::workflow_studio::activate(
            app.get_webview_window(LABEL),
            || {
                app.state::<HelpCenter>()
                    .ready
                    .store(false, Ordering::SeqCst);
                let monitor = window
                    .current_monitor()
                    .ok()
                    .flatten()
                    .or_else(|| window.primary_monitor().ok().flatten());
                let (width, height) = monitor
                    .as_ref()
                    .map(|m| {
                        let size = m.work_area().size.to_logical::<f64>(m.scale_factor());
                        (size.width, size.height)
                    })
                    .unwrap_or((890.0, 730.0));
                let (width, height, min_width, min_height) = dimensions(width, height);
                let help = tauri::WebviewWindowBuilder::new(
                    &app,
                    LABEL,
                    tauri::WebviewUrl::App("index.html?help-center".into()),
                )
                .title("NEXUS Help")
                .inner_size(width, height)
                .min_inner_size(min_width, min_height)
                .resizable(true)
                .decorations(true)
                .theme(Some(tauri::Theme::Dark))
                .visible(false)
                .build()
                .map_err(|e| e.to_string())?;
                if let Some(monitor) = monitor {
                    let area = monitor.work_area();
                    let scale = monitor.scale_factor();
                    let window_scale = help.scale_factor().map_err(|e| e.to_string())?;
                    let size = help
                        .outer_size()
                        .map_err(|e| e.to_string())?
                        .to_logical::<f64>(window_scale)
                        .to_physical::<u32>(scale);
                    help.set_position(tauri::PhysicalPosition::new(
                        area.position.x + (area.size.width.saturating_sub(size.width) / 2) as i32,
                        area.position.y + (area.size.height.saturating_sub(size.height) / 2) as i32,
                    ))
                    .map_err(|e| e.to_string())?;
                    help.set_size(tauri::LogicalSize::new(width, height))
                        .map_err(|e| e.to_string())?;
                }
                Ok(help)
            },
            || {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
                while !app.state::<HelpCenter>().ready.load(Ordering::SeqCst) {
                    if std::time::Instant::now() >= deadline {
                        return Err("NEXUS Help did not become ready. Try again.".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Ok(())
            },
            |help| {
                help.unminimize().map_err(|e| e.to_string())?;
                help.show().map_err(|e| e.to_string())?;
                help.set_focus().map_err(|e| e.to_string())?;
                let _ = help.emit("help-shown", ());
                Ok(())
            },
            || Ok(()),
        );
        if result.is_err() {
            // A failed chunk/readiness handshake must be retryable on the next open.
            if !app.state::<HelpCenter>().ready.load(Ordering::SeqCst)
                && let Some(help) = app.get_webview_window(LABEL)
            {
                let _ = help.destroy();
            }
            // No launcher-shown event: the failed query must survive recovery.
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
    fn permissions_are_scoped_to_the_actual_caller() {
        assert!(caller_is("main", "main").is_ok());
        assert!(caller_is(LABEL, LABEL).is_ok());
        for label in ["workflow-studio", "audio-feedback", "other"] {
            assert!(caller_is(label, "main").is_err());
            assert!(caller_is(label, LABEL).is_err());
        }
        assert!(caller_is(LABEL, "main").is_err());
    }
    #[test]
    fn sizing_fits_scaled_and_small_work_areas() {
        assert_eq!(dimensions(1920.0, 1080.0), (850.0, 650.0, 620.0, 440.0));
        assert_eq!(dimensions(800.0, 600.0), (760.0, 520.0, 620.0, 440.0));
        assert_eq!(dimensions(600.0, 400.0), (560.0, 320.0, 560.0, 320.0));
    }
}
