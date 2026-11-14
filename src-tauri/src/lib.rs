mod active_monitor;
mod commands;
mod file_actions;
pub mod file_search;
pub mod installed_apps;
mod launcher_window;
mod shortcut;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second process forwards activation then exits before
        // creating a window or attempting to register the shortcut.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let handle = app.clone();
            if let Err(error) = app.run_on_main_thread(move || {
                if let Some(window) = handle.get_webview_window("main") {
                    if let Err(error) = launcher_window::show(&window) {
                        shortcut::report_error(&handle, format!("Could not reopen NEXUS: {error}"));
                    }
                }
            }) {
                shortcut::report_error(app, format!("Could not activate NEXUS: {error}"));
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(std::sync::Arc::new(file_search::FileSearch::default()));
            app.manage(std::sync::Arc::new(installed_apps::CatalogCache::default()));
            app.manage(std::sync::Arc::new(
                installed_apps::icons::IconCache::default(),
            ));
            app.manage(std::sync::Arc::new(
                installed_apps::launching::LaunchActivity::default(),
            ));
            app.manage(shortcut::register(app.handle()));
            let window = app
                .get_webview_window("main")
                .ok_or("NEXUS main window was not created")?;
            launcher_window::show(&window)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Native window focus, not DOM/input blur: clicking results or moving
            // focus between controls inside NEXUS must not dismiss the window.
            if window.label() == "main"
                && matches!(event, tauri::WindowEvent::Focused(false))
                && !window
                    .app_handle()
                    .state::<std::sync::Arc<installed_apps::launching::LaunchActivity>>()
                    .is_active()
            {
                match window.is_focused() {
                    Ok(false) => {
                        if let Err(error) = window.hide() {
                            shortcut::report_error(
                                window.app_handle(),
                                format!("Could not dismiss NEXUS: {error}"),
                            );
                        }
                    }
                    Ok(true) => {} // Ignore a stale blur after another activation.
                    Err(error) => shortcut::report_error(
                        window.app_handle(),
                        format!("Could not check NEXUS focus: {error}"),
                    ),
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::execute_command,
            file_search::search_files,
            file_actions::open_file_result,
            launcher_window::hide_launcher,
            launcher_window::show_launcher,
            launcher_window::start_launcher_drag,
            shortcut::shortcut_status,
            installed_apps::list_installed_apps,
            installed_apps::refresh_installed_apps,
            installed_apps::launching::launch_installed_app,
            installed_apps::icons::get_installed_app_icon,
        ])
        .build(tauri::generate_context!())
        .expect("error while building NEXUS")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                shortcut::cleanup(app);
            }
        });
}
