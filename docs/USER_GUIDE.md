# NEXUS user guide

Detailed controls, setup, workflows, backup, troubleshooting, and development instructions for the v0.5.1 release candidate. For the product overview, return to the [README](../README.md). Release acceptance remains pending; see [v0.5 verification](verification/V051_VERIFICATION.md).

## Install and run

- Supported release target: Windows 11 x64 (including Intel Core Ultra systems).
- Install `nexus_0.5.1_x64-setup.exe`, or the equivalent x64 MSI. Use one installer
  format. Close a running NEXUS with Alt+F4 before upgrading.
- Microsoft Edge WebView2 Runtime is required. The installer can download it if
  absent; offline installation requires WebView2 to be installed beforehand.
- For **file search only**, install the Everything desktop client and its separate
  ES command-line tool from [voidtools](https://www.voidtools.com/downloads/).
  Run Everything in your Windows session and let its index become ready. The
  service alone is insufficient. ES 1.1.0.38 was verified on this computer.
- Put `es.exe` on Windows PATH, or set the **user environment variable**
  `NEXUS_ES_PATH` to its full path, without quote characters in the value.
  For example, ES may be installed under `%LOCALAPPDATA%\Microsoft\WindowsApps`.
  Fully exit and restart NEXUS after an environment change; summoning an existing
  process does not reload its environment.

Everything and ES are not bundled. Application search and launching work without
them. End users do not need Node, npm, Rust, Vite, Codex, or this source checkout.
After prerequisites are installed, NEXUS features run locally without internet.
The installers are unsigned; Windows may show an unknown-publisher warning.

## Controls

| Action | Control |
| --- | --- |
| Show / hide | Ctrl+Shift+Space |
| Dismiss | Escape or switch to another application |
| Move temporarily | Drag the top header |
| Select result | Up / Down; scroll or Show more for additional results |
| Open selected item | Enter or click |
| Search files and folders | Type `f ` followed by a filename or path fragment |
| Search saved workflows | Type `w `; an empty query lists all saved workflows |
| Create / manage workflows | Type `create workflow` / `edit workflow` |
| View workflow status | Select Workflow runs; closing progress leaves execution active |
| Reveal in containing folder | Ctrl+Enter or the result's Show button |
| Quit | Alt+F4 in the launcher; resolves Studio drafts first. During an active run this hides the launcher instead |

Each summon restores the upper-center position on the active monitor, with a
primary-monitor fallback, and focuses search. NEXUS stays running while hidden,
has no taskbar/tray entry, and uses a single instance. Launching its executable
again summons that instance. If another app owns the shortcut, a visible warning
explains this fallback; release the conflicting shortcut and restart NEXUS.

## Application search (v0.2 preserved)

Ordinary queries search a cached catalog of current-user/all-users Start Menu
shortcuts and Windows AppsFolder entries, including registered Win32 and Store
apps. Matching ranks exact names/aliases, name prefixes, word prefixes, substrings,
then conservative typos. Successful launch counts are stored locally and provide
a small tie-breaking bonus. Eight results are initially rendered; more are
available by scrolling or keyboard navigation.

Enter launches the selected cached application ID through native Windows APIs,
preserving shortcut arguments and working directories. Real application icons are
cached with a generic fallback. Refresh apps explicitly rebuilds the catalog.
The built-in `open notepad`, `notepad`, and `launch notepad` aliases still work.
Portable apps outside Windows' catalogs and unsupported/virtual shortcuts may not
appear; conservative deduplication can leave visually similar entries.

## File search and opening (v0.3)

Type `f ` followed by a **literal** query. For example, `f meeting notes` searches
indexed names first, with path-only matches as a lower-priority fallback. Queries
containing a path separator explicitly search paths. File mode has a teal input border,
filename/parent-folder labels, generic file/folder icons, and matching highlights.
Normal application search resumes when the prefix is removed.

Search waits 150 ms after typing, keeps only the newest pending query, and ignores
stale successes and errors. Previous matches remain visible while updating but
cannot be opened until current results arrive. The initial batch is ten; up to
100 matches can be browsed. Narrow the query to find more specific results.

The v0.3.1 working branch retrieves independent folder/file candidate pools before
ranking: exact name/stem, prefix, word-boundary, substring, then path-only matches.
Location and previous opens only break ties within a match tier. Windows Known
Folders supply Desktop/Documents/Downloads locations, including OneDrive redirection.
Optional `NEXUS_PROJECT_DIRS` contains semicolon-separated absolute project roots
(up to 32); set it before starting NEXUS. No file fuzzy search is enabled.
See [FILE_SEARCH_RANKING.md](features/file-search/FILE_SEARCH_RANKING.md) for limits and measured latency.

Files open with their Windows default association; folders open in Explorer.
Executables, scripts, shortcuts, installers, macro-capable and unknown formats
require explicit confirmation of the displayed full path. Cancel is selected by
default. Revealing an item in Explorer does not execute it. Rust checks existence
again before each action. Errors leave the launcher usable; successful opens hide
it. No deletion, renaming, or moving is implemented.

Limitations:

- Only Everything-indexed locations are searched. Results may be stale, offline,
  or inaccessible; opening checks the current filesystem.
- Everything operators (`ext:pdf`, wildcards, and so on) are literal text, not
  advanced expressions. Input is limited to 1024 UTF-8 bytes without controls.
- Named Everything instances are not configurable. Missing ES, unavailable
  Everything, timeouts, and empty results have distinct states; use Retry search.
- Opening depends on Windows associations and permissions. Unusual/long paths or
  shell extensions may fail. Sensitive-file confirmation is not malware scanning.
- No installer signing, tray icon, automatic startup, or auto-update is included.

See [FILE_SEARCH.md](features/file-search/FILE_SEARCH.md) for semantics and diagnostics,
[ARCHITECTURE.md](architecture/ARCHITECTURE.md) for module boundaries,
[CHANGELOG.md](../CHANGELOG.md) for release changes, and
[RELEASE_VERIFICATION.md](verification/RELEASE_VERIFICATION.md) for actual verification results.

## Creating and managing workflows

1. Type `create workflow` and activate the selected result. Workflow Studio opens
   in its own resizable window; the launcher hides after the open succeeds.
2. Enter a unique name, optional description and aliases (one per line). Choose
   Add Action, search the action catalog, then configure each action. Application
   steps select an installed-app reference; file/folder steps use Windows pickers.
3. Use Edit, Remove, Move Up and Move Down to configure the sequence. Choose
   **Save Workflow** to commit locally. No action runs while editing or saving.
4. Type `edit workflow` to see saved workflows. Open a row to edit it; rename and
   save preserve its identity. Delete shows the exact workflow ID/revision and
   requires a separate confirmation. Cancel leaves stored data intact.
5. In the launcher type `w ` to list workflows, or `w part of name` / `w alias` to
   filter. Enter or click runs the selected saved workflow, regardless of raw text.
   **Workflow runs** shows progress, failures, cancellation and explicit approvals.

Save / Discard / Cancel protects unsaved drafts on navigation, Studio close and
launcher quit. Closing Studio keeps NEXUS alive. Stale saves/deletes fail with a
revision conflict; keep the draft, cancel navigation, and reload deliberately.
Missing actions/apps/paths stay visible for repair or removal. A missing action's
unchanged data can be retained during other edits; execution still refuses it.

Runs use a saved snapshot, execute in sequence and stop on the first failure.
Duplicate runs of the same workflow are refused. Hiding/closing presentation
does not cancel an active run. Cancellation stops future steps; admitted Windows
requests cannot be recalled, and completed steps are not undone. Run history and
approvals last only for the current process; interrupted runs do not resume.

Workflows support the nine built-in [Action Registry](architecture/ACTION_REGISTRY.md) actions:
catalog app launch, document/folder opening, volume/relative volume/mute, four
allowlisted Settings pages, lock and sleep. Lock/sleep must be last and require
fresh explicit approval bound to that run and step. Workflow file steps reject
executables, scripts, shortcuts, installers, macro-capable and unknown formats.
There is no arbitrary executable/argument, shell, script, recursive workflow,
delay, parallel-step or readiness-wait action. App success means Windows accepted
the launch, not that loading finished. No AI or paid runtime service is involved.

Rust owns stable action IDs, parameter schemas, validation and confirmation policy.
New compiled actions using existing parameter types need no editor rewrite.
Unsupported action versions and future parameter types fail safely and retain
saved data; automatic action migrations are not provided. See
[storage](features/workflows/WORKFLOW_STORAGE.md), [execution](features/workflows/WORKFLOW_EXECUTION.md) and
[Studio](features/workflows/WORKFLOW_STUDIO.md) for details.

## Workflow data and backup

Workflows are stored in `%LOCALAPPDATA%\com.nexus.launcher\workflows.sqlite3`.
Development and installed builds with the default identifier share this location;
running `tauri dev` does not isolate your real data. Automated storage tests use
temporary databases. The database includes selected local paths/application IDs
in plaintext and inherits your Windows account permissions. It is not encrypted
or synchronized.

To back up, finish/cancel runs, close Studio, then fully quit NEXUS with Alt+F4
in the launcher and confirm the process exited. Copy `workflows.sqlite3` to a
safe location. If SQLite sidecar files remain, keep the entire data directory
together; never copy only a live database. To restore, quit NEXUS, back up the
current directory, and restore the saved set. Retain a copy before upgrades or
uninstalling. Moving to another computer can require repairing app/path references.
Installer/upgrade preservation must be verified using the release checklist.
The NSIS uninstaller offers an app-data deletion checkbox. Leave it unchecked
to retain workflows; selecting it can erase the entire local NEXUS data directory.

## Windows controls (v0.4 preserved)

NEXUS recognizes these typed commands:

- `volume 35` / `vol 35`: set volume, whole numbers from 0 to 100.
- `volume +10`, `volume -10`, `volume up`, `volume down`: relative percentage-point
  changes; up/down mean +10/-10. Explicit deltas must be between -100 and +100.
- `mute`, `unmute`, `sound settings`, `wifi settings`, `bluetooth settings`,
  `display settings`, `lock`, and `sleep`.
- `volume status` / `audio status` / `mute status`: read volume and mute state.

Names and whitespace are normalized. Decimal fractions, exponent/hex syntax,
NaN/Infinity, trailing arguments and shell-like suffixes are rejected. Partial
commands such as `volume` or `sound` show usage guidance. Aliases include
`audio settings`, `wi-fi settings`, `bt settings`, `screen settings`, and `lock screen`.

Audio commands have an **Audio** badge. Enter/click executes through Windows Core
Audio. Successful actions clear the query and hide NEXUS; failures keep the query
and show an error. Escape clears and hides, and summoning starts with an empty,
focused input. Partial fixed-command suggestions (for example `disp` and `blu`)
execute their selected actions; incomplete numeric commands remain help-only.
Audio suggestions show the most recently read volume/mute state, refreshed when
NEXUS opens. The footer retains the last successful action after reopening without
restoring its query. External volume changes while NEXUS stays open are not polled;
use `audio status` for an explicit fresh reading.
Successful volume/mute changes also show a small NEXUS overlay at the bottom of
the launcher's monitor for about two seconds. It displays the resulting percentage
and mute state without stealing focus. This is NEXUS feedback, not Windows' native
media-key flyout: no documented Core Audio flyout API was found, and NEXUS does not
simulate media keys or depend on private Shell interfaces.
Typing and suggestions never change audio. Relative changes clamp to 0–100;
changing volume preserves mute, and mute/unmute preserves volume. The current
default multimedia output is resolved for every operation. No administrator,
shell, key simulation, or external executable is required.

Settings and lock have a **System** badge and execute only on Enter/click.
Settings destinations are allowlisted; lock uses the Windows workstation API.
Sleep has a **Confirm** badge and opens a modal with Cancel focused first. Only
**Sleep now** sends the single-use confirmation required by Rust. Successful
requests hide NEXUS; failures display an error. No administrator prompt is requested.
See [SYSTEM_ACTIONS.md](features/SYSTEM_ACTIONS.md) for restrictions and safe manual checks.
Complete system commands win equal-name app collisions; exact apps beat partial
system hints. Apps remain selectable below complete commands. File mode is unchanged.

Workflow mode uses the same ranking tiers for names and aliases, ignoring case.
Only a leading `w` followed by a literal space activates it; bare `w` remains an
ordinary command/app query. `f ` stays exclusive to files. Workflow names and aliases
never enter ordinary command/app search, so names such as `lock` or `create workflow`
are unambiguous: use `w lock` to run the saved workflow. Enter and clicks execute
the selected saved UUID, never the query text. Searching never executes actions.

A successful workflow start clears input and opens progress; a rejected start keeps
the query and displays an error. Workflow runs shows current/completed steps,
failures, approval details and session run IDs. Cancel workflow stops between steps;
an admitted Windows request cannot be recalled and earlier actions are not undone.
Closing progress, Escape and hiding/reopening the launcher leave runs active.
Reopening gives an empty focused input; use Workflow runs to recover status. App
steps report Windows launch acceptance, never verified application readiness.

Manual checks: first run `audio status` and note the original state. Type `volume 35`
without Enter and confirm nothing changes. Press Enter and check the reported state
against Windows sound settings. Try `volume up`, `volume down`, `mute`, `unmute`,
and `volume NaN`. Restore your original state afterwards. Switch the default output
device and run `audio status` again. Check `open notepad`, `f nexus`, Escape and the
global shortcut through their existing modes. See [AUDIO_CONTROLS.md](features/AUDIO_CONTROLS.md)
for the implementation, actual test results and remaining manual checks.

## Development and release build

Requires Node 22.15+ (or 23.5+, for the test runner's module hooks; Node 26.13.0
was used for this audit), npm, Rust, Windows MSVC build tools/SDK, WebView2, and the locked
project dependencies. Engineering rules are in [AGENTS.md](../AGENTS.md).
No new installed runtime dependency is required for Windows controls. Audio needs
an enabled default output and the Windows Audio service. Settings pages and sleep
availability depend on hardware and Windows policy. Lock/sleep never request UAC.

```powershell
npm.cmd ci
npm.cmd run tauri dev
npm.cmd test
npm.cmd run build
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --doc
npm.cmd run tauri build -- --bundles nsis,msi -- --locked --offline
```

Offline builds require cached npm/Cargo dependencies and installer tooling.
Initial dependency/tool downloads need internet; they are not runtime services.
NSIS output is under `src-tauri/target/release/bundle/nsis/`; MSI output is under
`src-tauri/target/release/bundle/msi/`. Production assets are embedded in the native
application; Vite and its localhost server are not used by the release.

For a regression check, launch the release executable, use the shortcut, launch
Notepad and a catalog app, search a known Unicode filename, open a text file and
folder, and reveal a result. Verify confirmation cancellation, rapid query changes,
and recovery after Everything becomes unavailable. The release verification record
distinguishes executed checks from pending checks.
