# Changelog

## 0.5.0 release candidate — Stage G (2026-11-15)

- Workflow Studio supports creating, editing, ordered action configuration, local
  saving, filtering and confirmed deletion with stable workflow/step identities.
- Extensible Rust Action Registry, versioned transactional SQLite storage,
  sequential workflow execution, run-bound approval and cancellation, and exclusive
  launcher `w ` search from Stages A–F are included in this candidate.
- Fixed launcher close destroying the shortcut's window while auxiliary windows
  survived. Active runs retain the launcher; idle quit protects Studio drafts with
  Save / Discard / Cancel before exiting. Studio close still keeps NEXUS alive.
- Save-time validation and availability annotations now reject sensitive workflow
  file targets consistently with execution; existing steps stay visible for repair.
- Aligned npm/Cargo/Tauri metadata to 0.5.0; preserved dependency versions,
  installer identifier and workflow database schema/location.
- Added user workflow instructions, backup/restore and shared dev/production data
  warnings; replaced machine-specific setup paths with portable examples.
- Stage G automated results, actual packaging artifacts, known limitations and
  required installed-app/offline/manual checks: [V05_VERIFICATION.md](V05_VERIFICATION.md).
  This is not a declaration of completed desktop release acceptance.

## 0.5 development - Stage F launcher workflow search and execution UI

- Exclusive `w ` search lists saved workflows and ranks partial names/aliases with
  the existing case-insensitive matcher; workflow rows show description/action count.
- Enter and clicks dispatch selected stable UUIDs through the existing Rust engine.
- Synchronous duplicate-start guards; run-ID progress with ordered snapshot recovery,
  step results, useful failures, completion and truthful app activation wording.
- Explicit run/step/token-bound sensitive action approval/denial and cancellation;
  closing progress or dismissing/reopening the launcher leaves execution active.
- Accepted starts clear input; rejected starts retain it. Repository notifications,
  focus and manual refresh reflect renames/deletions without restarting.
- Existing Studio commands, apps, file mode, system controls and shortcut preserved.
- No dependencies, lockfile edits, new runtime services or AI automation.
- Verification: [WORKFLOW_LAUNCHER.md](WORKFLOW_LAUNCHER.md).

## 0.5 development - Stage E2 saved workflow management

- Saved list with local name/description/alias filtering, action counts and update times.
- Reuse the dynamic editor for UUID edits; preserve workflow and unchanged step IDs.
- Preserve unavailable action parameters and show missing references for repair.
- Confirm deletion of the exact workflow ID/revision; refresh after saves and deletes.
- Save / Discard / Cancel for navigation and Studio close, including launcher requests.
- Keep drafts on failed/stale/deleted saves; guard concurrent save/delete activations.
- No dependencies, lockfile changes, execution UI or launcher `w` search added by E2.
- Verification: [WORKFLOW_MANAGEMENT.md](WORKFLOW_MANAGEMENT.md).

## 0.5 development - Stage E1 dynamic workflow editor

- Interactive name, description, aliases and ordered action configuration in Studio.
- Searchable category groups and generic controls generated from registry metadata.
- Searchable installed-app selection and Studio-only native Windows path pickers.
- Stable step editing, removal, Up/Down ordering and terminal-action restrictions.
- Inline validation, unsaved changes, optimistic revisions and duplicate-save gates.
- Save-time Rust reference checks catch disappeared app/path selections without execution.
- No new dependencies, workflow-list management, execution UI or launcher `w` search.
- Verification and manual checklist: [WORKFLOW_EDITOR.md](WORKFLOW_EDITOR.md).

## 0.5 development - Stage D separate Workflow Studio

- Searchable Create workflow and Edit workflow commands dispatch selected IDs.
- One reusable, resizable 950x700 Studio window with create/list/UUID edit screens.
- Listener-ready startup, guarded launcher focus transfer, and failure recovery.
- Cancellable unsaved-draft navigation and close; closing Studio keeps NEXUS alive.
- Repository-backed views with scoped save/delete notifications and refresh.
- No complete step editor, save/delete controls, execution UI, or new dependencies.
- Verification and manual Windows checklist: [WORKFLOW_STUDIO.md](WORKFLOW_STUDIO.md).

