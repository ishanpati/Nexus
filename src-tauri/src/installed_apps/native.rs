use super::{
    InstalledApp, LaunchTarget, Source,
    icons::AppIcon,
    launching::{valid_text, validate},
};
use windows::{
    Win32::{
        Foundation::SIZE,
        Graphics::Gdi::*,
        System::Com::*,
        UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
    },
    core::{Interface, PCWSTR, w},
};

pub(super) fn on_sta<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    std::thread::Builder::new()
        .name("nexus-shell".into())
        .spawn(move || {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
                .map_err(|e| e.to_string())?;
            struct Apartment;
            impl Drop for Apartment {
                fn drop(&mut self) {
                    unsafe { CoUninitialize() };
                }
            }
            let _apartment = Apartment;
            operation()
        })
        .map_err(|e| format!("Could not start Windows Shell work: {e}"))?
        .join()
        .map_err(|_| "Windows Shell work stopped unexpectedly.".to_string())?
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn shell_path(path: &str) -> String {
    if let Some(tail) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{tail}")
    } else {
        path.strip_prefix("\\\\?\\").unwrap_or(path).to_string()
    }
}

// Resolve only an exact child of the current user's AppsFolder. Never interpret
// a catalog identifier as a filesystem path, URI, or free-form shell command.
fn registered_item(identifier: &str) -> Result<IShellItem, String> {
    if !valid_text(identifier) {
        return Err("The Windows application identifier is invalid.".into());
    }
    let folder: IShellItem =
        unsafe { SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) }
            .map_err(|e| e.to_string())?;
    let entries: IEnumShellItems =
        unsafe { folder.BindToHandler(None, &BHID_EnumItems) }.map_err(|e| e.to_string())?;
    loop {
        let mut slot = [None];
        let mut fetched = 0;
        unsafe { entries.Next(&mut slot, Some(&mut fetched)) }.map_err(|e| e.to_string())?;
        if fetched == 0 {
            break;
        }
        if let Some(item) = slot[0].take() {
            if let Ok(name) = unsafe { item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING) } {
                let text = unsafe { name.to_string() };
                unsafe { CoTaskMemFree(Some(name.0.cast())) };
                if text.is_ok_and(|text| text == identifier) {
                    return Ok(item);
                }
            }
        }
    }
    Err("This application is no longer registered with Windows. Refresh apps and try again.".into())
}

pub(super) fn launch(app: &InstalledApp) -> Result<(), String> {
    validate(app)?;
    let result = match &app.launch {
        LaunchTarget::Shortcut { path, .. } => {
            // Re-read the link to reject removed/broken/uninstaller entries. The
            // original link remains authoritative; we do not rebuild its args.
            super::windows::shortcut(std::path::Path::new(path), Source::UserStartMenu)?.ok_or(
                "This shortcut no longer points to a supported application. Refresh apps.",
            )?;
            let path = wide(&shell_path(path));
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
                lpFile: PCWSTR(path.as_ptr()),
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            unsafe { ShellExecuteExW(&mut info) }.map_err(|e| e.to_string())
        }
        LaunchTarget::PackagedApp {
            app_user_model_id, ..
        } => {
            if !super::windows::is_packaged_id(app_user_model_id) {
                return Err("Invalid packaged application ID. Refresh apps.".into());
            }
            let _registered = registered_item(app_user_model_id)?;
            let activation: IApplicationActivationManager = unsafe {
                CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)
            }
            .map_err(|e| e.to_string())?;
            let id = wide(app_user_model_id);
            unsafe { activation.ActivateApplication(PCWSTR(id.as_ptr()), w!(""), AO_NONE) }
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        LaunchTarget::ShellApp { parsing_name, .. } => {
            let item = registered_item(parsing_name)?;
            let pidl = unsafe { SHGetIDListFromObject(&item) }.map_err(|e| e.to_string())?;
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_INVOKEIDLIST | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
                lpIDList: pidl.cast(),
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            let result = unsafe { ShellExecuteExW(&mut info) }.map_err(|e| e.to_string());
            unsafe { CoTaskMemFree(Some(pidl.cast())) };
            result
        }
    };
    result.map_err(|e| format!("Windows could not open {}. It may have been removed, access may be denied, or opening was cancelled. Refresh apps and try again. {e}", app.display_name))
}

