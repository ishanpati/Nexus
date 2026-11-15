# NEXUS engineering rules

NEXUS is a local-first universal command bar for Windows 11. The user is the
product owner; coding agents are responsible for writing, testing, and maintaining
the code within the requested scope. The target computer has an Intel Core Ultra
7 225H and 16 GB RAM.

1. **Inspect before changing.** Read the existing source, manifests, lockfiles,
   configuration, and applicable instructions. Check the working tree and preserve
   unrelated user changes.
2. **Preserve the existing project.** Continue with Tauri 2, React, TypeScript, and
   Rust. Do not recreate the project or replace working configuration unnecessarily.
   Preserve dependency versions and lockfiles unless a change requires updating them.
3. **Keep responsibilities clear.** Native Windows operations and validation at
   the execution boundary belong in Rust. Rendering, input, selection, and other UI
   logic belong in React/TypeScript. Use typed, narrow Tauri calls between them.
4. **Keep commands modular.** Use stable command IDs, small command modules, and a
   simple registry/dispatcher so capabilities can be added independently. Keep
   handlers separate from UI components and application startup. Start with the
   smallest structure needed; avoid a speculative plugin framework.
5. **Require no runtime AI or cloud service.** The finished application must work
   locally without ChatGPT, an OpenAI API key, a subscription, paid AI services,
   or cloud infrastructure. Never use an LLM as a runtime dependency, including a
   local model. Development assistance must not become an application dependency.
6. **Minimize dependencies and complexity.** Prefer the standard library and
   existing dependencies. Add a library only for a concrete requirement that
   justifies it. Keep memory and background work appropriate for the target machine.
7. **Never turn user text into shell commands.** Do not pass command-bar input to
   cmd.exe, PowerShell, or another command interpreter. Dispatch only registered
   command IDs, validate arguments in Rust, and use native APIs or trusted executable
   paths with explicit argument lists. Reject unknown commands; never fall back to
   shell execution.
8. **Confirm destructive operations.** Before any destructive action, show what
   will change and the exact targets, then require explicit user confirmation.
   Cancellation must have no side effects. Enforce confirmation at the Rust
   execution boundary as well as presenting it in the UI.
9. **Deliver one feature at a time.** Keep changes focused and run relevant checks
   and tests. Test meaningful behavior, validation, failure paths, and confirmation
   when applicable. Do not add features during an inspection or documentation task.
10. **Report errors and verification honestly.** Explain failures clearly, including
    their cause when known and a useful next step. State which checks actually ran,
    their outcomes, and what remains unverified. Distinguish source inspection,
    compilation, automated tests, and manual Windows testing. Do not claim runtime
    behavior based only on a successful build.

## Existing layout and checks

- `src/`: React/TypeScript frontend; `App.tsx` contains the command bar UI,
  `commands.ts` contains the typed command registry, and `hooks/` contains UI lifecycle logic.
- `src-tauri/src/`: Rust backend; `lib.rs` registers commands and starts Tauri.
  `shortcut.rs` owns global shortcut registration/cleanup; all summon paths use
  `launcher_window::show`. Keep the single-instance plugin first in initialization.
- `src-tauri/tauri.conf.json` and `src-tauri/capabilities/`: native configuration
  and permissions. Keep permissions limited to what implemented features need.
- `package.json` / `package-lock.json`: frontend dependencies and npm scripts.
- `src-tauri/Cargo.toml` / `src-tauri/Cargo.lock`: Rust dependencies.
- `npm run build`: existing TypeScript check and Vite frontend build.
- `npm test`: command matching tests using Node's built-in runner (Node 22.6+).
- `cargo test --locked --manifest-path src-tauri/Cargo.toml`: native validation,
  execution/error handling, and placement tests. Add `--offline` when dependencies
  are already cached. No npm lint script is configured.

Choose verification appropriate to the change. Documentation-only edits need
content and diff checks, not application builds. Native behavior needs Windows
testing, and the completed feature must be checked without network access.
