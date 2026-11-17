# v0.5 Stage C: workflow execution engine

`src-tauri/src/workflow_execution/` owns execution; no editor, workflow search,
frontend execution service, dependency update or runtime network service is added.
Existing Stage A registrations and native adapters remain the only action dispatch
path. Stage B SQLite data is loaded by stable UUID on a dedicated worker.

## Run semantics

Start reserves a unique UUID and the workflow's active slot atomically, before any
database access. Loading and preflight are `pending`. The worker loads the saved
document, drops the connection, sorts by saved order, checks format/IDs/order/size,
validates every action and parameter schema, and checks all app/file references.
No step executes until this entire preflight succeeds. Missing IDs, unsupported
versions/actions, invalid parameters, unavailable references and storage failures
become queryable failures. The loaded revision is included in the run snapshot.
Later edits or deletion do not mutate the in-flight plan.

The runner dispatches each step sequentially and rechecks current prerequisites.
App checks resolve the current shortcut or Windows registration; file checks resolve
existence, kind and sensitivity; native audio and system adapters retain their
device, policy and privilege safeguards. Those transient native checks can still
fail after earlier steps complete. Failure stops later steps. No retry or rollback
is attempted, including when a native error reports that state may have changed.

Run and step status strings are `pending`, `running`, `confirmation-required`,
`succeeded`, `failed`, `cancelled`. Steps include stable saved UUIDs, action IDs,
optional structured errors and native outputs. Unexecuted steps are `cancelled`
with `not_executed`; their earlier successful siblings stay successful. A preflight
error may mark its offending step failed without a preceding `step_started` event.
Malformed document-level data may fail the run without one attributable step.

App success means Windows accepted the launch. Output retains `accepted: true`
and `readiness_verified: false`, and adds `launched`, `window_detected`,
`activation` (`foreground`, `denied`, `timed_out`, or `suppressed`), and `warning`.
Only `foreground` means the selected native window was observed in the foreground.
Activation failure, ambiguity, cancellation of activation, or a windowless app does
not turn an accepted launch into a failed action. Discovery finishes on the current
worker before the next step; there is no detached activation from earlier steps.
Cancellation interrupts discovery without rolling back an admitted launch. The
original workflow input/interaction intent also gates later steps' activation.
See [foreground launching](../FOREGROUND_LAUNCHING.md). Settings, lock and sleep also report Windows acceptance, not verified
arrival at a Settings page or a completed power/session transition.

## Confirmation and cancellation

Lock and sleep must be last. Each run uses a fresh registry confirmation scope.
The pending prompt contains a random UUID token, step ID, normalized action ID,
schema version, parameters, message and remaining lifetime in milliseconds.
It expires after 60 seconds using a monotonic deadline. The backend binds the public
token to this run and pending step, and retains the registry's action-bound ticket
privately. Stored approval flags, generic frontend booleans, a token from another
run/step, and replayed decisions cannot authorize execution.

Approval is an explicit separate request. It consumes the public token immediately,
returns `running`, revalidates prerequisites, and consumes the private ticket before
calling native code. The final action-admission check also checks cancellation and
expiry. Denial/cancellation invalidates pending authority. Expiry cancels with
`confirmation_timeout`, guaranteeing the pending action was not invoked.
The frontend must display the returned action details and send approval only after
the user chooses it; this protocol is not authentication against compromised UI code.

Cancellation is cooperative. `cancel_requested` records intent immediately, wakes
confirmation waits, and prevents subsequent action admission. An admitted native
call cannot be recalled: `action_in_flight` stays true until its result returns.
Its actual success/failure is recorded, then no further steps run. If the final
admitted step succeeds, the run succeeds even if cancellation arrived too late.
Already completed external actions are never undone.

Workflow file opens are deliberately stricter than standalone file opening:
executable, script, shortcut, installer, macro-capable and unknown types are refused,
even with approval. The native opener is never passed file-execution confirmation.
Applications can only be launched through current catalog IDs, not workflow-supplied
executables or arguments. Shell command text and recursive `workflow.*` steps are
unsupported. Windows associations and filesystem changes retain the existing native
opener's documented limitations.

## IPC and event recovery

These commands accept calls only from the main webview. Tauri argument names are
camelCase; serialized snapshots use the Rust snake_case field names.

