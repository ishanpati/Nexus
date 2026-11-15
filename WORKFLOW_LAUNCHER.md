# v0.5 Stage F: launcher workflow search and execution UI

Implemented on `feature/workflow-studio`. Existing staged A-E2 changes are preserved.
No dependency, lockfile, permission, Rust engine or native shortcut changes were needed.

## Search and identity

`w ` is an exclusive mode alongside `f `. Prefixes require a leading literal letter
and space, ignoring letter case. Bare `w`, leading whitespace and tab separators
stay in ordinary command/app search. Empty workflow queries list all saved rows.
The existing ranker handles normalized names and aliases, including exact, prefix,
word-prefix, substring and conservative typo tiers. Workflow alias prefixes and
substrings participate just like registered command aliases. Descriptions display
but do not affect launcher ranking. The Studio's separate list filtering is unchanged.

Workflow names/aliases are scoped to workflow mode: `lock` executes the existing
system command, while `w lock` selects that saved workflow. Likewise `create workflow`
and `edit workflow` retain their Studio commands. A workflow with one of those names
is found through `w create workflow` / `w edit workflow`; `f ` still searches files.

Rows contain name, description and action count. Up/Down selects results, with the
existing batch/scroll behavior. Both Enter and clicks pass the selected row's stable
UUID through the common dispatcher. No search, hover or preview calls the executor.
Repository events, native focus, launcher summon and Refresh workflows reload saved
rows. Stale reads/errors are ignored; activation is blocked during refresh and failed
reads clear old rows. Native preflight remains authoritative if a target disappears.

## Start, progress and lifecycle

Start sends only `workflowId` to the existing main-only Rust IPC. A synchronous UI
gate prevents overlapping activations, a service gate blocks duplicate workflow
starts/active runs, and Rust retains its independent atomic workflow slot.
Accepted starts clear input and show progress without waiting for steps to finish.
Rejected starts keep the query and show the backend message. An uncertain/lost start
response triggers a list refresh and a check-runs message; it never retries execution.
A subsequent asynchronous preflight failure is a failed run, not a rejected start.

The listener is installed before initial listing. Identifier-only events trigger
status reads; status is also fetched immediately after start and polled every second
while any run is active. Reads coalesce while pending. Lower sequence snapshots cannot
overwrite newer progress, even when events precede the start response. Retention is
bounded to 128 snapshots and preserves active runs. Runs are session-only, matching
the existing engine; force termination does not provide durable run recovery.

Workflow runs opens an independent progress view with run selection. It shows
completed counts, current and completed steps, stopped step/errors, completion,
and run/workflow/revision details. App steps explicitly report Windows activation
acceptance with readiness unverified. Failed runs warn that earlier actions are
not undone. Start acceptance is never presented as overall completion.

Cancel workflow is visible in the progress toolbar and sends explicit `runId` intent.
Cancellation stops subsequent admission; an in-flight native request cannot be
recalled. Sensitive prompts show the backend message, action ID, schema, parameters,
target computer and step ID. Only a separate Approve button sends the current run,
step and token; Deny is explicit too. Rust enforces expiry, binding and single use.
No saved flags or search text grant approval. Automation never approves real lock
or sleep during verification.

Closing progress, typing a new query, Escape, blur and launcher hiding do not send
cancel. Escape clears and hides; summon clears and focuses. The run store is separate
from input state, and Workflow runs recovers it after reopening. Input remains usable
during execution; only the brief start request uses the existing activation gate.
Existing file/sleep confirmation and app/system execution lifecycles are unchanged.

## Verification

- Targeted launcher/matching/dispatch/file/system tests passed.
- Final `npm test`: 103 passed, zero failed.
- `npm run build`: TypeScript and Vite production build passed.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`:
  passed, including existing workflow engine/storage and launcher regression tests.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --doc`:
  passed (zero documentation tests).

