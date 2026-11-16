# NEXUS architecture

NEXUS is a Windows 11 Tauri 2 application with React/TypeScript presentation and
Rust native operations. Production loads embedded local assets. There is no runtime
AI, cloud API, application server, or generic shell executor.

## v0.5 release boundaries

`action_registry/` owns nine stable compiled registrations, generic parameter
metadata, schema validation, prerequisite checks and native adapters. Frontend
controls switch on parameter type, never action ID; a registration using existing
types needs no editor rewrite. New parameter types require explicit editor support.
Saved steps retain action ID/version/parameters and stable step UUIDs. Unsupported
versions fail closed; future action transformations require explicit migrations.

`workflows.rs` owns SQLite schema version 1, immediate transactions, the shared
name/alias uniqueness index and revision conflicts. Save IPC also performs current
reference checks and rejects file types forbidden by the runner. Reads annotate
missing/obsolete references and sensitive file steps for repair; they never execute.
Data lives at `app_local_data_dir()/workflows.sqlite3`, normally
`%LOCALAPPDATA%\com.nexus.launcher\workflows.sqlite3`. Dev and production share it
because their identifier is unchanged. Tests use temporary databases. SQLite
reopen tests do not establish installed-app restart or installer upgrade behavior.

`workflow_execution/` owns detached sequential workers and session-only run state.
Whole-plan validation precedes effects; admission is guarded against cancellation.
Each run binds expiring, single-use approval to its step and normalized action.
Lock/sleep are final-only; file execution authority is never granted to workflows.
App activation reports acceptance without readiness guarantees. Runs have no
rollback, resume-after-process-exit, recursion, delay, or parallel-step facility.

`workflow_studio.rs` owns one retained native window and a sequenced request mailbox.
Studio close hides it after draft resolution. Launcher close always prevents native
destruction: active runs cause hide; otherwise an existing Studio handles a `quit`
intent through the same Save / Discard / Cancel guard, and Rust refuses that quit
while runs remain active. With no Studio/run, launcher close exits the application.
This preserves the main window used by global-shortcut and single-instance summon.