pub(super) fn icon(app: &InstalledApp) -> Result<AppIcon, String> {
    validate(app)?;
    let item: IShellItem = match &app.launch {
        LaunchTarget::Shortcut { path, .. } => {
            let path = wide(&shell_path(path));
            unsafe { SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None) }
                .map_err(|e| e.to_string())?
        }
        LaunchTarget::PackagedApp {
            app_user_model_id, ..
        } => registered_item(app_user_model_id)?,
        LaunchTarget::ShellApp { parsing_name, .. } => registered_item(parsing_name)?,
    };
    let factory: IShellItemImageFactory = item.cast().map_err(|e| e.to_string())?;
    let bitmap = unsafe { factory.GetImage(SIZE { cx: 48, cy: 48 }, SIIGBF_ICONONLY) }
        .map_err(|e| e.to_string())?;
    struct Bitmap(HBITMAP);
    impl Drop for Bitmap {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteObject(self.0.into());
            }
        }
    }
    let bitmap = Bitmap(bitmap);
    let mut details = BITMAP::default();
    if unsafe {
        GetObjectW(
            bitmap.0.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut details as *mut BITMAP).cast()),
        )
    } == 0
    {
        return Err("Windows could not read icon dimensions.".into());
    }
    let (width, height) = (details.bmWidth, details.bmHeight);
    if !(1..=256).contains(&width) || !(1..=256).contains(&height) {
        return Err("Invalid application icon dimensions.".into());
    }
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.0.is_null() {
        return Err("Windows could not create an icon image context.".into());
    }
    struct Context(HDC);
    impl Drop for Context {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteDC(self.0);
            }
        }
    }
    let dc = Context(dc);
    let mut pixels = vec![0_u8; width as usize * height as usize * 4];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    if unsafe {
        GetDIBits(
            dc.0,
            bitmap.0,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        )
    } != height
    {
        return Err("Windows could not copy the application icon.".into());
    }
    convert_pixels(&mut pixels);
    Ok(AppIcon {
        width: width as u32,
        height: height as u32,
        rgba: pixels,
    })
}

fn convert_pixels(pixels: &mut [u8]) {
    // Shell images use premultiplied BGRA. Canvas ImageData expects straight RGBA.
    let has_alpha = pixels.chunks_exact(4).any(|p| p[3] != 0);
    for p in pixels.chunks_exact_mut(4) {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        } else if p[3] > 0 && p[3] < 255 {
            for c in 0..3 {
                p[c] = ((p[c] as u32 * 255 + p[3] as u32 / 2) / p[3] as u32).min(255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converts_icon_channels_and_alpha() {
        let mut pixels = [10, 20, 30, 255, 10, 20, 30, 128];
        convert_pixels(&mut pixels);
        assert_eq!(pixels, [30, 20, 10, 255, 60, 40, 20, 128]);
        let mut opaque = [10, 20, 30, 0];
        convert_pixels(&mut opaque);
        assert_eq!(opaque, [30, 20, 10, 255]);
    }
    #[test]
    fn shell_paths_preserve_unc_and_spaces() {
        assert_eq!(
            shell_path(r"\\?\C:\Program Files\App.lnk"),
            r"C:\Program Files\App.lnk"
        );
        assert_eq!(
            shell_path(r"\\?\UNC\server\apps\App.lnk"),
            r"\\server\apps\App.lnk"
        );
    }

    #[test]
    fn missing_shortcut_and_malformed_package_fail_before_activation() {
        let missing =
            std::env::temp_dir().join(format!("nexus-missing-{}.lnk", std::process::id()));
        assert!(!missing.exists());
        let app = InstalledApp::new(
            "Missing app".into(),
            LaunchTarget::Shortcut {
                path: missing.to_string_lossy().into_owned(),
                target: "C:\\missing.exe".into(),
                arguments: "".into(),
                working_directory: "".into(),
                show_command: 1,
                app_user_model_id: None,
            },
            None,
            Source::UserStartMenu,
        );
        assert!(on_sta(move || launch(&app)).is_err());
        let app = InstalledApp::new(
            "Invalid package".into(),
            LaunchTarget::PackagedApp {
                app_user_model_id: "not a packaged id".into(),
                parsing_name: "invalid".into(),
            },
            None,
            Source::AppsFolder,
        );
        assert!(
            on_sta(move || launch(&app))
                .unwrap_err()
                .contains("Invalid packaged application ID")
        );
    }
}
