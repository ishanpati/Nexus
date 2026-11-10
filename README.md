# NEXUS

NEXUS is a local-first universal command bar being built for Windows 11. Its goal
is to make everyday computer actions accessible from one keyboard-driven interface.
The finished application must work without ChatGPT, API keys, paid AI services,
LLMs, or cloud infrastructure. Target hardware: Intel Core Ultra 7 225H, 16 GB RAM.

**Current status:** a dark, borderless command bar with local command filtering,
installed-application search, keyboard selection, real Notepad launching, and Escape-to-hide. The 650 x 310 window
opens near the upper center of the active monitor and skips the taskbar.
Drag the top header to move it temporarily; showing NEXUS again restores its default
upper-center position. **Ctrl+Shift+Space** toggles the bar; Escape and switching
to another app hide it while NEXUS keeps running. Launching the executable again
shows the existing instance. There is no tray entry yet.

## Existing project

| Location | Responsibility |
| --- | --- |
| `src/main.tsx`, `src/App.tsx`, `src/App.css` | React entry point, UI, and styles |
| `src/commands.ts` | Typed command registry, aliases, and matching |
| `src/services/commandService.ts` | Narrow typed execution request to Rust |
| `src/hooks/useLauncherWindow.ts` | Input focus lifecycle and native hide/drag requests |
| `src/appSearch.ts`, `src/launchUsage.ts` | Local search ranking and successful-launch counts |
| `src/appCatalog.ts`, `src/hooks/useAppCatalog.ts` | Shared in-memory catalog and asynchronous startup/refresh |
| `public/`, `src/assets/` | Static assets |
| `src-tauri/src/main.rs`, `src-tauri/src/lib.rs` | Native entry point, Tauri setup, and Rust commands |
| `src-tauri/src/launcher_window.rs` | Native positioning, visibility, and placement tests |
| `src-tauri/src/shortcut.rs` | Global shortcut registration, toggle handler, status, and cleanup |
| `src-tauri/src/active_monitor.rs` | Windows foreground-monitor lookup with primary fallback |
| `src-tauri/src/commands/` | Typed native dispatch, argument validation, and Notepad handler |
| `src-tauri/src/installed_apps/` | Read-only Windows application discovery, typed catalog, cache, and tests |
| `tests/commands.test.mjs` | Command matching tests using Node's built-in test runner |
| `src-tauri/tauri.conf.json`, `src-tauri/capabilities/` | Window/build configuration and permissions |
| `vite.config.ts`, `tsconfig*.json` | Frontend tooling and TypeScript checks |

Declared dependencies: React/React DOM `^19.1.0`, Tauri API and opener plugin `^2`;
development tooling includes TypeScript `~6.0.3`, Vite `^8.0.16`, the React Vite
plugin `^6.0.2`, React type definitions, and Tauri CLI `^2`. The Rust crate uses
edition 2024, Tauri/tauri-build/opener major 2, the official global-shortcut and
single-instance plugins major 2, and serde/serde_json major 1. Shortcut registration
runs entirely in Rust; the already-installed JavaScript shortcut package is not used.
These are manifest requirements; `package-lock.json` and `src-tauri/Cargo.lock`
record resolved dependencies. No runtime AI integration was found in the inspected
application source or manifests.

## Development

With Node/npm, Rust, the Windows Tauri build prerequisites, and project dependencies
installed, run these commands from the project root:

```powershell
npm.cmd run tauri dev     # Desktop development app
npm.cmd run build         # TypeScript check and frontend build
npm.cmd test              # Matching tests (Node 22.6+ with type stripping)
npm.cmd run tauri build   # Desktop release build and bundles
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
```

Use `npm.cmd` in PowerShell if its script policy blocks `npm.ps1`; no policy change
is needed. `npm run dev` starts only Vite on port 1420. Test this UI through the
Tauri desktop app because it uses native window events and commands. Offline Rust
tests require dependencies already present in the local Cargo cache.
Engineering rules are in [AGENTS.md](AGENTS.md).

## Open Notepad and window checks

1. Run `npm.cmd run tauri dev`. Confirm the dark rounded window appears near the
   upper center of the active monitor, without a title bar or taskbar button.
2. Start typing without clicking the input. Enter `open notepad`; the selected
   result should read **Open Notepad**. Matching ignores case and extra spaces.
3. Press Up and Down. With one available result, selection stays on that result.
   Press Enter or click it: Windows Notepad should open and NEXUS should hide.
   Repeat after reopening with `notepad` and `launch notepad`; both are aliases.
