# Release notes and publication status

NEXUS v0.5.1 is a **release candidate with desktop acceptance pending**. See the
[current audit](../verification/V051_VERIFICATION.md) and [notes](v0.5.1.md). A historical review
of the [GitHub Releases page](https://github.com/ishanpati/Nexus/releases) on
2026-11-16 found no published releases or downloadable release assets.
Local installers and existing tags do not establish a public, accepted release.

## Version history

These dates identify source commits and verification records, not publication
dates. The notes reconstruct implemented changes from those sources. Package
metadata was not retroactively corrected to match historical milestone labels.

| Notes | Code date | Source commit | Package version at that point | Status |
| --- | --- | --- | --- | --- |
| [v0.5.1](v0.5.1.md) | 2026-11-17 | `260bd74` + Stage B working tree | 0.5.1 | Current candidate; publication on hold |
| [v0.5.0](v0.5.0.md) | 2026-11-15 | `797b7af` | 0.5.0 | Previous candidate; acceptance pending |
| [v0.4.0](v0.4.0.md) | 2026-11-15 | `e8c4a35` | 0.4.0 | Historical candidate; packaged, desktop/installation gaps remain |
| [v0.3.1](v0.3.1.md) | 2026-11-14 | `f58fb9c` | 0.3.0 | Ranking milestone; no new installer from that task |
| [v0.3.0](v0.3.0.md) | 2026-11-14 | `54da00e` | 0.3.0 | Historical candidate; packaged, desktop/installation gaps remain |
| [v0.2.0](v0.2.0.md) | 2026-11-10 | `92cc084` | 0.1.0 | Installed-app development milestone |
| [v0.1.0](v0.1.0.md) | 2026-11-10 | `4201194` | 0.1.0 | Initial launcher milestone |

The [changelog](../../CHANGELOG.md) summarizes changes by category. Original
v0.5 stage entries remain in [development history](v0.5-development-history.md).
Repository maintenance and CI changes since v0.5.0 are included in the v0.5.1
changelog entry. Historical v0.5.0 checklists and hashes remain unchanged.

## Installation and upgrades

- **Supported target:** Windows 11 x64 with Microsoft Edge WebView2 Runtime.
  These notes do not claim validation of Windows 10, ARM64, or other platforms.
- **File search, v0.3 onward:** install the Everything desktop client and its
  separate ES command-line tool. ES 1.1.0.38 was verified. The desktop client must
  be running; the service alone is insufficient. Configure `NEXUS_ES_PATH` or
  user Path, then fully restart NEXUS. See [setup](../features/file-search/FILE_SEARCH.md#setup).
- Everything is optional for app search, Windows controls, and workflows.
  Audio controls need an enabled output device and Windows Audio service;
  Settings pages and sleep depend on hardware and Windows policy.
- Installers are unsigned. Setup can download missing WebView2; install WebView2
  beforehand for offline setup. End users do not need Node, Rust, a development
  server, an API key, or an AI service.
- No public installer is currently linked here. If given a candidate, close NEXUS
  and choose either its x64 NSIS `.exe` or x64 MSI, not both. Verify the exact
  file against the checksum manifest supplied for that build before installation.
- Back up local workflow data before upgrades, reinstalling, or uninstalling.
  Fully stop NEXUS, preserve the complete data directory including any SQLite
  sidecars, and keep a pre-upgrade copy. Do not overwrite a live database.

v0.5 workflows live in `%LOCALAPPDATA%\com.nexus.launcher\workflows.sqlite3`.
Default development and installed builds share this location. Even an older
installed application can share data created by newer development builds.
SQLite `user_version` and workflow document format remain 1; unsupported data
fails without automatic reset. This is a storage contract, not proof of a
successful installer upgrade. Missing apps and paths may need repair after moving
to another PC. Leave the NSIS uninstaller's app-data deletion checkbox unchecked
to preserve workflows; its live behavior still requires acceptance testing.

See [backup/restore](../USER_GUIDE.md#workflow-data-and-backup) and the
[release checklist](RELEASE_CHECKLIST.md) before relying on an upgrade.

## Prepare v0.5.0 publication

- [GitHub Release description](v0.5.0-github-release.md): prepared text, explicitly
  marked as a draft with publication on hold.
- [Release checklist](RELEASE_CHECKLIST.md): completed evidence, exact artifacts,
  version/tag review, and remaining acceptance/publication gates.
- [v0.5 verification](../verification/V05_VERIFICATION.md): historical automated
  checks, packaging, known risks, and required desktop scenarios.
- [CI scope](../development/CI.md): what a green check verifies and what remains manual.

An existing `v0.5.0` tag points to `797b7af`. Do not move or replace it as a
documentation task. The owner must reconcile the selected publication commit and
tag with the acceptance evidence before authorizing publication.

## Historical checksum records

[v0.3 installer hashes](RELEASE_SHA256.txt) and
[v0.5 Stage G hashes](V05_SHA256.txt) remain unchanged. They identify those
historical builds only. v0.3 records use installer basenames; v0.5 records use
repository-relative build paths. Rebuilt files can have the same version and
filename with different bytes. The local v0.5 artifacts inspected on 2026-11-16
do not match the Stage G hashes; those hashes must not accompany a later rebuild.
