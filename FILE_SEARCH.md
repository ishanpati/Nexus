# Everything file search (v0.3)

`search_files` accepts only `{ query: string, limit: number }`. It returns an array
of `{ full_path, filename, extension, is_directory }`. Extension excludes the dot
and is empty for directories or extensionless files. Results describe Everything's
index, so offline or recently deleted items may still appear. Search does not
open results or scan drives. The command bar now exposes these results through
the `f ` file-search mode described below.

## Setup

Install/run the Everything desktop client and install its separate **ES** command
line tool. The service alone is insufficient. ES must support `-argv`, `-regex`,
`-efu`, and `-code-page`; ES **1.1.0.38** was verified on this machine. The app uses
the default Everything instance in the current Windows session. Named instances
are not currently configurable.

NEXUS checks `NEXUS_ES_PATH` first, then absolute folders on Windows `PATH`.
The configured value must be an absolute path to an existing `es.exe`. Invalid
configuration returns an error rather than silently selecting a different tool.
It does not search the current directory or scan the disk for executables.
Configuration is environment-based for now; no settings interface was added.

For this computer, start from the project root in PowerShell:

```powershell
$env:NEXUS_ES_PATH = 'C:\Users\ishan\AppData\Local\Microsoft\WindowsApps\es.exe'
cargo run --locked --offline --manifest-path src-tauri/Cargo.toml --example search_files -- 'AGENTS.md' 8
```

The example calls the same Rust service as the Tauri command and prints JSON.
It does not start another NEXUS window, register shortcuts, or launch files.
Set this environment variable before starting NEXUS (`npm.cmd run tauri dev`)
or configure your user environment/PATH and fully restart an installed NEXUS.
Reopening an already-running single instance does not update its environment.
Do not include literal quote characters in the environment variable's value.

The frontend call is:

```typescript
import { invoke } from '@tauri-apps/api/core';
const results = await invoke('search_files', { query: 'meeting notes', limit: 8 });
```

## Semantics and failures

- Searches are case-insensitive literal substrings of the full path. Leading and
  trailing whitespace is trimmed; internal spaces are literal. For example,
  `meeting notes` is one phrase, not two separate Everything search terms.
- Every character is encoded as a regex hex literal and passed as one `-regex`
  argument. Everything operators such as `ext:pdf`, `!`, `|`, and `*` are literal
  characters, not an advanced search language. Even `-exit` is search data.
- Rust invokes the resolved executable directly with separate arguments, a null
  stdin, piped output, and no console window. No command interpreter is involved.
- Input is limited to 1–1024 UTF-8 bytes without control characters; limits must
  be integers from 1 to 100. Invalid input is rejected, not silently truncated.
- ES is asked to wait up to three seconds for its database. A separate five-second
  wall-clock deadline kills and reaps a stuck ES child. Output is capped at 4 MiB
  and stderr at 16 KiB, with concurrent draining to prevent pipe deadlocks.
- A shared service permits one active search. Overlapping requests return `busy`
  rather than queuing an unbounded number of processes. Blocking work runs off
  Tauri's UI thread. A future search UI should debounce and discard stale results.
- EFU output is decoded strictly as UTF-8 and parsed as CSV with quoted commas and
  escaped quotes. Directory classification comes from indexed Windows attributes,
  so there is no per-result filesystem access. Invalid or oversized results fail
  the request instead of returning silently truncated or corrupted paths.
- A successful zero-match search returns `[]`. Errors are serializable objects
  with `code` and `message`: `invalid_input`, `es_not_found`,
  `everything_unavailable`, `timeout`, `process_failed`, `malformed_output`,
  `busy`, or `unsupported_platform`.
- Availability is checked through the actual IPC query, not a process-name guess.
  ES codes 7/8 indicate inaccessible Everything IPC; codes 4/6 indicate unsupported
  options. No indexing/service configuration is changed. An empty index is not
  proof that all drives are indexed; check Everything's indexing status separately.

## Verification checklist

1. Run the diagnostic above with part of a filename you can see in Everything.
   Confirm paths and filename/extension fields and that at most eight results appear.
2. Try a directory name, then a filename containing spaces, commas, or non-ASCII
   letters. Confirm `is_directory` and exact Unicode spelling.
