# v0.5 Stage E1: dynamic workflow editor

Stage E2 reuses this editor for [saved workflow management](WORKFLOW_MANAGEMENT.md).
It adds Save / Discard / Cancel navigation, guarded reload, and retention of unchanged
unavailable saved steps. The stricter E1 unavailable-step saving behavior described
below is historical; E2 follows the Rust storage retention policy.

Implemented on `feature/workflow-studio`, extending the existing Stage D window
and Stage B persistence. Dependencies, package versions, lockfiles, launcher modes
and global shortcut registration are unchanged.

## Editing and metadata

Create and UUID edit routes now render workflow name, optional description/aliases
(one alias per line), an ordered action list, Add Action and Save Workflow.
Actions are searched by name/description/category and grouped by registry category,
with registry icon IDs mapped to presentation symbols. The backend remains the
source of action IDs, versions, parameters, allowed values, limits and policies.

`WorkflowParameters.tsx` chooses controls by schema type: text, number, boolean,
choice, application, file and folder. There are no per-action forms. Numbers stay
as text while editing so blank, malformed, fractional, unsafe and out-of-range
values can be rejected before conversion. Optional empty fields are omitted;
defaults and explicit false/zero survive. Optional booleans have an include toggle.
Unrecognized parameter types, malformed descriptors, unavailable actions and saved
schema mismatches have explicit unavailable states and block saving incomplete
workflows. Stored parameters are preserved until the step is removed or repaired.

Installed applications come from `list_installed_apps` with explicit refresh.
The picker filters display names and stores only the selected catalog ID. Rust
rechecks current registration before saving and again before execution. The native
path picker returns exact UTF-16 Windows paths as strings, including spaces and
Unicode; cancellation returns null and preserves the previous selection.

`pick_workflow_path` accepts only file/folder kinds from the exact Studio label.
It uses the existing Windows bindings for an owned, single-selection COM file
dialog on an STA worker. It never opens a target, changes working directory or
adds selected paths to recent items. No new dependencies or frontend permissions
were necessary; there are no filesystem/shell/dialog-plugin capability grants.

Step UUIDs remain stable during edits/reordering. Open forms follow UUIDs, not
array indexes. Removing another step cannot redirect an open form; late picker
responses cannot resurrect removed steps or overwrite a reordered draft. Up/Down
and all configuration controls are keyboard accessible. Terminal actions cannot
move out of the final position, new ordinary actions insert before a terminal,
and a second terminal cannot be added. Rust independently enforces final position.
Sensitive actions show registry-derived warnings; saving grants no approval.

## Saving and lifetime

Frontend validation checks name/aliases/description limits, 1–64 complete steps,
schema constraints, versions and terminal position. Rust repeats authoritative
validation and performs read-only prerequisite checks before `Store::save` commits.
Duplicate names/aliases, revision conflicts and missing references are displayed
with backend messages. No action executor or workflow-run IPC is called by save.

React and the typed service both use synchronous save gates, so rapid activations
cannot create concurrent writes. Clean saved drafts disable Save as well, so a
second click after a fast response cannot write another revision. All edits are disabled during persistence, and
navigation/close intent waits for its result. Success feedback and the clean
baseline/revision are updated only after backend confirmation. The editor stays
open and subsequent saves update the same UUID using the returned revision.
Repository events make the workflow immediately available to existing backend
reads and Studio's existing list. No list management or launcher workflow search
is added. Errors retain the draft and dirty state. Dirty drafts reject background
replacement; a cancelled focus refresh cannot leave the editor disabled.

Drafts remain in webview memory. Forced termination/reload can lose unsaved work.
File/app availability is a snapshot and may change after save. Execution retains
its own fresh checks, sensitive-file restrictions and confirmation protocol.
Unavailable old steps remain readable, but the editor requires a complete valid
plan to save; the storage layer's existing obsolete-step retention policy is intact.

## Repeatable manual Windows checks

Use embedded production frontend assets and cached dependencies:

```powershell
npm.cmd run build
cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --bins
```

Exit other NEXUS builds before testing the normal identifier. Alternatively,
build with a temporary `TAURI_CONFIG` identifier for isolated test storage; do not
commit that configuration. An existing release owning Ctrl+Shift+Space will cause
the isolated build's expected shortcut warning; launch its executable to summon it.

