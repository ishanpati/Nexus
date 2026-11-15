use super::*;
use crate::action_registry::{ErrorCode, RegistryError};
use serde_json::Value;

/// Admission is serialized with cancellation. Once admitted, an external operation
/// may finish even if cancellation arrives; never pretend it has been rolled back.
pub(super) struct Guarded<'a> {
    pub native: &'a mut dyn ActionRuntime,
    pub run: &'a Run,
    pub approved_until: Option<Instant>,
}
impl Guarded<'_> {
    fn admit(&self) -> crate::action_registry::Result<()> {
        let mut s = self.run.state();
        if s.snapshot.cancel_requested {
            return Err(RegistryError::new(
                ErrorCode::ExecutionFailed,
                "Run was cancelled before action admission.",
            ));
        }
        if self.approved_until.is_some_and(|d| Instant::now() >= d) {
            return Err(RegistryError::new(
                ErrorCode::ConfirmationRequired,
                "Approval expired before action admission; no action was invoked.",
            ));
        }
        s.snapshot.action_in_flight = true;
        s.snapshot.sequence += 1;
        Ok(())
    }
}
impl ActionRuntime for Guarded<'_> {
    fn check_application(&mut self, id: &str) -> crate::action_registry::Result<()> {
        self.native.check_application(id)
    }
    fn inspect_path(
        &mut self,
        path: &str,
        directory: bool,
    ) -> crate::action_registry::Result<bool> {
        if self.native.inspect_path(path, directory)? {
            return Err(RegistryError::new(
                ErrorCode::UnsupportedAction,
                "Workflows cannot open executable, script, shortcut, macro-capable or unknown file types. Select a document or a catalog application.",
            ));
        }
        Ok(false)
    }
    fn launch_application(&mut self, id: &str) -> crate::action_registry::Result<Value> {
        self.admit()?;
        self.native.launch_application(id)
    }
    fn open_path(
        &mut self,
        path: &str,
        directory: bool,
        _: bool,
    ) -> crate::action_registry::Result<Value> {
        self.inspect_path(path, directory)?;
        self.admit()?;
        // Never grant file execution authority. The native opener checks sensitivity
        // again on its resolved target, closing the preflight/activation change window.
        self.native.open_path(path, directory, false)
    }
    fn audio(
        &mut self,
        request: crate::audio::AudioRequest,
    ) -> crate::action_registry::Result<Value> {
        self.admit()?;
        self.native.audio(request)
    }
    fn settings(&mut self, page: &str) -> crate::action_registry::Result<Value> {
        self.admit()?;
        self.native.settings(page)
    }
    fn lock(&mut self) -> crate::action_registry::Result<Value> {
        self.admit()?;
        self.native.lock()
    }
    fn sleep(&mut self) -> crate::action_registry::Result<Value> {
        self.admit()?;
        self.native.sleep()
    }
}
