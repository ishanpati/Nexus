//! Backend-owned sequential runs. No window or request lifetime owns the worker.
mod ipc;
mod model;
mod runtime;
#[cfg(test)]
mod tests;
use crate::action_registry::{
    ActionRegistry, ActionRequest, ActionRuntime, DispatchOutcome, StepPosition,
};
use crate::workflows::Workflow;
pub use ipc::*;
pub use model::*;
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};
use uuid::Uuid;

const CONFIRMATION_TTL: Duration = Duration::from_secs(60);
const MAX_ACTIVE: usize = 8;
const MAX_RETAINED: usize = 128;
type Sink = Arc<dyn Fn(Progress) + Send + Sync>;

struct State {
    snapshot: Snapshot,
    approval: Option<Decision>,
    deadline: Option<Instant>,
}
struct Run {
    state: Mutex<State>,
    changed: Condvar,
    sink: Sink,
}
// No external action runs under this mutex. A panic in an executor cannot poison it.
impl Run {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn snapshot(&self) -> Snapshot {
        let s = self.state();
        let mut snapshot = s.snapshot.clone();
        if let Some(prompt) = &mut snapshot.confirmation {
            prompt.expires_in_ms = s.deadline.map_or(0, |d| {
                d.saturating_duration_since(Instant::now()).as_millis() as u64
            });
        }
        snapshot
    }
    fn event(&self, state: &mut State, kind: EventKind, index: Option<usize>) {
        state.snapshot.sequence += 1;
        let event = Progress {
            kind,
            run_id: state.snapshot.run_id,
            workflow_id: state.snapshot.workflow_id,
            step_id: index.map(|i| state.snapshot.steps[i].step_id),
            sequence: state.snapshot.sequence,
        };
        // Persist before notifying, in the same critical section, so events cannot overtake
        // one another. Sinks must be nonblocking and must not call back into the engine.
        (self.sink)(event);
    }
    fn finish(&self, status: Status, error: Option<Failure>, failed_step: Option<usize>) {
        let mut s = self.state();
        if s.snapshot.status.terminal() {
            return;
        }
        s.snapshot.status = status;
        s.snapshot.error = error.clone();
        s.snapshot.confirmation = None;
        s.snapshot.action_in_flight = false;
        s.deadline = None;
        s.approval = None;
        if let Some(i) = failed_step {
            s.snapshot.steps[i].status = Status::Failed;
            s.snapshot.steps[i].error = error;
            self.event(&mut s, EventKind::StepFailed, Some(i));
        }
        for step in &mut s.snapshot.steps {
            if !step.status.terminal() {
                step.status = Status::Cancelled;
                step.error = Some(Failure::new(
                    "not_executed",
                    "The run stopped before this step completed; completed external actions were not undone.",
                ));
            }
        }
        self.event(
            &mut s,
            match status {
                Status::Succeeded => EventKind::RunSucceeded,
                Status::Cancelled => EventKind::RunCancelled,
                _ => EventKind::RunFailed,
            },
            None,
        );
        self.changed.notify_all();
    }
    fn cancelled(&self) -> bool {
        self.state().snapshot.cancel_requested
    }
}

