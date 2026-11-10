use super::{
    model::{deduplicate, excluded},
    *,
};
use ::windows::{
    Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, PROPERTYKEY},
        Storage::Packaging::Appx::ParseApplicationUserModelId,
        System::{
            Com::{
                StructuredStorage::{PropVariantClear, PropVariantToStringAlloc},
                *,
            },
            Environment::ExpandEnvironmentStringsW,
        },
        UI::Shell::{
            PropertiesSystem::{IPropertyStore, PSGetPropertyKeyFromName},
            *,
        },
    },
    core::{GUID, Interface, PCWSTR, PWSTR, w},
};
use std::{
    fs,
    os::windows::fs::MetadataExt,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

pub(super) fn scan() -> Result<Catalog, String> {
    // All COM objects live and die on this dedicated thread, before Apartment.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
        .map_err(|e| format!("Could not initialize Windows application discovery: {e}"))?;
    let _apartment = Apartment;
    let mut catalog = Catalog::default();
    for (folder, source) in [
        (&FOLDERID_Programs, Source::UserStartMenu),
        (&FOLDERID_CommonPrograms, Source::CommonStartMenu),
    ] {
        let mut report = SourceReport {
            source: source.clone(),
            completed: true,
            accepted: 0,
            skipped: 0,
        };
        match known_folder(folder) {
            Ok(path) => walk(Path::new(&path), &source, &mut catalog, &mut report, 0),
            Err(e) => {
                report.completed = false;
                catalog.warnings.push(format!("{source:?}: {e}"));
            }
        }
        catalog.sources.push(report);
    }
    let mut report = SourceReport {
        source: Source::AppsFolder,
        completed: true,
        accepted: 0,
        skipped: 0,
    };
    if let Err(e) = apps_folder(&mut catalog, &mut report) {
        report.completed = false;
        catalog.warnings.push(format!("Windows AppsFolder: {e}"));
    }
    catalog.sources.push(report);
    if catalog.apps.is_empty() && catalog.sources.iter().all(|s| !s.completed) {
        return Err(format!(
            "Windows could not read any application source. {}",
            catalog.warnings.join("; ")
        ));
    }
    (catalog.apps, catalog.duplicates_removed) = deduplicate(catalog.apps);
    catalog.scanned_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    Ok(catalog)
}

fn owned_string(value: PWSTR) -> Result<String, String> {
    // Shell-returned strings use the COM allocator. Free even on invalid UTF-16.
    if value.is_null() {
        return Ok(String::new());
    }
    let result = unsafe { value.to_string() }.map_err(|e| e.to_string());
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    result
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn known_folder(id: &GUID) -> Result<String, String> {
    owned_string(
        unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.map_err(|e| e.to_string())?,
    )
}

fn walk(
    path: &Path,
    source: &Source,
    catalog: &mut Catalog,
    report: &mut SourceReport,
    depth: usize,
) {
    // Never follow junctions/symlinks out of the explicitly requested trees.
    let result = (|| -> Result<(), String> {
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if metadata.file_attributes() & 0x400 != 0 || depth > 32 {
            report.skipped += 1;
            return Ok(());
        }
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    report.completed = false;
                    catalog.warnings.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(value) => value,
                Err(e) => {
                    report.completed = false;
                    catalog.warnings.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            if metadata.file_attributes() & 0x400 != 0 {
                report.skipped += 1;
                continue;
            }
            if metadata.is_dir() {
                walk(&path, source, catalog, report, depth + 1);
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
            {
                match shortcut(&path, source.clone()) {
                    Ok(Some(app)) => {
                        report.accepted += 1;
                        catalog.apps.push(app);
                    }
                    Ok(None) => report.skipped += 1,
                    Err(e) => {
                        report.skipped += 1;
                        catalog
                            .warnings
                            .push(format!("Skipped {}: {e}", path.display()));
                    }
                }
            } else {
                report.skipped += 1;
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        report.completed = false;
        catalog.warnings.push(format!("{}: {e}", path.display()));
    }
}

fn link_text(
    read: impl FnOnce(&mut [u16]) -> ::windows::core::Result<()>,
) -> Result<String, String> {
    let mut buffer = vec![0_u16; 32768];
    read(&mut buffer).map_err(|e| e.to_string())?;
    let end = buffer
        .iter()
        .position(|c| *c == 0)
        .ok_or("Shortcut metadata is too long")?;
    if end == buffer.len() - 1 {
        return Err("Shortcut metadata may be truncated".into());
    }
    String::from_utf16(&buffer[..end]).map_err(|e| e.to_string())
}

fn expand(value: &str) -> Result<String, String> {
    let input = wide(value);
    let count = unsafe { ExpandEnvironmentStringsW(PCWSTR(input.as_ptr()), None) };
    if count == 0 || count > 32768 {
        return Err("Could not expand the shortcut target".into());
    }
    let mut output = vec![0; count as usize];
    let written = unsafe { ExpandEnvironmentStringsW(PCWSTR(input.as_ptr()), Some(&mut output)) };
    if written == 0 || written > count {
        return Err("Could not expand the shortcut target".into());
    }
    String::from_utf16(&output[..written as usize - 1]).map_err(|e| e.to_string())
}

pub(super) fn shortcut(path: &Path, source: Source) -> Result<Option<InstalledApp>, String> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid shortcut name")?
        .to_string();
    if excluded(&name, "") {
        return Ok(None);
    }
    // Canonicalize the shortcut identity, not the launch target. This merges an
    // AppsFolder reference to the same link without comparing command strings.
    let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
    let path = path
        .to_str()
        .ok_or("Shortcut path is not valid Unicode")?
        .to_string();
    let input = wide(&path);
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| e.to_string())?;
    let persist: IPersistFile = link.cast().map_err(|e| e.to_string())?;
    unsafe { persist.Load(PCWSTR(input.as_ptr()), STGM_READ) }.map_err(|e| e.to_string())?;
    // Deliberately never call Resolve: no repair, UI, search, or target execution.
    let target = link_text(|buffer| unsafe {
        link.GetPath(buffer, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
    })?;
    if target.is_empty() {
        return Ok(None);
    } // Non-filesystem/advertised links are covered only if in AppsFolder.
    let expanded = expand(&target)?;
    if excluded(&name, &expanded) {
        return Ok(None);
    }
    let target_path = Path::new(&expanded);
    if !target_path.is_absolute()
        || !target_path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("exe") || s.eq_ignore_ascii_case("com"))
    {
        return Ok(None);
    }
    // An unavailable target is omitted with a diagnostic, not silently marked launchable.
    let metadata = fs::metadata(target_path)
        .map_err(|e| format!("Target is unavailable ({expanded}): {e}"))?;
    if !metadata.is_file() {
        return Ok(None);
    }
    let arguments = link_text(|buffer| unsafe { link.GetArguments(buffer) })?;
    let working_directory = link_text(|buffer| unsafe { link.GetWorkingDirectory(buffer) })?;
    let show_command = unsafe { link.GetShowCmd() }.map_err(|e| e.to_string())?.0;
    let mut index = 0;
    let icon = link_text(|buffer| unsafe { link.GetIconLocation(buffer, &mut index) })
        .ok()
        .filter(|s| !s.is_empty())
        .map(|path| IconReference::Resource { path, index })
        .or_else(|| {
            Some(IconReference::ShellItem {
                parsing_name: path.clone(),
            })
        });
    let app_user_model_id = shortcut_app_id(&link);
    Ok(Some(InstalledApp::new(
        name,
        LaunchTarget::Shortcut {
            path,
            target,
            arguments,
            working_directory,
            show_command,
            app_user_model_id,
        },
        icon,
        source,
    )))
}

fn property(item: &IShellItem2, name: PCWSTR) -> Option<String> {
    let mut key = PROPERTYKEY::default();
    unsafe { PSGetPropertyKeyFromName(name, &mut key) }.ok()?;
    owned_string(unsafe { item.GetString(&key) }.ok()?)
        .ok()
        .filter(|s| !s.is_empty())
}

fn shortcut_app_id(link: &IShellLinkW) -> Option<String> {
    let store: IPropertyStore = link.cast().ok()?;
    let mut key = PROPERTYKEY::default();
    unsafe { PSGetPropertyKeyFromName(w!("System.AppUserModel.ID"), &mut key) }.ok()?;
    let mut value = unsafe { store.GetValue(&key) }.ok()?;
    let text = unsafe { PropVariantToStringAlloc(&value) }
        .ok()
        .and_then(|s| owned_string(s).ok());
    unsafe {
        let _ = PropVariantClear(&mut value);
    }
    text.filter(|s| !s.is_empty())
}

pub(super) fn is_packaged_id(id: &str) -> bool {
    let input = wide(id);
    let (mut family, mut relative) = (0, 0);
    unsafe {
        ParseApplicationUserModelId(
            PCWSTR(input.as_ptr()),
            &mut family,
            None,
            &mut relative,
            None,
        ) == ERROR_INSUFFICIENT_BUFFER
    }
}

fn apps_folder(catalog: &mut Catalog, report: &mut SourceReport) -> Result<(), String> {
    let folder: IShellItem =
        unsafe { SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None) }
            .map_err(|e| e.to_string())?;
    let items: IEnumShellItems =
        unsafe { folder.BindToHandler(None, &BHID_EnumItems) }.map_err(|e| e.to_string())?;
    loop {
        let mut slot = [None];
        let mut fetched = 0;
        unsafe { items.Next(&mut slot, Some(&mut fetched)) }.map_err(|e| e.to_string())?;
        if fetched == 0 {
            break;
        }
        let Some(item) = slot[0].take() else {
            return Err("Windows returned an empty application item".into());
        };
        match shell_app(&item) {
            Ok(Some(app)) => {
                report.accepted += 1;
                catalog.apps.push(app);
            }
            Ok(None) => report.skipped += 1,
            Err(e) => {
                report.skipped += 1;
                catalog
                    .warnings
                    .push(format!("Skipped AppsFolder item: {e}"));
            }
        }
    }
    Ok(())
}

fn shell_app(item: &IShellItem) -> Result<Option<InstalledApp>, String> {
    let name = owned_string(
        unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }.map_err(|e| e.to_string())?,
    )?;
    if name.trim().is_empty() || excluded(&name, "") {
        return Ok(None);
    }
    let parsing_name = owned_string(
        unsafe { item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING) }.map_err(|e| e.to_string())?,
    )?;
    if parsing_name.is_empty() {
        return Err(format!("{name} has no Shell identifier"));
    }
    let item2: IShellItem2 = item.cast().map_err(|e| e.to_string())?;
    // Use an original link when the Shell provider exposes one; otherwise keep
    // the opaque AppsFolder identifier rather than inventing an executable path.
    let link_path = property(&item2, w!("System.Link.TargetParsingPath"));
    if let Some(path) = &link_path {
        if Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
        {
            return shortcut(Path::new(path), Source::AppsFolder);
        }
        if excluded(&name, path) {
            return Ok(None);
        }
    }
    let id = property(&item2, w!("System.AppUserModel.ID"));
    if excluded(&name, &parsing_name) || non_application_path(&parsing_name) {
        return Ok(None);
    }
    let launch = if let Some(id) = id.as_ref().filter(|id| is_packaged_id(id)) {
        LaunchTarget::PackagedApp {
            app_user_model_id: id.clone(),
            parsing_name: parsing_name.clone(),
        }
    } else {
        LaunchTarget::ShellApp {
            parsing_name: parsing_name.clone(),
            app_user_model_id: id,
        }
    };
    Ok(Some(InstalledApp::new(
        name,
        launch,
        Some(IconReference::ShellItem { parsing_name }),
        Source::AppsFolder,
    )))
}

fn non_application_path(value: &str) -> bool {
    // Only interpret path-shaped identifiers, never opaque IDs such as Office's.
    value.contains('\\')
        && Path::new(value)
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "chm" | "txt" | "pdf" | "html" | "htm" | "url" | "bat" | "cmd" | "ps1" | "msc"
                )
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_validates_packaged_aumids() {
        assert!(is_packaged_id(
            "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"
        ));
        assert!(!is_packaged_id("Microsoft.Windows.Explorer"));
        assert!(!is_packaged_id(""));
        assert!(non_application_path("{folder}\\WinRAR\\WinRAR.chm"));
        assert!(!non_application_path("Microsoft.Office.WINWORD.EXE.15"));
    }

    #[test]
    fn native_shortcuts_preserve_metadata_and_skip_broken_targets() {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap() };
        let _apartment = Apartment;
        let root = std::env::temp_dir().join(format!(
            "nexus-catalog-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let create = |name: &str, target: &Path| {
            let link: IShellLinkW =
                unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.unwrap();
            let target = wide(target.to_str().unwrap());
            let cwd = wide(root.to_str().unwrap());
            let args = wide("--profile \"two words\" --literal=%TEMP%");
            let path = root.join(name);
            let output = wide(path.to_str().unwrap());
            unsafe {
                link.SetPath(PCWSTR(target.as_ptr())).unwrap();
                link.SetArguments(PCWSTR(args.as_ptr())).unwrap();
                link.SetWorkingDirectory(PCWSTR(cwd.as_ptr())).unwrap();
                link.cast::<IPersistFile>()
                    .unwrap()
                    .Save(PCWSTR(output.as_ptr()), true)
                    .unwrap();
            }
            path
        };
        let valid = create("Editor.lnk", &std::env::current_exe().unwrap());
        let app = shortcut(&valid, Source::UserStartMenu).unwrap().unwrap();
        match app.launch {
            LaunchTarget::Shortcut {
                arguments,
                working_directory,
                path,
                ..
            } => {
                assert_eq!(arguments, "--profile \"two words\" --literal=%TEMP%");
                assert_eq!(working_directory, root.to_str().unwrap());
                assert_eq!(path, fs::canonicalize(&valid).unwrap().to_str().unwrap());
            }
            _ => panic!("expected shortcut"),
        }
        let missing = create("Broken.lnk", &root.join("missing.exe"));
        assert!(
            shortcut(&missing, Source::UserStartMenu)
                .unwrap_err()
                .contains("Target is unavailable")
        );
        create("Uninstall Editor.lnk", &std::env::current_exe().unwrap());
        let mut catalog = Catalog::default();
        let mut report = SourceReport {
            source: Source::UserStartMenu,
            completed: true,
            accepted: 0,
            skipped: 0,
        };
        walk(&root, &Source::UserStartMenu, &mut catalog, &mut report, 0);
        assert_eq!(catalog.apps.len(), 1);
        assert_eq!(report.skipped, 2);
        assert_eq!(catalog.warnings.len(), 1);
        walk(
            &root.join("nonexistent"),
            &Source::UserStartMenu,
            &mut catalog,
            &mut report,
            0,
        );
        assert!(!report.completed);
        assert_eq!(catalog.apps.len(), 1); // Failure in another source preserves valid entries.
    }
}
