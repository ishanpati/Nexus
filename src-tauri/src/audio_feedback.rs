//! A local, non-activating NEXUS overlay. No simulated keys or private Shell APIs.
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[derive(Clone, Serialize)]
pub struct Snapshot {
    volume: u8,
    muted: bool,
}
#[derive(Default)]
pub struct Feedback(Mutex<(u64, Option<Snapshot>)>);

#[tauri::command]
pub fn audio_feedback_state(state: tauri::State<'_, Feedback>) -> Result<Option<Snapshot>, String> {
    Ok(state
        .0
        .lock()
        .map_err(|_| "Audio feedback is unavailable")?
        .1
        .clone())
}

pub fn show(app: &tauri::AppHandle, volume: u8, muted: bool) -> Result<(), String> {
    let state = app.state::<Feedback>();
    let mut state = state
        .0
        .lock()
        .map_err(|_| "Audio feedback is unavailable")?;
    state.0 = state.0.wrapping_add(1);
    let generation = state.0;
    let snapshot = Snapshot { volume, muted };
    state.1 = Some(snapshot.clone());
    let window = app
        .get_webview_window("audio-feedback")
        .ok_or("Audio overlay is unavailable")?;
    let monitor = app
        .get_webview_window("main")
        .and_then(|main| main.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let size = window.outer_size().map_err(|e| e.to_string())?;
        let (x, y) = placement(
            area.position.x,
            area.position.y,
            area.size.width,
            area.size.height,
            size.width,
            size.height,
            (32.0 * monitor.scale_factor()) as u32,
        );
        window
            .set_position(tauri::PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
    }
    window
        .set_ignore_cursor_events(true)
        .map_err(|e| e.to_string())?;
    window
        .emit("audio-feedback", snapshot)
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2200));
        let state = app.state::<Feedback>();
        if let Ok(state) = state.0.lock() {
            if state.0 == generation {
                if let Some(window) = app.get_webview_window("audio-feedback") {
                    let _ = window.hide();
                }
            }
        }
    });
    Ok(())
}

fn placement(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    overlay_width: u32,
    overlay_height: u32,
    margin: u32,
) -> (i32, i32) {
    (
        x + (width.saturating_sub(overlay_width) / 2) as i32,
        y + height.saturating_sub(overlay_height.saturating_add(margin)) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bottom_center_respects_negative_origins_and_small_work_areas() {
        assert_eq!(placement(-1920, 0, 1920, 1040, 320, 88, 32), (-1120, 920));
        assert_eq!(placement(0, 0, 100, 50, 320, 88, 32), (0, 0));
    }
}
