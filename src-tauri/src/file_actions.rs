//! Only filesystem items are accepted. No shell command strings, URL handlers,
//! command-line arguments, or arbitrary verbs cross this boundary.
use crate::installed_apps::launching::LaunchActivity;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileAction {
    Open,
    Reveal,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRequest {
    path: String,
    action: FileAction,
    #[serde(default)]
    confirmed: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FileOutcome {
    ConfirmationRequired { path: String, message: String },
    Opened { warning: Option<String> },
}

fn validate_path(path: &str) -> Result<(), String> {
    let bytes = path.as_bytes();
    let drive =
        bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\';
    let unc = path.starts_with("\\\\")
        && !path.starts_with("\\\\?\\")
        && !path.starts_with("\\\\.\\")
        && path[2..].split('\\').filter(|s| !s.is_empty()).count() >= 2;
    if (!drive && !unc)
        || path.encode_utf16().count() >= 32768
        || path.chars().any(char::is_control)
        || path.contains(['"', '<', '>', '|', '*', '?', '/'])
        || path[if drive { 2 } else { 0 }..].contains(':')
        || path.split('\\').any(|part| {
            part == "."
                || part == ".."
                || part.ends_with(' ')
                || part.ends_with('.')
                || reserved_device(part)
        })
    {
        return Err("Select a valid absolute filesystem path. URLs, device paths, alternate streams, and relative paths cannot be opened.".into());
    }
    Ok(())
}

// Win32 treats these as devices even with an extension (for example NUL.txt).
fn reserved_device(part: &str) -> bool {
    let stem = part
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}

fn io_error(error: std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => {
            "This item no longer exists. Search again to refresh the results.".into()
        }
        std::io::ErrorKind::PermissionDenied => {
            "Access to this item was denied. Check its permissions.".into()
        }
        _ => format!(
            "Could not access this item. Check that its drive or network share is available. {error}"
        ),
    }
}

// Unknown formats are sensitive too; a denylist alone misses custom script
// associations. This is a convenience policy, not a malware/content scanner.
fn sensitive(path: &Path, is_directory: bool) -> bool {
    if is_directory {
        return false;
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    !matches!(
        ext.as_str(),
        "txt"
            | "md"
            | "log"
            | "csv"
            | "json"
            | "xml"
            | "pdf"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "tif"
            | "tiff"
            | "mp3"
            | "wav"
            | "flac"
            | "ogg"
            | "mp4"
            | "mkv"
            | "mov"
            | "avi"
    )
}

fn resolve(path: &str) -> Result<(PathBuf, bool), String> {
    validate_path(path)?;
    let canonical = std::fs::canonicalize(path).map_err(io_error)?;
    let metadata = std::fs::metadata(&canonical).map_err(io_error)?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err("This item is not a regular file or folder.".into());
    }
    Ok((canonical, metadata.is_dir()))
}

fn perform(
    request: FileRequest,
    open: impl FnOnce(&Path, bool, FileAction) -> Result<Option<String>, String>,
) -> Result<FileOutcome, String> {
    let (resolved, directory) = resolve(&request.path)?;
    if request.action == FileAction::Open
        && !request.confirmed
        && (sensitive(Path::new(&request.path), directory) || sensitive(&resolved, directory))
    {
        return Ok(FileOutcome::ConfirmationRequired { path: request.path,
            message: "This file may run code, install software, or follow a shortcut. Open it only if you trust it.".into() });
    }
    // Revalidate on every confirmed request, not only when displaying the prompt.
    open(&resolved, directory, request.action).map(|warning| FileOutcome::Opened { warning })
}

#[tauri::command]
pub async fn open_file_result(
    request: FileRequest,
    activity: tauri::State<'_, Arc<LaunchActivity>>,
    window: tauri::WebviewWindow,
) -> Result<FileOutcome, String> {
    let _guard = activity.inner().acquire()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            crate::installed_apps::native::on_sta(move || perform(request, native_open))
        }
        #[cfg(not(windows))]
        {
            let _ = request;
            Err("Opening files is supported on Windows only.".into())
        }
    })
    .await
    .map_err(|e| format!("Could not complete the file action: {e}"))??;
    Ok(finish(result, || window.hide().map_err(|e| e.to_string())))
}

fn finish(result: FileOutcome, hide: impl FnOnce() -> Result<(), String>) -> FileOutcome {
    match result {
        FileOutcome::Opened { warning } => FileOutcome::Opened {
            warning: match hide() {
                Ok(()) => warning,
                Err(error) => Some(format!(
                    "The item opened, but NEXUS could not hide. Press Escape; do not open it again. {error} {}",
                    warning.unwrap_or_default()
                )),
            },
        },
        other => other,
    }
}

fn shell_error(code: u32) -> String {
    match code {
        2 | 3 => "This item no longer exists. Search again.".into(),
        5 => "Windows denied access to this item.".into(),
        1155 | 31 => "No default application is associated with this file type. Choose one in Windows Settings.".into(),
        1223 => "Opening was cancelled in Windows.".into(),
        _ => format!("Windows could not open this item (error {code}). Check its association, permissions, and availability."),
    }
}

