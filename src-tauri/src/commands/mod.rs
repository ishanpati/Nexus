mod notepad;

use serde::Deserialize;
use tauri::WebviewWindow;

/// A closed, typed native registry. Each future variant must declare its own args.
#[derive(Debug, Deserialize)]
#[serde(tag = "id", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandRequest {
    OpenNotepad { args: NoArguments },
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "serde_json::Map<String, serde_json::Value>")]
pub struct NoArguments {}

impl TryFrom<serde_json::Map<String, serde_json::Value>> for NoArguments {
    type Error = &'static str;

    fn try_from(
        arguments: serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self, Self::Error> {
        if arguments.is_empty() {
            Ok(Self {})
        } else {
            Err("Open Notepad does not accept arguments.")
        }
    }
}

fn launch_and_hide(
    launch: impl FnOnce() -> Result<(), String>,
    hide: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    launch()?;
    hide().map_err(|error| {
        format!("Notepad was opened, but NEXUS could not hide. Press Escape to dismiss it. {error}")
    })
}

#[tauri::command]
pub fn execute_command(request: CommandRequest, window: WebviewWindow) -> Result<(), String> {
    match request {
        CommandRequest::OpenNotepad {
            args: NoArguments {},
        } => launch_and_hide(notepad::launch, || {
            window.hide().map_err(|error| error.to_string())
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;

    #[test]
    fn accepts_registered_id_with_empty_arguments() {
        assert!(matches!(
            serde_json::from_value::<CommandRequest>(json!({"id":"open_notepad", "args":{}})),
            Ok(CommandRequest::OpenNotepad { .. })
        ));
    }

    #[test]
    fn rejects_unregistered_ids_and_aliases_at_native_boundary() {
        for id in [
            "notepad",
            "launch notepad",
            "open-notepad",
            "open_calculator",
            "cmd.exe",
            "",
        ] {
            assert!(serde_json::from_value::<CommandRequest>(json!({"id":id,"args":{}})).is_err());
        }
    }

    #[test]
    fn rejects_user_paths_executables_and_extra_fields() {
        for request in [
            json!({"id":"open_notepad","args":{"path":"C:\\untrusted.exe"}}),
            json!({"id":"open_notepad","args":{"executable":"cmd.exe"}}),
            json!({"id":"open_notepad","args":{"arguments":["/c","echo unexpected"]}}),
            json!({"id":"open_notepad","args":{"text":"notepad & calc"}}),
            json!({"id":"open_notepad","args":{},"path":"C:\\untrusted.exe"}),
        ] {
            assert!(serde_json::from_value::<CommandRequest>(request).is_err());
        }
    }

    #[test]
    fn rejects_missing_and_malformed_arguments() {
        for request in [
            json!({"id":"open_notepad"}),
            json!({"args":{}}),
            json!({"id":"open_notepad","args":null}),
            json!({"id":"open_notepad","args":"notepad.exe"}),
            json!({"id":"open_notepad","args":[]}),
        ] {
            assert!(serde_json::from_value::<CommandRequest>(request).is_err());
        }
    }

    #[test]
    fn launch_failure_keeps_window_visible_and_preserves_error() {
        let result = launch_and_hide(
            || Err("Notepad is not installed.".into()),
            || panic!("must not hide"),
        );
        assert_eq!(result.unwrap_err(), "Notepad is not installed.");
    }

    #[test]
    fn hides_only_after_successful_launch() {
        let launched = Cell::new(false);
        let result = launch_and_hide(
            || {
                launched.set(true);
                Ok(())
            },
            || {
                assert!(launched.get());
                Ok(())
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn distinguishes_hide_failure_from_launch_failure() {
        let error = launch_and_hide(|| Ok(()), || Err("window unavailable".into())).unwrap_err();
        assert!(error.contains("Notepad was opened"));
        assert!(error.contains("window unavailable"));
    }
}
