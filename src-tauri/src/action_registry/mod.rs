//! Backend-owned, single-action building blocks. Workflow storage and execution
//! compose this registry in separate modules; this module exposes metadata IPC.
mod builtins;
pub mod native;
mod schema;
#[cfg(test)]
mod tests;

pub use schema::{Parameter, ParameterType};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::HashSet,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Icon {
    Application,
    File,
    Folder,
    Volume,
    Settings,
    Lock,
    Moon,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationPolicy {
    None,
    SensitiveTarget,
    Always,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionDescriptor {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub icon: Icon,
    pub parameters: Vec<Parameter>,
    pub workflow_compatible: bool,
    pub confirmation: ConfirmationPolicy,
    pub terminal: bool,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    pub action_id: String,
    pub schema_version: u32,
    pub parameters: Value,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnknownAction,
    UnsupportedVersion,
    InvalidParameters,
    InvalidRegistration,
    UnsupportedAction,
    InvalidPosition,
    ConfirmationRequired,
    InvalidReference,
    ExecutionFailed,
    Busy,
}

#[derive(Debug, Serialize)]
pub struct RegistryError {
    pub code: ErrorCode,
    pub message: String,
    pub parameter: Option<String>,
    pub details: Option<Value>,
}
impl RegistryError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            parameter: None,
            details: None,
        }
    }
    fn field(parameter: &str, message: impl Into<String>) -> Self {
        Self {
            parameter: Some(parameter.into()),
            ..Self::new(ErrorCode::InvalidParameters, message)
        }
    }
}
pub type Result<T> = std::result::Result<T, RegistryError>;
pub type Arguments = Map<String, Value>;

/// Runtime inputs are resolved afresh, not copied from workflow-supplied targets.
pub trait ActionRuntime {
    fn check_application(&mut self, id: &str) -> Result<()>;
    fn inspect_path(&mut self, path: &str, directory: bool) -> Result<bool>;
    fn launch_application(&mut self, id: &str) -> Result<Value>;
    fn open_path(&mut self, path: &str, directory: bool, confirmed: bool) -> Result<Value>;
    fn audio(&mut self, request: crate::audio::AudioRequest) -> Result<Value>;
    fn settings(&mut self, page: &str) -> Result<Value>;
    fn lock(&mut self) -> Result<Value>;
    fn sleep(&mut self) -> Result<Value>;
}

pub type Validator = fn(&Arguments, &mut dyn ActionRuntime) -> Result<bool>;
pub type Executor = fn(&Arguments, &mut dyn ActionRuntime, &Authorization) -> Result<Value>;

/// Not serializable or constructible by workflow input. Passed only after policy checks.
pub struct Authorization {
    confirmed: bool,
}
impl Authorization {
    pub fn is_confirmed(&self) -> bool {
        self.confirmed
    }
}

#[derive(Clone)]
pub struct Registration {
    pub descriptor: ActionDescriptor,
    pub validate: Validator,
    pub execute: Executor,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidatedAction {
    pub action_id: String,
    pub schema_version: u32,
    pub parameters: Arguments,
    pub confirmation: ConfirmationPolicy,
    pub terminal: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DispatchOutcome {
    ConfirmationRequired {
        token: String,
        action: ValidatedAction,
        message: String,
    },
    Succeeded {
        output: Value,
    },
}

/// Trusted runner context, deliberately separate from ActionRequest. The runner
/// derives this from actual plan order, never a stored action parameter.
#[derive(Clone, Copy)]
pub enum StepPosition {
    Intermediate,
    Final,
}
struct Pending {
    token: String,
    request: ActionRequest,
    issued: Instant,
}
#[derive(Default)]
struct Confirmations {
    next: u64,
    pending: Option<Pending>,
}
pub struct ActionRegistry {
    entries: Vec<Registration>,
    confirmations: Mutex<Confirmations>,
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new(builtins::registrations()).expect("built-in action descriptors must be valid")
    }
}
impl ActionRegistry {
    /// Independent confirmation scope for one workflow run; registrations stay backend-owned.
    pub(crate) fn for_run(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            confirmations: Mutex::default(),
        }
    }

