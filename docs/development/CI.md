# Continuous integration

[Workflow source](../../.github/workflows/ci.yml) ·
[GitHub Actions runs](https://github.com/ishanpati/Nexus/actions/workflows/ci.yml)

## Triggers and environment

The `CI` workflow runs on pushes to `master`, pull requests targeting `master`,
and manual dispatch. `master` is the repository's default branch. Update both
branch filters and the README badge if the default branch changes. Manual dispatch
becomes available in GitHub's UI once the workflow exists on the default branch.

The single `Windows checks` job uses the GitHub-hosted `windows-2025` x64 image,
Node.js 26, and Rust 1.99.1 with rustfmt and Clippy. Node 26 supports the test
runner's TypeScript stripping and `node:module` hooks. Rust is pinned so formatting
and lint results do not change with an unreviewed toolchain update. Action
implementations are pinned to full commit SHAs; review those pins and toolchain
versions periodically.

The runner supplies MSVC C++ tools and the Windows SDK. The workflow verifies
the C++ installation and prints tool versions before building. No additional
installer SDK, Everything/ES setup, running desktop application, or developer
machine path is needed. WebView2 is a desktop runtime prerequisite; these checks
compile the application without starting a webview.
Windows steps use `npm.cmd` to avoid PowerShell execution-policy restrictions
on npm's `.ps1` shim; it executes the same package scripts shown below.

## Checks

| Step | Command and scope |
| --- | --- |
| Install dependencies | `npm ci --no-audit --no-fund`, using the existing npm lockfile |
| Rust formatting | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` |
| Frontend tests | `npm test`, the existing package script and complete Node test suite |
| Frontend build | `npm run build`, which runs TypeScript checking and the Vite production build |
| Clippy | `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets` |
| Rust tests | `cargo test --locked --manifest-path src-tauri/Cargo.toml --all-targets` |
| Rust documentation tests | `cargo test --locked --manifest-path src-tauri/Cargo.toml --doc` |
| Native build | `npm run tauri build -- --debug --no-bundle -- --locked`; compiles a Windows executable with embedded production frontend assets through Tauri's normal configuration |

The native build invokes the configured `beforeBuildCommand` (`npm run build`)
again. It checks Tauri's asset embedding and native build path, but does not create
installers or exercise the optimized release profile. All Cargo dependency-resolving
commands use `--locked`. Fresh runners need network access to download pinned
dependencies; caches are an optimization, not a requirement for success.

Clippy was investigated locally with Rust 1.99.1. The existing code returns success
with style warnings (17 for the library, plus one additional test warning). CI keeps
those diagnostics visible and fails on Clippy errors. It does not use `-D warnings`,
lint allowances, or `continue-on-error`. A warnings-as-errors policy can be adopted
after a separate, reviewed cleanup of the existing warnings.

## Safety, caching, and diagnostics

The normal Rust suite uses fake operations for lock, sleep, workflow approval,
and execution failure paths. The live audio round-trip is already marked ignored;
CI never opts into ignored tests. The other ignored case is a sleeping-child
fixture explicitly invoked by its timeout test, not an OS sleep operation.
CI does not execute diagnostic examples, launch NEXUS, modify audio, lock the
workstation, suspend Windows, or touch production workflow data.

The token grants only `contents: read`, and checkout does not persist credentials.
No repository secrets, API keys, signing certificates, deployment permissions,
or release publishing are used. Pull requests run with the normal
`pull_request` event, including forks subject to GitHub's contributor approval
policy. The workflow does not use `pull_request_target`.

`setup-node` caches npm's download cache using `package-lock.json`; `npm ci`
still performs a clean dependency installation. `rust-cache` caches Cargo registry
and dependency build output for `src-tauri`, keyed by the toolchain and Cargo
inputs. Cargo cache writes are restricted to `master` runs. Incremental compilation
is disabled and Cargo uses two workers to keep memory and cache growth bounded.

The job has a 45-minute timeout. New runs cancel superseded runs for the same
pull request or branch. Every check has its own named step, native command failures
propagate, Rust failures include backtraces, and GitHub retains step logs. Inspect
the failed step and its complete log before retrying; caches can be deleted through
GitHub Actions settings if cache corruption is suspected.

## What still needs desktop verification

A green CI result establishes only the automated checks run for that commit.
Windows Server runner compilation does not establish Windows 11 interaction,
hardware behavior, performance, or release acceptance. Continue the
[v0.5 manual acceptance checklist](../verification/V05_VERIFICATION.md#manual-checks-excluded-by-the-no-computer-control-instruction),
including shortcut/focus/multi-monitor behavior, native app/file activation,
Everything failure recovery, audio devices and overlays, explicit lock/sleep
confirmation, Studio/picker interaction, installed-app restart, offline runtime,
installer upgrade/uninstall, and workflow backup/data preservation.

Historical verification reports retain their original results. They are not
evidence of a successful GitHub Actions run. The README badge reports the actual
`ci.yml` push status for `master`; it can remain unknown until that workflow has
run on the default branch.

## Repository configuration

Enable GitHub Actions and allow the pinned checkout, setup-node, rust-toolchain,
and rust-cache actions under the repository/organization policy. No custom
secrets are required. After the first successful pull-request run, optionally
require the `Windows checks` status in a branch protection rule or ruleset for
`master`. Do not grant write permissions or enable publishing for this workflow.
