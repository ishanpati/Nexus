# v0.5.2 Stage A: command discovery metadata

Implemented on `feature/help-center`. This stage supplies a read-only discovery
catalog; it does not create a Help Center window, voice input, parser, dispatcher,
runtime service, or additional execution permission. Package versions stay 0.5.1.

The current catalog contains 42 entries: 13 implemented typed command families,
one unavailable Help entry, three search modes, eight interactions, eight shortcuts
and nine executable workflow action descriptors.

## Inventory and ownership

| Source | Discoverable capabilities |
| --- | --- |
| `src/commands.ts` | Open Notepad (`notepad`, `launch notepad`), Create workflow, Edit workflow |
| `src/systemCommands.ts` | Audio status (`volume status`, `vol status`, `audio status`, `mute status`); volume/vol absolute, signed relative and up/down; mute/unmute; Sound, Wi-Fi, Bluetooth and Display settings; lock; sleep |
| `src/launcherCapabilities.ts` search registrations | Ordinary installed-app search, literal `f ` file/folder search, literal `w ` saved workflow name/alias search |
| Launcher/Studio interaction registrations | Refresh apps, retry files, refresh workflows, Workflow runs (progress, approvals, cancellation), Show more, move launcher, manage workflows (filter, rename, aliases, edit/add/remove/reorder/save steps), confirmed workflow deletion |
| Keyboard registrations | Global toggle, Up/Down navigation, Enter execution, Ctrl+Enter file reveal, Escape reset/dismiss, Tab/Shift+Tab control focus (Enter/Space activation), Alt+F4 lifecycle, Escape in Studio action search |
| Rust public Action Registry metadata | `app.launch`, `file.open`, `folder.open`, `audio.set_volume`, `audio.adjust_volume`, `audio.set_mute`, `settings.open`, `system.lock`, `system.sleep` |
| Reserved metadata-only help entry | `nexus.help`, syntax `help`, explicitly unavailable until a Help Center handler/UI exists |

Fixed system aliases come directly from the parser's `names` arrays, including
`mute audio`, `unmute audio`, `audio settings`, `wi-fi settings`, `wireless settings`,
`bt settings`, `screen settings`, `lock screen`, `lock computer`, `sleep computer`.
No separate list supplies aliases to discovery. Partial matching and app ranking
continue using the existing matcher. An action row is a Studio action, not a typed
launcher command. Its syntax describes where to select it in Studio.

There is no implemented typed `clear list` command. Escape clears transient query,
selection, confirmations and result presentation and hides the launcher; it does
not erase app launch history, saved workflows, or active workflow runs. Bare `f`
and `w`, leading whitespace, or tab separators remain ordinary app/command queries.
The Help entry is metadata only and cannot be selected/executed in launcher search.
No unimplemented launcher handler is advertised as usable.

## Contract and boundaries

`CapabilityRegistration` / `DiscoverableCapability` in `src/capabilityMetadata.ts`
are serializable data contracts. Kinds distinguish commands, search modes,
interactions, shortcuts and actions. IDs are namespaced and stable; names and
categories are display text, not identities. Aliases remain phrase arrays suitable
for future consumers; they do not introduce speech recognition or interpretation.

`registeredCapabilities()` adapts the actual central command registrations rather
than maintaining parallel help commands. Required discovery metadata is attached
to the existing built-in and system definitions. Search prefixes drive both the
existing `searchMode()` parser and discovery. Launcher key constants drive its
event handlers. Button labels reuse interaction registrations. There is no change
to native executor selection, query dispatch, foreground behavior or validation.

The typed `createCommandDiscoveryService(invoke)` retrieves
`command_discovery_metadata`, then composes frontend-owned registrations with Rust
context using `discoverCapabilities()`. It returns the entire catalog without
category-specific rendering or filtering out unavailable entries. Categories are
grouped dynamically with `groupCapabilities()`; there is no fixed category enum.
Invalid/duplicate public IDs, missing required text and unresolved action links
fail visibly. Hidden registrations are excluded. Explicit field projection omits
execution requests, handlers, parser prefixes and private registration fields.

Rust exposes its existing public workflow action descriptors, the actual global
shortcut registration status/keys, and file prerequisite status. `shortcut::LABEL`
is parsed to register the actual chord and returned to discovery and the launcher
footer, eliminating the previous separate footer string. Current action publication
uses the same `ActionRegistry::descriptors()` filter as `list_workflow_actions`;
nonpublic/non-workflow registrations and test actions are not enumerated.

Associated action descriptors are reused intact, including parameter constraints,
confirmation policy, workflow compatibility, terminal placement and schema version.
`workflowCompatible` means at least one associated action is workflow compatible;
the search mode, shortcut or typed phrase itself is not a stored workflow step.
Open Notepad's legacy handler and audio status have no corresponding workflow action
and are not marked compatible. Numeric syntax has one volume command descriptor
linked to both absolute and relative action schemas.

`launcherConfirmation` describes the launcher/interaction context separately from
`actions[].confirmation`, which remains the authoritative workflow/Action Registry
policy. System launcher policy is derived from the existing
`requiresSystemConfirmation()` function. In particular, launcher lock is immediate;
workflow lock requires fresh approval and final placement. Sleep requires launcher
confirmation as well. File search describes sensitive-target confirmation; workflows
continue rejecting executable/script/unknown file types under their stricter runner
policy. Workflow runs use per-step confirmation. Discovery authorizes nothing.