#[derive(Default)]
pub struct Engine {
    runs: Mutex<VecDeque<Arc<Run>>>,
}
impl Engine {
    fn reserve(&self, workflow_id: Uuid, sink: Sink) -> Result<Arc<Run>> {
        if workflow_id.is_nil() {
            return Err(Failure::new("invalid_id", "Use a saved workflow UUID."));
        }
        let mut runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
        let active: Vec<_> = runs
            .iter()
            .map(|r| r.snapshot())
            .filter(|s| !s.status.terminal())
            .collect();
        if active.iter().any(|s| s.workflow_id == workflow_id) {
            return Err(Failure::new(
                "already_running",
                "This workflow already has an active run.",
            ));
        }
        if active.len() >= MAX_ACTIVE {
            return Err(Failure::new(
                "busy",
                "Too many active workflows. Wait or cancel a run.",
            ));
        }
        if runs.len() >= MAX_RETAINED {
            if let Some(i) = runs.iter().position(|r| r.snapshot().status.terminal()) {
                runs.remove(i);
            }
        }
        let run = Arc::new(Run {
            state: Mutex::new(State {
                snapshot: Snapshot {
                    run_id: Uuid::new_v4(),
                    workflow_id,
                    workflow_revision: None,
                    sequence: 0,
                    status: Status::Pending,
                    cancel_requested: false,
                    action_in_flight: false,
                    steps: vec![],
                    confirmation: None,
                    error: None,
                },
                approval: None,
                deadline: None,
            }),
            changed: Condvar::new(),
            sink,
        });
        runs.push_back(run.clone());
        Ok(run)
    }
    fn find(&self, id: Uuid) -> Result<Arc<Run>> {
        self.runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|r| r.snapshot().run_id == id)
            .cloned()
            .ok_or_else(|| Failure::new("run_not_found", "Run is unknown or no longer retained."))
    }
    pub fn status(&self, id: Uuid) -> Result<Snapshot> {
        Ok(self.find(id)?.snapshot())
    }
    pub fn list(&self) -> Vec<Snapshot> {
        self.runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|r| r.snapshot())
            .collect()
    }
    pub fn has_active(&self) -> bool {
        self.runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|r| !r.state().snapshot.status.terminal())
    }
    pub fn cancel(&self, id: Uuid) -> Result<Snapshot> {
        let run = self.find(id)?;
        let mut s = run.state();
        if !s.snapshot.status.terminal() {
            s.snapshot.cancel_requested = true;
            s.snapshot.confirmation = None;
            s.approval = None;
            s.snapshot.sequence += 1;
            run.changed.notify_all();
        }
        Ok(s.snapshot.clone())
    }
    pub fn decide(
        &self,
        id: Uuid,
        step: Uuid,
        token: Uuid,
        decision: Decision,
    ) -> Result<Snapshot> {
        let run = self.find(id)?;
        let mut s = run.state();
        if s.snapshot.cancel_requested
            || s.snapshot.status != Status::ConfirmationRequired
            || s.approval.is_some()
            || s.deadline.is_none_or(|d| Instant::now() >= d)
            || !s
                .snapshot
                .confirmation
                .as_ref()
                .is_some_and(|p| p.step_id == step && p.token == token)
        {
            return Err(Failure::new(
                "invalid_confirmation",
                "Approval is expired, used, or belongs to another run or step.",
            ));
        }
        s.approval = Some(decision);
        if matches!(decision, Decision::Deny) {
            s.snapshot.cancel_requested = true;
        } else {
            // Return a useful state before Windows can immediately lock or suspend.
            s.snapshot.status = Status::Running;
            if let Some(pending) = s.snapshot.steps.iter_mut().find(|s| s.step_id == step) {
                pending.status = Status::Running;
            }
        }
        s.snapshot.confirmation = None; // consume public capability immediately
        s.snapshot.sequence += 1;
        run.changed.notify_all();
        Ok(s.snapshot.clone())
    }

    fn launch(
        &self,
        workflow_id: Uuid,
        sink: Sink,
        work: impl FnOnce(Arc<Run>) + Send + 'static,
    ) -> Result<Snapshot> {
        let run = self.reserve(workflow_id, sink)?;
        let initial = run.snapshot();
        let worker = run.clone();
        if std::thread::Builder::new().name("nexus-workflow".into()).spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(worker.clone())));
            if result.is_err() {
                let index = worker.snapshot().steps.iter().position(|s| matches!(s.status, Status::Running | Status::ConfirmationRequired));
                worker.finish(Status::Failed, Some(Failure::new("worker_panicked", "Worker stopped unexpectedly. The external action may have occurred; no retry was attempted.")), index);
            }
        }).is_err() {
            run.finish(Status::Failed, Some(Failure::new("worker_unavailable", "Could not start workflow worker.")), None);
            return Ok(run.snapshot());
        }
        Ok(initial)
    }
}

fn request(step: &crate::workflows::Step) -> ActionRequest {
    ActionRequest {
        action_id: step.action_id.clone(),
        schema_version: step.action_schema_version,
        parameters: step.parameters.clone(),
    }
}
fn position(i: usize, count: usize) -> StepPosition {
    if i + 1 == count {
        StepPosition::Final
    } else {
        StepPosition::Intermediate
    }
}