## 0.5 development - Stage C workflow execution

- Backend-owned sequential execution by saved workflow UUID, with whole-plan
  validation, fresh native prerequisites, stop-on-first-failure and no rollback.
- Unique run IDs, per-step results, duplicate-start prevention, cancellation and
  retained status snapshots with ordered identifier-only progress events.
- Final-step-only lock/sleep require fresh, expiring, single-use approval bound to
  the run, step and normalized action. No stored flags grant execution authority.
- Main-window execution IPC; closing/hiding presentation preserves active runs.
- Reject recursive workflow calls and executable/script-like file opens. Catalog
  app launches report Windows acceptance without claiming application readiness.
- Mock execution tests never lock or suspend Windows. No workflow UI or search.
  Protocol, verification and limitations: [WORKFLOW_EXECUTION.md](WORKFLOW_EXECUTION.md).

## 0.4.0 — Windows Control Center

- Typed deterministic system-command registry, aliases, validation and contextual
  suggestions integrated with existing app ranking; file mode remains exclusive.
- Native Core Audio absolute/relative volume, mute/unmute and status. Relative
  changes clamp; devices are resolved per request and failures are structured.
- Four allowlisted Settings destinations, native workstation lock, and sleep
  requiring a separate modal and expiring single-use Rust confirmation ticket.
- Shared selected-result dispatch for clicks/Enter. Partial fixed commands now
  execute their actions; numeric commands retain validated arguments.
- Successful execution clears/hides; failures retain input. Escape and summon
  reset input. Lock/confirmed sleep reset before IPC with restoration on failure.
- Last-read audio state and retained last-action feedback in the existing footer.
- Local non-activating volume/mute overlay with percentage and progress bar.
  This is a NEXUS overlay, not Windows' private native media-key flyout.
- Preserved application launching, icons, Everything file/folder search, strict
  ranking, file confirmation, global shortcut, monitor placement and single instance.
- Package versions aligned to 0.4.0 without dependency version changes. No new
  runtime service, administrator requirement, cloud dependency or API key.
- See V04_VERIFICATION.md for executed checks and outstanding Windows checks.

## 0.3.0 — 2026-11-14

### Added

- Everything ES backend: bounded async searches, literal queries, UTF-8 EFU parsing,
  availability errors, and process timeouts.
- `f ` file mode: teal border, file/folder labels/icons, highlighting, 150 ms debounce,
  stale-response protection, batches of ten, and 100-result maximum.
- Default-association file opening, Explorer folder opening, and Ctrl+Enter/Show
  to reveal an item in its containing folder.
- Explicit confirmation for executable, script, shortcut, and unknown file types;
  current-path validation and understandable opening failures.
- Focused tests and file-search, architecture, and verification documentation.

### Fixed / release preparation

- Reject Win32 reserved device names such as `NUL.txt` at the opening boundary.
- Escape cancels pending file confirmation while hiding NEXUS.
- Align npm, Rust, and Tauri versions to 0.3.0 without changing dependency versions
  or installer identity.
- Replace obsolete preview-only README instructions with current launch behavior.

### Preserved

- Application discovery, ranking, native launching, cached icons and local usage.
- Open Notepad aliases, Ctrl+Shift+Space, single instance, Escape/focus dismissal,
  autofocus, temporary dragging, and monitor-aware placement.

### Requirements / limitations

- Windows 11 x64 and WebView2; file search also needs the Everything desktop client
  and separately installed ES (verified: 1.1.0.38).
- No cloud/AI runtime. Everything/ES are not bundled. Setup may download WebView2
  if absent; install it beforehand for offline setup.
- Literal indexed-path search only; no advanced syntax, named instances, content
  search, file mutations, signing, tray icon, or automatic updates.
- RELEASE_VERIFICATION.md records executed checks and outstanding desktop checks.

## 0.2 — development milestone

Installed application discovery/search/launching, Windows icons, and local launch
frequency ranking. Package metadata still used 0.1.0 during development.

## 0.1 — initial milestone

Floating command bar, native Open Notepad, global shortcut, placement/dragging,
focus dismissal, and single-instance handling.
