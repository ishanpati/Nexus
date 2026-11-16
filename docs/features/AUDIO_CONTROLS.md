# Native audio controls — v0.4 development

Execution lifecycle update: successful audio actions now clear the input and hide
NEXUS like other commands. Failed actions retain the query and display the native
error. This supersedes the original in-place feedback behavior described below.

The existing parser/registry produces `set_volume`, `adjust_volume`, `set_mute`, and
`get_audio_state` action objects. Only explicit Enter/click on a ready audio result
calls `execute_audio`. Typing, ranking and suggestion construction make no audio
calls. `volume status`, `vol status`, `audio status`, and `mute status` read both
volume and mute. Settings/lock/sleep now have separate handlers described in
[SYSTEM_ACTIONS.md](SYSTEM_ACTIONS.md).

## Native implementation

`src-tauri/src/audio/` uses the existing windows 0.62.2 binding with Audio and
Audio_Endpoints features. Rust deserializes a closed tagged enum, rejects extra
fields, fractional/non-numeric/out-of-range values, and validates before COM/device
access. Absolute percentages are 0–100; signed changes are -100–100 points. Up/down
already parse as +10/-10. Relative operations clamp the resulting scalar to 0–1,
without rounding the current value first. Displayed percentages are rounded.

Each request uses a fresh COM STA thread, `IMMDeviceEnumerator`,
`GetDefaultAudioEndpoint(eRender, eMultimedia)`, and `IAudioEndpointVolume`.
Interfaces are released before balanced `CoUninitialize` on that same thread.
No endpoint interface is cached between requests. NEXUS serializes its own audio
operations; an overlapping request receives a structured busy error.

Volume changes preserve mute; mute/unmute preserves volume. After acting, NEXUS
reads the endpoint back and returns `{ volume_percent, muted, endpoint_id }`.
Errors contain `{ code, message, hresult, may_have_changed }`. Codes distinguish
invalid input, no device, device changes, audio-service unavailability, access
denied, busy, unsupported platform and other native failures. If a setter was
attempted before an error, the frontend advises reading status before retrying.
Relative adjustments are never automatically retried on another endpoint.

The current default ID is checked before/after the operation (and again after the
read for a relative change). Windows does not offer an atomic “change the current
default only if it is still default” operation: a change in the narrow interval
around a setter may affect the previous endpoint. Such detected changes return an
error with possible-write status. Other programs can also change volume concurrently.
Device switches/disconnections are not prevented, and normal status is a snapshot,
not a live subscription. The next request resolves the new default.

No administrator privileges, shell command, helper executable, keyboard simulation,
network service, or new frontend permissions are required. The read-only development
example and opt-in test are diagnostics, not runtime dependencies.

## Verification

Executed on 2026-11-15:

- **53 frontend tests passed**, including explicit-activation-only audio transport,
  status aliases, invalid/non-audio request rejection, structured error feedback,
  and existing application/file search/parser tests.
- TypeScript and Vite production compilation passed.
- Final full Rust suite: **66 passed, 0 failed, 2 ignored**. The opt-in live test
  passed separately; the other ignored test is the timeout subprocess fixture.
  The native desktop executable and audio-status example compiled successfully
  with locked, offline dependencies.
- The native read-only diagnostic returned the actual default endpoint state.
- The opt-in live Core Audio test **passed**: read state, mute, set 35%, +10 to 45%,
  -10 to 35%, set zero, unmute at zero, and restore. Original and verified restored
  state were **100%, unmuted**. Other volume changes ran muted.
- Automated backend tests exercise validation before device access, clamping,
  fractional current scalar handling, independent mute/volume behavior, invalid
  device values, HRESULT mapping, busy requests, and endpoint-change/readback errors
  without duplicate writes.

The live test is ignored in the normal suite because it temporarily changes system
audio. The existing ignored sleeping-child fixture is exercised by its timeout test.

```powershell
cargo run --locked --offline --manifest-path src-tauri/Cargo.toml --example audio_status
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --all-targets
# Opt-in: briefly changes audio and restores the original endpoint's state.
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --lib audio::native::tests::live_audio_round_trip -- --exact --ignored --nocapture
```

Manual command-bar interaction, live device switching/disconnection, missing-device
hardware, stopped Windows Audio service, and full GUI regression were **not**
manually verified. Those failure paths are covered by simulated endpoint/error
tests; this is distinct from physically disconnecting hardware. No release installer
was created.

API reference: [Microsoft IAudioEndpointVolume](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume).