fn execute(
    run: &Arc<Run>,
    mut workflow: Workflow,
    registry: ActionRegistry,
    native: &mut dyn ActionRuntime,
    ttl: Duration,
) {
    if workflow.data.id != run.snapshot().workflow_id || workflow.data.steps.len() > 64 {
        run.finish(
            Status::Failed,
            Some(Failure::new(
                "invalid_workflow",
                "Stored workflow ID or step count is invalid.",
            )),
            None,
        );
        return;
    }
    workflow.data.steps.sort_by_key(|s| s.order);
    {
        let mut s = run.state();
        s.snapshot.workflow_revision = Some(workflow.revision);
        s.snapshot.steps = workflow
            .data
            .steps
            .iter()
            .map(|step| StepState {
                step_id: step.id,
                action_id: step.action_id.clone(),
                status: Status::Pending,
                error: None,
                output: None,
            })
            .collect();
        s.snapshot.sequence += 1;
    }
    let mut runtime = runtime::Guarded {
        native,
        run,
        approved_until: None,
    };
    // Check all schemas first, then all current references. Neither pass executes anything.
    for (i, step) in workflow.data.steps.iter().enumerate() {
        let result = if step.action_id.starts_with("workflow.") {
            Err(Failure::new(
                "recursive_workflow",
                "Workflow actions cannot invoke workflows in v0.5.",
            ))
        } else {
            registry
                .validate(&request(step))
                .map(|_| ())
                .map_err(Into::into)
        };
        if let Err(e) = result {
            run.finish(Status::Failed, Some(e), Some(i));
            return;
        }
    }
    if let Err(e) = crate::workflows::validate(&mut workflow.data, None, &registry) {
        run.finish(Status::Failed, Some(e.into()), None);
        return;
    }
    for (i, step) in workflow.data.steps.iter().enumerate() {
        if run.cancelled() {
            run.finish(Status::Cancelled, None, None);
            return;
        }
        if let Err(e) = registry.preflight(
            &request(step),
            &mut runtime,
            position(i, workflow.data.steps.len()),
        ) {
            run.finish(Status::Failed, Some(e.into()), Some(i));
            return;
        }
    }
    {
        let mut s = run.state();
        s.snapshot.status = Status::Running;
        run.event(&mut s, EventKind::RunStarted, None);
    }
    for (i, step) in workflow.data.steps.iter().enumerate() {
        if run.cancelled() {
            run.finish(Status::Cancelled, None, None);
            return;
        }
        {
            let mut s = run.state();
            s.snapshot.steps[i].status = Status::Running;
            run.event(&mut s, EventKind::StepStarted, Some(i));
        }
        runtime.approved_until = None;
        let req = request(step);
        let pos = position(i, workflow.data.steps.len());
        let mut result = registry.dispatch(&req, &mut runtime, pos, None);
        if let Ok(DispatchOutcome::ConfirmationRequired {
            token,
            action,
            message,
        }) = result
        {
            let deadline = Instant::now() + ttl;
            let mut s = run.state();
            if !s.snapshot.cancel_requested {
                s.deadline = Some(deadline);
                s.approval = None;
                s.snapshot.status = Status::ConfirmationRequired;
                s.snapshot.steps[i].status = Status::ConfirmationRequired;
                s.snapshot.confirmation = Some(Confirmation {
                    token: Uuid::new_v4(),
                    step_id: step.id,
                    action,
                    message,
                    expires_in_ms: ttl.as_millis() as u64,
                });
                run.event(&mut s, EventKind::ConfirmationRequested, Some(i));
            }
            while !s.snapshot.cancel_requested && s.approval.is_none() && Instant::now() < deadline
            {
                s = run
                    .changed
                    .wait_timeout(s, deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
            let cancelled = s.snapshot.cancel_requested;
            let approved = !cancelled
                && matches!(s.approval, Some(Decision::Approve))
                && Instant::now() < deadline;
            s.snapshot.confirmation = None;
            s.approval = None;
            s.deadline = None;
            if approved {
                s.snapshot.status = Status::Running;
                s.snapshot.steps[i].status = Status::Running;
                s.snapshot.sequence += 1;
            }
            drop(s);
            if !approved {
                let _ = registry.cancel_confirmation(&token);
                run.finish(
                    Status::Cancelled,
                    if cancelled {
                        None
                    } else {
                        Some(Failure::new(
                            "confirmation_timeout",
                            "Confirmation expired; the pending action was not invoked.",
                        ))
                    },
                    None,
                );
                return;
            }
            runtime.approved_until = Some(deadline);
            result = registry.dispatch(&req, &mut runtime, pos, Some(&token));
        }
        match result {
            Ok(DispatchOutcome::Succeeded { output }) => {
                let mut s = run.state();
                s.snapshot.action_in_flight = false;
                s.snapshot.steps[i].status = Status::Succeeded;
                s.snapshot.steps[i].output = Some(output);
                run.event(&mut s, EventKind::StepSucceeded, Some(i));
            }
            Err(e) => {
                // Cancellation only wins if the native operation was never admitted.
                let s = run.snapshot();
                if s.cancel_requested && !s.action_in_flight {
                    run.finish(Status::Cancelled, None, None);
                } else if !s.action_in_flight
                    && runtime.approved_until.is_some_and(|d| Instant::now() >= d)
                {
                    run.finish(Status::Cancelled, Some(Failure::new("confirmation_timeout", "Approval expired during prerequisite checks; the action was not invoked.")), None);
                } else {
                    run.finish(Status::Failed, Some(e.into()), Some(i));
                }
                return;
            }
            Ok(DispatchOutcome::ConfirmationRequired { .. }) => {
                run.finish(
                    Status::Failed,
                    Some(Failure::new(
                        "confirmation_changed",
                        "Action approval changed; start a fresh run.",
                    )),
                    Some(i),
                );
                return;
            }
        }
    }
    // Completion of the final admitted action wins over a late cancellation.
    run.finish(Status::Succeeded, None, None);
}
