//! Studio-only selection; returns a path and never opens or executes the target.
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickerKind {
    File,
    Folder,
}

#[tauri::command]
pub async fn pick_workflow_path(
    window: tauri::WebviewWindow,
    kind: PickerKind,
) -> Result<Option<String>, String> {
    if window.label() != crate::workflow_studio::LABEL {
        return Err("Path selection belongs to Workflow Studio.".into());
    }
    #[cfg(windows)]
    {
        // The owned dialog is modal to Studio. COM objects stay on the STA worker.
        let owner = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
        tauri::async_runtime::spawn_blocking(move || {
            crate::installed_apps::native::on_sta(move || pick(owner, kind))
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = kind;
        Err("Path selection is supported on Windows only.".into())
    }
}

#[cfg(windows)]
fn pick(owner: isize, kind: PickerKind) -> Result<Option<String>, String> {
    use windows::{
        Win32::{Foundation::HWND, System::Com::*, UI::Shell::*},
        core::HRESULT,
    };
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| e.to_string())?;
        let mut options =
            FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR | FOS_DONTADDTORECENT;
        options |= match kind {
            PickerKind::File => FOS_FILEMUSTEXIST,
            PickerKind::Folder => FOS_PICKFOLDERS,
        };
        dialog.SetOptions(options).map_err(|e| e.to_string())?;
        if let Err(e) = dialog.Show(Some(HWND(owner as *mut _))) {
            if e.code() == HRESULT::from_win32(1223) {
                return Ok(None); // ERROR_CANCELLED: leave the previous selection intact.
            }
            return Err(e.to_string());
        }
        let item = dialog.GetResult().map_err(|e| e.to_string())?;
        let path = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|e| e.to_string())?;
        let text = path.to_string().map_err(|e| e.to_string());
        CoTaskMemFree(Some(path.0.cast()));
        text.map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_file_and_folder_kinds_are_accepted() {
        for kind in ["file", "folder"] {
            assert!(serde_json::from_value::<PickerKind>(serde_json::json!(kind)).is_ok());
        }
        for kind in ["save", "shell", "C:\\target", ""] {
            assert!(serde_json::from_value::<PickerKind>(serde_json::json!(kind)).is_err());
        }
    }
}