Automated checks cover empty/partial/alias/case search, exclusive modes/collisions,
different selected UUIDs, shared Enter/click dispatch, duplicate starts, accepted/
rejected start lifecycles, lost-response recovery without retry, out-of-order events
and snapshots, repository rename/delete refresh and stale reads/errors, explicit
approval/denial/cancel IPC, concurrent-decision guards, bounded retention, and rendered
current/completed/failed/confirmation states. Existing Rust tests mock Windows actions
for approval, replay/expiry, cancellation between steps and stop-on-failure behavior.

Native Windows verification used the computer-use skill, embedded local assets and
isolated identifier `com.nexus.fverification`. Three fixture workflows were stored
only in its separate AppData database: Coding Mode (two catalog Notepad requests),
A Approval Check (lock prompt), and Failure Check (nonexistent folder). The production
workflow database was untouched. Detailed native results are recorded below.

No network fetches or application server were required. Cargo was locked/offline;
GUI assets and data were local. A final smoke run restarted only the isolated build
with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--proxy-server=http://127.0.0.1:9
--proxy-bypass-list=<-loopback> --disable-quic`. Process inspection confirmed the proxy
and bypass flags on its WebView2 process. This uses Microsoft's documented
[WebView2 browser flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags).
The embedded UI loaded, `w dev` found the renamed workflow by alias, and both app
steps completed with the query cleared and truthful acceptance feedback. HTTP/HTTPS
was routed away from the network; the machine adapter stayed enabled. This was a
per-process offline smoke check, not a physically disconnected desktop session or
a system-wide network isolation test.

Native outcomes verified:

- `w ` listed all three fixtures with descriptions and counts. Down moved from
  A Approval Check to Coding Mode; Enter started Coding Mode's saved UUID, not the
  first row. `w cod` found it; clicking the result executed the same workflow UUID
  in a distinct intentional run. Additional Enter in progress did not start a run.
- Coding Mode showed two completed steps, clearing input on accepted start.
  Application success explicitly said Windows accepted launch and readiness was
  unverified. No claim about a fully loaded application was made.
- `w approval` matched an alias. The lock step waited for separate approval with
  action/target/step details. Close progress left one active run. Escape hid the
  launcher; Ctrl+Shift+Space reopened with empty input and retained pending approval.
  Its first prompt expired without executing. In the final layout, Cancel workflow
  immediately produced Cancelled with the pending step unexecuted. Real lock/sleep
  approval was deliberately not exercised; authorization/replay is tested with fakes.
- `w broken` selected Failure Check. Native preflight stopped at the missing folder,
  showing Failed, the first step and `invalid_reference` error. No later step ran.
- `edit workflow` opened/reused Studio's saved list. Renaming Coding Mode to Renamed
  Coding Mode and saving made `w ren` find it immediately without process restart.
  Read-only SQLite inspection confirmed revision 2 and the unchanged workflow/both
  step UUIDs. Launcher deletion refresh and rejected/uncertain start behavior were
  checked automatically, not induced in the native GUI.
- `f nexus` returned 100 real Everything file/folder results. Ordinary `notepad`
  retained both built-in and catalog rows; the catalog result was activated.
  `audio status` executed read-only, cleared/hid the launcher, and retained Volume
  40% / Unmuted feedback on reopening. All existing system grammar/dispatch cases
  passed automated regression tests; manual testing did not invoke every native
  setting/power action. No audio writes were performed.

The 650x310 launcher keeps detailed step history and approval parameters scrollable.
Cancel is in the progress toolbar, before that history; long IDs are available under
Run details. The final layout was rechecked natively after this adjustment.

`cargo fmt --check` and staged/unstaged diff whitespace checks passed. The normal
application identifier/embedded debug binary was rebuilt after isolated testing.
The isolated verification process was stopped; its fixture database and ignored build
files remain available for repeat checks. No installer was produced. Existing Cargo
profile-path warnings did not prevent builds or checks. Native Windows transport/
event-loss injection and cancellation during a hung Windows call remain untested;
engine fakes cover cooperative cancellation and listener loss without real side effects.
