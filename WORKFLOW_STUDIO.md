# v0.5 Stage D: separate Workflow Studio

Stage G fixes launcher quit with auxiliary windows: the main window is retained
while runs are active; an idle quit routes through Studio's Save / Discard / Cancel
guard before process exit. Closing Studio itself still hides it and leaves NEXUS
alive. New automated close-policy coverage and outstanding native verification
are recorded in [V05_VERIFICATION.md](V05_VERIFICATION.md). Results below are historical.

Stage E2 adds [saved workflow management](WORKFLOW_MANAGEMENT.md) and replaces the
discard-only prompt with Save / Discard / Cancel for navigation and Studio close.

Stage E1 now extends these window/routes with the [dynamic editor](WORKFLOW_EDITOR.md).
The name-only preview and lack of saving described below are historical Stage D
scope; create/edit routes now support full configuration and persistence.

Implemented on `feature/workflow-studio`; existing staged Stage A-C work is retained.
Package versions, dependencies, lockfiles and global-shortcut registration are unchanged.

## Window and routes

`create workflow` and `edit workflow` are ordinary searchable registry entries.
Partial selections dispatch their stable command IDs through the existing shared
Enter/click dispatcher. They send `{ mode: "create" }` or `{ mode: "list" }` to
`open_workflow_studio`. Saved rows send `{ mode: "edit", id: <UUID> }`; names never
serve as identity. Rust rejects unknown route fields/modes and invalid/nil edit IDs.

Rust lazily creates one `workflow-studio` webview in the existing process, using
local `index.html?workflow-studio` assets. It has a 950x700 logical initial size,
680x480 minimum, standard window controls, resizing, and a dark theme. Creation
runs on a blocking worker, avoiding Windows WebView2's synchronous-command deadlock.
The shared launch guard serializes opening and suppresses the main window's native
blur dismissal. Studio never mounts launcher hooks or registers another shortcut.

React installs listeners before acknowledging readiness. Rust waits up to eight
seconds for readiness, then restores, shows and focuses Studio before delivering
navigation. The existing launcher success lifecycle clears input and hides the
bar only after this succeeds. Creation/readiness/focus errors retain the query;
recovery shows/focuses main without emitting the reset-on-summon event. The timeout
covers frontend readiness after native creation returns, not a hung Windows API.

## Draft protection and synchronization

Studio close requests are always prevented natively and delivered as retained,
sequenced requests. React checks local dirty state for both navigation and close.
The modal defaults to Keep editing; Escape cancels. Discard is a separate explicit
choice. Accepted close hides the window rather than destroying it or quitting.
Repeated opens reuse it. A pending request cannot be replaced, and stale/replayed
resolution IDs are rejected. Delayed frontend responses are deduplicated by sequence.
The next requested route appears after confirmation; while confirmation is pending,
the previous draft stays intact. A launcher command can successfully focus Studio
and clear the launcher while Studio waits for that confirmation.

There is a name-only draft field to exercise protection. It is deliberately not
saved. The saved list and edit details come from the existing backend repository.
Step editing, save/delete buttons and workflow execution UI are outside this stage.
Drafts are in this webview's memory, not shared React state or persistent storage;
forced app termination, webview reload and OS shutdown can lose them.

Repository IPC permits only main and Studio. Successful save/delete commits emit
payload-free `workflows-changed` events only to those windows. Studio reloads on
notifications, focus and explicit Refresh, using effect cleanup to ignore stale
reads. Dirty drafts retain their loaded snapshot; a later editor must use the
existing optimistic revision checks when saving. Events are hints, not data.
Studio's capability grants only event listen/unlisten, with an exact window label;
there are no added shell, filesystem, arbitrary-window or shortcut permissions.

## Manual Windows verification (pending)

Use a build from this branch with network disconnected. Exit the older installed
NEXUS first so single-instance activation does not forward into the older binary.
Run `src-tauri/target/debug/nexus.exe` after building with embedded assets:

```powershell
npm.cmd run build
cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --bins
```

1. Type `cre`, select Create workflow, and press Enter. Expect create mode, a normal
   resizable titled Studio window, and a hidden launcher. Summon the launcher:
   its input should be empty. Repeat by clicking the selected result.
2. Type `edit work` and activate Edit workflow. Expect the saved list (or an explicit
   empty/storage-error state). Open a saved row and verify its UUID and revision.
   If no workflows exist, stable-ID edit needs an existing Stage B test fixture;
   this stage intentionally provides no save UI.
3. Repeat both commands, including with Studio minimized. Expect the same Studio
   window, restored and focused. Check Task Manager for one NEXUS backend process
   (WebView2 child processes are expected).
4. Enter a draft name. Request Saved workflows, Create workflow again, or an edit
   route. Choose Keep editing / press Escape: the name and view must remain.
   Repeat and choose Discard changes: the intended view must open.
5. With a dirty draft, click X or press Alt+F4 in Studio. Cancel: Studio and the draft
   remain. Repeat and discard: Studio disappears, NEXUS remains running. With a
   clean draft it should close without a prompt. Reopen from the launcher.
6. While Studio is visible, use Ctrl+Shift+Space repeatedly. The command bar must
   remain summonable. Check Notepad, a catalog app, `audio status`, `f nexus`, Escape
   and normal focus-loss dismissal. Clicking within Studio must not hide Studio.
7. For a development-only readiness failure check, pause Studio JavaScript before
   its `studio_ready` call for more than eight seconds. Expect a launcher error
   with its query preserved. Resume and retry: the existing window must be reused.
   Automated tests inject creation/readiness/focus failures without native windows;
   a real WebView2 creation/focus failure has not been induced.
8. If using an isolated Stage B storage fixture, save/delete via the repository
   while the list is open: it should refresh after the notification. Bring Studio
   back into focus to recover from a missed event. A dirty draft must not be reset.

Automated results are recorded below. Compilation and unit tests do not establish
native focus, window-control, shortcut, or offline GUI behavior.

## Executed verification

- Targeted Node command, dispatch and Studio tests passed. The final full
  `npm test` run passed all 72 tests, zero failures.
- Four targeted Rust Studio tests passed: create/reuse sequencing, injected
  creation/readiness/focus failures, retained pending requests/replay rejection,
  and closed route validation with UUID edit identity.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`
  passed, including the existing storage, execution and launcher regression tests.
- `npm run build` passed TypeScript checking and Vite production compilation.
- Native debug builds passed both normally and with `--features tauri/custom-protocol`
  to embed the production frontend for the server-free manual check above.
- `cargo fmt --check` and Git diff whitespace checks passed. Existing Cargo profile
  canonicalization warnings and Git CRLF notices did not prevent checks.

The initial targeted Rust run exposed extra fields accepted on unit enum variants;
empty struct variants now reject them. The initial full Node run exposed an old
empty-query assertion assuming Notepad was the only built-in. It now verifies
Notepad still precedes the matching catalog app; ranking code is unchanged.

No dependencies were downloaded, no production workflows were written, and no
installer was produced. The older running release process was left untouched.
Manual Windows interaction, actual WebView2 failure injection, and disconnected
GUI regression remain pending. Automated lifecycle tests use fakes; they do not
prove Windows focus transfer or modal interaction in a running webview.
