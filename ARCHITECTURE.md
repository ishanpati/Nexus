# NEXUS architecture

NEXUS is a Windows 11 Tauri 2 application with React/TypeScript presentation and
Rust native operations. Production loads embedded local assets. There is no runtime
AI, cloud API, application server, or generic shell executor.

## Frontend

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
is not implemented. See FILE_SEARCH_RANKING.md for the verified behavior and limits.

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
after Windows accepts the request. See SYSTEM_ACTIONS.md for API limitations.

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
not prove GUI interaction; see [RELEASE_VERIFICATION.md](RELEASE_VERIFICATION.md).
