use crate::action_registry::{RegistryError, ValidatedAction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pending,
    Running,
    ConfirmationRequired,
    Succeeded,
    Failed,
    Cancelled,
}
impl Status {
    pub(super) fn terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Failure {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}
impl Failure {
    pub(super) fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
}
impl From<RegistryError> for Failure {
    fn from(error: RegistryError) -> Self {
        Self {
            code: serde_json::to_value(error.code)
                .unwrap()
                .as_str()
                .unwrap()
                .into(),
            message: error.message.clone(),
            details: serde_json::to_value(error).ok(),
        }
    }
}
impl From<crate::workflows::Error> for Failure {
    fn from(error: crate::workflows::Error) -> Self {
        Self {
            code: error.code,
            message: error.message,
            details: None,
        }
    }
}
pub type Result<T> = std::result::Result<T, Failure>;

#[derive(Clone, Debug, Serialize)]
pub struct StepState {
    pub step_id: Uuid,
    pub action_id: String,
    pub status: Status,
    pub error: Option<Failure>,
    pub output: Option<Value>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Confirmation {
    pub token: Uuid,
    pub step_id: Uuid,
    pub action: ValidatedAction,
    pub message: String,
    pub expires_in_ms: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub run_id: Uuid,
    pub workflow_id: Uuid,
    pub workflow_revision: Option<u64>,
    pub sequence: u64,
    pub status: Status,
    pub cancel_requested: bool,
    /// True means the current external call has been admitted and cannot be undone.
    pub action_in_flight: bool,
    pub steps: Vec<StepState>,
    pub confirmation: Option<Confirmation>,
    pub error: Option<Failure>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    RunStarted,
    StepStarted,
    StepSucceeded,
    StepFailed,
    ConfirmationRequested,
    RunSucceeded,
    RunFailed,
    RunCancelled,
}
/// Notifications contain identifiers only; fetch the authoritative snapshot for details.
#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub kind: EventKind,
    pub run_id: Uuid,
    pub workflow_id: Uuid,
    pub step_id: Option<Uuid>,
    pub sequence: u64,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approve,
    Deny,
}
