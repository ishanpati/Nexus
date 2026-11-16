# v0.5 Stage E2: saved workflow management

Implemented on `feature/workflow-studio`, reusing `WorkflowEditor.tsx` and its
registry-driven parameter controls. E2 adds no dependencies, schema migrations,
runtime services, execution UI or launcher `w` search. Existing staged work is
preserved; E2 does not change package versions, manifests or lockfiles.

## List and editing

`edit workflow` opens the saved list through the existing Rust Studio route.
`SavedWorkflows.tsx` renders repository names, descriptions, action counts,
local-formatted updated times and availability details. Filtering is in memory
over name, description and aliases; newest updates appear first. Empty repository,
no matches, loading and backend errors have separate states. Refresh and focus
recover missed repository notifications. Create New Workflow opens the existing
create editor, and a workflow name or Edit button opens its UUID edit route.

The editor loads every stored field and ordered step. Save passes the same workflow
UUID and expected revision. Renaming and moving steps preserve their UUIDs. Only a
confirmed save response advances the baseline and revision; errors keep the draft.
Numeric reversions to the saved value return to clean state. A guarded Reload saved
workflow action is available after a save error: users must choose whether to save,
discard or cancel before replacing a dirty snapshot.

Unknown actions, obsolete schema versions and unsupported parameter metadata are
shown with preserved raw parameters. Unchanged saved references may survive a rename
or reorder, matching Rust's existing retention policy; the editor never drops them
automatically. Modified or new references must validate. Supported controls remain
available to repair missing apps, paths or invalid values. Obsolete choice values
stay visible. Known actions with obsolete schemas or extra keys offer an explicit
reset to current registry fields, preserving the step UUID. Unknown actions can be
removed and replaced explicitly. Saving does not grant execution approval.

## Navigation, deletion and concurrency

All editor navigation, including retained launcher requests and native Studio close,
uses the same Save / Discard / Cancel controller. Cancel and Escape stay in the
current editor. Save awaits validation and successful persistence before accepting
the Rust request. Failure leaves the prompt and draft available; Cancel returns to
the editor errors. Discard explicitly accepts the pending route or close. Studio
close hides its window and leaves NEXUS alive. Ordinary saves defer incoming requests
until the save settles and then recheck dirty state. Replayed request sequences and
duplicate choices are ignored. Native request retention remains owned by Rust.

Delete shows the workflow name, UUID, revision and action count, defaults to Cancel,
and sends `confirmed: true` only from the explicit Delete workflow button. Rust
rechecks confirmation and revision in a transaction. A stale revision or a workflow
deleted elsewhere produces an error; users cancel and refresh before retrying.
React and the typed service prevent duplicate mutations, including save/delete
overlap. Successful saves update local list state and successful deletes remove
their row immediately, with repository events and a fresh read reconciling changes.

Rust's existing optimistic checks reject stale saves and never recreate a workflow
when an edit's expected revision refers to a deleted row. Dirty editors ignore
background replacements. Forced process termination, OS shutdown and webview reload
can still lose unsaved work; this stage adds no draft autosave.

## Verification

Targeted Node tests passed for navigation choices, save-before-navigation ordering,
failures retaining drafts, incoming requests during saves, duplicate mutation gates,
exact delete arguments, stable IDs, unavailable-step retention, numeric reversions,
local filtering and rendering missing references. Targeted Rust tests passed for
real SQLite CRUD/reopen, stable IDs, validation, unavailable actions, migrations,
rollback, and two independent connections rejecting stale saves/deletes and
resurrection after deletion. No test executes lock, sleep or application actions.

Native Windows checks used the computer-use skill and embedded assets under isolated
identifier `com.nexus.e2verification`. No production workflow database was modified.
Verified creation with two actions; full process stop/restart; Edit workflow opening
the list; local matching/no-match filtering; loading the existing editor; rename;
parameter change from 25 to 30; reorder with the form open; Cancel on unsaved native
close retaining the editor; Save from the navigation prompt; immediate updated list;
another full process stop/restart preserving all edits; deletion cancellation;
explicit deletion displaying UUID/revision; and immediate empty state. Read-only
SQLite assertions also confirmed one workflow, the original workflow and both step
UUIDs, revision 2, reversed order and the changed parameter. The isolated test
workflow was deleted through Studio.

Unsupported metadata and concurrent-session errors were checked in automated tests,
not injected into the native GUI. Builds use cached dependencies with Cargo locked
and offline; tests use local fakes and SQLite. The GUI used local embedded assets,
but the machine's network adapter was not disconnected. No installer was produced.

An existing window-lifetime limitation was observed: Alt+F4 on the launcher while
Studio exists closes the launcher without terminating the process. Controlled
process stops were used for genuine restart verification. E2 does not change main
window/application shutdown behavior. Studio's own unsaved-close protection worked.

Final checks:

- `npm test`: 93 passed, zero failed.
- `npm run build`: TypeScript and Vite production build passed.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`:
  112 passed, zero failed, 2 existing opt-in/fixture tests ignored.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --doc`:
  passed, zero documentation tests present.
- `cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --bins`:
  passed, restoring the normal application identifier and final embedded frontend.
- `cargo fmt --check` and staged/unstaged Git whitespace checks passed. Existing
  Cargo profile-path warnings and Git CRLF notices did not prevent the checks.

The isolated verification process was stopped. Read-only SQLite assertions after
the GUI deletion confirmed both workflow rows and their name index were empty.
Ignored build/test fixtures and the separate verification database remain outside
production data. The normal final binary was compiled after a small save-path
hardening change (validate the latest draft reference and ignore late field changes
during persistence); the native interaction above preceded that final hardening.
