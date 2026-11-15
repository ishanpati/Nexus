use super::*;
use windows::Win32::{
    Media::Audio::{Endpoints::IAudioEndpointVolume, *},
    System::Com::*,
};

fn failure(error: windows::core::Error) -> AudioError {
    let code = error.code().0 as u32;
    let (kind, message) = match code {
        0x80070490 => (
            ErrorCode::NoDevice,
            "No default audio output is available. Connect or enable an output device in Windows sound settings.",
        ),
        0x88890004 | 0x88890026 => (
            ErrorCode::DeviceChanged,
            "The audio device became unavailable. Check the selected output device and retry.",
        ),
        0x88890010 | 0x800706BA => (
            ErrorCode::ServiceUnavailable,
            "Windows Audio is unavailable. Check the Windows Audio service and retry.",
        ),
        0x80070005 => (
            ErrorCode::AccessDenied,
            "Windows denied access to this audio device. Check device permissions or restrictions.",
        ),
        _ => (
            ErrorCode::NativeFailure,
            "Windows could not complete the audio request. Check the default output device and retry.",
        ),
    };
    AudioError {
        code: kind,
        message: message.into(),
        hresult: Some(format!("0x{code:08X}")),
        may_have_changed: false,
    }
}

fn device_id(device: &IMMDevice) -> Result<String> {
    unsafe {
        let id = device.GetId().map_err(failure)?;
        let text = id.to_string().map_err(|_| {
            AudioError::new(
                ErrorCode::NativeFailure,
                "Windows returned an invalid audio endpoint identifier.",
            )
        });
        CoTaskMemFree(Some(id.0.cast()));
        text
    }
}
struct WindowsEndpoint {
    enumerator: IMMDeviceEnumerator,
    volume: IAudioEndpointVolume,
    id: String,
}
impl WindowsEndpoint {
    fn connect() -> Result<Self> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
                    .map_err(failure)?;
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(failure)?;
            let id = device_id(&device)?;
            let volume = device
                .Activate(CLSCTX_INPROC_SERVER, None)
                .map_err(failure)?;
            Ok(Self {
                enumerator,
                volume,
                id,
            })
        }
    }
}
impl Endpoint for WindowsEndpoint {
    fn id(&self) -> &str {
        &self.id
    }
    fn ensure_current(&self) -> Result<()> {
        let current = unsafe {
            self.enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
        }
        .map_err(failure)?;
        if device_id(&current)? == self.id {
            Ok(())
        } else {
            Err(AudioError::new(
                ErrorCode::DeviceChanged,
                "The default audio output changed during the request. Read audio status, then retry if needed.",
            ))
        }
    }
    fn read(&self) -> Result<(f32, bool)> {
        unsafe {
            Ok((
                self.volume.GetMasterVolumeLevelScalar().map_err(failure)?,
                self.volume.GetMute().map_err(failure)?.as_bool(),
            ))
        }
    }
    fn set_volume(&self, scalar: f32) -> Result<()> {
        unsafe {
            self.volume
                .SetMasterVolumeLevelScalar(scalar, std::ptr::null())
        }
        .map_err(failure)
    }
    fn set_mute(&self, muted: bool) -> Result<()> {
        unsafe { self.volume.SetMute(muted, std::ptr::null()) }.map_err(failure)
    }
}

pub(super) fn execute(request: AudioRequest) -> Result<AudioState> {
    on_sta(move || {
        let endpoint = WindowsEndpoint::connect()?;
        apply(&endpoint, request)
    })
}

