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
    pub accepted: bool,
    pub window_detected: bool,
    pub activation: super::foreground::Activation,
    pub readiness_verified: bool,
    // A hide failure must not invite a duplicate launch or discard usage credit.
    pub warning: Option<String>,
}
impl LaunchOutcome {
    pub(super) fn accepted(
        window_detected: bool,
        activation: super::foreground::Activation,
        warning: Option<String>,
    ) -> Self {
        Self {
            launched: true,
            accepted: true,
            window_detected,
            activation,
            readiness_verified: false,
            warning,
        }
    }
}

/// A workflow's activation permission belongs to the original user gesture,
/// rather than being renewed by every later step after the user moved on.
pub(crate) fn activation_intent() -> impl Fn() -> bool + Send + Sync {
    #[cfg(windows)]
    {
        super::foreground_windows::intent()
    }
    #[cfg(not(windows))]
    {
        || false
    }
}

pub(super) fn valid_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.encode_utf16().count() < 32768
        && !value.chars().any(char::is_control)
}

pub(crate) fn validate(app: &InstalledApp) -> Result<(), String> {
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

#[tauri::command]
pub async fn launch_installed_app(
    app_id: String,
    cache: tauri::State<'_, Arc<CatalogCache>>,
    activity: tauri::State<'_, Arc<LaunchActivity>>,
    window: tauri::WebviewWindow,
) -> Result<LaunchOutcome, String> {
    let guard = activity.inner().acquire()?;
    // Capture on admission, before worker queueing, COM setup or catalog reads.
    let intent = activation_intent();
    let launcher_owner = super::foreground::interaction();
    let cache = cache.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        launch_catalog_app_with_context(
            &cache,
            &app_id,
            &|| {
                if !super::foreground::owns_launcher(
                    launcher_owner,
                    super::foreground::interaction(),
                    window.is_focused().map_err(|e| e.to_string())?,
                    intent(),
                ) {
                    return Ok(());
                }
                window
                    .hide()
                    // Flush the window dispatcher before sampling the restored
                    // foreground; hide itself may only enqueue a native message.
                    .and_then(|_| window.is_visible().map(|_| ()))
                    .map_err(|e| format!("Application opened, but NEXUS could not hide: {e}"))
            },
            &|| !intent(),
        )
    })
    .await
    .map_err(|e| format!("Could not complete the launch request: {e}"))?
}

pub(crate) fn launch_catalog_app_with_context(
    cache: &CatalogCache,
    app_id: &str,
    hide: &(dyn Fn() -> Result<(), String> + Sync),
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<LaunchOutcome, String> {
    let (_, app) = cache.lookup(app_id)?;
    validate(&app)?;
    #[cfg(windows)]
    {
        super::native::on_sta(move || {
            let app = if let LaunchTarget::Shortcut { path, .. } = &app.launch {
                super::windows::shortcut(std::path::Path::new(path), super::Source::UserStartMenu)?
                    .ok_or("This shortcut is no longer available.")?
            } else {
                app
            };
            super::foreground_windows::launch(&app, hide, cancelled)
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (hide, cancelled);
        Err("Application launching is supported on Windows only.".into())
    }
}

pub(crate) fn check_catalog_app(cache: &CatalogCache, app_id: &str) -> Result<(), String> {
    let (_, app) = cache.lookup(app_id)?;
    validate(&app)?;
    #[cfg(windows)]
    {
        super::native::on_sta(move || super::native::check_available(&app))
    }
    #[cfg(not(windows))]
    {
        Err("Application launching is supported on Windows only.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_launch_is_success_for_all_activation_outcomes_and_hide_warnings() {
        use super::super::foreground::Activation::*;
        for activation in [Foreground, Denied, TimedOut, Suppressed] {
            let result = LaunchOutcome::accepted(
                activation == Foreground || activation == Denied,
                activation,
                Some("hide failed".into()),
            );
            assert!(result.launched && result.accepted);
            assert!(!result.readiness_verified);
            assert_eq!(result.warning.as_deref(), Some("hide failed"));
            assert_eq!(serde_json::to_value(&result).unwrap()["accepted"], true);
        }
    }
    #[test]
    fn launch_failure_retains_launcher() {
        let result = launch_catalog_app_with_context(
            &CatalogCache::default(),
            "missing",
            &|| panic!("must not hide after failure"),
            &|| false,
        );
        assert!(result.is_err());
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
