# Changelog

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
