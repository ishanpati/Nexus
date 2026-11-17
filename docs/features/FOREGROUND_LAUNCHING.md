# v0.5.1 Stage A: foreground application launching

Stage A was implemented on fix/foreground-launch with 0.5.0 metadata.
Stage B updates metadata to 0.5.1; see the [release audit](../verification/V051_VERIFICATION.md).
No tag or publication is part of either stage.

## Investigation and root cause

The existing shortcut and AppsFolder paths already used ShellExecuteExW, with
SEE_MASK_NOASYNC and SEE_MASK_FLAG_NO_UI. Packaged apps used
IApplicationActivationManager::ActivateApplication. No command interpreter was used.
Shell process information and packaged activation PIDs were discarded. Native
acceptance was followed by hiding the always-on-top NEXUS launcher; React then hid
it again. Windows may restore a previously active window when the launcher hides.
There was no target-window discovery, minimized-window restore, activation request,
or verification to correct that handoff. The registry shared the launch call but
reported acceptance alone. This is a source-confirmed focus handoff gap; the original
intermittent symptom was not reproduced under controlled desktop input here.

## Shared implementation and contract

installed_apps/launching.rs validates a catalog ID and refreshes shortcut metadata.
Both launch_installed_app and the registry app.launch adapter enter this path.
The shortcut itself remains authoritative for arguments, working directory, show
metadata, and Shell behavior. AppsFolder still launches its registered PIDL;
packaged apps still use ActivateApplication with supported COM foreground delegation.

foreground_windows.rs retains Shell handles using SEE_MASK_NOCLOSEPROCESS and RAII
CloseHandle. A missing handle is valid. Activation-manager PIDs are retained with a
query handle where possible. No arbitrary input, reconstructed command, executable
name search, window title, or user-supplied identity is used for activation.

Candidate windows must be visible, uncloaked, unowned, ordinary top-level windows belonging
to the intended identity. Matching uses explicit window AppUserModelID, process
AppUserModelID, an exact full executable path from the current shortcut, or a
retained Shell process identity where stronger metadata is unavailable. Explicit
IDs override path matches. While a returned launch process is active, old background windows of other
processes are deferred; after a single-instance forwarder exits, exact-identity
fallback remains available. Shortcuts with arguments cannot select an existing
background window by shared app ID or executable path alone; a new path-only
window must belong to the live process returned by this launch. Multiple ambiguous windows remain
unselected; prefer the matching foreground window, then a unique new window, then
a unique existing window. Intermediate processes are followed only when their
windows expose the intended app identity; unrelated launcher windows are not guessed.

After launch acceptance, the caller-owned hide callback runs once through the native
path. Interaction ownership is checked separately from activation intent: it can
hide the old main bar after focus moved, while preserving a new summon or query. A synchronous visibility query flushes the window dispatcher
before sampling the foreground restored by hide. A hide warning preserves launch
success and React does not retry hiding. Real launch failures retain the query;
success clears it. App/workflow Enter activation retains the selected row on key-down
and dispatches once on key-up so its own release cannot invalidate native intent.
Repeat key-down does not replace that row; blur, query changes, another action,
Escape and summon discard it. Click dispatch is unchanged. Existing frontend and
native execution guards remain in place.
Workflow hiding targets only the owned main launcher, never Workflow Studio.

Discovery runs on a backend worker, at most 100 polls separated by 50 ms. There is
also a six-second session deadline reported as timeout and checked between native calls, including time
spent launching; OS/COM calls themselves cannot be forcibly interrupted. There is
one activation request per selected target, followed by up to 200 ms verification.
Only minimized windows receive ShowWindowAsync(SW_RESTORE); windows are not maximized.
SetForegroundWindow is followed by GetForegroundWindow and identity revalidation.
On denial, a still-current target receives three taskbar flashes via FlashWindowEx.

The serialized result separates accepted/launched, window_detected, activation,
readiness_verified (always false), and warning. Window detection may be true while
selection is ambiguous and activation times out. No visible window is a successful
accepted launch with unverified activation. Foreground success is a point-in-time
observation, not a promise that the application stays foreground or finishes loading.

## Cancellation and stale work

Direct launch intent is captured on command admission, before worker/COM/catalog
delay; workflows capture run-wide intent before preflight. A generation token invalidates activation on a newer launch, summon, Escape, or
shortcut dismissal. GetLastInputInfo and the foreground baseline suppress activation
on newer user input or an unrelated foreground transition. This is deliberately
conservative: mouse movement or typing may also suppress an
extra activation request. A run-wide input/interaction intent prevents later workflow
steps from renewing focus authority after the user moved on. Workflow cancellation
stops discovery but preserves the actual result of an already admitted native launch.
Each step finishes its discovery before the next action starts. There are no detached
pollers, persistent focus hooks, synthetic keys, attach-thread-input tricks,
elevation, new always-on-top flags, registry changes, or runtime network requests.
Windows or the launched application can still independently activate its own window.

