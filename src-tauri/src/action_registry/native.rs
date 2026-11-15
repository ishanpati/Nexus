//! Thin adapters only. Workflow execution calls dispatch on a backend worker.
//! No Windows implementation, UI lifecycle, or workflow loop is duplicated here.
use super::*;
use crate::{
    audio::AudioService,
    installed_apps::{CatalogCache, launching::LaunchActivity},
    system_actions::SystemActions,
};
use std::sync::Arc;

pub struct NativeRuntime<'a> {
    pub catalog: &'a CatalogCache,
    pub audio: &'a AudioService,
    pub system: &'a SystemActions,
    pub activity: &'a Arc<LaunchActivity>,
}
fn failure(message: String) -> RegistryError {
    RegistryError::new(ErrorCode::ExecutionFailed, message)
}
fn reference(message: String) -> RegistryError {
    RegistryError::new(ErrorCode::InvalidReference, message)
}
fn serialized(value: impl Serialize) -> Result<Value> {
    serde_json::to_value(value).map_err(|e| failure(e.to_string()))
}
impl NativeRuntime<'_> {
    fn system_request(&self, request: Value, confirmed: bool) -> Result<Value> {
        let _guard = self
            .activity
            .acquire()
            .map_err(|message| RegistryError::new(ErrorCode::Busy, message))?;
        let outcome = self
            .system
            .execute_registered(request, confirmed)
            .map_err(|error| RegistryError {
                details: serde_json::to_value(&error).ok(),
                ..failure(error.message)
            })?;
        serialized(outcome)
    }
}
impl ActionRuntime for NativeRuntime<'_> {
    fn check_application(&mut self, id: &str) -> Result<()> {
        crate::installed_apps::launching::check_catalog_app(self.catalog, id).map_err(reference)
    }
    fn inspect_path(&mut self, path: &str, directory: bool) -> Result<bool> {
        crate::file_actions::inspect_selected(path, directory).map_err(reference)
    }
    fn launch_application(&mut self, id: &str) -> Result<Value> {
        let _guard = self
            .activity
            .acquire()
            .map_err(|message| RegistryError::new(ErrorCode::Busy, message))?;
        crate::installed_apps::launching::launch_catalog_app(self.catalog, id).map_err(failure)?;
        Ok(serde_json::json!({ "accepted": true, "readiness_verified": false }))
    }
    fn open_path(&mut self, path: &str, directory: bool, confirmed: bool) -> Result<Value> {
        let _guard = self
            .activity
            .acquire()
            .map_err(|message| RegistryError::new(ErrorCode::Busy, message))?;
        match crate::file_actions::open_selected(path, directory, confirmed).map_err(failure)? {
            crate::file_actions::FileOutcome::ConfirmationRequired { message, .. } => {
                Err(RegistryError::new(ErrorCode::ConfirmationRequired, message))
            }
            outcome => serialized(outcome),
        }
    }
    fn audio(&mut self, request: crate::audio::AudioRequest) -> Result<Value> {
        let state = self.audio.execute(request).map_err(|error| RegistryError {
            details: serde_json::to_value(&error).ok(),
            ..failure(error.message)
        })?;
        serialized(state)
    }
    fn settings(&mut self, page: &str) -> Result<Value> {
        self.system_request(
            serde_json::json!({ "id": "open_settings", "args": { "page": page } }),
            false,
        )
    }
    fn lock(&mut self) -> Result<Value> {
        self.system_request(serde_json::json!({ "id": "lock", "args": {} }), true)
    }
    fn sleep(&mut self) -> Result<Value> {
        self.system_request(serde_json::json!({ "id": "sleep", "args": {} }), true)
    }
}
