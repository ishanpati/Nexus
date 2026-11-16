# v0.5.0 release checklist

**Publication status: HOLD.** Snapshot: 2026-11-16, source inspected at `4163806`.
Package version: 0.5.0. Final publication commit/artifact set: **not selected or
accepted**. No release publication or tag mutation is part of this task.

Checked evidence below records specific completed inspections or prior runs.
It does not complete the unchecked gates for the final publication build.

## Evidence already available

- [x] Reviewed the changelog, six version tags/commits, manifests/lockfiles,
  installer configuration, user instructions, and historical verification reports.
- [x] Verified all six current application version fields equal 0.5.0:
  `package.json`, both root-package fields in `package-lock.json`, Cargo's package
  in `Cargo.toml` and `Cargo.lock`, and `tauri.conf.json`. No version changes made.
- [x] Recorded milestone discrepancies: v0.2.0 has 0.1.0 package metadata;
  v0.3.1 has 0.3.0 metadata. Kept historical records and checksums unchanged.
- [x] Prepared [versioned notes](README.md), [GitHub Release text](v0.5.0-github-release.md),
  installation/upgrade guidance, and links to verification evidence.
- [x] Confirmed [CI run 43806412518](https://github.com/ishanpati/Nexus/actions/runs/43806412518)
  passed for `4163806`: 103 frontend tests, 113 Rust tests with 2 existing ignored
  opt-in/fixture cases, formatting, Clippy with existing warnings, doctests,
  TypeScript/Vite, and an embedded debug Tauri build. This run did not build installers.
- [x] Reviewed [Stage G packaging](../verification/V05_VERIFICATION.md#production-packaging)
  and the later [maintenance packaging check](../development/REPOSITORY_MAINTENANCE.md#verification):
  both NSIS and MSI builds succeeded with normal WiX validation.
- [x] Inspected the currently available local installers: both are present and
  Authenticode reports `NotSigned`. Their SHA-256 hashes do not match the preserved
  Stage G manifest. This is a later artifact set, not approved release evidence.

### Local artifact audit

Paths are relative to the repository root. These are existing ignored outputs,
not downloadable or newly accepted assets. The current raw release executable
also differs from the historical Stage G digest.

| Inspected file | Bytes on 2026-11-16 | Matches `V05_SHA256.txt` | Signature |
| --- | ---: | --- | --- |
| `src-tauri/target/release/bundle/nsis/nexus_0.5.0_x64-setup.exe` | 2,476,386 | No | NotSigned |
| `src-tauri/target/release/bundle/msi/nexus_0.5.0_x64_en-US.msi` | 3,452,928 | No | NotSigned |
| `src-tauri/target/release/nexus.exe` | 6,927,872 | No | Not checked in this audit |

## Source, version, and test gate

- [ ] Select and record the exact publication commit; include the required
  documentation in the repository before publishing references to it.
- [ ] Review the existing `v0.5.0` tag at `797b7af` against that commit. Resolve
  the intended tag/commit relationship explicitly with the owner. Do not force-move,
  delete, recreate, or introduce a final production tag during preparation.
- [ ] Verify the six version fields, installer identity, schema/document version,
  and chosen release label again at the final commit. Do not change package
  versions merely to make a release title look consistent.
- [ ] Run CI against the final commit and record the run URL, tool versions,
  test counts, skips, and Clippy warnings. Do not substitute the historical counts.
- [ ] Confirm tests use fake lock/sleep operations and do not enable the live-audio
  opt-in. Treat deliberate real-device checks as separate owner-controlled tests.

## Build and installer gate

- [ ] Build optimized NSIS and MSI outputs from the selected commit with the
  committed lockfiles and unchanged installer identity. Record command, compiler,
  source revision, output sizes, and build result. Keep WiX validation enabled.
- [ ] Verify the production executable embeds assets and starts without Vite,
  a source checkout, API keys, or a development process.
- [ ] Verify each installer format offered to users in a disposable Windows 11 x64
  profile: install, installed shortcut launch, close/restart, and uninstall.
- [ ] Check WebView2 requirements and offline setup with WebView2 preinstalled.
  State clearly whether setup would otherwise need a download.
- [ ] Record actual signature status for the final bytes. Describe unsigned
  installers as unsigned; any later signing requires checksums of the signed files.

From the repository root, the existing packaging command is:

```powershell
npm.cmd run tauri build -- --bundles nsis,msi -- --locked --offline
```

Offline dependency/tool caches must already be populated. Preserve full logs and
read a bounded failure summary. Installer validation can require access to the
Windows Installer service outside a restricted development sandbox.

## Windows desktop acceptance gate

Record the environment, exact binary, observed result, and any failure for each
scenario in the [full Stage G checklist](../verification/V05_VERIFICATION.md#manual-checks-excluded-by-the-no-computer-control-instruction).
Earlier isolated Stage E/F GUI checks do not complete this gate.

- [ ] Global shortcut, single instance, focus/Escape, dragging/reset, multi-monitor
  placement, and supported display scaling remain usable.
- [ ] App search/icons and built-in/catalog launches work; Everything Unicode,
  literal search, open/reveal, sensitive-file cancellation, and unavailable/retry work.
- [ ] Create, save, edit, rename, reorder, filter, repair, and confirm/cancel deletion
  in Studio; native pickers and Save / Discard / Cancel preserve the intended draft.
- [ ] Final launcher-close fixes work with Studio drafts and active runs; quitting
  when idle actually exits and later restores the shortcut through a new process.
- [ ] Run the selected workflow through `w ` name/alias search and a nonfirst
  result; verify progress, first-failure stop, cancellation, duplicate-start gates,
  and hide/reopen recovery without implicit execution retries.
- [ ] Verify explicit sensitive-action denial, expiry, cancellation, and final-only
  placement. Only with deliberate owner consent, test actual lock/sleep/resume.
- [ ] Verify audio controls/overlay and device changes, restoring the original
  audio state; check supported Settings pages and hardware/policy failure behavior.
- [ ] Repeat installed startup and representative app/workflow use with internet
  disconnected. Everything remains a local optional dependency for file search.
- [ ] Measure installed startup and process-tree memory, including WebView2 children,
  idle/Studio/repeated-run behavior, and comparison with the prior version.

## Upgrade and data gate

- [ ] Back up stopped disposable test data before every upgrade/reinstall/uninstall.
  Include the complete directory and any SQLite sidecars; never test against the
  owner's production workflow data.
- [ ] Upgrade the prior installation using the same format, then reinstall 0.5.0.
  Verify installed version, workflow/step identities, parameters, order, revisions,
  database schema, and behavior after genuine process restart.
- [ ] Check the shared dev/installed path and test restoration of a stopped backup.
  Confirm missing app/file references can be repaired without losing unrelated data.
- [ ] Verify uninstall preservation when the NSIS data-removal checkbox stays
  unchecked, and its explicit deletion behavior only in a backed-up disposable profile.

## Artifact and checksum gate

- [ ] Select the accepted `nexus_0.5.0_x64-setup.exe` and/or
  `nexus_0.5.0_x64_en-US.msi` for the formats actually offered and validated.
  Exclude old bundles, debug executables, fixtures, personal paths, logs, and databases.
- [ ] Hash the exact final installer bytes after every rebuild or signing change.
  Create a new manifest with asset basenames and verify each file against it.
  Do not reuse [Stage G hashes](V05_SHA256.txt) or overwrite historical records.
- [ ] Recheck filenames, version, size, signature status, and hashes after transfer
  into the release staging area. Retain evidence tying the files to the chosen source.

After acceptance, this example writes a **new ignored** checksum manifest for the
two installer formats. It has not been run by this documentation task:

```powershell
$releaseArtifacts = @(
  'src-tauri/target/release/bundle/nsis/nexus_0.5.0_x64-setup.exe',
  'src-tauri/target/release/bundle/msi/nexus_0.5.0_x64_en-US.msi'
)
$checksumLines = foreach ($artifact in $releaseArtifacts) {
  $digest = Get-FileHash -Algorithm SHA256 -LiteralPath $artifact
  '{0}  {1}' -f $digest.Hash.ToLowerInvariant(), (Split-Path -Leaf $artifact)
}
$checksumLines | Set-Content -Encoding ascii -LiteralPath 'src-tauri/target/SHA256SUMS-0.5.0.txt'
```

## Documentation and publication gate

- [ ] Reconcile README, changelog, versioned notes, GitHub draft, exact artifact
  names, requirements, backup guidance, and verification evidence. Validate links
  on the selected revision and check that release-body links work outside the repo.
- [ ] Keep the metadata discrepancies and historical results visible. Preserve
  prior reports instead of rewriting an incomplete check as a pass.
- [ ] Review CSP, plaintext storage, signature status, operational limits, and the
  README licensing notice with the owner; record the acceptance decision.
- [ ] Fill the final commit, run/build evidence, acceptance results, exact asset
  list, checksums, and owner sign-off into the release record. Revise the draft's
  HOLD/candidate wording only when the evidence supports the new status.
- [ ] Obtain explicit owner authorization after all required acceptance gates pass.
  Only then consider GitHub Release publication and any final production tag action.
  Do not claim stable/public acceptance or attach unverified binaries in advance.