1. Open Create workflow. Enter a name, description and newline-separated aliases.
   Check Unsaved draft and discard protection on navigation/close.
2. Add Launch application. Search and select Notepad from the installed catalog.
   Add Open folder and select a real path containing spaces and Unicode. Reopen
   its picker and cancel; the previous path must remain.
3. Add Set volume, leave it blank, then try `bad`, `25.5`, `101`, and finally `25`.
   Check inline errors. Move it Up with its form open; value and step must follow.
4. Edit a saved step, remove another step, then remove the active step. Ensure no
   other parameters change. Re-add enough actions for a three-step workflow.
5. Save twice rapidly. Expect one in-flight write, success only after IPC returns,
   clean state, and an open editor. Open Saved workflows, then reopen this UUID;
   check all values and order. Save an edit and verify revision advances.
6. Create another workflow using that name/alias. Expect the duplicate-name error
   and preserved dirty draft. Try a blank name and no steps as well.
7. Add lock or sleep without running it. It must stay last; adding an ordinary
   action inserts before it. A second terminal is unavailable. Never execute
   lock/sleep just to test the editor.
8. Refresh applications after uninstalling/removing a selected test registration,
   then save. Expect a step-specific reference error and no partial write.
   Restore the registration or select a current app. Use an expendable test app.
9. Repeat with network disconnected. Verify the launcher, Escape, shortcut,
   ordinary app/file search and existing audio controls separately.

Automated tests cover unknown action/parameter types and versions, defaults,
optional values, numeric edge cases, duplicate-save rejection/retry, native picker
kind validation, save-time reference loss and a new action rendering all seven
existing parameter types without custom editor code. Rust execution is mocked in
save tests and panics if any executor is invoked.

## Executed verification

- Targeted Node draft, parameter-rendering and Studio tests: 13 passed.
- Targeted Rust save-reference test passed; disappeared applications are rejected,
  Unicode/space paths reach the validator intact, and every executor is forbidden.
- `npm test -- --test-reporter=dot`: all 81 frontend tests passed, zero failures
  (npm's existing script placed the reporter argument after its file glob; output
  was still filtered to a bounded tail).
- `npm run build`: TypeScript and Vite production compilation passed.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`:
  passed, including storage, registry, engine, Studio and picker tests.
- `cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --bins`:
  passed with the normal application identifier and embedded final frontend.
- `cargo fmt --check`, staged and unstaged Git whitespace checks passed. Existing
  Cargo profile-path and Git CRLF notices did not prevent checks.

Windows interaction used the computer-use skill and an isolated temporary build
identifier, `com.nexus.e1verification`, with a separate local database. The original
release backend remained running and retained its shortcut. Verified through the
actual native window: registry groups/search; Notepad selection; Windows folder
selection of `src-tauri/target/e1-fixtures/資料 space`; numeric error and value 25;
three actions; reorder with form open (UUID/value preserved); backend-confirmed
save with clean state; immediate list discoverability; reload and process restart
with all three values/order retained; editing to 30; removing the app while the
volume form stayed attached; picker cancellation retaining its path; and both
Escape/Keep editing and explicit discard on navigation. Test edits were discarded,
leaving the isolated saved workflow at three steps, revision 1. The isolated test
process was stopped after verification; the normal debug binary was rebuilt.

Manual testing exposed and fixed a focus-refresh race that could leave a dirty
editor disabled. The corrected native build passed the edit/removal recheck.
One intermediate rebuild failed because the running test executable was locked;
it passed after the isolated process was stopped. No application action was run,
no real volume changed, and lock/sleep were never executed. No production workflows
were written. Test fixtures remain in the ignored target directory and isolated
app data, separate from the user's workflow database.

All dependency builds used local caches; Cargo used locked/offline mode. A
physically disconnected GUI run, native **file** dialog selection (the folder
dialog was tested), deliberate real-app uninstallation, fast-double-click GUI
timing, and complete launcher/global-shortcut regression remain manual checks.
Automated tests cover reference loss and duplicate-save gates; those mocks and
compilation are distinct from the Windows checks above. No installer was produced.
