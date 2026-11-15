use super::*;
use crate::audio::{AudioRequest, Delta, Mute, Percent};

fn no_reference_check(_: &Arguments, _: &mut dyn ActionRuntime) -> Result<bool> {
    Ok(false)
}
fn app_check(args: &Arguments, runtime: &mut dyn ActionRuntime) -> Result<bool> {
    runtime.check_application(args["application_id"].as_str().unwrap())?;
    Ok(false)
}
fn file_check(args: &Arguments, runtime: &mut dyn ActionRuntime) -> Result<bool> {
    runtime.inspect_path(args["path"].as_str().unwrap(), false)
}
fn folder_check(args: &Arguments, runtime: &mut dyn ActionRuntime) -> Result<bool> {
    runtime.inspect_path(args["path"].as_str().unwrap(), true)
}

fn entry(
    id: &str,
    name: &str,
    description: &str,
    category: &str,
    icon: Icon,
    parameters: Vec<Parameter>,
    confirmation: ConfirmationPolicy,
    terminal: bool,
    validate: Validator,
    execute: Executor,
) -> Registration {
    Registration {
        descriptor: ActionDescriptor {
            id: id.into(),
            display_name: name.into(),
            description: description.into(),
            category: category.into(),
            icon,
            parameters,
            workflow_compatible: true,
            confirmation,
            terminal,
            schema_version: 1,
        },
        validate,
        execute,
    }
}
pub(super) fn registrations() -> Vec<Registration> {
    use ConfirmationPolicy::*;
    use ParameterType::*;
    vec![
        entry(
            "app.launch",
            "Launch application",
            "Launch an application selected from the installed-app catalog.",
            "Applications",
            Icon::Application,
            vec![Parameter::required(
                "application_id",
                "Application",
                Application,
            )],
            None,
            false,
            app_check,
            |args, runtime, _| runtime.launch_application(args["application_id"].as_str().unwrap()),
        ),
        entry(
            "file.open",
            "Open file",
            "Open a selected file using its Windows association; sensitive files need confirmation.",
            "Files",
            Icon::File,
            vec![Parameter::required("path", "File", File)],
            SensitiveTarget,
            false,
            file_check,
            |args, runtime, authorization| {
                runtime.open_path(
                    args["path"].as_str().unwrap(),
                    false,
                    authorization.is_confirmed(),
                )
            },
        ),
        entry(
            "folder.open",
            "Open folder",
            "Open a selected folder in File Explorer.",
            "Files",
            Icon::Folder,
            vec![Parameter::required("path", "Folder", Folder)],
            SensitiveTarget,
            false,
            folder_check,
            |args, runtime, authorization| {
                runtime.open_path(
                    args["path"].as_str().unwrap(),
                    true,
                    authorization.is_confirmed(),
                )
            },
        ),
        entry(
            "audio.set_volume",
            "Set volume",
            "Set the current default output volume to a percentage.",
            "Audio",
            Icon::Volume,
            vec![Parameter::required(
                "percent",
                "Volume (%)",
                Number {
                    min: 0.0,
                    max: 100.0,
                    integer: true,
                },
            )],
            None,
            false,
            no_reference_check,
            |args, runtime, _| {
                runtime.audio(AudioRequest::SetVolume(Percent {
                    percent: args["percent"].as_i64().unwrap() as i16,
                }))
            },
        ),
        entry(
            "audio.adjust_volume",
            "Adjust volume",
            "Adjust the default output by percentage points, clamped to 0–100%.",
            "Audio",
            Icon::Volume,
            vec![Parameter::required(
                "delta",
                "Change (percentage points)",
                Number {
                    min: -100.0,
                    max: 100.0,
                    integer: true,
                },
            )],
            None,
            false,
            no_reference_check,
            |args, runtime, _| {
                runtime.audio(AudioRequest::AdjustVolume(Delta {
                    delta: args["delta"].as_i64().unwrap() as i16,
                }))
            },
        ),
        entry(
            "audio.set_mute",
            "Set mute state",
            "Mute or unmute the default output without changing its volume.",
            "Audio",
            Icon::Volume,
            vec![Parameter::required("muted", "Muted", Boolean)],
            None,
            false,
            no_reference_check,
            |args, runtime, _| {
                runtime.audio(AudioRequest::SetMute(Mute {
                    muted: args["muted"].as_bool().unwrap(),
                }))
            },
        ),
        entry(
            "settings.open",
            "Open Windows Settings",
            "Open one of the supported Windows Settings pages.",
            "Windows",
            Icon::Settings,
            vec![Parameter::required(
                "page",
                "Settings page",
                Choice {
                    options: ["sound", "wifi", "bluetooth", "display"]
                        .map(String::from)
                        .to_vec(),
                },
            )],
            None,
            false,
            no_reference_check,
            |args, runtime, _| runtime.settings(args["page"].as_str().unwrap()),
        ),
        entry(
            "system.lock",
            "Lock Windows",
            "Lock the session. Requires confirmation and must be the final step.",
            "Windows",
            Icon::Lock,
            vec![],
            Always,
            true,
            no_reference_check,
            |_, runtime, authorization| {
                if !authorization.is_confirmed() {
                    return Err(RegistryError::new(
                        ErrorCode::ConfirmationRequired,
                        "Confirm locking Windows.",
                    ));
                }
                runtime.lock()
            },
        ),
        entry(
            "system.sleep",
            "Sleep",
            "Suspend the computer. Requires confirmation and must be the final step.",
            "Windows",
            Icon::Moon,
            vec![],
            Always,
            true,
            no_reference_check,
            |_, runtime, authorization| {
                if !authorization.is_confirmed() {
                    return Err(RegistryError::new(
                        ErrorCode::ConfirmationRequired,
                        "Confirm putting the computer to sleep.",
                    ));
                }
                runtime.sleep()
            },
        ),
    ]
}
