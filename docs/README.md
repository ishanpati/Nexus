# NEXUS documentation

Start with the [project overview](../README.md) or [user guide](USER_GUIDE.md).
NEXUS v0.5.1 is a release candidate; see the [foreground-launch audit](verification/V051_VERIFICATION.md)
for current checks, build artifacts, and remaining desktop/installer acceptance.

## Use NEXUS

- [User guide](USER_GUIDE.md): installation, controls, workflows, troubleshooting,
  and [workflow data backup and restore](USER_GUIDE.md#workflow-data-and-backup).
- [File-search setup](features/file-search/FILE_SEARCH.md#setup): optional Everything and ES requirements.
- [Changelog](../CHANGELOG.md): implemented changes by release and development stage.
- [Versioned release notes](releases/README.md): version comparison, candidate
  status, installation requirements, upgrade precautions, and publication checklist.

## Understand the implementation

For developers and curious visitors, [architecture](architecture/ARCHITECTURE.md)
describes the v0.5 boundaries, module responsibilities, local data flow, and IPC.

| Area | Technical documentation |
| --- | --- |
| Action contracts | [Action Registry: metadata, validation, extension, and confirmation](architecture/ACTION_REGISTRY.md) |
| File search | [Everything integration and opening policy](features/file-search/FILE_SEARCH.md) · [Retrieval, ranking, and limits](features/file-search/FILE_SEARCH_RANKING.md) |
| Foreground app launching | [Native handoff, identity, outcomes, and acceptance checks](features/FOREGROUND_LAUNCHING.md) |
| Windows controls | [Native audio](features/AUDIO_CONTROLS.md) · [Settings, lock, and sleep](features/SYSTEM_ACTIONS.md) |
| Workflow persistence and execution | [Storage and migrations](features/workflows/WORKFLOW_STORAGE.md) · [Execution, approval, and cancellation](features/workflows/WORKFLOW_EXECUTION.md) |
| Workflow authoring | [Studio lifecycle](features/workflows/WORKFLOW_STUDIO.md) · [Dynamic editor](features/workflows/WORKFLOW_EDITOR.md) · [Saved workflow management](features/workflows/WORKFLOW_MANAGEMENT.md) |
| Workflow launcher | [Search, dispatch, progress, and recovery](features/workflows/WORKFLOW_LAUNCHER.md) |

## Develop and contribute

- [Continuous integration](development/CI.md): automated checks, runner/toolchain,
  caching, diagnostics, and remaining manual Windows verification.
- [Development and release builds](USER_GUIDE.md#development-and-release-build):
  prerequisites, commands, offline caches, and installer output locations.
- [Repository maintenance](development/REPOSITORY_MAINTENANCE.md): tracked-file
  boundaries, installer caches, safe housekeeping, and the hygiene audit.
- [Engineering rules](../AGENTS.md): stack, execution boundaries, repository layout,
  and required checks. Run command examples from the repository root.

## Review verification and release evidence

Current candidate: [v0.5.1 notes](releases/v0.5.1.md) and [v0.5.1 audit](verification/V051_VERIFICATION.md).

Historical publication preparation: [v0.5.0 notes](releases/v0.5.0.md),
[GitHub Release draft](releases/v0.5.0-github-release.md), and
[release checklist](releases/RELEASE_CHECKLIST.md). Acceptance remains pending.

These records preserve the checks performed at the time and their outstanding
manual checks. Historical passes do not establish acceptance of the current build.

| Record | Scope |
| --- | --- |
| [v0.5.1 Stage B](verification/V051_VERIFICATION.md) | Foreground regressions, release metadata, production packaging, and acceptance gaps |
| [v0.5 Stage G](verification/V05_VERIFICATION.md) | Candidate audit, packaging, known risks, and pending release acceptance |
| [v0.4](verification/V04_VERIFICATION.md) | Windows controls regression, diagnostics, and packaging |
| [v0.3](verification/RELEASE_VERIFICATION.md) | File-search release checks and pending desktop/installation checks |
| [README presentation](verification/README_VERIFICATION.md) | Screenshot provenance and limits of the visual evidence in `assets/` |

Historical checksums: [v0.3 installers](releases/RELEASE_SHA256.txt) and
[v0.5 candidate artifacts](releases/V05_SHA256.txt). Contents are unchanged;
v0.3 entries name installer files, and v0.5 paths are relative to the repository root.
These are records of local builds, not published downloads or hashes of later rebuilds.

## Read stage documents in context

Feature documents retain development-stage scope, test results, and future-tense
notes. Use the architecture, user guide, changelog, and v0.5.1 report for current
implemented behavior. A stage's exclusions or proposed next steps describe that
stage, not a current roadmap or an unimplemented feature claim.

Some descriptions overlap intentionally: the user guide explains operation,
architecture explains boundaries, and feature documents preserve contracts and
verification history. Superseded details include Stage D's name-only Studio,
Stage E1's stricter unavailable-step saving, and Stage E2's launcher-close limitation;
later stages document their replacements. No duplicate documents were removed.
