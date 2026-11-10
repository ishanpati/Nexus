use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    UserStartMenu,
    CommonStartMenu,
    AppsFolder,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum LaunchTarget {
    // The shortcut itself remains authoritative, including additional Shell
    // metadata not represented here. Never rebuild its command from these fields.
    Shortcut {
        path: String,
        target: String,
        arguments: String,
        working_directory: String,
        show_command: i32,
        app_user_model_id: Option<String>,
    },
    PackagedApp {
        app_user_model_id: String,
        parsing_name: String,
    },
    ShellApp {
        parsing_name: String,
        app_user_model_id: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IconReference {
    Resource { path: String, index: i32 },
    ShellItem { parsing_name: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InstalledApp {
    pub id: String,
    pub display_name: String,
    pub launch: LaunchTarget,
    pub alternate_launches: Vec<LaunchTarget>,
    pub icon: Option<IconReference>,
    pub sources: Vec<Source>,
}

impl InstalledApp {
    pub(super) fn new(
        display_name: String,
        launch: LaunchTarget,
        icon: Option<IconReference>,
        source: Source,
    ) -> Self {
        // Full, namespaced native identity: deterministic and collision-free,
        // unlike a process-random hash. Do not use labels or executable paths:
        // two shortcuts to one EXE can have different arguments/elevation/etc.
        let id = match &launch {
            LaunchTarget::Shortcut { path, .. } => format!("shortcut:{path}"),
            LaunchTarget::PackagedApp {
                app_user_model_id, ..
            } => format!("packaged:{app_user_model_id}"),
            LaunchTarget::ShellApp { parsing_name, .. } => format!("shell:{parsing_name}"),
        };
        Self {
            id,
            display_name,
            launch,
            alternate_launches: Vec::new(),
            icon,
            sources: vec![source],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Catalog {
    pub apps: Vec<InstalledApp>,
    pub generation: u64,
    pub scanned_at_unix_ms: u64,
    pub warnings: Vec<String>,
    pub sources: Vec<SourceReport>,
    pub duplicates_removed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceReport {
    pub source: Source,
    pub completed: bool,
    pub accepted: usize,
    pub skipped: usize,
}

pub(super) fn deduplicate(apps: Vec<InstalledApp>) -> (Vec<InstalledApp>, usize) {
    let count = apps.len();
    let mut unique: BTreeMap<String, InstalledApp> = BTreeMap::new();
    for app in apps {
        if let Some(existing) = unique.get_mut(&app.id) {
            for source in app.sources {
                if !existing.sources.contains(&source) {
                    existing.sources.push(source);
                }
            }
            if existing.icon.is_none() {
                existing.icon = app.icon;
            }
        } else {
            unique.insert(app.id.clone(), app);
        }
    }
    // A Shell entry and exactly one shortcut with the same explicit Windows
    // AppUserModelID represent the same application. Preserve both launch forms.
    // Multiple shortcuts sharing an ID are ambiguous (profiles, switches, etc.).
    let mut shortcuts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for app in unique.values() {
        if let LaunchTarget::Shortcut {
            app_user_model_id: Some(id),
            ..
        } = &app.launch
        {
            shortcuts
                .entry(id.clone())
                .or_default()
                .push(app.id.clone());
        }
    }
    let bridges: Vec<_> = unique
        .values()
        .filter_map(|app| {
            let id = match &app.launch {
                LaunchTarget::ShellApp {
                    app_user_model_id: Some(id),
                    ..
                } => id,
                LaunchTarget::PackagedApp {
                    app_user_model_id, ..
                } => app_user_model_id,
                _ => return None,
            };
            let matches = shortcuts.get(id)?;
            (matches.len() == 1).then(|| (app.id.clone(), matches[0].clone()))
        })
        .collect();
    for (shell_id, shortcut_id) in bridges {
        if let Some(shell) = unique.remove(&shell_id) {
            let shortcut = unique
                .get_mut(&shortcut_id)
                .expect("indexed shortcut remains present");
            shortcut.alternate_launches.push(shell.launch);
            for source in shell.sources {
                if !shortcut.sources.contains(&source) {
                    shortcut.sources.push(source);
                }
            }
        }
    }
    let mut apps: Vec<_> = unique.into_values().collect();
    apps.sort_by_cached_key(|a| (a.display_name.to_lowercase(), a.id.clone()));
    let removed = count - apps.len();
    (apps, removed)
}

pub(super) fn excluded(name: &str, target: &str) -> bool {
    let name = name.trim().to_lowercase();
    let file = target
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(target)
        .to_ascii_lowercase();
    name == "uninstall"
        || name.starts_with("uninstall ")
        || name.starts_with("uninstall-")
        || matches!(
            file.as_str(),
            "uninstall.exe" | "uninst.exe" | "crashpad_handler.exe"
        )
        || file
            .strip_prefix("unins")
            .and_then(|s| s.strip_suffix(".exe"))
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app(path: &str, args: &str, source: Source) -> InstalledApp {
        InstalledApp::new(
            "Editor".into(),
            LaunchTarget::Shortcut {
                path: path.into(),
                target: "C:\\editor.exe".into(),
                arguments: args.into(),
                working_directory: "C:\\".into(),
                show_command: 1,
                app_user_model_id: None,
            },
            None,
            source,
        )
    }
    #[test]
    fn exact_native_identity_merges_sources_but_not_same_name_or_executable() {
        let a = app("C:\\a.lnk", "--profile a", Source::UserStartMenu);
        let b = app("C:\\a.lnk", "--profile a", Source::AppsFolder);
        let c = app("C:\\b.lnk", "--profile b", Source::CommonStartMenu);
        let (apps, removed) = deduplicate(vec![a.clone(), b, c]);
        assert_eq!(removed, 1);
        assert_eq!(apps.len(), 2);
        assert_eq!(
            apps[0].sources,
            vec![Source::UserStartMenu, Source::AppsFolder]
        );
        assert_eq!(apps[0].launch, a.launch);
    }
    #[test]
    fn stable_ids_do_not_depend_on_label_or_enumeration_order() {
        let a = app("C:\\a.lnk", "", Source::UserStartMenu);
        let mut b = a.clone();
        b.display_name = "Localized editor".into();
        assert_eq!(a.id, b.id);
        let c = app("C:\\c.lnk", "", Source::UserStartMenu);
        assert_eq!(
            deduplicate(vec![a.clone(), c.clone()]),
            deduplicate(vec![c, a])
        );
        assert_eq!(deduplicate(vec![]), (vec![], 0));
    }
    #[test]
    fn packaged_identity_deduplicates_without_merging_distinct_apps_in_package() {
        let app = |id: &str| {
            InstalledApp::new(
                "Store app".into(),
                LaunchTarget::PackagedApp {
                    app_user_model_id: id.into(),
                    parsing_name: id.into(),
                },
                None,
                Source::AppsFolder,
            )
        };
        let (apps, removed) = deduplicate(vec![
            app("family!one"),
            app("family!one"),
            app("family!two"),
        ]);
        assert_eq!((apps.len(), removed), (2, 1));
    }
    #[test]
    fn filters_are_narrow() {
        assert!(excluded("Uninstall Editor", ""));
        assert!(excluded("Remove", "C:\\App\\unins000.exe"));
        assert!(excluded("Crash handler", "crashpad_handler.exe"));
        assert!(!excluded("Update Assistant", "C:\\app.exe"));
        assert!(excluded("Uninstall Tool", "")); // Explicit uninstall label.
        assert!(!excluded("Helpful Editor", "C:\\helper.exe"));
    }

    #[test]
    fn cross_source_ids_merge_only_unambiguous_shortcuts_and_preserve_both_launch_forms() {
        let with_id = |path| {
            let mut item = app(path, "--profile a", Source::UserStartMenu);
            if let LaunchTarget::Shortcut {
                app_user_model_id, ..
            } = &mut item.launch
            {
                *app_user_model_id = Some("Editor.App".into());
            }
            item
        };
        let shell = InstalledApp::new(
            "Editor".into(),
            LaunchTarget::ShellApp {
                parsing_name: "Editor.App".into(),
                app_user_model_id: Some("Editor.App".into()),
            },
            None,
            Source::AppsFolder,
        );
        let (merged, removed) = deduplicate(vec![shell.clone(), with_id("C:\\a.lnk")]);
        assert_eq!(removed, 1);
        assert_eq!(merged[0].alternate_launches, vec![shell.launch.clone()]);
        assert_eq!(
            merged[0].sources,
            vec![Source::UserStartMenu, Source::AppsFolder]
        );
        let (ambiguous, removed) =
            deduplicate(vec![shell, with_id("C:\\a.lnk"), with_id("C:\\b.lnk")]);
        assert_eq!((ambiguous.len(), removed), (3, 0));
    }
}
