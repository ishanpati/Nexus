# NEXUS architecture

NEXUS is a Windows 11 Tauri 2 application with React/TypeScript presentation and
Rust native operations. Production loads embedded local assets. There is no runtime
AI, cloud API, application server, or generic shell executor.

## Frontend

- `App.tsx` owns input, selection, batches, feedback and confirmation presentation.
  `App.css` preserves the compact 650 × 310 design.
- `commands.ts` and `services/commandService.ts` provide the typed built-in registry
  and narrow execution IPC. Open Notepad retains its original handler.
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

`file_actions.rs` validates absolute paths, rejects device/URL/stream and ambiguous
paths, canonicalizes and checks existence/type, and enforces confirmation for
potentially executable/unknown files. Confirmed requests are revalidated; original
and resolved extensions are checked. Files use `ShellExecuteExW`, directories use
`explore`, and reveal uses `SHOpenFolderAndSelectItems`. Successful actions hide the
window; hide failure is a warning after success, not a reason to execute again.

## IPC and trust boundaries

| Command | Accepted input |
| --- | --- |
| `execute_command` | Registered ID and validated typed arguments |
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

Node's built-in runner covers matching, ranking, usage, catalog caching, search
scheduling/staleness, path display/highlighting, and action IPC. Rust tests cover
validation, caching, native shortcut metadata, launch/error handling, placement,
shortcut logic, ES parsing/process limits, and confirmation. Read-only examples
exercise the real Windows catalog and Everything index. Compilation/unit tests do
not prove GUI interaction; see [RELEASE_VERIFICATION.md](RELEASE_VERIFICATION.md).
