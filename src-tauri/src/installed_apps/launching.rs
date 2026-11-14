use super::{CatalogCache, InstalledApp, LaunchTarget};
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub struct LaunchActivity(AtomicBool);
impl LaunchActivity {
    pub fn is_active(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    pub(crate) fn acquire(self: &Arc<Self>) -> Result<LaunchGuard, String> {
        self.0
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "An application is already opening. Please wait.".to_string())?;
        Ok(LaunchGuard(self.clone()))
    }
}
pub(crate) struct LaunchGuard(Arc<LaunchActivity>);
impl Drop for LaunchGuard {
    fn drop(&mut self) {
        self.0.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Serialize)]
pub struct LaunchOutcome {
    pub launched: bool,
    // A hide failure must not invite a duplicate launch or discard usage credit.
    pub warning: Option<String>,
}

pub(super) fn valid_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.encode_utf16().count() < 32768
        && !value.chars().any(char::is_control)
}

pub(super) fn validate(app: &InstalledApp) -> Result<(), String> {
    let valid = match &app.launch {
        LaunchTarget::Shortcut { path, target, .. } => {
            valid_text(path)
                && valid_text(target)
                && std::path::Path::new(path).is_absolute()
                && std::path::Path::new(path)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
        }
        LaunchTarget::PackagedApp {
            app_user_model_id, ..
        } => valid_text(app_user_model_id),
        LaunchTarget::ShellApp { parsing_name, .. } => valid_text(parsing_name),
    };
    if valid {
        Ok(())
    } else {
        Err("This application's launch information is invalid. Refresh apps and try again.".into())
    }
}

fn finish(
    launch: impl FnOnce() -> Result<(), String>,
    hide: impl FnOnce() -> Result<(), String>,
) -> Result<LaunchOutcome, String> {
    launch()?;
    Ok(LaunchOutcome { launched: true, warning: hide().err().map(|error|
        format!("The application opened, but NEXUS could not hide. Press Escape; do not launch it again. {error}")) })
}

#[tauri::command]
pub async fn launch_installed_app(
    app_id: String,
    cache: tauri::State<'_, Arc<CatalogCache>>,
    activity: tauri::State<'_, Arc<LaunchActivity>>,
    window: tauri::WebviewWindow,
) -> Result<LaunchOutcome, String> {
    let _guard = activity.inner().acquire()?;
    let cache = cache.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let (_, app) = cache.lookup(&app_id)?;
        validate(&app)?;
        #[cfg(windows)]
        {
            super::native::on_sta(move || super::native::launch(&app))
        }
        #[cfg(not(windows))]
        {
            Err("Application launching is supported on Windows only.".into())
        }
    })
    .await
    .map_err(|e| format!("Could not complete the launch request: {e}"))?;
    finish(|| result, || window.hide().map_err(|e| e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_does_not_hide_and_success_hides_once() {
        assert!(
            finish(
                || Err("Windows refused launch".into()),
                || panic!("must stay visible")
            )
            .unwrap_err()
            .contains("Windows refused")
        );
        let hidden = std::cell::Cell::new(0);
        let outcome = finish(
            || Ok(()),
            || {
                hidden.set(hidden.get() + 1);
                Ok(())
            },
        )
        .unwrap();
        assert!(outcome.launched);
        assert!(outcome.warning.is_none());
        assert_eq!(hidden.get(), 1);
    }
    #[test]
    fn hide_failure_is_success_with_warning_not_a_second_launch() {
        let result = finish(|| Ok(()), || Err("window unavailable".into())).unwrap();
        assert!(result.launched);
        assert!(result.warning.unwrap().contains("application opened"));
    }
    #[test]
    fn launch_guard_rejects_concurrent_requests_and_releases_after_failure() {
        let activity = Arc::new(LaunchActivity::default());
        let guard = activity.acquire().unwrap();
        assert!(activity.acquire().is_err());
        drop(guard);
        assert!(!activity.is_active());
        assert!(activity.acquire().is_ok());
    }
    #[test]
    fn malformed_targets_are_rejected() {
        for name in ["", " \t", "bad\0target", "bad\nname"] {
            assert!(!valid_text(name));
        }
        let mut app = InstalledApp::new(
            "Bad".into(),
            LaunchTarget::ShellApp {
                parsing_name: "".into(),
                app_user_model_id: None,
            },
            None,
            super::super::Source::AppsFolder,
        );
        assert!(validate(&app).is_err());
        app.launch = LaunchTarget::Shortcut {
            path: "relative.exe".into(),
            target: "app.exe".into(),
            arguments: "".into(),
            working_directory: "".into(),
            show_command: 1,
            app_user_model_id: None,
        };
        assert!(validate(&app).is_err());
    }
}
