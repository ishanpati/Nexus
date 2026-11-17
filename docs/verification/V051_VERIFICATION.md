# NEXUS v0.5.1 Stage B verification

**Release readiness: HOLD for desktop, installed/upgrade, and disconnected-runtime acceptance.**
Audit date: 2026-11-17. Branch: fix/foreground-launch. Source baseline: 260bd74 plus
the uncommitted Stage B changes listed by git diff; no publication commit or tag is
selected. The initial working tree was clean. No push, merge, tag, or publication
is performed. This is a candidate audit, not a public release acceptance record.

[Release notes](../releases/v0.5.1.md) ? [Implementation](../features/FOREGROUND_LAUNCHING.md).

## Inspection and fixes

Read AGENTS.md, README, changelog, release index/notes/checklist, architecture,
registry/workflow contracts, CI documentation, manifests/lockfiles, native launch,
launcher/shortcut/Studio lifecycle, and existing tests. No separate tracked
CONTRIBUTING file exists. Inspected recent history and preserved the stack,
installer identifier, workflow schema, dependencies, and CI checks.

Significant issues fixed within foreground-launch scope:

1. Direct commands captured input intent only after worker/COM/catalog work.
   Capture it at command admission so a switch or summon during that delay cannot
   renew activation authority. Workflow intent remains bound to the original run.
2. The six-second deadline was included in the suppression predicate. Distinguish
   expiry as timed_out before discovery, selection, and activation verification.
3. A shortcut's shared AUMID could select an old background profile; an argument
   shortcut could also associate an unrelated new window of the same host EXE.
   Reject old background windows for argument-bearing shortcuts; require the live
   returned launch process for new path-only argument matches. IDs override paths.
4. Workflow hiding depended on the launcher still being focused; if Shell had
   already focused the target, the always-on-top launcher could remain visible.
   Separate hide ownership from activation permission. Hide the old main window
   after focus moves, preserve a new summon or newer focused query, and never hide
   Workflow Studio through this callback. Successful launches still have one hide
   owner; actual failures retain input and errors.
5. A slow new instance could be confused with an older same-EXE instance. While
   the returned process remains live, defer old background windows from other
   processes. Allow a trustworthy new/foreground target; after a single-instance
   forwarder exits, normal exact-identity fallback remains available.
6. IsWindowVisible alone includes DWM-cloaked windows. Filter DWMWA_CLOAKED before
   selection. DWM query failure conservatively excludes an unverified candidate.

Native launching remains ShellExecuteExW on the authoritative .lnk or registered
AppsFolder PIDL, and ActivateApplication for packages. Process handles are retained
and closed with RAII; PID fallback checks the returned process is still active.
COM apartments, property variants, Shell allocations, and temporary query handles
are released. No title matching, interpreter dispatch, elevation, keyboard tricks,
persistent focus flags, new runtime service, or network client was introduced.

## Automated checks

Evidence is recorded from this audit's executions, not copied from Stage A.
Local toolchain: Node 26.13.0; Rust/Cargo 1.99.1 (matching the CI Rust pin).
Windows registry: DisplayVersion 25H2, build 26200.9974. The legacy ProductName value
still reports Windows 10 Home Single Language; WMI OS-caption lookup was denied.
No stronger edition claim is inferred from that stale value.

| Check executed | Outcome |
| --- | --- |
| npm ci --offline --no-audit --no-fund | Passed; 28 packages installed from cache |
| npm test (complete package script) | 106 passed; 0 failed/skipped |
| npm run build | TypeScript and Vite passed |
| cargo test --locked --offline --manifest-path src-tauri/Cargo.toml installed_apps --lib --quiet | 32 passed; 0 failed; 96 filtered |
| cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets --quiet | 126 passed; 0 failed; 2 existing ignored; 128 cases total |
| cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --doc --quiet | Passed; 0 doctests |
| cargo fmt --manifest-path src-tauri/Cargo.toml -- --check | Passed |
| cargo clippy --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets | Passed; existing 17 library / 18 library-test warnings retained |
| npm run tauri build -- --debug --no-bundle -- --locked --offline | Passed; embedded assets, no Vite server |
| npm run tauri build -- --bundles nsis,msi -- --locked --offline | Final approved build passed; optimized compilation 3m 41s; 2 bundles |
| Version/lock/config checks | Six 0.5.1 fields; unchanged dependency locks apart from root version; Tauri config unchanged apart from version |
| Markdown/diff checks | 127 local links/anchors checked; git diff --check passed; 3 artifact hashes rechecked |

CI-equivalent commands were run locally with the same Node major and exact Rust
pin; no remote GitHub Actions run was triggered or claimed. The repository has no
npm lint script. Existing CI workflow contents are unchanged.

Logs are retained in ignored src-tauri/target/v051-audit/. Tests use mocks for
foreground activation and sensitive Windows controls. Neither ignored live-audio
nor child-fixture tests are indiscriminately enabled. No automatic lock, sleep,
volume mutation, personal-workflow execution, or destructive file action is used.

## Production packaging

The initial offline optimized build compiled the Windows executable, then failed
MSI packaging with WiX LGHT0217/LGHT0216: the sandbox could not access the Windows
Installer service for ICE validation. It is not counted as installer success.
The same command is retried outside the sandbox with explicit execution approval,
normal WiX validation, and no altered CI/security checks:

    npm.cmd run tauri build -- --bundles nsis,msi -- --locked --offline

The final approved rebuild includes both launcher ownership and new-instance
selection fixes. Earlier successful bundles were replaced by this final artifact
set. Exact output evidence follows. No old 0.5.0 installer or Stage A debug executable is used as
v0.5.1 acceptance evidence.