## Windows APIs

Launch/resource APIs: ShellExecuteExW, ActivateApplication,
CoAllowSetForegroundWindow, GetProcessId, OpenProcess, CloseHandle,
CoTaskMemFree, PropVariantClear, and scoped COM apartment cleanup.

Identity/window APIs: EnumWindows, GetWindowThreadProcessId, GetWindow,
IsWindowVisible, GetWindowLongPtrW, DwmGetWindowAttribute(DWMWA_CLOAKED),
SHGetPropertyStoreForWindow,
GetApplicationUserModelId, QueryFullProcessImageNameW, GetLastInputInfo,
IsIconic, ShowWindowAsync, SetForegroundWindow, GetForegroundWindow, FlashWindowEx.
Input and DWM API features on the existing windows crate are enabled. Dependency
versions, permissions and database schemas are unchanged; Stage B updates only
the root application versions in manifests/lockfiles.

Microsoft references: [Shell execution and nullable process handles](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-shellexecuteinfow),
[foreground restrictions](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow),
[window app identity](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetpropertystoreforwindow).

## Stage A verification, 2026-11-17

Automated checks on this branch:

- Frontend: 106 tests passed using the npm test suite with Node's dot reporter.
- Rust: cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets --quiet: 119 passed, 2 existing opt-in tests ignored.
- Rust doctests: passed (0 doctests).
- npm run build: TypeScript and Vite passed.
- cargo fmt --check and git diff --check: passed.
- Clippy --all-targets: passed with existing repository warnings; no lint gate was relaxed.
- npm run tauri build -- --debug --no-bundle -- --locked --offline: passed;
  src-tauri/target/debug/nexus.exe produced, no installers or release artifacts.

Focused mock tests cover candidate ambiguity/priorities, slow/windowless timeouts,
verified activation, denied activation and flashing, cancellation/stale suppression,
generation invalidation, accepted-launch outcomes, query retention and avoiding a
second hide. Existing tests cover catalog identity, preserved shortcut metadata,
selected partial/fuzzy rows, execution guards, workflow sequencing/cancellation,
and Studio lifecycle.

An intermediate offline isolated debug build (identifier com.nexus.foreground-verification)
started and rendered its catalog with a fresh local data directory. The computer-use
helper could capture the launcher but failed to establish editable input focus.
This is startup/rendering evidence only: no target application was verified foregrounded.
The verification process was stopped; production workflow data was not used.

Required desktop acceptance remains: closed Win32 app; running single-instance app;
minimized app; slow startup; Store app; tray/windowless app; saved single/multi-app
workflow; cancellation; intentional switch while pending; repeated Enter; click with
partial query; Escape/global shortcut; Studio close/reopen. Observe the actual native
foreground window for every claimed activation. Repeat with networking disabled.

Windows can deny activation even after a successful API return. Protected processes,
missing identities, ambiguous profiles, shared hosts without a matching top-level
identity, transient splash windows, custom/owned windows, and startups exceeding the
budget can remain unverified. This implementation stops safely instead of selecting
an unrelated window. It cannot suppress focus changes initiated independently by
the Shell or the target application.

## Changed files

- src-tauri/src/installed_apps/{foreground.rs, foreground_windows.rs}: policy, identity, resources, discovery, activation and mock tests.
- src-tauri/src/installed_apps/{launching.rs, native.rs, mod.rs}: shared result/context, launch receipt and COM worker scope, module wiring.
- src-tauri/src/action_registry/{native.rs, tests.rs}: shared launcher adapter and runtime context.
- src-tauri/src/workflow_execution/{ipc.rs, tests.rs}: run-wide activation intent, focused-main hide, workflow outcomes regression.
- src-tauri/src/workflows.rs: save-reference runtime supplies no activation context; validation does not launch.
- src-tauri/src/launcher_window.rs: invalidate stale activation on explicit lifecycle interactions.
- src-tauri/Cargo.toml: enable existing windows crate keyboard/input API feature.
- src/{App.tsx, installedAppActions.ts, resultExecution.ts}: Enter release, typed results, single native hide ownership.
- tests/resultExecution.test.mjs: launch outcomes, failure retention, hide ownership and Enter gesture regressions.
- docs/README.md, docs/features/workflows/WORKFLOW_EXECUTION.md, and this document: contract and verification record.
