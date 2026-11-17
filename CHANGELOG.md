# Changelog

Changes are grouped by implemented version or documented development milestone.
Dates identify commits and verification records, not public publication dates.
Tags, built installers, and passing automated checks do not establish desktop
acceptance. See the [release index](docs/releases/README.md) for status and setup.

## Unreleased

- v0.5.2 Stage A: registry-driven command, search-mode, action, interaction and
  shortcut discovery metadata with a typed read-only service, authoritative Rust
  action contracts, and explicit dependency/shortcut availability. Help is reserved
  as unavailable metadata; the Help Center UI is not included.

## 0.5.1 ? 2026-11-17 ? Release candidate

[Release notes](docs/releases/v0.5.1.md) ? [Verification](docs/verification/V051_VERIFICATION.md)

### Fixed

- Installed-app launches share bounded native foreground discovery, minimized-window
  restoration, verified activation, and taskbar flashing when Windows denies focus.
- Preserve shortcut arguments/working directories and packaged activation. Match
  trusted native identities, exclude DWM-cloaked windows, and reject ambiguous
  background profiles or unrelated new windows sharing an argument-driven host.
- Capture activation intent at action admission, before worker/COM/catalog delay;
  preserve run-wide workflow intent and suppress stale or cancelled activation.
- Hide the owned launcher after accepted launches even when Shell already moved
  focus. Preserve a newly summoned launcher or a newer query. Avoid a second hide.
- Report activation deadline expiry as timeout, independently of launch acceptance.
- App/workflow Enter dispatch retains the selected row until key release and
  consumes it once; interruption and repeated key-down do not duplicate execution.

### Known limitations

- Windows can deny foreground activation. Missing/ambiguous identities, hosted or
  unusual windows, windowless apps and slow startup can leave activation unverified.
- Foreground verification is point-in-time; readiness is not verified. Cancellation
  cannot recall an admitted native launch or stop an app's own future focus changes.
- Desktop, installed/upgrade and disconnected-runtime acceptance are reported
  separately in the verification record; passing tests/builds do not establish them.
- Installer identity, workflow data/schema, dependency versions and runtime local-first
  requirements are unchanged. Packages are prepared without tags or publication.

### Added

- Windows GitHub Actions CI for frontend tests, TypeScript/Vite, Rust formatting,
  Clippy, Rust tests, and a Tauri build with embedded assets.
- Versioned release notes, a v0.5.0 GitHub Release draft, and an acceptance checklist.

### Changed

- Organized technical and historical documentation under `docs/`, with a user guide
  and documentation index.
- Removed redundant WiX tooling, an instruction backup, and an empty local
  placeholder from tracking; preserved their local copies. Removed unused starter
  SVGs and expanded ignore rules for build output and private local files.

## 0.5.0 — 2026-11-15 · Release candidate

[Release notes](docs/releases/v0.5.0.md) · [Verification](docs/verification/V05_VERIFICATION.md)

### Added

- Workflow Studio: create named workflows with descriptions and aliases; choose
  applications, documents, folders, audio controls, Settings pages, lock, or sleep.
- Edit, reorder, rename, filter, and delete saved workflows. Save / Discard / Cancel
  protects unsaved work when navigating, closing Studio, or quitting the launcher.
- Local SQLite storage with stable workflow/step identities, revision conflicts,
  and availability details for missing or obsolete references.
- `w ` search and launching by saved name or alias, using the selected workflow's ID.
- Sequential execution with whole-plan validation, stop-on-failure behavior,
  duplicate-start protection, progress, step results, and explicit cancellation.
- A Rust Action Registry with versioned action contracts and generic editor controls.

### Changed

- Closing progress or hiding the launcher leaves active runs available through
  Workflow runs. Accepted starts clear input; rejected starts preserve it.
- Package metadata is 0.5.0. Installer identity, workflow schema version 1, and the
  existing local data location are retained. Added backup/shared-data guidance.

### Fixed

- Launcher close no longer destroys the window needed by the global shortcut
  while auxiliary windows survive. Active runs hide the launcher; idle quit resolves
  Studio drafts before exit.
- Workflow save checks and availability annotations now reject sensitive file
  targets consistently with execution, leaving older steps visible for repair.

### Security

- Workflow lock and sleep must be final and require fresh, expiring, single-use
  approval bound to the run, step, and action. Saving never grants approval.
- Workflow file actions reject executable, script, shortcut, installer, macro-capable,
  and unknown types. Compiled actions validate parameters and current references
  in Rust; no shell/script action or arbitrary executable arguments are provided.

### Known limitations

- Installed-app, offline, upgrade, and full desktop acceptance remain pending.
  The existing `v0.5.0` tag is not an acceptance or publication record.
- No rollback, persisted run history, crash resume, delays, parallel steps, or app
  readiness waits. Cancellation cannot undo an admitted Windows request.
- Workflows are plaintext local data. No sync, encryption, export/import UI, or
  automatic backup. Dev and installed builds share the default data directory.
- Installers are unsigned; CSP remains disabled. Windows controls and filesystem
  activation retain the documented hardware, policy, and association limitations.

