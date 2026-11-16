# README presentation and capture record

Date: 2026-11-16. Source baseline: `797b7af` on `master`, v0.5.0.
This documents the README redesign and its visual evidence. It does not replace
the [v0.5 release acceptance report](V05_VERIFICATION.md).

## Source and distribution review

Reviewed the previous README, architecture, changelog, action registry, workflow
storage/execution/Studio/editor/management/launcher documents, file search and ranking,
audio/system actions, and the v0.3/v0.4/v0.5 verification reports. Cross-checked the
frontend command/mode registries, launcher and Studio components, Rust action
registrations, startup/shortcut behavior, manifests, lockfiles, and Tauri configuration.

The public GitHub API reported zero releases and zero Actions workflows on this date.
The README therefore uses a static candidate-version badge and no CI badge, published
download link, license badge, performance claim, or claim of completed release acceptance.
No license file is present. Locally built NSIS/MSI candidates exist; they were not
installed or published during this task. Older release hashes are historical evidence,
not checksums for newly rebuilt screenshot binaries.

All detailed user/developer instructions from the old README's **Install and run**
section onward are preserved in [USER_GUIDE.md](../USER_GUIDE.md), with relative links
adjusted. Only the obsolete opening branch/status paragraph was replaced.

## Real Windows captures

Captured the existing release application first, then compiled the current source
with embedded frontend assets and a build-time identifier override:

```powershell
npm.cmd run build
$env:TAURI_CONFIG = '{"identifier":"com.nexus.readme"}'
cargo build --locked --offline --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --bins
```

Both builds passed. No source/configuration/dependency/lockfile changes were required.
The ignored capture executable was copied to
`src-tauri/target/readme-capture/nexus-readme.exe`. Its SHA-256 was
`7f8f57df503c1b9c460fb74fb715a075c476879867bbbcB1782affe56e45f94c`
(hexadecimal is case-insensitive).

Final README screenshots come from this isolated current-source build, controlled
through the Windows computer-use skill (`@oai/sky`). It used
`%LOCALAPPDATA%\com.nexus.readme`, separate from production workflow storage.
The **Quick workspace** fixture was inserted only into that isolated SQLite database
using the existing schema and genuine catalog IDs for Notepad and Calculator. It is
demonstration data, not a shipped template or evidence of editor save/CRUD acceptance.

| Asset in `docs/assets/` | Evidence |
| --- | --- |
| `launcher.png` | Actual empty launcher, suggested commands, and keyboard hints |
| `application-search.png` | Actual installed-app matches for `visual`, with loaded app icons |
| `file-search.png` | Live Everything results for `f README.md`, highlighting and reveal control |
| `windows-controls.png` | Preview of `volume 35` and last-read device state; no volume write executed |
| `workflow-studio.png` | Actual Studio edit view scrolled to the two ordered application steps |
| `workflow-search.png` | Actual `w quick` result showing name, description and action count |
| `workflow-execution.png` | Actual completed run, reporting 2/2 steps and Windows launch acceptance |
| `launcher-demo.gif` | Six actual captured states: launcher, `note`, `notepad`, volume preview, workflow search, completed run |
| `nexus-mark.svg` | Branding derived from the launcher's lavender tile and N; not an application screenshot |

The workflow was activated through the launcher. The UI reported both steps succeeded;
this establishes reported native launch acceptance, not that both apps finished loading.
The capture session was closed afterwards. No production workflows were added, edited,
or deleted; no audio write, lock, sleep, installer, or release publication was performed.

An earlier isolated draft session ended unexpectedly during UI automation. The draft
was not used as save evidence. A fresh session then displayed and ran the explicitly
seeded fixture. This task does not establish the cause of that interruption or claim
complete desktop interaction acceptance.

The capture API supplies JPEG window images. Windows/.NET image tools cropped the
launcher's 10-pixel outer desktop margin, yielding consistent **630 × 290** PNGs.
The Studio capture retains its **953 × 731** native window frame. No interface text,
controls, results, or success states were painted over or synthesized. The PNG format
does not restore detail lost in the source JPEGs. GIF pauses were adjusted for reading;
this is an edited state sequence, not a continuous or speed-accurate recording.
Intermediate captures and processing scripts remain under the ignored build directory.
No new tool installation or project dependency was needed.

## Verification scope

Documentation/image/link/render checks are recorded here after completion. The frontend
production build and locked offline Rust compilation were run to obtain current native
captures. Application tests were not rerun for documentation-only edits. Historical
test counts in other reports are not presented as newly executed tests.

Installer installation, upgrade/data preservation, complete Windows hardware/policy
coverage, and a disconnected-network desktop session remain outside this README task.
No release acceptance report was rewritten to imply those checks passed.
