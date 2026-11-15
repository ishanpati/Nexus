use super::*;
use std::cell::Cell;

struct Fake {
    volume: Cell<f32>,
    muted: Cell<bool>,
    writes: Cell<u32>,
    changed: Cell<bool>,
    fail_read: Cell<bool>,
    fail_write: bool,
    change_on_write: bool,
}
impl Fake {
    fn new(volume: f32) -> Self {
        Self {
            volume: Cell::new(volume),
            muted: Cell::new(false),
            writes: Cell::new(0),
            changed: Cell::new(false),
            fail_read: Cell::new(false),
            fail_write: false,
            change_on_write: false,
        }
    }
}
impl Endpoint for Fake {
    fn id(&self) -> &str {
        "test-output"
    }
    fn ensure_current(&self) -> Result<()> {
        if self.changed.get() {
            Err(AudioError::new(ErrorCode::DeviceChanged, "device changed"))
        } else {
            Ok(())
        }
    }
    fn read(&self) -> Result<(f32, bool)> {
        if self.fail_read.get() {
            Err(AudioError::new(ErrorCode::NoDevice, "missing device"))
        } else {
            Ok((self.volume.get(), self.muted.get()))
        }
    }
    fn set_volume(&self, scalar: f32) -> Result<()> {
        self.writes.set(self.writes.get() + 1);
        if self.fail_write {
            return Err(AudioError::new(ErrorCode::AccessDenied, "denied"));
        }
        self.volume.set(scalar);
        if self.change_on_write {
            self.changed.set(true);
        }
        Ok(())
    }
    fn set_mute(&self, muted: bool) -> Result<()> {
        self.writes.set(self.writes.get() + 1);
        self.muted.set(muted);
        Ok(())
    }
}

#[test]
fn validates_at_boundary_before_device_access() {
    for json in [
        r#"{"id":"set_volume","args":{"percent":101}}"#,
        r#"{"id":"set_volume","args":{"percent":-1}}"#,
        r#"{"id":"set_volume","args":{"percent":3.5}}"#,
        r#"{"id":"set_volume","args":{"percent":"NaN"}}"#,
        r#"{"id":"adjust_volume","args":{"delta":-101}}"#,
        r#"{"id":"sleep","args":{}}"#,
        r#"{"id":"get_audio_state","args":{"path":"cmd.exe"}}"#,
        r#"{"id":"set_mute","args":{"muted":1}}"#,
        r#"{"id":"set_mute","args":{"muted":true},"extra":true}"#,
    ] {
        assert_eq!(
            parse_request(serde_json::from_str(json).unwrap())
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput,
            "{json}"
        );
    }
    for percent in [0, 100] {
        assert!(
            parse_request(serde_json::json!({"id":"set_volume","args":{"percent":percent}}))
                .is_ok()
        );
    }
    let fake = Fake::new(0.5);
    assert!(apply(&fake, AudioRequest::SetVolume(Percent { percent: 101 })).is_err());
    assert_eq!(fake.writes.get(), 0);
}

#[test]
fn relative_changes_clamp_and_preserve_fraction_and_mute() {
    for (initial, delta, expected) in [
        (0.95, 10, 1.0),
        (0.05, -10, 0.0),
        (0.353, 10, 0.453),
        (0.5, 0, 0.5),
    ] {
        let fake = Fake::new(initial);
        fake.muted.set(true);
        let result = apply(&fake, AudioRequest::AdjustVolume(Delta { delta })).unwrap();
        assert!((fake.volume.get() - expected).abs() < 0.00001);
        assert!(result.muted);
    }
}

#[test]
fn get_is_read_only_and_mute_does_not_change_volume() {
    let fake = Fake::new(0.355);
    let read = apply(&fake, AudioRequest::GetAudioState(Empty {})).unwrap();
    assert_eq!(read.volume_percent, 36);
    assert_eq!(fake.writes.get(), 0);
    assert!(
        apply(&fake, AudioRequest::SetMute(Mute { muted: true }))
            .unwrap()
            .muted
    );
    assert!(
        !apply(&fake, AudioRequest::SetMute(Mute { muted: false }))
            .unwrap()
            .muted
    );
    assert_eq!(fake.volume.get(), 0.355);
    assert_eq!(
        apply(&fake, AudioRequest::SetVolume(Percent { percent: 35 }))
            .unwrap()
            .volume_percent,
        35
    );
}

#[test]
fn errors_do_not_report_success_or_retry_writes() {
    let fake = Fake::new(0.5);
    fake.changed.set(true);
    let error = apply(&fake, AudioRequest::SetVolume(Percent { percent: 35 })).unwrap_err();
    assert_eq!(error.code, ErrorCode::DeviceChanged);
    assert!(!error.may_have_changed);
    assert_eq!(fake.writes.get(), 0);
    fake.changed.set(false);
    fake.fail_read.set(true);
    let error = apply(&fake, AudioRequest::SetVolume(Percent { percent: 35 })).unwrap_err();
    assert!(error.may_have_changed);
    assert_eq!(fake.writes.get(), 1);
    let mut denied = Fake::new(0.5);
    denied.fail_write = true;
    assert_eq!(
        apply(&denied, AudioRequest::SetVolume(Percent { percent: 35 }))
            .unwrap_err()
            .code,
        ErrorCode::AccessDenied
    );
    assert_eq!(denied.writes.get(), 1);
    for scalar in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
        let fake = Fake::new(scalar);
        assert!(apply(&fake, AudioRequest::AdjustVolume(Delta { delta: 10 })).is_err());
        assert_eq!(fake.writes.get(), 0);
    }
}

#[test]
fn simultaneous_request_returns_busy_without_touching_device() {
    let service = AudioService::default();
    let _held = service.0.lock().unwrap();
    assert_eq!(
        service
            .execute(AudioRequest::GetAudioState(Empty {}))
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
}

#[test]
fn default_endpoint_change_after_write_is_not_silent_success() {
    let mut fake = Fake::new(0.5);
    fake.change_on_write = true;
    let error = apply(&fake, AudioRequest::AdjustVolume(Delta { delta: 10 })).unwrap_err();
    assert_eq!(error.code, ErrorCode::DeviceChanged);
    assert!(error.may_have_changed);
    assert_eq!(fake.writes.get(), 1); // No automatic reapplication on a new endpoint.
}