4. Replace the query with `unmatched command`. Confirm the no-results message;
   pressing Enter should do nothing. Clear the input to restore the suggestion.
5. Drag the header containing the NEXUS name to move the window. The search field
   and result remain interactive. Switching to another app dismisses NEXUS.
   Ctrl+Shift+Space reopens it at the default position with the input focused.
6. Press Escape (or click esc). The window hides and the process stays running.
   Press Ctrl+Shift+Space to reopen. Press it again while visible to hide the bar.
   Closing the visible window with Alt+F4 exits the app and releases the shortcut.
7. If Windows cannot launch Notepad, NEXUS must stay visible and show an error.
   Missing-app and permission failures are covered by automated tests; do not
   rename or remove Windows system files to simulate them manually.

The frontend build, five matching tests, and fifteen Rust tests pass. Rust tests
cover native argument validation, launch/hide ordering, error messages, trusted-path
resolution, and window placement without opening applications during unit tests.
The desktop app compiled and ran; Enter on `launch notepad` opened Windows Notepad
and hid NEXUS. Manual mouse dragging was confirmed by the product owner. Windows
checks also verified shortcut show/hide, immediate typing after reopening, Escape,
native focus-loss dismissal, result clicking, duplicate executable launch, the
shortcut-conflict warning/relaunch fallback, and shortcut release on exit.
Multiple physical monitors, other display scales, a packaged installer, and
operation with networking disabled have not been manually tested.

## Command boundary

`open_notepad` is the only registered executable command. The frontend sends
`{ id: "open_notepad", args: {} }` inside the `execute_command` request; the query
text never crosses that boundary. Rust rejects unknown IDs, extra fields, paths,
executable names, non-object arguments, and nonempty argument objects.

Rust resolves `notepad.exe` beneath the directory returned by Windows'
`GetSystemDirectoryW`, then starts that absolute executable with no arguments.
It does not use a shell, PATH lookup, or user-supplied executable. A successful
process start is followed by hiding the bar; a launch error leaves it visible.
If hiding fails after launch, the error explicitly says Notepad was already opened.

## Shortcut and lifecycle details

The official single-instance plugin is registered first. A second executable
launch activates the original window and exits; its arguments are not executed.
To test this while the dev session is running, use a second terminal:

```powershell
& .\src-tauri\target\debug\nexus.exe
```

Do not start a second Vite/dev server for this check. Task Manager should show one
`nexus.exe` process after the second launch exits.

All summon entry points use `launcher_window::show` (also exposed as
`show_launcher`). On Windows, it looks up the foreground application's monitor
before taking focus, then uses that monitor's work area and display scale. If
lookup fails, it uses the primary monitor. Active means the focused application's
monitor, not the mouse pointer's monitor. With multiple displays, focus an app on
each monitor and summon NEXUS to check placement. Every summon resets temporary
dragging; a mere focus change inside NEXUS does not reposition it.

Only a shortcut press toggles visibility; release does not. Native window focus
loss hides the bar, while DOM/input blur does not, so result clicks remain usable.
Hidden windows keep the process alive. Alt+F4 on the visible window quits; the
exit handler unregisters the shortcut and the single-instance plugin releases
its lock. Windows also releases registrations when a process terminates.

If registration fails, NEXUS shows a warning instead of aborting startup. Launch
the executable again to reopen it while the shortcut is unavailable. After
releasing the conflicting shortcut in the other app, exit and restart NEXUS.

To reproduce a conflict without changing other applications, exit NEXUS, then run
this test-only fixture in one terminal:

```powershell
cargo run --locked --offline --manifest-path src-tauri/Cargo.toml --example shortcut_conflict
```

Leave it waiting, start NEXUS in another terminal, and confirm the warning appears.
Confirm Escape still hides it and launching the executable again reopens the same
instance. Exit NEXUS, press Enter in the fixture's terminal to release the shortcut,
then start NEXUS normally and check Ctrl+Shift+Space again. The fixture is not
included in the application and does not persist any shortcut settings.

Frontend capabilities grant only event listen/unlisten permissions. Shortcut
registration, cleanup, window operations, and single-instance activation are
native Rust operations and need no frontend plugin permissions. The desktop
capability grants no additional permissions; opener permissions are not granted.

## Installed-application discovery (v0.2 backend)

