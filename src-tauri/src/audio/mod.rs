//! Closed audio operations. Search text, device paths and executable names are
//! never accepted. Each request resolves the current multimedia render endpoint.
#[cfg(windows)]
mod native;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct Percent {
    pub percent: i16,
}
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct Delta {
    pub delta: i16,
}
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct Mute {
    pub muted: bool,
}
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(
    tag = "id",
    content = "args",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AudioRequest {
    GetAudioState(Empty),
    SetVolume(Percent),
    AdjustVolume(Delta),
    SetMute(Mute),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidInput,
    NoDevice,
    DeviceChanged,
    ServiceUnavailable,
    AccessDenied,
    Busy,
    PlatformUnsupported,
    NativeFailure,
}
#[derive(Debug, Serialize)]
pub struct AudioError {
    pub code: ErrorCode,
    pub message: String,
    pub hresult: Option<String>,
    /// A setter may have succeeded before a device/readback failure. Never retry automatically.
    pub may_have_changed: bool,
}
impl AudioError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hresult: None,
            may_have_changed: false,
        }
    }
}
impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for AudioError {}
pub type Result<T> = std::result::Result<T, AudioError>;

#[derive(Debug, Serialize, Clone)]
pub struct AudioState {
    pub volume_percent: u8,
    pub muted: bool,
    pub endpoint_id: String,
}

fn validate(request: AudioRequest) -> Result<()> {
    match request {
        AudioRequest::SetVolume(Percent { percent }) if !(0..=100).contains(&percent) => {
            Err(AudioError::new(
                ErrorCode::InvalidInput,
                "Volume must be a whole number from 0 to 100.",
            ))
        }
        AudioRequest::AdjustVolume(Delta { delta }) if !(-100..=100).contains(&delta) => {
            Err(AudioError::new(
                ErrorCode::InvalidInput,
                "Volume changes must be whole percentage points from -100 to +100.",
            ))
        }
        _ => Ok(()),
    }
}

pub fn parse_request(value: serde_json::Value) -> Result<AudioRequest> {
    let request = serde_json::from_value(value).map_err(|_| {
        AudioError::new(
            ErrorCode::InvalidInput,
            "Invalid audio action or arguments. Select a valid audio command.",
        )
    })?;
    validate(request)?;
    Ok(request)
}

trait Endpoint {
    fn id(&self) -> &str;
    fn ensure_current(&self) -> Result<()>;
    fn read(&self) -> Result<(f32, bool)>;
    fn set_volume(&self, scalar: f32) -> Result<()>;
    fn set_mute(&self, muted: bool) -> Result<()>;
}

fn valid_scalar(scalar: f32) -> Result<f32> {
    if scalar.is_finite() && (0.0..=1.0).contains(&scalar) {
        Ok(scalar)
    } else {
        Err(AudioError::new(
            ErrorCode::NativeFailure,
            "The audio device returned an invalid volume. Reconnect it and retry.",
        ))
    }
}

fn apply(endpoint: &impl Endpoint, request: AudioRequest) -> Result<AudioState> {
    validate(request)?;
    endpoint.ensure_current()?;
    let mut attempted_write = false;
    let result = (|| {
        match request {
            AudioRequest::SetVolume(Percent { percent }) => {
                attempted_write = true;
                endpoint.set_volume(percent as f32 / 100.0)?;
            }
            AudioRequest::AdjustVolume(Delta { delta }) => {
                let scalar = valid_scalar(endpoint.read()?.0)?;
                endpoint.ensure_current()?;
                attempted_write = true;
                endpoint.set_volume((scalar + delta as f32 / 100.0).clamp(0.0, 1.0))?;
            }
            AudioRequest::SetMute(Mute { muted }) => {
                attempted_write = true;
                endpoint.set_mute(muted)?;
            }
            AudioRequest::GetAudioState(_) => {}
        }
        let (scalar, muted) = endpoint.read()?;
        let scalar = valid_scalar(scalar)?;
        endpoint.ensure_current()?;
        Ok(AudioState {
            volume_percent: (scalar * 100.0).round() as u8,
            muted,
            endpoint_id: endpoint.id().into(),
        })
    })();
    result.map_err(|mut error: AudioError| {
        error.may_have_changed = attempted_write;
        error
    })
}

#[derive(Default)]
pub struct AudioService(Mutex<()>);
impl AudioService {
    pub fn execute(&self, request: AudioRequest) -> Result<AudioState> {
        validate(request)?;
        let _guard = self.0.try_lock().map_err(|_| {
            AudioError::new(
                ErrorCode::Busy,
                "Another audio request is in progress. Try again shortly.",
            )
        })?;
        #[cfg(windows)]
        {
            native::execute(request)
        }
        #[cfg(not(windows))]
        {
            Err(AudioError::new(
                ErrorCode::PlatformUnsupported,
                "Audio controls require Windows.",
            ))
        }
    }
}

#[tauri::command]
pub async fn execute_audio(
    request: serde_json::Value,
    audio: tauri::State<'_, Arc<AudioService>>,
    app: tauri::AppHandle,
) -> Result<AudioState> {
    // Deserialize here so malformed requests also receive a structured error.
    let request = parse_request(request)?;
    let show_feedback = !matches!(request, AudioRequest::GetAudioState(_));
    let audio = audio.inner().clone();
    let state = tauri::async_runtime::spawn_blocking(move || audio.execute(request))
        .await
        .map_err(|_| {
            AudioError::new(
                ErrorCode::NativeFailure,
                "The audio worker stopped unexpectedly. Try again.",
            )
        })??;
    if show_feedback {
        if let Err(error) = crate::audio_feedback::show(&app, state.volume_percent, state.muted) {
            // Audio already succeeded: never turn a display failure into a retryable write.
            eprintln!("Could not show audio feedback: {error}");
        }
    }
    Ok(state)
}
