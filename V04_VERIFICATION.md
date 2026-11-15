# NEXUS 0.4.0 regression verification

Checks performed on Windows 11 during release preparation, 2026-11-15.

## Automated checks

- Frontend: `npm test` — 67 passed, zero failed.
- `npm run build` — TypeScript and production Vite assets passed.
- Rust: `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`
  — 74 passed, zero failed, two ignored (live audio opt-in and child timeout fixture).
- Separately executed the opt-in `audio::native::tests::live_audio_round_trip`:
  passed against the actual output device. Read, mute, absolute 35%, +10 to 45%,
  -10 to 35%, unmute at zero, and restoration were checked. Original and restored
  state: 50%, unmuted. No lock or sleep was executed by tests.
- Source review plus validation tests: fixed Settings allowlist, native
  `LockWorkStation`, `SetSuspendState` behind expiring single-use confirmation,
  numeric bounds, non-finite/malformed input rejection, typed selected-result
  dispatch, failure retention, repeat guards, file sensitivity and stale results.
- No-device, endpoint-change and policy errors are mocked/mapped tests, not tests
  that disable hardware or change Windows policy.
- Dependency/source review found no runtime AI/cloud client or API key requirement.
  Parser requests cannot supply a shell command, executable, URI or arbitrary
  arguments. Existing file/app launches intentionally use their native Shell paths.
- Rust doctests completed successfully (zero doctests); formatting and diff checks
  passed. Cargo printed a sandbox user-profile canonicalization warning, but the
  tests completed successfully.

## Production packaging

`npm run tauri build -- --bundles nsis,msi -- --locked --offline` built the optimized
native executable and both Windows installer formats. The initial sandbox run
could not access Windows Installer during WiX ICE validation; the outside-sandbox
retry passed. No validation checks were disabled. Dependency versions were retained.

Artifacts are under `src-tauri/target/release/bundle/`:

- `nsis/nexus_0.4.0_x64-setup.exe`
- `msi/nexus_0.4.0_x64_en-US.msi`

## Live read-only diagnostics

- App catalog: 194 apps, zero diagnostics; cache equality and explicit refresh
  verified. Sandbox-only discovery initially reported 69 apps and an AppsFolder
  error; repeating outside isolation resolved it without code changes.
- Everything uses the existing configured ES executable in WindowsApps. Sandbox
  isolation initially hid that path; outside it the real search passed.
- `nexus`: exact project folder first (843 ms in the measured diagnostic).
- `finance_tracker`: OneDrive Desktop `Finance_Tracker.exe` first (592 ms).
- `nexus-search-fixture-资料 space`: existing Unicode/space fixture folder and
  descendant paths found (1817 ms).
- `[release] & price+$ (资料)`: matching literal punctuation/Unicode paths found
  (1590 ms). Query text did not become an Everything operator or shell command.
- `nexus-no-match-8d436a1e`: successful empty results (1818 ms).
  Times include diagnostic process startup; these are single samples, not a benchmark.

## Runtime requirements and limits

Windows 11 x64, WebView2, and an enabled output/Windows Audio service for audio.
Everything desktop plus ES remain optional dependencies only for file search.
No new service, paid software, cloud key or administrator elevation is required.
Offline setup needs WebView2 already installed. Installers are unsigned.

The audio overlay is implemented by NEXUS; Core Audio does not provide a documented
native media-key flyout entry point. No media keys are synthesized. Sleep/hardware
policy and Settings page availability remain controlled by Windows. API acceptance
does not prove a completed sleep/lock transition. No installation/upgrade was
performed automatically, and actual lock/sleep transitions require user testing.
