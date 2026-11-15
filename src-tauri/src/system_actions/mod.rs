//! Fixed Settings destinations and local workstation actions. Never accepts a URI,
//! executable, shell text, or caller-controlled power flags.
#[cfg(windows)]
mod native;
#[cfg(test)]
mod tests;
use crate::installed_apps::launching::LaunchActivity;
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SettingsPage {
    Sound,
    Wifi,
    Bluetooth,
    Display,
}
impl SettingsPage {
    pub fn uri(self) -> &'static str {
        match self {
            Self::Sound => "ms-settings:sound",
            Self::Wifi => "ms-settings:network-wifi",
            Self::Bluetooth => "ms-settings:bluetooth",
            Self::Display => "ms-settings:display",
        }
    }
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsArgs {
    page: SettingsPage,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(
    tag = "id",
    content = "args",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SystemRequest {
    OpenSettings(SettingsArgs),
    Lock(Empty),
    Sleep(Empty),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidInput,
    ConfirmationRequired,
    Busy,
    Unsupported,
    Restricted,
    NativeFailure,
}
#[derive(Debug, Serialize)]
pub struct ActionError {
    pub code: ErrorCode,
    pub message: String,
}
impl ActionError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
type Result<T> = std::result::Result<T, ActionError>;
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ActionOutcome {
    ConfirmationRequired {
        token: String,
        message: String,
    },
    Accepted {
        message: String,
        warning: Option<String>,
    },
}
struct Pending {
    token: String,
    issued: Instant,
}
#[derive(Default)]
struct Confirmations {
    next: u64,
    pending: Option<Pending>,
}
#[derive(Default)]
pub struct SystemActions(Mutex<Confirmations>);
const CONFIRMATION_TTL: Duration = Duration::from_secs(60);

trait Operations {
    fn settings(&self, page: SettingsPage) -> Result<Option<String>>;
    fn lock(&self) -> Result<Option<String>>;
    fn sleep(&self) -> Result<Option<String>>;
}
impl SystemActions {
    /// Registry-only adapter: the registry has already checked its own mandatory
    /// confirmation policy. Existing launcher confirmation tickets remain separate.
    pub(crate) fn execute_registered(
        &self,
        value: serde_json::Value,
        confirmed: bool,
    ) -> Result<ActionOutcome> {
        let request = parse(value)?;
        if matches!(request, SystemRequest::Lock(_) | SystemRequest::Sleep(_)) && !confirmed {
            return Err(ActionError::new(
                ErrorCode::ConfirmationRequired,
                "Confirm this terminal system action before executing.",
            ));
        }
        #[cfg(windows)]
        {
            // Use a separate ticket scope so registry dispatch cannot replace a
            // pending launcher sleep prompt. Both scopes use the same native handler.
            let scope = SystemActions::default();
            let first = scope.dispatch(request, None, &native::WindowsOperations)?;
            if let ActionOutcome::ConfirmationRequired { token, .. } = first {
                scope.dispatch(request, Some(&token), &native::WindowsOperations)
            } else {
                Ok(first)
            }
        }
        #[cfg(not(windows))]
        {
            Err(ActionError::new(
                ErrorCode::Unsupported,
                "These actions require Windows.",
            ))
        }
    }
    fn dispatch(
        &self,
        request: SystemRequest,
        token: Option<&str>,
        operations: &impl Operations,
    ) -> Result<ActionOutcome> {
        self.dispatch_at(request, token, operations, Instant::now())
    }
    fn dispatch_at(
        &self,
        request: SystemRequest,
        token: Option<&str>,
        operations: &impl Operations,
        now: Instant,
    ) -> Result<ActionOutcome> {
        if matches!(request, SystemRequest::Sleep(_)) {
            let mut state = self.0.lock().map_err(|_| {
                ActionError::new(
                    ErrorCode::NativeFailure,
                    "Sleep confirmation is unavailable. Restart NEXUS.",
                )
            })?;
            if let Some(token) = token {
                let valid = state.pending.as_ref().is_some_and(|pending| {
                    pending.token == token && now.duration_since(pending.issued) < CONFIRMATION_TTL
                });
                if !valid {
                    return Err(ActionError::new(
                        ErrorCode::ConfirmationRequired,
                        "Sleep confirmation expired or was already used. Select sleep and confirm again.",
                    ));
                }
                state.pending = None; // Consume before invoking the native action, including failures.
            } else {
                state.next = state.next.checked_add(1).ok_or_else(|| {
                    ActionError::new(
                        ErrorCode::NativeFailure,
                        "Restart NEXUS to request a new confirmation.",
                    )
                })?;
                let token = state.next.to_string(); // One-use ticket, not an authentication credential.
                state.pending = Some(Pending {
                    token: token.clone(),
                    issued: now,
                });
                return Ok(ActionOutcome::ConfirmationRequired { token, message: "Put this computer to sleep now? Running work and network activity may pause. Save your work first.".into() });
            }
        } else if token.is_some() {
            return Err(ActionError::new(
                ErrorCode::InvalidInput,
                "Confirmation tickets apply only to sleep.",
            ));
        }
        let (message, warning) = match request {
            SystemRequest::OpenSettings(args) => (
                "Windows accepted the Settings request.",
                operations.settings(args.page)?,
            ),
            SystemRequest::Lock(_) => ("Windows accepted the lock request.", operations.lock()?),
            SystemRequest::Sleep(_) => ("Windows accepted the sleep request.", operations.sleep()?),
        };
        Ok(ActionOutcome::Accepted {
            message: message.into(),
            warning,
        })
    }
    fn cancel(&self, token: &str) -> Result<()> {
        let mut state = self.0.lock().map_err(|_| {
            ActionError::new(
                ErrorCode::NativeFailure,
                "Could not clear sleep confirmation.",
            )
        })?;
        if state
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            state.pending = None;
        }
        Ok(())
    }
}

fn parse(value: serde_json::Value) -> Result<SystemRequest> {
    serde_json::from_value(value).map_err(|_| {
        ActionError::new(
            ErrorCode::InvalidInput,
            "Unknown system action or invalid arguments. Select a supported command.",
        )
    })
}
fn finish(
    outcome: ActionOutcome,
    hide: impl FnOnce() -> std::result::Result<(), String>,
) -> ActionOutcome {
    match outcome {
        ActionOutcome::Accepted { message, warning } => {
            let hide_warning = hide().err().map(|error| format!("The action was accepted, but NEXUS could not hide. Press Escape; do not repeat it. {error}"));
            ActionOutcome::Accepted {
                message,
                warning: match (warning, hide_warning) {
                    (Some(a), Some(b)) => Some(format!("{a} {b}")),
                    (a, b) => a.or(b),
                },
            }
        }
        other => other,
    }
}
#[tauri::command]
pub async fn execute_system_action(
    request: serde_json::Value,
    confirmation_token: Option<String>,
    state: tauri::State<'_, Arc<SystemActions>>,
    activity: tauri::State<'_, Arc<LaunchActivity>>,
    window: tauri::WebviewWindow,
) -> Result<ActionOutcome> {
    let request = parse(request)?;
    let _guard = activity
        .inner()
        .acquire()
        .map_err(|message| ActionError::new(ErrorCode::Busy, message))?;
    let state = state.inner().clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            state.dispatch(
                request,
                confirmation_token.as_deref(),
                &native::WindowsOperations,
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (state, request, confirmation_token);
            Err(ActionError::new(
                ErrorCode::Unsupported,
                "These actions require Windows.",
            ))
        }
    })
    .await
    .map_err(|_| {
        ActionError::new(
            ErrorCode::NativeFailure,
            "The system action worker stopped unexpectedly. Check Windows before retrying.",
        )
    })??;
    Ok(finish(outcome, || {
        window.hide().map_err(|error| error.to_string())
    }))
}
#[tauri::command]
pub fn cancel_system_confirmation(
    token: String,
    state: tauri::State<'_, Arc<SystemActions>>,
) -> Result<()> {
    state.cancel(&token)
}