#[cfg(windows)]
fn native_open(path: &Path, directory: bool, action: FileAction) -> Result<Option<String>, String> {
    use windows::{
        Win32::{
            System::Com::CoTaskMemFree,
            UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
        },
        core::{PCWSTR, w},
    };
    let path_text = path.to_str().ok_or("The path is not valid Unicode.")?;
    let path_text = if let Some(rest) = path_text.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}")
    } else {
        path_text
            .strip_prefix("\\\\?\\")
            .unwrap_or(path_text)
            .to_string()
    };
    let wide: Vec<u16> = path_text.encode_utf16().chain(Some(0)).collect();
    let shell_result = |error: windows::core::Error| shell_error(error.code().0 as u32 & 0xffff);
    if action == FileAction::Reveal || directory {
        let mut pidl = std::ptr::null_mut();
        unsafe { SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None) }
            .map_err(shell_result)?;
        // With no child list, the absolute item is selected in its parent.
        // For opening a directory, use the explicit explore verb.
        if action == FileAction::Reveal {
            let result = unsafe { SHOpenFolderAndSelectItems(pidl, None, 0) };
            unsafe { CoTaskMemFree(Some(pidl.cast())) };
            return result.map(|_| None).map_err(shell_result);
        }
        unsafe { CoTaskMemFree(Some(pidl.cast())) };
    }
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: if directory { w!("explore") } else { w!("open") },
        lpFile: PCWSTR(wide.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(shell_result)?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_non_filesystem_and_ambiguous_paths() {
        for path in [
            "",
            "file.txt",
            "C:relative.txt",
            "https://host/file",
            "shell:AppsFolder",
            r"\\.\pipe\x",
            r"\\?\C:\a",
            r"C:\a.txt:payload.exe",
            "C:\\a\0.txt",
            r"C:\..\a",
            "C:\\a.exe ",
            r"C:\folder\NUL.txt",
            r"C:\folder\COM1",
            r"C:\folder\lpt².log",
        ] {
            assert!(validate_path(path).is_err(), "{path}");
        }
        for path in [
            r"C:\Docs\résumé 日本語.txt",
            r"\\server\share\folder\",
            r"C:\a & b.txt",
        ] {
            assert!(validate_path(path).is_ok());
        }
    }
    #[test]
    fn sensitive_types_and_unknown_formats_require_confirmation() {
        for name in [
            "a.EXE",
            "a.bat",
            "a.cmd",
            "a.ps1",
            "a.lnk",
            "a.url",
            "a.js",
            "a.vbs",
            "a.hta",
            "a.msi",
            "a.docm",
            "a.unknown",
            "noextension",
        ] {
            assert!(sensitive(Path::new(name), false));
        }
        assert!(!sensitive(Path::new("report.txt"), false));
        assert!(!sensitive(Path::new("folder.exe"), true));
    }
    #[test]
    fn missing_path_fails_without_opening_and_errors_are_clear() {
        let path =
            std::env::temp_dir().join(format!("nexus-missing-{}-file.txt", std::process::id()));
        assert!(!path.exists());
        assert!(
            perform(
                FileRequest {
                    path: path.to_string_lossy().into(),
                    action: FileAction::Open,
                    confirmed: false
                },
                |_, _, _| panic!("must not open")
            )
            .unwrap_err()
            .contains("no longer exists")
        );
        assert!(shell_error(5).contains("denied"));
        assert!(shell_error(1155).contains("default application"));
        assert!(shell_error(1223).contains("cancelled"));
    }
    #[test]
    fn confirmation_and_failure_never_hide_and_hide_failure_is_not_launch_failure() {
        let pending = FileOutcome::ConfirmationRequired {
            path: "x".into(),
            message: "confirm".into(),
        };
        assert!(matches!(
            finish(pending, || panic!("must stay visible")),
            FileOutcome::ConfirmationRequired { .. }
        ));
        assert!(matches!(
            finish(FileOutcome::Opened { warning: None }, || Err(
                "failed".into()
            )),
            FileOutcome::Opened { warning: Some(_) }
        ));
    }
    #[test]
    fn confirmation_is_enforced_at_native_boundary() {
        let path = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let request = |confirmed, action| FileRequest {
            path: path.clone(),
            action,
            confirmed,
        };
        assert!(matches!(
            perform(request(false, FileAction::Open), |_, _, _| panic!(
                "no consent"
            ))
            .unwrap(),
            FileOutcome::ConfirmationRequired { .. }
        ));
        assert!(matches!(
            perform(request(true, FileAction::Open), |_, _, _| Ok(None)).unwrap(),
            FileOutcome::Opened { .. }
        ));
        assert!(matches!(
            perform(request(false, FileAction::Reveal), |_, _, action| {
                assert_eq!(action, FileAction::Reveal);
                Ok(None)
            })
            .unwrap(),
            FileOutcome::Opened { .. }
        ));
        assert_eq!(
            perform(request(true, FileAction::Open), |_, _, _| Err(
                "denied".into()
            ))
            .unwrap_err(),
            "denied"
        );
        assert!(
            serde_json::from_str::<FileRequest>(r#"{"path":"x","action":"open","args":"/c calc"}"#)
                .is_err()
        );
    }
}