3. Search a unique nonexistent string: expect `[]`. Search `-exit`: it should act
   as literal text and leave Everything running.
4. Optionally exit the Everything desktop client yourself and repeat. Expect
   `everything_unavailable` (or `timeout` if it stops responding), not `[]`.
   Restart Everything afterwards. No need to stop the Windows service.
5. Set a nonexistent `NEXUS_ES_PATH`: expect `es_not_found`. Restore it afterwards.
6. Pass limit `0` or `101`: expect `invalid_input`. Empty/control-character queries
   are also rejected. Use the automated tests for malformed-output and stuck-child
   cases rather than changing your ES installation.

```powershell
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --bins --example search_files
```

Native module: `src-tauri/src/file_search/`. No new dependencies, capabilities,
installer identity, or existing command handlers were changed for the backend.

Verified on 2026-11-14: **43 Rust tests passed**. One ignored test is a sleeping
child fixture explicitly invoked by the passing timeout test, not an untested
feature. The desktop executable and diagnostic built with locked, offline Cargo
dependencies. Live ES 1.1.0.38 queries found real `AGENTS.md` files and the local
fixture `src-tauri/target/nexus-search-fixture-资料 space/résumé 日本語, sample.txt`,
including the parent directory with the correct type. UTF-8 accents, Japanese,
Chinese, spaces, and commas were preserved. No matches returned `[]`; a literal
`-exit` search returned limited results without stopping Everything. The fixture
remains under ignored `target/` for repeatable checks.

These live checks used the Rust diagnostic outside the restricted development
sandbox, which cannot access the WindowsApps ES path. Everything-unavailable exit
codes, malformed output, and timeouts were tested automatically; the running
Everything instance was not stopped. Desktop IPC invocation, the v0.2 GUI regression
sequence, and a rebuilt installer were not manually tested for this backend change.

## Command-bar file mode

Type **`f `** (the letter f followed by a space), then a literal filename, folder
name, or path fragment. `F ` also works. `f` alone and ordinary queries such as
`firefox` stay in application/command mode. Removing the prefix immediately restores
the existing v0.2 results and launch behavior.

File mode uses a subtle teal search-field border and a **FILES & FOLDERS** label.
Each row shows the filename, parent directory, and a generic file or folder icon.
Literal matching text is highlighted as React text nodes, never interpreted as
HTML. Hover the parent label to see the full path. Enter or click opens the selected
item; Ctrl+Enter or its **Show** button reveals it in its containing folder.
File actions do not change application launch-frequency counts.

Search requests wait **150 ms** after typing. Only one backend request runs at a
time; while it runs, only the newest pending query is retained. Both late successes
and late failures are ignored after query/mode changes. Existing file rows stay
visible with an **Updating** message until the new response arrives, preventing
list clearing on every keystroke. They cannot be activated while updating. Errors
clear stale rows and show an explanation and Retry search button. Bare `f ` prompts
for a query without contacting Everything.

Each request retrieves at most 100 matches; the interface initially renders ten.
Scrolling near the bottom, Show more, or arrow navigation reveals further batches
without another ES request. Up/Down wraps through the available matches and keeps
selection visible. At 100 matches the count is marked as limited; narrow the query
to find more specific results. Window dimensions and existing Escape, shortcut,
drag, and focus handlers are unchanged.

Frontend modules: `src/fileSearch.ts` owns mode parsing and the testable request
controller; `src/hooks/useFileSearch.ts` connects it to Tauri; and
`src/components/FileResultContent.tsx` renders file rows in the existing list.

### UI verification checklist

1. Set `NEXUS_ES_PATH` as above, ensure Everything is running, then run
   `npm.cmd run tauri dev`. If NEXUS is already running, fully exit it first so
   it inherits the environment. No configuration setting is silently changed.
2. Type `f `: confirm the teal border, file-mode label, and query prompt.
3. Type `f AGENTS.md`: confirm filenames, parent folders, and highlighting.
   Try `f nexus-search-fixture-资料 space` to find the previously created Unicode
   fixture (if still present), including its folder with a different icon.
4. Search a common term with more than ten matches. Use Up/Down, scroll, or Show
   more to reach additional rows. The compact window should retain its dimensions.
