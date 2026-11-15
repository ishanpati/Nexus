# Windows system actions — v0.4 development

The existing parser and ranking produce typed actions without executing them.
Enter or clicking a ready result invokes the dedicated system-action service.
Application search, file search, audio controls and their handlers remain separate.

| Command | Native operation |
| --- | --- |
| `sound settings` | `ms-settings:sound` |
| `wifi settings` | `ms-settings:network-wifi` |
| `bluetooth settings` | `ms-settings:bluetooth` |
| `display settings` | `ms-settings:display` |
| `lock` | `LockWorkStation` |
| `sleep` | Confirm, then `SetSuspendState(false, false, false)` |

Only those four Settings page IDs are accepted. Rust maps them to static URIs
and uses `ShellExecuteExW` on the existing COM STA worker. Requests cannot supply
a URI, executable, command string, power flags or extra arguments. No shell,
external helper, network service, dependency version change or frontend permission
is required. Additional API features use the existing Windows Rust binding.

## Confirmation and execution

Sleep initially returns a prompt without calling the power API. A modal shows
what will happen and focuses **Cancel**. **Sleep now** makes a separate request
with the ticket issued by Rust. Tickets expire after 60 seconds and are single-use,
including when the native operation fails. Cancel invalidates the ticket;
requesting a fresh prompt replaces the previous one. This is a local confirmation
protocol, not authentication against an already compromised frontend.

Repeated Enter events are ignored. The dialog gates confirm/cancel callbacks once;
the UI running guard and native launch guard prevent overlapping operations.
There are no automatic retries. Escape cancels the dialog and retains the normal
hide behavior. Successful requests hide NEXUS; a hide failure is reported as a
warning after acceptance, not as an invitation to repeat the operation.

Sleep checks for traditional sleep or Modern Standby support. It temporarily
enables `SeShutdownPrivilege` already assigned to the current process token,
then restores its previous state. It never requests UAC elevation or adds account
privileges. Missing privileges, unsupported states and Windows API failures return
structured errors. No hibernation or force request is made; wake events stay enabled.
The worker can remain occupied until resume.

## Verification

Executed during this implementation:

- `npm test`: **58 passed**, including five new system-action transport/gate tests.
- `npm run build`: TypeScript and Vite production frontend build passed.
- `cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --bins`:
  native Windows debug executable compiled successfully.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`:
  **73 passed, 2 ignored**, including seven new dispatch, confirmation, allowlist,
  capability and failure tests. The ignored tests are the existing opt-in live audio
  test and a subprocess fixture exercised by its parent timeout test.
- `cargo fmt` and `git diff --check` completed successfully. Cargo printed an
  environment path-canonicalization warning for the user profile; builds and tests
  still completed successfully. No release installer was created in this task.

Lock and sleep tests use fake operations exclusively. No test locks or suspends
the computer. Actual Settings navigation, lock, sleep/resume, policy restrictions
and dialog keyboard behavior require the manual checks below. Regression tests
cover prior search/launch logic; that is not a fresh end-to-end Windows launch test.

## Manual checks

Start the updated app with `npm run tauri dev` (avoid leaving an older NEXUS
instance running). Use the existing Ctrl+Shift+Space shortcut between checks.

1. Type each Settings command without Enter: nothing should open. Press Enter:
   Windows Settings should open the corresponding page and NEXUS should hide.
2. Type `sleep`, then Enter. The computer must stay awake and the modal must show
   Cancel as its initial focus. Cancel or Escape must not suspend the computer.
   Hold Enter when activating the result: repeated key events must not confirm it.
3. Request sleep again, wait over 60 seconds, then choose Sleep now: an expired
   confirmation error should appear without suspending. Select sleep afresh to retry.
4. Only when ready to test power behavior, save work, request sleep and explicitly
   choose Sleep now. Wake the computer and verify NEXUS responds to the shortcut.
5. Only when ready to sign back in, execute `lock`. Windows should lock the session.
6. Check `open notepad`, an installed application, `f nexus`, file opening,
   `audio status`, arrow navigation, Escape, dragging and the global shortcut.

## Limitations

Windows acceptance is not proof of a completed transition. Lock is asynchronous;
policy, drivers or hardware may prevent sleep. A Settings URI can open Settings
while the requested page is unavailable or redirected (for example, no Wi-Fi
adapter or policy-hidden pages). Shell activation cannot verify the destination
page. These conditions are not bypassed and no administrator prompt is requested.
Hardware/policy combinations and actual sleep/resume have not been tested here.

API references: [Settings URIs](https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings),
[LockWorkStation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-lockworkstation),
[SetSuspendState](https://learn.microsoft.com/en-us/windows/win32/api/powrprof/nf-powrprof-setsuspendstate).