The Rust commands `list_installed_apps` and `refresh_installed_apps` return a
catalog object with `apps`, `generation`, `scanned_at_unix_ms`, `sources`,
`warnings`, and `duplicates_removed`. Both commands take no application paths,
search text, or executable names. Discovery does not launch anything. Catalog apps
are searchable as preview results in the command bar. The existing Notepad command and window
behavior are unchanged; release version numbers and installer identity are unchanged.

Each app has an opaque stable `id`, `display_name`, typed `launch`, optional `icon`,
and `sources`. A shortcut retains its original `.lnk` path, raw target, arguments,
working directory, show mode, and explicit AppUserModelID when present. Its `.lnk`
remains authoritative for any future launching: do not reconstruct a command from
these metadata fields. Packaged apps retain their AUMID; other catalog apps retain
their AppsFolder parsing identifier. `alternate_launches` preserves the Shell
identifier when an entry is merged with its shortcut. Icon references describe a
resource path/index or a Shell item; image extraction is not implemented.

The first list call scans lazily on a dedicated COM STA thread, off the UI thread.
Further list calls return the in-memory snapshot. Concurrent first requests share
one scan; refresh explicitly replaces the snapshot, including removing old apps.
A failed refresh returns an error and keeps the last snapshot. Partial results
include source completion flags and diagnostics. No files or settings are written
by discovery; there is no disk cache, watcher, service, or network dependency.

Supported sources and limits:

- Current-user and all-users Start Menu Programs folders, located through Windows
  Known Folder APIs. Recursive scanning stays inside these trees, skips reparse
  points, and has a depth limit. Existing `.lnk` targets ending in `.exe` or `.com`
  are included. Missing/inaccessible targets are omitted with diagnostics.
- Current-user Windows AppsFolder, including registered Win32 applications and
  launchable UWP/MSIX/Microsoft Store applications. Packaged identifiers are
  classified using Windows' `ParseApplicationUserModelId`. Framework/resource
  packages without an AppsFolder application are not enumerated.
- Exact native identities are deduplicated. An explicit AppUserModelID shared by
  one shortcut and a Shell entry also merges them, retaining both launch forms.
  Different shortcuts sharing an ID remain separate because they can represent
  profiles or different arguments. Names and executable paths alone never merge
  entries, so some apparent duplicates intentionally remain. IDs are stable while
  the native shortcut path or application identifier is unchanged.
- Recognizable uninstall labels/executables, a known crash-handler executable,
  and path-identified help/document/script entries are excluded. Filtering is
  conservative; localized uninstallers or obscure internal helpers can remain.
- Portable apps outside these sources, apps registered only to another user,
  services/drivers, URL/document/script shortcuts, and advertised MSI or virtual
  shortcuts without a filesystem target are not independently discovered. Some
  may still appear through AppsFolder as opaque Shell applications. Shell catalog
  membership is not a guarantee that an application can launch; opaque targets
  cannot be fully validated without activating them, which this feature never does.