    /// Read-only prerequisite checks, without issuing approval or executing an action.
    pub(crate) fn preflight(
        &self,
        request: &ActionRequest,
        runtime: &mut dyn ActionRuntime,
        position: StepPosition,
    ) -> Result<bool> {
        let action = self.validate(request)?;
        if action.terminal && matches!(position, StepPosition::Intermediate) {
            return Err(RegistryError::new(
                ErrorCode::InvalidPosition,
                "Terminal actions must be last.",
            ));
        }
        (self.lookup(&action.action_id)?.validate)(&action.parameters, runtime)
    }
    pub fn new(entries: Vec<Registration>) -> Result<Self> {
        let mut ids = HashSet::new();
        for entry in &entries {
            let d = &entry.descriptor;
            let parts: Vec<_> = d.id.split('.').collect();
            if parts.len() < 2
                || d.id.len() > 128
                || parts.iter().any(|part| {
                    part.is_empty()
                        || !part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                })
                || !ids.insert(d.id.clone())
                || d.schema_version == 0
                || d.display_name.trim().is_empty()
                || d.description.trim().is_empty()
                || d.category.trim().is_empty()
                || (d.terminal && d.confirmation != ConfirmationPolicy::Always)
            {
                return Err(RegistryError::new(
                    ErrorCode::InvalidRegistration,
                    "Invalid or duplicate action definition; terminal actions must require confirmation.",
                ));
            }
            schema::check_schema(&d.parameters)?;
        }
        Ok(Self {
            entries,
            confirmations: Mutex::default(),
        })
    }
    pub fn descriptors(&self) -> Vec<ActionDescriptor> {
        self.entries
            .iter()
            .filter(|e| e.descriptor.workflow_compatible)
            .map(|e| e.descriptor.clone())
            .collect()
    }
    fn lookup(&self, id: &str) -> Result<&Registration> {
        self.entries
            .iter()
            .find(|e| e.descriptor.id == id)
            .ok_or_else(|| {
                RegistryError::new(
                    ErrorCode::UnknownAction,
                    "Unknown action ID. Select an action from the backend registry.",
                )
            })
    }
    pub fn descriptor(&self, id: &str) -> Result<&ActionDescriptor> {
        Ok(&self.lookup(id)?.descriptor)
    }
    pub fn validate(&self, request: &ActionRequest) -> Result<ValidatedAction> {
        let d = &self.lookup(&request.action_id)?.descriptor;
        if !d.workflow_compatible {
            return Err(RegistryError::new(
                ErrorCode::UnsupportedAction,
                "This action is not workflow-compatible.",
            ));
        }
        if request.schema_version != d.schema_version {
            return Err(RegistryError::new(
                ErrorCode::UnsupportedVersion,
                format!(
                    "{} needs action schema version {}. Explicit migration is required; input was not changed.",
                    d.id, d.schema_version
                ),
            ));
        }
        Ok(ValidatedAction {
            action_id: d.id.clone(),
            schema_version: d.schema_version,
            parameters: schema::validate(&d.parameters, &request.parameters)?,
            confirmation: d.confirmation,
            terminal: d.terminal,
        })
    }
    /// Single action only; no workflow loop, UI hiding or automatic retries.
    pub fn dispatch(
        &self,
        request: &ActionRequest,
        runtime: &mut dyn ActionRuntime,
        position: StepPosition,
        token: Option<&str>,
    ) -> Result<DispatchOutcome> {
        self.dispatch_with_clock(request, runtime, position, token, Instant::now)
    }
    #[cfg(test)]
    fn dispatch_at(
        &self,
        request: &ActionRequest,
        runtime: &mut dyn ActionRuntime,
        position: StepPosition,
        token: Option<&str>,
        now: Instant,
    ) -> Result<DispatchOutcome> {
        self.dispatch_with_clock(request, runtime, position, token, || now)
    }
    fn dispatch_with_clock(
        &self,
        request: &ActionRequest,
        runtime: &mut dyn ActionRuntime,
        position: StepPosition,
        token: Option<&str>,
        clock: impl FnOnce() -> Instant,
    ) -> Result<DispatchOutcome> {
        let action = self.validate(request)?;
        if action.terminal && matches!(position, StepPosition::Intermediate) {
            return Err(RegistryError::new(
                ErrorCode::InvalidPosition,
                "This action requires confirmation and is valid only as the final step.",
            ));
        }
        let entry = self.lookup(&action.action_id)?;
        let sensitive = (entry.validate)(&action.parameters, runtime)?;
        let required = action.confirmation == ConfirmationPolicy::Always
            || (action.confirmation == ConfirmationPolicy::SensitiveTarget && sensitive);
        let canonical = ActionRequest {
            action_id: action.action_id.clone(),
            schema_version: action.schema_version,
            parameters: Value::Object(action.parameters.clone()),
        };
        // Reference validation can block on Windows. Check freshness afterwards,
        // immediately before consuming authority, rather than at dispatch entry.
        let now = clock();
        if required || token.is_some() {
            let mut state = self.confirmations.lock().map_err(|_| {
                RegistryError::new(
                    ErrorCode::Busy,
                    "Confirmation state is unavailable. Restart NEXUS.",
                )
            })?;
            if let Some(token) = token {
                let valid = state.pending.as_ref().is_some_and(|pending| {
                    pending.token == token
                        && pending.request == canonical
                        && now.saturating_duration_since(pending.issued) < Duration::from_secs(60)
                });
                if !valid {
                    return Err(RegistryError::new(
                        ErrorCode::ConfirmationRequired,
                        "Confirmation is expired, changed or already used. Request confirmation again.",
                    ));
                }
                state.pending = None;
            } else {
                state.next = state.next.checked_add(1).ok_or_else(|| {
                    RegistryError::new(ErrorCode::Busy, "Restart NEXUS to renew confirmations.")
                })?;
                let token = state.next.to_string();
                state.pending = Some(Pending {
                    token: token.clone(),
                    request: canonical,
                    issued: now,
                });
                return Ok(DispatchOutcome::ConfirmationRequired {
                    token,
                    action,
                    message: format!(
                        "Confirm {} with the displayed parameters before executing.",
                        entry.descriptor.display_name
                    ),
                });
            }
        }
        let authorization = Authorization {
            confirmed: required && token.is_some(),
        };
        (entry.execute)(&action.parameters, runtime, &authorization)
            .map(|output| DispatchOutcome::Succeeded { output })
    }
    pub fn cancel_confirmation(&self, token: &str) -> Result<()> {
        let mut state = self.confirmations.lock().map_err(|_| {
            RegistryError::new(ErrorCode::Busy, "Confirmation state is unavailable.")
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

#[tauri::command]
pub fn list_workflow_actions(registry: tauri::State<'_, ActionRegistry>) -> Vec<ActionDescriptor> {
    registry.descriptors()
}