fn on_sta<T: Send + 'static>(operation: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    // A fresh thread owns all COM objects, avoiding the apartment state of pooled
    // Tauri workers. All interfaces drop before CoUninitialize on this same thread.
    std::thread::Builder::new()
        .name("nexus-audio".into())
        .spawn(move || {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(failure)?;
            struct Apartment;
            impl Drop for Apartment {
                fn drop(&mut self) {
                    unsafe { CoUninitialize() };
                }
            }
            let _apartment = Apartment;
            operation()
        })
        .map_err(|_| {
            AudioError::new(
                ErrorCode::NativeFailure,
                "Could not start the audio worker.",
            )
        })?
        .join()
        .map_err(|_| {
            AudioError::new(
                ErrorCode::NativeFailure,
                "The audio worker stopped unexpectedly.",
            )
        })?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "interactive Windows test: temporarily changes audio and restores original state"]
    fn live_audio_round_trip() {
        on_sta(|| {
            let endpoint = WindowsEndpoint::connect()?;
            let original = endpoint.read()?;
            println!(
                "Original volume={:.3}% muted={}",
                original.0 * 100.0,
                original.1
            );
            // The same endpoint is retained for restoration even if default output
            // changes. Never restore a different device by resolving it again.
            struct Restore<'a>(&'a WindowsEndpoint, (f32, bool), bool);
            impl Drop for Restore<'_> {
                fn drop(&mut self) {
                    if self.2 {
                        let volume = self
                            .0
                            .set_mute(true)
                            .and_then(|_| self.0.set_volume(self.1.0));
                        // Keep muted if restoring volume failed.
                        if volume.is_ok() {
                            let _ = self.0.set_mute(self.1.1);
                        }
                        eprintln!("Emergency audio restoration attempted after test interruption.");
                    }
                }
            }
            let mut restore = Restore(&endpoint, original, true);
            let check = |condition, label| {
                if condition {
                    Ok(())
                } else {
                    Err(AudioError::new(
                        ErrorCode::NativeFailure,
                        format!("Live check failed: {label}"),
                    ))
                }
            };
            let tested = (|| {
                let state = apply(&endpoint, AudioRequest::GetAudioState(Empty {}))?;
                check(state.muted == original.1, "read mute")?;
                check(
                    apply(&endpoint, AudioRequest::SetMute(Mute { muted: true }))?.muted,
                    "mute",
                )?;
                check(
                    apply(&endpoint, AudioRequest::SetVolume(Percent { percent: 35 }))?
                        .volume_percent
                        == 35,
                    "set 35",
                )?;
                check(
                    apply(&endpoint, AudioRequest::AdjustVolume(Delta { delta: 10 }))?
                        .volume_percent
                        == 45,
                    "increase 10",
                )?;
                check(
                    apply(&endpoint, AudioRequest::AdjustVolume(Delta { delta: -10 }))?
                        .volume_percent
                        == 35,
                    "decrease 10",
                )?;
                apply(&endpoint, AudioRequest::SetVolume(Percent { percent: 0 }))?;
                check(
                    !apply(&endpoint, AudioRequest::SetMute(Mute { muted: false }))?.muted,
                    "unmute at zero volume",
                )?;
                apply(&endpoint, AudioRequest::SetMute(Mute { muted: true }))?;
                Ok(())
            })();
            endpoint.set_mute(true)?;
            endpoint.set_volume(original.0)?;
            endpoint.set_mute(original.1)?;
            restore.2 = false;
            let restored = endpoint.read()?;
            check(
                (restored.0 - original.0).abs() < 0.001 && restored.1 == original.1,
                "restore original state",
            )?;
            println!(
                "Restored volume={:.3}% muted={}",
                restored.0 * 100.0,
                restored.1
            );
            tested
        })
        .unwrap();
    }
    #[test]
    fn native_failures_are_structured_and_actionable() {
        for (code, expected) in [
            (0x80070490u32, ErrorCode::NoDevice),
            (0x88890004, ErrorCode::DeviceChanged),
            (0x88890010, ErrorCode::ServiceUnavailable),
            (0x80070005, ErrorCode::AccessDenied),
            (0x80004005, ErrorCode::NativeFailure),
        ] {
            let error = failure(windows::core::Error::from_hresult(windows::core::HRESULT(
                code as i32,
            )));
            assert_eq!(error.code, expected);
            assert!(error.hresult.unwrap().starts_with("0x"));
            assert!(!error.message.is_empty());
        }
    }
}