| Final artifact (repository-relative) | Bytes | Signature | ProductVersion |
| --- | ---: | --- | --- |
| `src-tauri/target/release/bundle/nsis/nexus_0.5.1_x64-setup.exe` | 2485693 | NotSigned | 0.5.1 |
| `src-tauri/target/release/bundle/msi/nexus_0.5.1_x64_en-US.msi` | 3469312 | NotSigned | Not exposed by FileVersionInfo |
| `src-tauri/target/release/nexus.exe` | 6949376 | NotSigned | 0.5.1 |

SHA-256 manifest: [V051_SHA256.txt](../releases/V051_SHA256.txt). Hashes identify
these exact local candidate bytes, not a published or installed acceptance set.
The optimized executable and NSIS version resources report 0.5.1. MSI was built
from the matching WiX source with validation enabled; no MSI Property-table read
is claimed. Normal Tauri NSIS/MSI bundle markers may differ between executable
payloads. No signing, installer-script edit, or validation bypass occurred.

## Desktop and installed verification

The previous Stage A startup/capture check is historical and not treated as a
v0.5.1 launch/foreground pass. Each new native result must be observed explicitly;
launch acceptance or a screenshot alone does not prove foreground activation.

A limited new live check launched the final-code debug executable with embedded
assets (not a Vite development server). The real catalog rendered; typing the
partial query character showed two Character Map app rows. Enter was pressed on
the selected row and a charmap.exe process was observed. This establishes only
that a launch was dispatched, not closed/running-app classification, clearing,
hiding, duplicate suppression, or successful foreground activation.

The sandbox's foreground probe returned HWND 0, so it was unusable as evidence.
An approved read-only probe outside the sandbox later observed VS Code as the
foreground process. The delay and intervening user activity prevent attributing
that sample as a deterministic activation failure or success. Computer Use then
reported that the user stopped it with physical Escape; all further desktop
input stopped. No native foreground pass is claimed.

Before/after read-only digests confirmed existing workflow records were unchanged.
No personal workflow was run or edited, and no audio/session/power action was
performed. No NEXUS or Character Map process remained in the final process inventory.

Both installers were built, hashed and signature-checked, but neither was installed
or exercised after the user stopped desktop automation. Installed startup, actual
installer registration/shortcuts, upgrade and uninstall remain pending. No MSI
extraction or portable run is represented as a completed installation.

| Acceptance scenario | Evidence/status |
| --- | --- |
| Closed traditional Win32 target | Partial-query Enter dispatched Character Map; foreground and prior-state classification unverified |
| Already running / single-instance / minimized | Mock identity/selection regressions pass; native check pending |
| Slow startup / intermediate process / multiple windows | Bounded policy and identity mocks pass; native check pending |
| Packaged / Store application | Existing activation path preserved and compiled; real launch/foreground pending |
| Windowless application | Accepted outcomes and timeout mocks pass; native check pending |
| Saved single/multi-app workflow and cancellation | Existing/added fake engine regressions pass; installed native run pending |
| Repeated Enter / selected partial rows / success clear / error retention | Frontend and native guards tested automatically; full GUI acceptance pending |
| Global shortcut / Escape / focus / Studio lifecycle | Existing regressions pass; physical Escape stopped automation, which is not an app Escape/global-shortcut pass |
| File/folder search and Windows controls | Full existing automated suites pass; implementations unchanged; native regression pending |
| Internet disconnected / installed startup / upgrade | Pending; offline builds and mocks do not establish disconnected desktop use |

The automated suite covers identity precedence, argument/profile rejection,
missing-handle fallback, multiple-window ambiguity, closed/slow/windowless mock
outcomes, bounded timeout, cancellation and stale intent, hide ownership after
Shell focus, duplicate-admission guards, partial/fuzzy selected-row dispatch,
query success/failure policies, sequential workflow outcomes/cancellation, and
Studio lifecycle. File/folder search and Windows controls run their existing fake
regressions; their feature implementations are unchanged.

## Known limitations and remaining risks

Windows can deny foreground requests. Missing or ambiguous identity, profiles,
intermediate launchers, custom/owned or shared-host windows and protected processes
can remain unverified. Argument-bearing shortcuts may deliberately skip activation
rather than target an existing window whose launch arguments cannot be established.
New user input (including movement) conservatively suppresses extra activation.
Shell or the target app can independently activate later, beyond NEXUS control.

Discovery uses at most 100 polls with 50 ms waits and a six-second session budget,
with up to 200 ms foreground verification for one selected target. An OS/COM call
itself cannot be forcibly interrupted; the deadline is checked between operations.
No visible window is not a launch error; acceptance, detection, activation and
readiness are separate. Readiness is always unverified. Cancellation stops future
steps and activation attempts, without undoing admitted native actions.

Installed/upgrade/uninstall, truly disconnected desktop use, multi-monitor/DPI,
and startup/memory acceptance require an appropriate disposable profile and actual
observations. CSP remains disabled, workflow data is plaintext, and packages are
unsigned unless final-byte evidence says otherwise. Existing dependency and hardware
requirements remain, including optional local Everything/ES for file search.

## Release metadata

All six root application version values change from 0.5.0 to 0.5.1: package.json,
package-lock.json top/root-package, src-tauri/Cargo.toml, Cargo.lock's nexus package,
and tauri.conf.json. Transitive dependency versions/resolutions are unchanged.
Only the existing windows crate's DWM API feature is additionally enabled.
Identifier com.nexus.launcher, SQLite user_version 1 and workflow document format 1
remain unchanged. Updated changelog, versioned notes/index, current README/guide,
and relevant architecture/foreground documentation. Historical notes, checklist,
checksums, and existing tags are preserved.