## Availability

Everything/ES requirements remain visible even when missing. The read-only Rust
check reuses the actual ES path resolver on a blocking worker. It starts no ES
process and does not touch the index. Missing/invalid ES configuration is
`unavailable`; configured ES is `unknown` until a real search checks Everything
client/index readiness. Readiness is never inferred from a configured executable.
Global shortcut conflicts expose `unavailable` and the existing warning. Other
device, target and permission checks generally remain `unknown` until use rather
than performing native operations just to render help. Status is a snapshot; reads
are not polled or cached by the discovery service. IPC failures propagate to callers.

## Adding capabilities

1. Add the actual typed capability and its metadata to its authoritative location:
   `commands.ts`, `systemCommands.ts`, or the mode/interaction/shortcut registrations
   in `launcherCapabilities.ts`. Required command metadata prevents a new built-in
   registration from silently omitting its category/examples/discoverability.
2. For action-backed capabilities, reference existing Rust action IDs. Do not copy
   parameter limits, confirmation policy, or workflow compatibility into help.
   New public workflow actions registered in `action_registry/builtins.rs` appear
   automatically as Studio action rows with their existing descriptor data.
3. Keep internal helpers outside public registries, or set `discoverable: false`.
   Add useful syntax/examples and prerequisites beside the capability. A new
   category requires no catalog or future category-specific UI change.
4. Add behavior/parser coverage. If Rust action metadata changes, regenerate
   `tests/fixtures/discovery-actions.json` from the existing `workflow_actions`
   example (raw UTF-8 JSON, preserving numeric spelling such as `0.0`). This is a test-only contract fixture, not a runtime help
   inventory. A Rust test compares it with compiled descriptors to prevent drift.

Discovery never grants arbitrary execution. New executable commands still require
the existing explicit TypeScript/Rust handler and boundary validation; registering
descriptive data alone cannot create an executor.

## Verification

New tests cover IDs and required metadata, hidden registrations, automatic command
and category extension, authoritative action links/schemas/policies, real parser
examples and aliases, literal search prefixes, shortcut config/conflicts, missing
dependencies, JSON projection and the read-only service failure path. Rust checks
compare frontend contract data with compiled Action Registry metadata and verify
the configured shortcut still registers Ctrl+Shift+Space.

Executed locally without dependency downloads:

- Targeted frontend discovery/commands/system/parser/file tests: 34 passed.
- Full frontend suite via Node's test runner with the same files/options as
  `npm test`, plus TAP summary output: 113 passed, zero failures.
- `npm run build`: TypeScript and production Vite build passed.
- Targeted Rust discovery tests: three passed. The first contract-fixture check
  caught numeric formatting lost during JSON regeneration; preserving the native
  example's raw output corrected the fixture, and the check passed.
- `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`:
  130 passed, two intentionally ignored (live audio and subprocess fixture), zero
  failures. Existing workflow, native-boundary, app launch/foreground and Windows
  controls regressions remain covered by their automated tests.
- Rust doctests passed (zero doctests).
- `cargo clippy --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets`:
  passed with existing style warnings; none in the new discovery module.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `git diff --check`:
  passed. Git reports normal LF/CRLF conversion notices; Cargo reports the existing
  profile-path canonicalization warning.
- `npm run tauri build -- --debug --no-bundle -- --locked --offline`, with
  `CARGO_NET_OFFLINE=true`: passed, producing an embedded-assets Windows executable.
- The read-only `workflow_actions` example ran to produce the contract fixture;
  it did not execute actions or change Windows state.

Manual Windows launcher, native control, foreground activation, actual discovery
IPC and disconnected desktop acceptance were not performed. Compilation and
automated tests do not establish those behaviors. No installers, tags, merge,
push or release publication were performed.

## Files changed

- Added `src/capabilityMetadata.ts`, `src/commandDiscovery.ts`,
  `src/launcherCapabilities.ts`, `src/services/commandDiscoveryService.ts` and
  `src-tauri/src/command_discovery.rs` for typed registrations, composition and
  read-only native context.
- Updated `src/commands.ts`, `src/systemCommands.ts`, `src/fileSearch.ts`,
  `src/App.tsx`, `src/hooks/useLauncherWindow.ts` and
  `src/components/WorkflowEditor.tsx` to share command metadata, prefixes, keys
  and existing control labels.
- Updated `src-tauri/src/lib.rs`, `src-tauri/src/shortcut.rs` and
  `src-tauri/src/file_search/mod.rs` for endpoint registration, shortcut
  configuration/status and read-only prerequisite resolution.
- Added `tests/commandDiscovery.test.mjs` and the generated test-only
  `tests/fixtures/discovery-actions.json` contract.
- Updated `CHANGELOG.md`, `docs/README.md` and this architecture document.

No dependency manifests, lockfiles, capability permissions, workflow data schemas
or version fields changed. No existing implemented command family was omitted;
interactions and action descriptors retain their own kinds rather than being
represented as additional typed commands.
