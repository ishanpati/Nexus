# NEXUS v0.5.0 Stage G release-readiness audit

Date: 2026-11-15. Windows x64 development workspace, branch
`feature/workflow-studio`, HEAD `e8c4a35` (v0.4 implementation).
The v0.5 Stages A–F were already staged and uncommitted at audit entry. Existing
changes and staging were preserved; Stage G changes are left for owner review.
No merge, commit, tag, installation or release publication was performed.

Status: release candidate, **desktop acceptance pending**. All GUI/system-control
checks were deliberately excluded at the owner's request. Historical verification
in other documents is not counted as a newly executed Stage G test.

## Audit and fixes

Inspected engineering rules, README, architecture, changelog, action/storage/
execution/Studio/editor/management/launcher notes, historical release notes,
source, existing tests, Git history, manifests/lockfiles, native configuration,
capabilities and Tauri's cached path resolver. Stack and dependency versions are
preserved. Only NEXUS package versions changed, to 0.5.0; identifier remains
`com.nexus.launcher`, database `user_version` remains 1.

Fixed two source-level defects:

- Launcher CloseRequested could destroy `main` while Studio/audio windows survived,
  removing the global shortcut's summon target. Close now retains `main` and hides
  it for active workflows. With no run, an existing Studio resolves a `quit` intent
  through Save / Discard / Cancel before Rust exits. Rust refuses the deferred quit
  if a workflow has become active. No Studio/run means direct application exit.
- Save IPC accepted sensitive file steps that execution always rejects, and reads
  labelled them executable. Registry prerequisite checks now return sensitivity;
  save rejects those targets and read annotations expose them for repair. Existing
  database rows are not deleted or rewritten by the audit.

Added close-policy/mailbox regression coverage, exercised quit through every
Save / Discard / Cancel navigation case, and extended save-reference coverage to
assert sensitive rejection leaves the stored revision unchanged. Current setup
examples no longer expose the developer's home path. README now describes workflow
CRUD, execution, backup/restore and shared dev/production data.

## Executed automated checks

Commands used the installed Node 26.13.0 directory on the process PATH only.
Cargo used locked offline dependencies. Test/build output was filtered; no global
PATH or network settings were changed.

| Exact command | Observed outcome |
| --- | --- |
| `node.exe --experimental-strip-types --test --test-reporter=dot tests/workflowManagement.test.mjs tests/workflowStudio.test.mjs` | Passed, 13 tests |
| `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml workflow -- --quiet` | Passed, 29 tests (targeted run before metadata version alignment) |
| `npm.cmd test -- --test-reporter=dot` | Passed, 103 tests, 0 failures/skips; npm's script printed the default summary |
| `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets -- --quiet` | Passed, 113 tests, 0 failures, 2 ignored; binary/example test targets contain no tests |
| `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --doc` | Passed, 0 doctests |
| `npm run build` (Tauri beforeBuildCommand) | TypeScript check and Vite production build passed; 60 modules, JS 304.01 kB / gzip 93.22 kB |

`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` passed. Both
`git diff --check` and `git diff --cached --check` returned 0. A local diagnostic
(`node.exe src-tauri/target/stage-g-audit.mjs`, ignored build-folder artifact)
passed: all six version fields equal 0.5.0, unchanged application identifier,
unchanged dependency lock entries compared with the pre-audit staged baseline,
37 root-document local links resolve, and no developer home path in current root
Markdown documents. It checked 139 tracked text files for selected OpenAI token,
GitHub PAT and PEM private-key patterns with no matches. This is a limited pattern
scan, not an exhaustive secret/history audit; old Git history was not rewritten.
No database/log/env/key files were found in the tracked-file inventory. The tracked
`Finance_record.txt` is empty; the pre-existing staged instruction backup remains
untouched for owner review. SQLite files/sidecars are now ignored to reduce accidental
commits. Cargo's sandbox home-canonicalization warning and Git CRLF notices did
not fail checks.

Two existing ignored Rust tests are the live audio
round-trip opt-in and the child timeout fixture (the parent timeout test runs its
fixture). No opt-in audio test, lock or sleep was executed. Action side effects in
workflow/confirmation tests use fakes. Compilation and React server markup tests
do not establish live WebView interaction or actual Windows action behavior.

## Coverage and inspection outcomes