5. Rapidly change the query; only matches for the newest query should replace the
   previous list. Switch to `open notepad` while a search is pending: late file
   results must not replace command results. The command should still launch.
6. Enter a nonexistent query and verify the no-match state. Optionally exit the
   Everything desktop client yourself, search again, and verify the unavailable
   message; restart it and use Retry search to recover.
7. Press Enter on a trusted text file: confirm its default application opens.
   Press Escape, then Ctrl+Shift+Space: the existing hide/show behavior should work.

Frontend checks use `npm.cmd test` and `npm.cmd run build`. Automated tests cover
mode detection, debounce timing, serialized/latest-only requests, stale success and
error suppression, leaving/re-entering file mode, invalid input, empty/error/retry
states, result limits, parent paths, and safe highlighting. They do not substitute
for visual desktop verification of the checklist above.

File-mode implementation verification on 2026-11-14: **36 frontend tests passed**,
TypeScript checking and Vite production build passed, and **43 Rust tests passed**
(plus the intentionally ignored subprocess fixture exercised by the timeout test).
This session's shell did not expose npm/node on PATH, so the existing Node
installation was used directly to run the same test runner, TypeScript, and Vite.
The file-mode desktop visuals and live keyboard interactions have not been manually
verified in this session. The backend's live Everything checks are recorded above.

References: [official ES CLI](https://www.voidtools.com/support/everything/command_line_interface/)
and [EFU format](https://www.voidtools.com/support/everything/file_lists/).

## Opening and revealing results

`src/fileActions.ts` sends a narrow `open_file_result` request containing the selected
absolute path, action (`open` or `reveal`), and explicit confirmation flag. Rust's
`src-tauri/src/file_actions.rs` validates the path and checks its current filesystem
type on every request, including after confirmation. Relative paths, URLs, device
namespace paths, alternate streams, and ambiguous path components are rejected.
No query text is passed to a command interpreter. Work runs off the UI thread.

Files use `ShellExecuteExW` with the Windows default `open` association; directories
use its `explore` verb. Revealing uses `SHOpenFolderAndSelectItems` to select the
item in its parent. Unavailable locations, access failures, and missing associations
produce errors rather than hiding the launcher. Successful actions hide NEXUS.

Executables, scripts, shortcuts, installers, macro-capable documents, extensionless
files, and unknown formats require the **Open this file** confirmation button.
Cancel receives initial focus, so a repeated Enter cannot approve the prompt.
Only a small list of common text/image/audio/video/PDF formats bypasses this prompt;
this policy is not content scanning and cannot guarantee a file or association is
safe. Both original and resolved paths are checked. Revealing never launches the
selected item and does not require execution confirmation. File changes between
validation and Windows opening cannot be completely prevented.

### Opening verification checklist

1. Start NEXUS with Everything and ES configured as above. Search `f ` plus the
   name of a known text file. Select it with Up/Down and press Enter. Its default
   application should open and NEXUS should hide.
2. Summon NEXUS again, search for a folder, and press Enter. Explorer should open
   that folder. Repeat with a Unicode filename containing spaces.
3. Select a file and press Ctrl+Enter, or click **Show**. Explorer should display
   its containing folder and select that item where Windows supports it.
4. Search for a known executable or script. Enter should show its full path and
   a confirmation prompt. Cancel must launch nothing; holding Enter must not
   approve it. Only approve a harmless executable you trust to check success.
5. Check Escape and Ctrl+Shift+Space, then launch `open notepad` and an installed
   application in normal search mode. Existing app behavior should be unchanged.
6. If an indexed result is already missing or inaccessible, try it: expect a clear
   error and a usable launcher. No real files need to be deleted for this test.

Opening implementation checks: **40 frontend tests passed**, **48 Rust tests
passed** (one ignored subprocess fixture is exercised by the timeout test), and
TypeScript/Vite production compilation and the offline Rust desktop build passed.
Tests cover selected-path IPC,
stale selections, explicit confirmation, path validation, missing items, and
failure/hide handling. Native association opening and Explorer selection have not
been manually verified in this development session. No dependencies or permissions
were added, and no deletion, rename, or move actions exist.