Original Stage G and C–F entries are preserved in
[v0.5 development history](docs/releases/v0.5-development-history.md), including
stage-specific scope exclusions and links to detailed engineering reports.

## 0.4.0 — 2026-11-15 · Historical release candidate

[Release notes](docs/releases/v0.4.0.md) · [Verification](docs/verification/V04_VERIFICATION.md)

### Added

- Absolute and relative volume, mute/unmute, audio status, and a brief NEXUS audio overlay.
- Sound, Wi-Fi, Bluetooth, and Display Settings commands, workstation lock, and
  separately confirmed sleep.
- Typed system-command matching, aliases, usage guidance, and last-read audio feedback.

### Changed

- Successful commands clear input and hide the launcher; failures retain the query.
  Escape and summon reset input. Lock and confirmed sleep restore input on failure.
- Package metadata is 0.4.0; existing app/file search, native launching, icons,
  shortcut, monitor placement, installer identity, and dependency versions are preserved.

### Fixed

- Partial fixed-command selections execute the selected action consistently for
  clicks and Enter; numeric commands retain validated arguments.

### Security

- Settings destinations are allowlisted. Sleep requires an expiring, single-use
  Rust confirmation ticket; no general executable, URI, or shell input is accepted.

### Known limitations

- Windows acceptance does not prove arrival at a Settings page or a completed
  lock/sleep transition. Hardware, device, and policy scenarios require manual checks.
- Installers are unsigned. Installation/upgrade and actual lock/sleep transitions
  were not verified by the release report. The audio overlay is NEXUS UI.

## 0.3.1 — 2026-11-14 · Development milestone

[Release notes](docs/releases/v0.3.1.md) · [Ranking evidence](docs/features/file-search/FILE_SEARCH_RANKING.md)

### Added

- Local history of successful file opens and optional configured project roots
  as small ranking signals within an existing match tier.

### Changed

- File search retrieves independent folder/file pools and ranks exact names/stems,
  prefixes, word boundaries, substrings, then path-only matches before limiting results.
- Windows Known Folders, including OneDrive redirection, favor nearby user files;
  cache/build paths receive penalties without being excluded.

### Fixed

- Exact project folders and directly stored Desktop files are no longer crowded
  out by descendant paths or cache copies in the reproduced diagnostic cases.

### Known limitations

- The `v0.3.1` tag retains 0.3.0 package metadata. No 0.3.1 installer was produced
  by the ranking task; no dependency or package-version bump was made.
- Finite candidate pools can omit matches. No file fuzzy search, guaranteed
  retrieval of every copy, fresh interactive regression, or new installer verification.

## 0.3.0 — 2026-11-14 · Historical release candidate

[Release notes](docs/releases/v0.3.0.md) · [Verification](docs/verification/RELEASE_VERIFICATION.md)

### Added

- Everything/ES-backed literal file and folder search through exclusive `f ` mode.
- Filename/path highlighting, 150 ms debounce, stale-response protection, batches
  of ten, a 100-result limit, availability errors, and retry.
- Default-association file opening, Explorer folder opening, and Ctrl+Enter/Show
  to reveal a selected result. File actions keep app launch-frequency data separate.

### Changed

- npm, Cargo, and Tauri metadata are aligned to 0.3.0, preserving installer identity
  and dependency versions. README instructions describe actual launch behavior.

### Fixed

- Escape cancels pending file confirmation while hiding NEXUS.
- The file-opening boundary rejects Win32 reserved device names such as `NUL.txt`.

### Security

- Executable, script, shortcut, installer, and unknown file types require explicit
  confirmation. Rust revalidates selected paths and current file types.
- Literal queries run through a trusted ES executable with explicit arguments,
  bounded output, and process timeouts; query text is not shell input.

### Known limitations

- Requires separate Everything desktop and ES installations for file search only.
  Indexed names/paths are searched, not contents or advanced Everything syntax.
- No file mutations, signing, tray icon, or automatic updates. Desktop interaction,
  disconnected operation, and install/upgrade/uninstall acceptance were pending.

## 0.2.0 — 2026-11-10 · Development milestone

[Release notes](docs/releases/v0.2.0.md)

### Added

- Installed-app discovery from Start Menu and Windows AppsFolder registrations,
  application search/launching, cached native icons, and explicit catalog refresh.
- Local matching/ranking and successful-launch frequency as a small tie-breaker.

### Known limitations

- The `v0.2.0` tag still reports package version 0.1.0. It is a development
  milestone, not evidence of a versioned 0.2.0 installer or public release.
- Portable apps outside Windows catalogs and unsupported registrations may not appear.

## 0.1.0 — 2026-11-10 · Initial milestone

[Release notes](docs/releases/v0.1.0.md)

### Added

- Floating command bar with local command filtering, keyboard/click selection,
  native Open Notepad, Ctrl+Shift+Space, and single-instance handling.
- Escape/focus dismissal, autofocus, temporary dragging, and active-monitor placement.

### Known limitations

- No installed-app discovery, indexed file search, Windows control commands, or
  workflows yet. Multi-monitor/scaling, installer, and offline acceptance were incomplete.