| Area | Evidence and limits |
| --- | --- |
| Registry | Tests cover stable unique IDs, serializable metadata, all parameter types/defaults, unknown fields/IDs/versions, invalid registrations, native adapter selection and reference rejection. Text/optional/defaulted action extension and all editor control types are tested. Rust remains authoritative. |
| Storage | Real temporary SQLite tests cover CRUD, identity/rename/order, connection reopen, duplicate name/alias rejection, stale save/delete and deleted-record resurrection rejection, confirmation cancellation, migration refusal and transactional rollback. These are connection-reopen tests, not installed-process restart tests. |
| Missing references | Tests preserve unchanged obsolete action data and permit repair; save prerequisite tests reject missing app selections without execution. Actual removed/uninstalled app repair via the Windows picker remains manual. |
| Execution | Fakes cover ordered execution, whole-plan preflight, first-failure stop, atomic duplicate prevention, run/step progress, bounded retention, cancellation races, lost listeners, owned DB snapshots, terminal position, denial/expiry/replay/wrong-run approval rejection and last-admission checks. |
| Studio | Automated navigation tests cover open failure retaining input, create/reuse sequence, retained requests, revision failures, Save / Discard / Cancel and mutation gates. Native focus, window reuse, picker, keyboard/mouse and scaling remain manual. |
| Search/regressions | Node suites cover exclusive `w ` and `f `, partial names/aliases, selected-ID dispatch, stale responses, app/system collisions, existing app/file/audio/system transport and confirmation handling. No fresh GUI regression was performed. |
| Execution boundary | Source uses registered actions, catalog-resolved app IDs, safe filesystem paths and fixed Settings destinations. User text is not passed to an interpreter. Notepad/ES are trusted paths with explicit args. Catalog shortcuts intentionally preserve their pre-existing arguments; workflows cannot supply arguments. |
| Permissions | Capabilities expose scoped event listen/unlisten only. New repository/picker/Studio/run handlers check caller labels. No general script/process IPC or frontend shell/filesystem/opener permission was added. Existing v0.4 commands retain their behavior. |
| Privacy/local runtime | Source/manifests show no AI model/API, key, subscription or cloud runtime requirement. No workflow documents are deliberately logged. Current-file scan and caveats are recorded below; this is not a forensic history/dependency security audit. |

## Production packaging

Command: `npm.cmd run tauri build -- --bundles nsis,msi -- --locked --offline`,
with `CARGO_NET_OFFLINE=true`.

The first sandbox build compiled the optimized executable (3m 34s) and NSIS
installer, but MSI packaging failed: WiX ICE validation could not access the
Windows Installer service (`LGHT0217`/`LGHT0216`, error 0x643). The approved
outside-sandbox retry **passed**, compiling the optimized application (3m 02s)
and producing both installers with normal validation enabled. No validation
bypass, dependency update, install, launch, shortcut registration or system action
was performed by packaging. Frontend build passed on both attempts.

| Actual artifact, relative to repository root | Bytes |
| --- | ---: |
| `src-tauri/target/release/bundle/nsis/nexus_0.5.0_x64-setup.exe` | 2,478,254 |
| `src-tauri/target/release/bundle/msi/nexus_0.5.0_x64_en-US.msi` | 3,457,024 |
| `src-tauri/target/release/nexus.exe` | 6,929,920 |

SHA-256 hashes are in [V05_SHA256.txt](../releases/V05_SHA256.txt), computed after the successful
retry. These installers are unsigned. Local ignored logs are `stage-g-build.log`
and `stage-g-build-approved.log`; they include build-machine paths and are not
release content. Older versioned bundles in the same output directories are not
0.5 artifacts. Do not use folder presence as proof of a successful current build.

## Data location and upgrade assessment

Both dev/debug and installed builds use
`%LOCALAPPDATA%\com.nexus.launcher\workflows.sqlite3`. This follows the unchanged
identifier and Tauri's `dirs::data_local_dir().join(identifier)` resolver; there is
no build-profile suffix, custom override, installer database resource, or reset
routine. Database open is lazy, and unsupported schema/unversioned nonempty data
fails without reset. The audit did not read or modify the user's real database.

Generated 0.5 NSIS script inspection found app-data deletion only behind the
uninstaller's deletion checkbox and `$UpdateMode <> 1` (not updating). Its targets
are the application directories under APPDATA/LOCALAPPDATA. Leave that checkbox
unchecked to preserve workflows. Generated MSI source has no workflow database
payload or app-data removal directive. These are installer-source inspections;
neither script was exercised by installing/uninstalling the application.

That source assessment does **not** verify actual installation or upgrade
preservation. Back up the stopped data directory first. Runtime data is plaintext
and may include private app/path references. Do not commit it or distribute it
with installers. Dev builds touch the same data unless deliberately isolated.

## Known limitations and remaining risks

- Native GUI behavior of the two fixes remains untested. Treat these as automated
  regression coverage plus source fixes, pending the checklist below.
- CSP is still disabled (`csp: null`); this is an inherited defense-in-depth gap.
  React renders saved strings as text, but a compromised trusted frontend could
  invoke available application IPC. Confirmation tokens are not authentication
  against compromised frontend code. No CSP hardening is claimed.
- SQLite corruption or one unreadable document can fail the whole list; no repair
  UI, export/import UI, encryption, automatic backup or obsolete-action migration.
  Back up before modifying or manually repairing the database.
- Missing app/path selections must be repaired or removed before saving through
  current-reference validation. Unknown action data can be retained unchanged;
  unsupported actions/versions cannot execute.