Capabilities grant only event listen/unlisten to relevant local windows; repository,
picker, Studio and runner IPC add Rust caller-label checks. No frontend shell,
filesystem, opener, shortcut-registration or app-exit plugin permission is granted.
The existing opener plugin remains installed without frontend grants. CSP remains
disabled (`csp: null`), a known defense-in-depth gap; local text is rendered through
React without raw HTML. Permissions/approval tokens are not protection against a
fully compromised trusted frontend. No network runtime client is implemented.
The SQLite document is plaintext and may contain private local references; logs do
not deliberately print workflow documents. Back up stopped databases as described
in the [user guide](../USER_GUIDE.md#workflow-data-and-backup). Current release evidence and remaining risks: [V05_VERIFICATION.md](../verification/V05_VERIFICATION.md).

## Frontend

Stage F integrates saved workflows into the existing launcher. `fileSearch.ts`
parses exclusive `f ` / `w ` prefixes; bare letters remain ordinary app/command
queries. `workflowSearch.ts` indexes saved UUIDs/names/aliases through `appSearch.ts`
and owns generation-checked repository refreshes. `resultExecution.ts` dispatches
workflow rows through the same Enter/click path. Only accepted starts clear input;
the launcher stays available to show progress. Existing command success lifecycles
and file/system confirmations are preserved.

`useLauncherWorkflows.ts` attaches scoped repository/progress/shown/focus listeners
before fetching snapshots, and refreshes run status once per second while active.
`workflowRunService.ts` coalesces reads, merges by per-run sequence, gates starts
and decisions synchronously, recovers lost start responses through list without
retry, and keeps at most 128 snapshots while retaining active runs. Rust remains
authoritative for execution, duplicate admission, cancellation and approval tokens.
`WorkflowProgress.tsx` presents selected session runs, completed/current/failed
steps, exact sensitive action details and explicit Approve / Deny / Cancel controls.
Closing its presentation or resetting search never calls cancel. No dependencies,
permissions, native execution paths or shortcut registrations are added. See
[WORKFLOW_LAUNCHER.md](../features/workflows/WORKFLOW_LAUNCHER.md).

Stage E2 adds `SavedWorkflows.tsx`, using the existing typed repository service
and `WorkflowEditor.tsx`. `studioNavigation.ts` sequences Rust navigation/close
requests against editor saves; Save must succeed before resolving an accepted
request. Cancel retains the current draft. The service serializes save/delete
mutations, and Rust remains authoritative for revisions and delete confirmation.
Unavailable saved steps retain their IDs and parameters under the existing storage
policy. See [WORKFLOW_MANAGEMENT.md](../features/workflows/WORKFLOW_MANAGEMENT.md).

Stage D mounts `WorkflowStudio.tsx` only in the Rust-created `workflow-studio`
webview. The launcher and audio overlay retain their own roots and lifecycle hooks.
Studio owns its local draft, fetches repository data through typed IPC, and uses
stable UUID edit routes. A retained, sequenced Rust request plus a listener-ready
handshake carries navigation/close intent. Dirty drafts require an explicit discard
choice; closing hides Studio and leaves the launcher and backend alive. Scoped
`workflows-changed` events and focus/explicit refresh reload saved data without
overwriting dirty drafts. See [WORKFLOW_STUDIO.md](../features/workflows/WORKFLOW_STUDIO.md).

Stage E1 mounts `WorkflowEditor.tsx` for create and UUID edit routes. Parameter
controls are selected by registry schema type in `WorkflowParameters.tsx`, with no
action-ID-specific forms. `workflowDraft.ts` keeps editable numeric text separate
from typed persistence payloads. Step UUIDs preserve editing identity across moves
and removals. Save gates protect against duplicate activations; confirmed responses
update the baseline and revision. Dirty or saving drafts defer refresh/navigation.
`workflow_picker.rs` exposes only file/folder selection to Studio via existing
Windows COM bindings; it grants no frontend filesystem or shell capability.
Rust checks schema, revision and live registry prerequisites before committing,
without executing actions. See [WORKFLOW_EDITOR.md](../features/workflows/WORKFLOW_EDITOR.md).

- `App.tsx` owns input, selection, batches, feedback and confirmation presentation.
  `App.css` preserves the compact 650 × 310 design.
- `commands.ts` and `services/commandService.ts` provide the typed built-in registry
  and narrow execution IPC. Open Notepad retains its original handler.
- `systemCommands.ts` provides the pure v0.4 system-command registry/parser,
  typed action validation, contextual suggestions and confirmation metadata.
  `commandSearch.ts` combines these with built-ins and apps through the existing
  `appSearch.ts` ranker. `services/audioService.ts` sends only validated audio
  actions on explicit activation. `services/systemActionService.ts` sends closed
  Settings/lock/sleep requests; `SleepConfirmation.tsx` requires a separate choice
  using a one-use callback gate. `resultExecution.ts` dispatches selected rows for
  both Enter and clicks, preserving catalog IDs and parsed arguments. Its shared
  lifecycle clears/hides after success and retains the query on failure. Lock and
  confirmed sleep reset before IPC, restoring the query if the action fails.
  Escape and launcher-shown reset transient input state; ordinary focus/mode
  changes do not. Audio does not update application launch usage.
  A read-only audio snapshot on startup/summon supplies an explicitly labelled
  last-read status for audio rows. Generations suppress obsolete reads; typing
  does not perform writes or start status polling. Last-success feedback is kept
  separately from the cleared query and confirmation state.
  `AudioFeedback.tsx` is a second, preloaded, non-focusable window, not another
  launcher instance. Rust `audio_feedback.rs` emits successful audio write results,
  positions it at the bottom of the work area and hides it after 2.2 seconds.
  Generation checks prevent an older timer hiding newer feedback. The window has
  only event listen/unlisten permissions, uses local assets, and ignores pointer
  input. An overlay failure never retries an already successful audio write.
  Sleep confirmation is enforced again at the Rust boundary.
- `appCatalog.ts` / `hooks/useAppCatalog.ts` coalesce loads and cache the catalog;
  refresh failure retains usable results. `appSearch.ts` ranks names/aliases locally.
- `launchUsage.ts` records successful app launches in WebView local storage.
  `services/installedAppService.ts` and `components/AppIcon.tsx` use catalog IDs
  for launches and cached, bounded icon loading.
- `fileSearch.ts` parses `f ` mode and owns the asynchronous search store. A 150 ms
  debounce, one active request, newest-pending query, and generation checks prevent
  stale results/errors. `useFileSearch` connects it to Tauri; `FileResultContent`
  renders safe text highlights and icons.
- `fileActions.ts` sends selected paths, fixed open/reveal actions, and explicit
  confirmation. Old/loading results cannot be activated. `FileConfirmation.tsx`
  uses a modal dialog with Cancel focused initially.
- `hooks/useLauncherWindow.ts` owns input focus and narrow hide/drag requests.

## Native backend

Stage C adds `workflow_execution/`: bounded backend-owned workers, immutable saved
workflow snapshots, full-plan preflight, sequential registry dispatch, cancellation,
and run/step-bound confirmation. Main-window-only IPC exposes start/status/list/
cancel/decision; identifier-only progress events complement retained snapshots.
Active runs survive hiding, closing the main window, and disconnected listeners.
See [WORKFLOW_EXECUTION.md](../features/workflows/WORKFLOW_EXECUTION.md) for the protocol and limitations.

Stage B adds `workflows.rs`: lazy SQLite access on blocking workers, transactional
versioned workflow documents, a unique name/alias index, optimistic revisions and
availability annotations from Stage A. Database failures are returned through IPC
without failing launcher startup. See [WORKFLOW_STORAGE.md](../features/workflows/WORKFLOW_STORAGE.md).

Stage A of v0.5 adds `action_registry/`: Rust-owned descriptors, a shared parameter
schema validator, exact ID/version lookup, single-action dispatch and thin adapters
to existing native services. `list_workflow_actions` exposes descriptive metadata
only. Registry lock/sleep actions require confirmation and final-step placement;
existing launcher commands keep their current behavior. Stage B/C now compose these
adapters with storage and execution; Stage E1 adds the schema-driven editor. See
[ACTION_REGISTRY.md](ACTION_REGISTRY.md) for contracts and extension guidance.

`src-tauri/src/lib.rs` registers commands and managed state. Single-instance
initialization comes first. `shortcut.rs` registers/cleans up Ctrl+Shift+Space and
reports conflicts. All summon paths call `launcher_window::show`; `active_monitor`
selects the foreground monitor with primary fallback. Native focus loss hides the
window except during guarded launch operations. Hiding never terminates the app.

`installed_apps/` discovers Start Menu `.lnk` and AppsFolder entries, deduplicates
safe native identities, and retains original launch information. Its in-memory
catalog is loaded lazily and explicitly refreshed. Launch IPC accepts only a cached
ID; Shell activation runs on a COM STA worker. Icon pixels/misses are cached until
refresh. `LaunchActivity` prevents overlapping app/file launches and suppresses
premature blur dismissal while an operation is running.

`file_search/` has three layers:

1. `mod.rs`: validates query/limit, locates configured ES or PATH ES, constructs
   literal regex arguments, maps availability errors, exposes async `search_files`.
2. `process.rs`: runs ES directly with separate arguments, no console, bounded
   stdout/stderr, and a five-second timeout with kill/reap. One search runs at a time.
3. `output.rs`: strictly parses UTF-8 EFU/CSV into full path, filename, extension,
   and directory flag. No drive scan or per-row filesystem lookup occurs.

The v0.3.1 working branch adds `retrieval.rs` (bounded exact/prefix/substring/path
passes, independent folder/file pools, two concurrent ES children maximum) and
`ranking.rs` (normalized-path deduplication, strict match tiers, Known Folder and
configured-project context). Final result limits follow ranking. A five-second
deadline covers all passes. `fileUsage.ts` stores the latest 256 successful file
opens locally; optional `openedPaths` is bounded, ranking-only IPC data. No open
target or ES process argument is derived from that history. File fuzzy retrieval
is not implemented. See [FILE_SEARCH_RANKING.md](../features/file-search/FILE_SEARCH_RANKING.md) for the verified behavior and limits.

`file_actions.rs` validates absolute paths, rejects device/URL/stream and ambiguous
paths, canonicalizes and checks existence/type, and enforces confirmation for
potentially executable/unknown files. Confirmed requests are revalidated; original
and resolved extensions are checked. Files use `ShellExecuteExW`, directories use
`explore`, and reveal uses `SHOpenFolderAndSelectItems`. Successful actions hide the
window; hide failure is a warning after success, not a reason to execute again.

## IPC and trust boundaries

`system_actions/` owns a closed Settings/lock/sleep request enum. Four static
`ms-settings:` URIs use the shared COM STA Shell worker; lock calls
`LockWorkStation`. Sleep checks power capabilities and calls `SetSuspendState`
only after consuming a current confirmation ticket (60-second lifetime). It
temporarily enables the sleep privilege already assigned to the process token
and restores its previous state; no elevation is requested. Cancellation and
replacement invalidate tickets; failures never retry automatically. These actions
share the existing launch activity guard, run off the UI thread, and hide only
after Windows accepts the request. See [SYSTEM_ACTIONS.md](../features/SYSTEM_ACTIONS.md) for API limitations.

`audio/` implements a closed Rust audio request enum, argument validation and
structured errors, plus `IAudioEndpointVolume` on a fresh COM STA thread. A shared
service rejects concurrent requests; each operation resolves the default multimedia
render endpoint afresh and verifies its identity before and after acting. Relative
changes use the unrounded scalar and clamp to 0–1. Device/readback failures after
a setter mark `may_have_changed`; there is no automatic retry. The command runs
off the UI thread and never invokes a process. Read/status requests are read-only.
Only two features on the existing Windows binding were enabled; no frontend
permissions, package versions, or dependency versions were changed.

| Command | Accepted input |
| --- | --- |
| `execute_command` | Registered ID and validated typed arguments |
| `execute_audio` | Closed audio action ID and validated numeric/boolean/empty arguments |
| `execute_system_action` | Closed Settings/lock/sleep action; optional single-use sleep ticket |
| `cancel_system_confirmation` | Ticket to invalidate, with no native action |
| `list_installed_apps`, `refresh_installed_apps` | None |
| `launch_installed_app`, `get_installed_app_icon` | Catalog app ID |
| `search_files` | Literal query and bounded result limit |
| `open_file_result` | Absolute selected path, open/reveal, confirmation flag |
| Window and shortcut commands | Narrow fixed operations/status |

The main window capability grants only event listen/unlisten. Windows operations
are Rust commands; no frontend shell/opener/global-shortcut permissions are granted.
ES configuration comes from the process environment, not search text. File opening
is not restricted to cached search results: its boundary is a validated absolute
filesystem path and native sensitivity check. No general executable/argument IPC
endpoint exists.

File-type confirmation is not content scanning; custom associations and filesystem
changes between validation and activation remain outside its guarantee. Catalogs,
search results, and icons are in memory; usage counts are local. No file mutation
actions exist. Installer identity remains `com.nexus.launcher`.

## Verification

System-command collisions are resolved by text tier first. Complete recognized
system grammar is an exact match; incomplete/invalid hints have prefix-level
priority. Equal text matches prefer working built-ins, then system commands, then
apps. Colliding applications remain selectable, and exact app names beat incomplete
system hints. Empty queries introduce no system results; system suggestions never
use fuzzy matching. The existing `f ` mode is exclusive and bypasses this pipeline.

Node's built-in runner covers matching, ranking, usage, catalog caching, search
scheduling/staleness, path display/highlighting, and action IPC. Rust tests cover
validation, caching, native shortcut metadata, launch/error handling, placement,
shortcut logic, ES parsing/process limits, and confirmation. Read-only examples
exercise the real Windows catalog and Everything index. Compilation/unit tests do
not prove GUI interaction; see [RELEASE_VERIFICATION.md](../verification/RELEASE_VERIFICATION.md).
