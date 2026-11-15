# NEXUS engineering rules

NEXUS is a local-first Windows 11 universal command bar. The user owns product decisions; agents implement, test, and maintain only the requested scope. Target hardware: Intel Core Ultra 7 225H, 16 GB RAM.

## Rules

1. **Inspect first.** Read relevant source, manifests, lockfiles, config, and instructions; check the working tree and preserve unrelated user changes.
2. **Preserve the stack.** Continue with Tauri 2, React, TypeScript, and Rust. Do not recreate the project or replace working config. Keep dependency versions and lockfiles unless the requested change requires updates.
3. **Keep boundaries clear.** Put native Windows operations and execution-boundary validation in Rust; UI rendering, input, and selection in React/TypeScript. Use narrow, typed Tauri calls.
4. **Keep commands modular.** Use stable IDs, small modules, and a simple registry/dispatcher. Separate handlers from UI and startup; avoid a speculative plugin framework.
5. **Stay local-first.** Runtime must require no ChatGPT, API key, subscription, paid/cloud service, or LLM (including local models). Development assistance must not become a runtime dependency.
6. **Minimize complexity.** Prefer the standard library and existing dependencies. Add a library only for a concrete need; keep memory and background work suitable for the target hardware.
7. **Never shell user text.** Do not pass command-bar input to cmd.exe, PowerShell, or any interpreter. Dispatch registered IDs only; validate arguments in Rust; use native APIs or trusted executable paths with explicit argument lists; reject unknown commands.
8. **Confirm destructive actions.** Show the change and exact targets, require explicit confirmation, and enforce it at the Rust boundary. Cancellation must have no side effects.
9. **Deliver one feature at a time.** Keep scope focused and run relevant checks. Test behavior, validation, failure paths, and confirmation where applicable. Inspection/documentation tasks must not add features.
10. **Report verification honestly.** Explain failures and useful next steps. State exactly what ran and distinguish inspection, compilation, automated tests, and manual Windows testing. A build alone does not prove runtime behavior.

## Folder map

- `src/`: React/TypeScript UI and client logic
  - `App.tsx`: command bar UI
  - `commands.ts`: typed command registry
  - `components/`, `hooks/`, `services/`: UI, lifecycle, and Tauri service boundaries
- `src-tauri/src/`: Rust backend
  - `lib.rs`: command registration and Tauri startup; keep single-instance first
  - `launcher_window.rs`, `shortcut.rs`: shared summon path and shortcut lifecycle
  - feature modules: `action_registry/`, `audio/`, `commands/`, `file_search/`, `installed_apps/`, `system_actions/`
- `src-tauri/capabilities/`, `src-tauri/tauri.conf.json`: least-privilege permissions and native config
- `tests/`: frontend/Node tests
- Root `*.md`: feature and architecture documentation
- `package*.json`, `src-tauri/Cargo.*`: dependency manifests and lockfiles

## Checks

- Frontend build/typecheck: `npm run build`
- Command matching tests (Node 22.6+): `npm test`
- Rust tests: `cargo test --locked --manifest-path src-tauri/Cargo.toml` (add `--offline` when cached)
- No npm lint script exists.
- Documentation-only changes need content/diff checks, not application builds. Native behavior requires Windows testing; completed features must be checked without network access.

## Token-Saving Execution Rules
- When running tests or builds, always pipe output to filter out passing noise (e.g., `--silent`, `--reporter=dot`, or `tail -n 60`) so only failures and stack traces enter context.
- Read specific line ranges around target functions instead of reading entire large files.

## Agent Loop Efficiency
- Never dump full test/build logs into context; always use silent/dot reporters or pipe output through `tail -n 50` so only failures are read.
- Run targeted tests for modified modules first; run the full suite only once at the very end.
- Read specific line ranges (`sed -n` / bounded reads) rather than reading entire files over 300 lines.