- No scripts, arbitrary executable arguments, recursive workflows, delays,
  parallel steps, rollback, resume after process exit or app readiness waits.
  Up to 64 steps/workflow, 8 concurrent workflows, 128 retained session runs.
- Run history/approvals disappear on process exit. Cooperative cancellation cannot
  undo an admitted Windows request. A blocked native call can delay cancellation.
- Filesystem/association changes between checks and open retain Windows' existing
  limits; file-type policy is not malware scanning. UNC/offline-drive checks may
  block workers. Distinct workflows can contend for the shared native action guard
  and fail busy without automatic retry.
- Catalog-dependent availability may be temporarily stale while the catalog loads;
  refresh after loading. A Windows launch acceptance is not proof of app readiness.
- Release panic strategy remains `abort`; worker panic recovery is tested in the
  unwind-enabled test profile, and does not promise recovery from release panics.
- Unsigned installers; WebView2 required. Offline setup needs WebView2 preinstalled.
  Everything desktop and ES remain optional external requirements for file search.
- No startup/memory baseline or actual installed-app performance sample was taken.
  Frontend bundle size is a build measurement, not runtime memory evidence.

## Manual checks excluded by the no-computer-control instruction

None of the following was executed by this audit. These are release acceptance
requirements, not implied passes:

1. **Installed standalone startup:** close existing NEXUS after backing up data;
   install one 0.5.0 installer and launch from its installed shortcut outside
   VS Code/Codex. Confirm embedded assets work without Vite/port 1420, an API key,
   or any development process. Repeat with internet disconnected.
2. **Real workflow/persistence:** create a named workflow with an installed app,
   harmless text document and folder; save and run it. Confirm ordered acceptance
   and final success. Fully exit, confirm the process stopped, reopen the installed
   app, and confirm identical workflow/step IDs, order, name and saved parameters.
3. **Upgrade/data preservation:** record database/workflow details and keep a backup;
   upgrade an earlier installation using the same installer format. Confirm the
   database is not erased/replaced. Repeat 0.5 reinstall. Test uninstall behavior
   and any data-removal prompt only with a backed-up disposable test profile.
4. **Studio lifecycle:** create/edit/list repeatedly with no duplicate window;
   confirm launcher hides only after successful opening. Close Studio and summon
   the launcher via Ctrl+Shift+Space; reopen Studio. With a dirty draft, test Save,
   Discard and Cancel for navigation, native Studio close and launcher Alt+F4.
   Verify Cancel keeps the main window/shortcut usable; successful idle quit must
   really end NEXUS. Test an active-run Alt+F4, which must only hide presentation.
5. **CRUD and repair:** edit/rename without duplicate creation; reorder; confirm
   delete target/ID/revision and cancel deletion; test stale revision recovery.
   Uninstall/remove a referenced app/file in a disposable setup, then repair the
   selection without losing unrelated steps. Test native pickers and action search.
6. **Input/layout:** use mouse and keyboard, Tab/Enter/Escape and long Unicode names;
   resize Studio to 680x480 and check 100/125/150/200% scaling on small/multiple
   monitors. Ensure controls, scrolling, confirmation prompts and errors remain usable.
7. **Run behavior:** start via a partial `w ` match and a nonfirst highlighted row;
   rapid Enter/click must not duplicate. Hide/reopen launcher and progress while
   running; verify correct run IDs and status. Test failure stopping later steps
   and cancellation before a pending step. No implicit retry or undo is expected.
8. **Sensitive actions:** with intentional owner consent, verify final-only lock/
   sleep prompts, Deny, expiry, cancellation and fresh explicit approval. Only the
   owner should perform actual lock/sleep/resume; automated tests must keep mocks.
   Verify executable/script file selections fail without executing.
9. **v0.1–v0.4 regression:** global shortcut, single instance, focus dismissal,
   monitor placement and drag reset; built-in Notepad/catalog Win32 and Store app
   launching/icons; `f ` Unicode/literal query, open/reveal, sensitive-file Cancel,
   Everything-unavailable Retry; audio read/set/adjust/mute and overlay (restore
   original audio state); allowlisted Settings and standalone sleep confirmation.
10. **Performance:** measure cold installed startup until input is usable and
    process-tree working set/private bytes after idle, Studio open/close and repeated
    workflow runs. Include WebView2 children; compare with 0.4 on the same machine
    and workload. Check for retained-window/worker/listener growth.
11. **Backup/restore:** stop NEXUS, copy the database and any sidecars as a set,
    restore into a disposable profile, reopen and confirm workflow identities/data.

## Final release procedure

Complete and record the manual checklist, resolve any failures and decide whether
the documented security/runtime limitations are acceptable. Review the original
staged v0.5 changes plus the unstaged Stage G changes and this report. Remove any
unwanted personal/test material from the intended commit; keep local build logs
and databases out. If code changes, rerun relevant checks and rebuild/re-hash both
installers. The owner decides commits, merge into master, release tag and publication.
Do not publish a candidate as fully verified while installed/offline/upgrade checks
remain unexecuted.
