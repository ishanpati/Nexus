use tauri::{Emitter, PhysicalPosition, PhysicalSize, WebviewWindow};

// Physical coordinates preserve monitor origins and Windows display scaling.
fn upper_center(
    origin: PhysicalPosition<i32>,
    work_area: PhysicalSize<u32>,
    window: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let spare_width = work_area.width.saturating_sub(window.width);
    let spare_height = work_area.height.saturating_sub(window.height);
    PhysicalPosition::new(
        origin.x + (spare_width / 2) as i32,
        origin.y + ((work_area.height as f64 * 0.16) as u32).min(spare_height) as i32,
    )
}

/// All entry points that summon NEXUS use this function. Dragged positions are
/// intentionally temporary; focus changes alone must never reposition the bar.
pub fn show(window: &WebviewWindow) -> tauri::Result<()> {
    let active = crate::active_monitor::center()
        .and_then(|(x, y)| window.monitor_from_point(x, y).ok().flatten());
    let monitor = match active {
        Some(monitor) => Some(monitor),
        None => window.primary_monitor()?,
    };
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let logical_size = window
            .inner_size()?
            .to_logical::<f64>(window.scale_factor()?);
        let size = logical_size.to_physical::<u32>(monitor.scale_factor());
        window.set_position(upper_center(area.position, area.size, size))?;
        window.set_size(size)?;
    } else {
        window.center()?;
    }
    window.show()?;
    window.set_focus()?;
    window.emit("launcher-shown", ())
}

pub fn toggle(window: &WebviewWindow) -> tauri::Result<()> {
    if window.is_visible()? {
        window.hide()
    } else {
        show(window)
    }
}

#[tauri::command]
pub fn show_launcher(window: WebviewWindow) -> Result<(), String> {
    show(&window).map_err(|error| format!("Could not show NEXUS at its default position: {error}"))
}

#[tauri::command]
pub fn start_launcher_drag(window: WebviewWindow) -> Result<(), String> {
    window
        .start_dragging()
        .map_err(|error| format!("Could not move the NEXUS window: {error}"))
}

#[tauri::command]
pub fn hide_launcher(window: WebviewWindow) -> Result<(), String> {
    window
        .hide()
        .map_err(|error| format!("Windows could not hide the command bar: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_with_scaled_physical_dimensions() {
        assert_eq!(
            upper_center((0, 0).into(), (1920, 1080).into(), (975, 465).into()),
            PhysicalPosition::new(472, 172)
        );
    }

    #[test]
    fn respects_negative_monitor_origin_and_taskbar_offset() {
        assert_eq!(
            upper_center((-1920, 40).into(), (1920, 1040).into(), (650, 310).into()),
            PhysicalPosition::new(-1285, 206)
        );
    }

    #[test]
    fn does_not_underflow_on_small_work_areas() {
        assert_eq!(
            upper_center((10, 20).into(), (600, 250).into(), (650, 310).into()),
            PhysicalPosition::new(10, 20)
        );
    }
}
