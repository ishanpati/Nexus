# NEXUS 0.3.0 release verification

Date: 2026-11-14. Platform: this Windows 11 x64 development computer.
This record separates executed automated checks, live diagnostics, and pending
interactive/installation checks. Earlier historical checks are not counted as new
release verification.

## Executed checks

| Check | Actual result |
| --- | --- |
| `npm.cmd test` | 40 passed, 0 failed |
| `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets` | 48 passed, 0 failed; one ignored sleeping-child fixture is invoked by the passing timeout test; example targets have zero tests |
| `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --doc` | Passed; zero doctests |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | Passed |
| TypeScript and Vite production build (Tauri beforeBuildCommand) | Passed; 37 modules transformed |
| Rust optimized production compilation | Passed |
| `git diff --check` | Passed; Git reports only LF/CRLF normalization notices |
| Rapid typing / stale results | Automated debounce, generation, stale success/error, and mode-switch tests passed; live typing confirmation pending |
| File opening policy | Automated selected-path IPC, stale-selection, confirmation, missing-path, error/hide handling, and reserved-device-path tests passed |
| App launching / global shortcut | Existing automated regression tests passed; current-release interactive confirmation pending |
| Source review for cloud/AI dependency | No application network request/API key/AI integration found in frontend/backend source or manifests; no dependency additions |

The shell lacked Node/npm on PATH; tests/build used the already-installed Node
v26.13.0 directory prepended to the process PATH. Cargo dependencies were locked
and offline. Cargo inside the sandbox emitted a non-failing home canonicalization
warning. No global PATH or environment setting was changed.

## Live Everything checks

The read-only diagnostic uses the same Rust `FileSearch` service as `search_files`.
It ran outside the restricted sandbox because WindowsApps ES is inaccessible inside
it. `NEXUS_ES_PATH` was set only for the diagnostic process.

- Query `nexus-search-fixture-资料 space` returned the indexed fixture directory
  and `résumé 日本語, sample.txt` with correct paths and directory flags.
- Query `[release] & price+$ (资料)` returned exactly the newly created harmless
  `[release] & price+$ (资料).txt` fixture. Unicode, spaces, comma, brackets,
  ampersand, plus, dollar sign, and parentheses were exercised with real indexed files.
- A unique nonexistent query returned `[]` successfully.
- Literal `-exit` returned matching paths within the requested limit and did not
  terminate Everything.
- Direct ES queried the deliberately absent instance `nexus-release-test-absent-8d436a1e`
  and returned code 8, IPC not found. Production Rust error mapping and frontend
  messaging for that code passed automated tests. The default Everything client
  was not stopped; the unavailable state was not forced in the production GUI.

Fixtures remain under ignored `src-tauri/target/nexus-search-fixture-资料 space/`.
The diagnostic does not itself verify React-to-Tauri IPC or keyboard interaction.

## Packaging and standalone startup

Application version is 0.3.0 in npm, Cargo, and Tauri, preserving `com.nexus.launcher`.
The production build embeds local frontend assets. A copy of the optimized EXE
started from `src-tauri/target/release-smoke/` with its own working directory and
the configured ES path. A NEXUS process was observed; no Vite listener was found
on port 1420. This proves process startup without a development server, not every
GUI interaction. The process was left available for the product owner's checks.

The first packaging attempt produced the NSIS installer but MSI ICE validation
failed because the sandbox could not access the Windows Installer service. The full
packaging retry outside the sandbox **passed**, producing both bundles with normal
validation enabled:

- `src-tauri/target/release/bundle/nsis/nexus_0.3.0_x64-setup.exe` (1.51 MiB).
- `src-tauri/target/release/bundle/msi/nexus_0.3.0_x64_en-US.msi` (2.21 MiB).

Command: `npm.cmd run tauri build -- --bundles nsis,msi -- --locked --offline`.
The process also set `CARGO_NET_OFFLINE=true`; cached installer tooling was used.
The second optimized build completed in 2m 59s. SHA-256 hashes are recorded in
RELEASE_SHA256.txt. Validation was not disabled or bypassed.

Installers are unsigned. WebView2 is required; setup may download it if missing.
Offline installation therefore requires WebView2 already installed. Everything/ES
must be installed separately. Runtime code has no cloud or Codex dependency; a
network-disconnected machine and fresh Windows profile were not tested.

## Pending desktop / installation checks

The production executable was provided for these manual checks; they are not
marked passed until a result is observed or reported by the product owner:

1. Ctrl+Shift+Space toggles; Escape/focus loss dismisses; single-instance reopen,
   autofocus, dragging/reset and multi-monitor placement remain usable.
2. `open notepad` and a catalog application launch and hide the bar; packaged and
   shortcut application types should also be checked on the target installation.
3. `f nexus-search-fixture-资料 space` finds the fixture in the GUI. Enter opens
   its text file and folder. Ctrl+Enter/Show reveals the selected file in Explorer.
4. Rapid query changes leave only the latest matches. Confirmation Cancel and
   Escape launch nothing; holding Enter does not approve a sensitive-file prompt.
5. With Everything unavailable, the GUI gives a useful error and Retry recovers.
6. Run one installer, launch from its installed location, then test upgrade from
   the prior installed version. Fresh-install/upgrade/uninstall tests are pending;
   merely building a bundle does not verify installation.

Known limitations are documented in README.md and FILE_SEARCH.md. Release artifacts
may be built and shared for testing, but these pending checks prevent claiming a
fully verified release on all requested behaviors.
