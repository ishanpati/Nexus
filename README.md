# NEXUS v0.3

NEXUS is a local-first universal command bar for Windows 11. Search and launch
installed applications, or find files and folders through your local Everything
index. It runs independently of Codex and ChatGPT, with no API key, AI model,
subscription, cloud account, or application server.

## Install and run

- Supported release target: Windows 11 x64 (including Intel Core Ultra systems).
- Install `nexus_0.3.0_x64-setup.exe`, or the equivalent x64 MSI. Use one installer
  format. Close a running NEXUS with Alt+F4 before upgrading.
- Microsoft Edge WebView2 Runtime is required. The installer can download it if
  absent; offline installation requires WebView2 to be installed beforehand.
- For **file search only**, install the Everything desktop client and its separate
  ES command-line tool from [voidtools](https://www.voidtools.com/downloads/).
  Run Everything in your Windows session and let its index become ready. The
  service alone is insufficient. ES 1.1.0.38 was verified on this computer.
- Put `es.exe` on Windows PATH, or set the **user environment variable**
  `NEXUS_ES_PATH` to its full path, without quote characters in the value.
  On this computer: `C:\Users\ishan\AppData\Local\Microsoft\WindowsApps\es.exe`.
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
| Reveal in containing folder | Ctrl+Enter or the result's Show button |
| Quit | Alt+F4 while NEXUS is visible |

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
See [FILE_SEARCH_RANKING.md](FILE_SEARCH_RANKING.md) for limits and measured latency.

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

See [FILE_SEARCH.md](FILE_SEARCH.md) for semantics and diagnostics,
[ARCHITECTURE.md](ARCHITECTURE.md) for module boundaries,
[CHANGELOG.md](CHANGELOG.md) for release changes, and
[RELEASE_VERIFICATION.md](RELEASE_VERIFICATION.md) for actual verification results.

## Development and release build

Requires Node/npm, Rust, Windows MSVC build tools/SDK, WebView2, and the locked
project dependencies. Engineering rules are in [AGENTS.md](AGENTS.md).

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
