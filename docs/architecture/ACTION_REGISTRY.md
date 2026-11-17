# v0.5 Stage A — backend Action Registry

This stage provides descriptors, lookup, validation and single-action adapters.
It does not add workflow storage, an editor, multi-step execution, plugins or a
new launcher UI. Existing v0.4 command IDs and IPC handlers remain supported.
Package/installer versions remain 0.4.0 until a later release task.

## Public metadata contract

`list_workflow_actions` returns only workflow-compatible `ActionDescriptor`s.
Rust owns stable namespaced IDs, names/descriptions, categories, safe icon IDs,
parameter schemas, confirmation policy, terminal status and action schema version.
Handlers and validators stay in Rust; no function pointers are serialized.
`src/services/workflowActionService.ts` supplies a typed metadata client, unused by
the current launcher UI. Action IDs are strings, not a frontend enumeration.

| ID | Required parameters | Confirmation | Position |
| --- | --- | --- | --- |
| `app.launch` | `application_id`: catalog application reference | None | Any |
| `file.open` | `path`: selected file | Existing sensitive-file policy | Any |
| `folder.open` | `path`: selected folder | Existing target policy (ordinary folders need none) | Any |
| `audio.set_volume` | `percent`: integer 0–100 | None | Any |
| `audio.adjust_volume` | `delta`: integer −100–100 | None | Any |
| `audio.set_mute` | `muted`: boolean | None | Any |
| `settings.open` | `page`: sound, wifi, bluetooth, display | None | Any |
| `system.lock` | Empty object | Always | Final only |
| `system.sleep` | Empty object | Always | Final only |

All initial action schemas use version 1. Parameters support text (bounded length),
number (min/max/integer), boolean, choice, application, file and folder. Fields may
be required or optional and may specify a schema-valid default. Optional omitted
fields remain absent unless a default is declared. Explicit null is not omission.
Text/reference fields reject empty/control-character values. Numeric strings,
fractions for integer fields, non-finite and out-of-range values are rejected.
Unknown fields fail, including `confirmed`, `terminal`, `executor`, or a URI field.
Errors include a stable code, message, optional parameter name and native details.

## Rust API and execution boundary

- `ActionRegistry::descriptor(id)` performs exact lookup.
- `validate(ActionRequest)` checks schema version and parameter shape/constraints,
  applies defaults and returns normalized parameters plus backend policy metadata.
  It does **not** scan apps, inspect the filesystem or authorize an operation.
- `dispatch(request, runtime, position, token)` revalidates, checks terminal
  placement, resolves current references through the registered validator and then
  calls exactly one registered executor. There is no loop or automatic retry.
- `NativeRuntime` delegates to the existing catalog launcher, file path resolver/
  safe opener, `AudioService`, Settings allowlist and native system-action handler.
  It rechecks catalog membership, path existence/type and sensitivity before open.
  App IDs cannot supply executable names or argument strings. Settings cannot
  supply an arbitrary URI. Native audio errors retain `may_have_changed` details.
- Adapters perform no query reset or usage writes. In v0.5.1 the app adapter
  forwards caller-owned hide/cancellation callbacks through the shared native
  launcher; the workflow callback targets only the owned main window. A future runner must call dispatch on a
  worker thread and decide presentation/usage policy separately.

Stage C now adds [workflow execution IPC](../features/workflows/WORKFLOW_EXECUTION.md) and an independent
registry confirmation scope per run. Registry expiry is checked after prerequisite
validation. Native app results report acceptance with readiness explicitly unverified.
Workflow execution rejects sensitive file types; the standalone registry retains
its original confirmation policy for trusted callers.

Only the metadata-list command was exposed through Tauri in Stage A. There was no
new execute-workflow or execute-registry-action IPC command. The Rust dispatch API
is available for later trusted backend integration and is exercised with mocks.

## Confirmation and future runner responsibilities

Policy comes from compiled registrations, never workflow parameters. Lock has the
stricter workflow confirmation policy while the existing launcher lock behavior is
unchanged. Lock/sleep reject intermediate placement before touching native code.

Single-action dispatch returns a confirmation prompt containing the action ID,
version and normalized parameters. Its ticket is bound to those values, expires
after 60 seconds, is replaced by a new prompt, and is consumed before execution
(even on failure). Cancellation invalidates it. References and sensitivity are
rechecked on the confirmed dispatch. Registry tickets do not interfere with pending
launcher sleep confirmations. Tickets are an in-process confirmation protocol,
not authentication against a compromised frontend or malicious Rust code.

A future workflow runner must derive `StepPosition` from actual plan order and
present a separate explicit confirmation UI; neither belongs in stored action
parameters. This stage does not claim to validate a whole workflow or authorize
unattended sensitive operations. File association/content changes and filesystem
changes between checks and activation retain the existing safe-opener limitations.

## Extension and versioning

Register an `ActionDescriptor`, reference validator, and executor in
`action_registry/builtins.rs`. Existing parameter types need no editor/database
changes. `ActionRegistry::new` checks IDs, duplicates, defaults, field keys and
terminal-policy consistency. A test-only text/optional/defaulted-number action
demonstrates registration without modifying registry infrastructure.

Stored workflow data is not defined here. Future stored references should retain
`action_id`, `schema_version` and parameters, not copy authoritative descriptors.
Unsupported versions currently return `unsupported_version`; they are never
silently coerced to a different schema. Future migrations must be explicit,
versioned transformations followed by current backend validation and confirmation.

## Inspect and test

Read-only metadata diagnostic (does not launch applications or change Windows):

```powershell
cargo run --locked --offline --manifest-path src-tauri/Cargo.toml --example workflow_actions
```

From a future frontend integration, call `invoke("list_workflow_actions")` or use
the typed `createWorkflowActionCatalog(invoke)` client. No capabilities, packages,
runtime services, cloud dependencies or installer requirements were added.

## Stage A verification

Executed on `feature/workflow-studio`:

- `npm test`: **68 passed**, zero failed.
- `npm run build`: TypeScript check and production frontend build passed.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`:
  **84 passed, 2 ignored**, zero failed. The ignored cases are the existing live
  audio opt-in and a child fixture used by the timeout test.
- `cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --bins`:
  Windows debug executable compiled successfully.
- The `workflow_actions` example ran and printed all nine descriptors with their
  parameter schemas, policy and version fields, without executing actions.
- Rust doctests completed (zero doctests); `cargo fmt` and `git diff --check` passed.
  Cargo printed the existing sandbox profile-path warning; Git printed CRLF
  normalization notices. Neither prevented checks from completing.

Tests cover serializable metadata, unique IDs, parameter/default validation,
malformed/unknown/version-mismatched requests, reference failures, executor
selection, sensitive-file and terminal confirmation, expiry/cancellation/replay,
wrong file/folder types and extensibility. Existing launcher/audio/app/file/search
regression suites also passed. New execution tests use mocks; no app launches,
audio changes, lock or sleep were performed to test this stage. A fresh GUI/native
end-to-end regression and a new release installer were not produced in Stage A.