The direct Windows-only `windows` 0.62.2 dependency uses the version already in the
lockfile. No new frontend packages or permissions were added. Native API references:
[Windows Known Folders](https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid),
[Shell link metadata](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishelllinkw),
and [AppUserModelIDs](https://learn.microsoft.com/en-us/windows/win32/shell/appids).

### Inspect and test the catalog

From the project root, use the read-only diagnostic (the same Rust cache and
scanner as the Tauri commands; no GUI instance or global shortcut is registered):

```powershell
cargo run --locked --offline --manifest-path src-tauri/Cargo.toml --example installed_apps > src-tauri/target/installed-apps.json
$catalog = Get-Content -Raw src-tauri/target/installed-apps.json | ConvertFrom-Json
$catalog.apps.Count
$catalog.sources | Format-Table
$catalog.apps | Select-Object display_name, id | Format-Table
$catalog.apps | Where-Object display_name -Match 'Notepad|Calculator' | ConvertTo-Json -Depth 8
$catalog.warnings
```

The diagnostic checks that a second list is identical to the cached first list,
then explicitly refreshes and verifies the generation increment before printing
JSON. The output is local, under ignored `target/`, and includes installed paths
and shortcut arguments; it is not sent anywhere. Run it from your normal Windows
user session: a restricted sandbox can hide AppsFolder or user-specific paths.

Future frontend code can call the registered commands without new permissions:

```typescript
import { invoke } from '@tauri-apps/api/core';
const catalog = await invoke('list_installed_apps');
const refreshed = await invoke('refresh_installed_apps');
```

For native regression tests and compilation:

```powershell
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
cargo build --locked --offline --manifest-path src-tauri/Cargo.toml
```

Tests cover identity-based deduplication, ambiguous profiles, preserved alternate
launch forms, sorting, empty catalogs, concurrent cache reads, refresh replacement,
failed-refresh recovery, and native temporary `.lnk` metadata/broken-target handling.
They never launch the shortcut targets. To check the desktop regression manually,
run `npm.cmd run tauri dev` and repeat the existing Notepad, drag, Escape, focus-loss,
and Ctrl+Shift+Space checks above.

Verified on this Windows machine on 2026-11-10: **191 catalog entries** after
19 native-ID merges (78 shortcuts, 46 packaged apps, 67 other Shell entries).
All three sources completed without diagnostics in the normal user session.
The diagnostic verified cache reuse and refresh; 26 Rust tests and 5 frontend
tests passed, and both the native executable and frontend compiled. The sandbox
could not access AppsFolder, so the machine count comes from the authorized
read-only diagnostic outside it. Desktop IPC invocation, the full v0.1 mouse/
keyboard regression sequence, and a rebuilt installer were not manually tested
for this backend change.

## Application search

At frontend startup NEXUS calls `list_installed_apps` once and retains the snapshot
in memory. React development remounts share the same pending request. Typing only
searches the local index; it never calls discovery. **Refresh apps** explicitly
calls `refresh_installed_apps`, keeps the previous results usable while waiting,
and retains them if refresh fails. Loading, empty-catalog, no-match, partial-source,
and error/retry messages appear alongside the working built-in commands.

Ranking uses exact name/alias, name prefix, word prefix, substring, then conservative
typo matching. Fuzzy matching requires at least four characters, accepts at most
one edit for queries under eight characters or two for longer queries, and requires
80% similarity. Adjacent transpositions count as one edit. Punctuation-containing
queries and queries longer than 64 characters do not receive fuzzy matching.
Curated aliases include `vscode`, `vs code`, `chrome`, `edge`, and `calc`; executable
paths are never used as aliases. Existing command aliases and partial matching remain.

Successful-launch frequency is a capped tie-breaker after textual tier and edit
distance, so popularity cannot outrank a better text match. Counts are local to
NEXUS, stored in WebView local storage, and never inferred from clicks or previews.
Currently only successful executions of the existing Open Notepad command create
counts. App previews do not launch or accumulate usage; the app-ID ranking support
is ready for a future launching feature. No Windows-wide usage tracking is added.

The initial batch contains eight ranked results in the existing compact scroll
area (the window does not grow to fit eight rows). Scrolling near the bottom, the
Show more button, or arrow navigation reveals additional batches. Up/Down wraps
through all matches and brings the selected row into view. Escape still hides
the window. Enter/click on an **app preview** only displays an explanatory message;
the **Open Notepad command** still launches Notepad through its existing handler.

Manual checks:

1. Run `npm.cmd run tauri dev`. The initial results should contain Open Notepad
   plus app previews after the catalog loads.
2. Try `chrome`, `chorme`, `calc`, and an installed app's full name. Try `zzzzzzzz`
   to check the no-results message.
3. Search `windows` (or another term with more than eight matches). Scroll down
   to reveal more; press Up from the first result to wrap to the last, then Down
   to return to the first. Changing the query resets selection and the batch.
4. Press Enter on an app preview: it should explain that launching is not enabled
   and leave NEXUS visible. Search `open notepad` and press Enter to check the
   existing real command; it should open Notepad and hide NEXUS.
5. Click Refresh apps, then keep typing. Existing results remain searchable during
   discovery. Press Escape and summon again with Ctrl+Shift+Space.

`npm.cmd test` covers ranking tiers, aliases, typo limits, nonsense input, usage
tie-breaking, v0.1 command integration, batch navigation, startup request coalescing,
cache reuse, refresh replacement, and failure/retry handling. No new packages,
native permissions, or application-launch commands were added for search.

Search verification on 2026-11-10: all **17 frontend tests passed**, and the
TypeScript/Vite production build passed. The Tauri desktop app started successfully
and loaded 191 catalog entries. Desktop checks verified `chorme` matching Chrome,
non-launching app previews, no-results text, refresh feedback and completion,
eight-result batches, scroll expansion, arrow wraparound with automatic scrolling,
the existing Notepad launch/hide behavior, reopening, and Escape-to-hide. Initial
failure, empty-catalog, and refresh-failure recovery were tested in the catalog
store tests, not forced against the live Windows catalog. No installer was rebuilt.