| Command | Arguments | Result |
| --- | --- | --- |
| `start_workflow` | `workflowId` | Initial run snapshot; worker is already detached |
| `workflow_run_status` | `runId` | Current authoritative snapshot |
| `list_workflow_runs` | none | Retained active and terminal snapshots |
| `cancel_workflow_run` | `runId` | Snapshot with cancellation requested |
| `decide_workflow_action` | `runId`, `stepId`, `token`, `decision: "approve" / "deny"` | Updated snapshot |

Main-window `workflow-progress` events contain `kind`, `run_id`, `workflow_id`,
nullable `step_id`, and monotonic per-run `sequence`. Kinds are `run_started`,
`step_started`, `step_succeeded`, `step_failed`, `confirmation_requested`,
`run_succeeded`, `run_failed`, `run_cancelled`. No paths, parameters, approval tokens,
native outputs or error messages are included in events or application logs.
Full details are available only in scoped status responses.

State is updated before notification under the run mutex; event order matches state
order. Events are best effort, not the source of truth. Future clients should:

1. Attach the event listener, then fetch/list snapshots.
2. After start returns, fetch status immediately; execution may already be terminal.
3. Refresh status on events and periodically while active. Apply only snapshots
   whose sequence is at least the latest applied sequence for that run. Do not let
   a delayed start response overwrite a newer state.
4. On reconnect/reopen, list runs, including terminal runs. A lost start response
   is recoverable through this list; never blindly retry an action.

All semantic state changes advance the sequence. Remaining confirmation time is
advisory and decreases independently; only the backend deadline grants authority.
Up to 8 workflows may be active and 128 runs are retained. When full, starting a new
run evicts the oldest retained terminal entry; active entries are never evicted.
Evicted IDs return `run_not_found`. Run history is in memory for this app session,
not persisted or resumed after process termination.

## Lifetimes and timeouts

Workers belong to the backend, not the initiating IPC request or editor/listener.
Hiding/unmounting/disconnecting does not cancel them. Closing the main window while
a run is active hides it and prevents close; without active runs, existing Alt+F4
quit behavior remains. Closing a future editor must likewise avoid sending cancel
implicitly. Process termination, crashes, OS shutdown and forced exit still end the
process; no durable recovery or automatic replay is promised.

SQLite retains its five-second busy timeout. Confirmation has a 60-second timeout.
Windows Shell, COM, filesystem and power calls have no safe general cancellation
primitive in the existing adapters. The engine does not detach a timed-out call and
pretend it failed or release its duplicate guard while it might still act. Such a
call remains active and occupies one bounded worker until it returns. Sleep may
return only after resume. Confirmation/approval state is saved in memory before
calling it, so progress does not depend on a visible UI or delivery after suspension.

Native failure details remain structured, including existing `may_have_changed`
information. Panic recovery is available in unwind builds; the existing release
profile uses `panic = "abort"`, so a release panic terminates the process.

## Verification

Automated execution tests use fake action runtimes, channels and isolated temporary
SQLite data. No automated test invokes real lock or sleep. Targeted engine tests
cover ordered three-step success, stop-on-failure, malformed plans, full preflight,
fresh references, atomic concurrent starts, cancellation before/between steps and
after approval, expiry/replay/cross-run/cross-step rejection, dropped listeners,
immutable storage snapshots, bounded retention and worker panic recovery.

Executed on `feature/workflow-studio`:

- Targeted engine suite: 16 passed, zero failed.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`:
  105 passed, zero failed, 2 existing opt-in/fixture tests ignored.
- `npm test` (with output filtered): 68 passed, zero failed.
- `npm run build`: TypeScript and Vite production build passed with installed dependencies.
- `cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --bins`:
  Windows debug executable built successfully.
- `cargo fmt` and `git diff --check` passed. Cargo's existing profile-path warning
  and Git's CRLF normalization notices did not prevent these checks.

No network fetches, new dependencies, release installer, or production-database
mutations were needed. Manual desktop IPC, close/reopen, actual app/file/audio
workflow behavior, and lock/sleep/resume remain separate Windows checks; mock tests
and compilation do not establish those behaviors. Listener/request lifetime is
covered at the engine level; the native close-event handler was compiled, not
manually exercised in the running desktop app.
