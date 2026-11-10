# NEXUS

NEXUS is a local-first universal command bar being built for Windows 11. Its goal
is to make everyday computer actions accessible from one keyboard-driven interface.
The finished application must work without ChatGPT, API keys, paid AI services,
LLMs, or cloud infrastructure. Target hardware: Intel Core Ultra 7 225H, 16 GB RAM.

**Current status:** a dark, borderless command bar with local command filtering,
keyboard selection, real Notepad launching, and Escape-to-hide. The 650 x 310 window
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
| `public/`, `src/assets/` | Static assets |
| `src-tauri/src/main.rs`, `src-tauri/src/lib.rs` | Native entry point, Tauri setup, and Rust commands |
| `src-tauri/src/launcher_window.rs` | Native positioning, visibility, and placement tests |
| `src-tauri/src/shortcut.rs` | Global shortcut registration, toggle handler, status, and cleanup |
| `src-tauri/src/active_monitor.rs` | Windows foreground-monitor lookup with primary fallback |
| `src-tauri/src/commands/` | Typed native dispatch, argument validation, and Notepad handler |
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
