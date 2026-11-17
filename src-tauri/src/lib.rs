pub mod action_registry;
mod active_monitor;
pub mod audio;
mod audio_feedback;
mod command_discovery;
mod commands;
mod file_actions;
pub mod file_search;
mod help_center;
pub mod installed_apps;
mod launcher_window;
mod shortcut;
mod system_actions;
mod workflow_execution;
mod workflow_picker;
mod workflow_studio;
mod workflows;

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
            app.manage(help_center::HelpCenter::default());
            app.manage(workflow_studio::Studio::default());
            app.manage(action_registry::ActionRegistry::default());
            app.manage(workflow_execution::Engine::default());
            app.manage(audio_feedback::Feedback::default());
            app.manage(std::sync::Arc::new(system_actions::SystemActions::default()));
            app.manage(std::sync::Arc::new(audio::AudioService::default()));
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
            if window.label() == help_center::LABEL
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
            if window.label() == workflow_studio::LABEL {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    workflow_studio::request_close(window.app_handle());
                }
            }
            // Never destroy the shortcut's launcher while auxiliary windows survive.
            // Active runs keep running; an idle quit first protects Studio's draft.
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let app = window.app_handle();
                    match workflow_studio::launcher_close(
                        app.state::<workflow_execution::Engine>().has_active(),
                        app.get_webview_window(workflow_studio::LABEL).is_some(),
                    ) {
                        workflow_studio::LauncherClose::Hide => {
                            let _ = window.hide();
                        }
                        workflow_studio::LauncherClose::Exit => app.exit(0),
                        workflow_studio::LauncherClose::AskStudio => {
                            if let Err(error) = workflow_studio::request_quit(app) {
                                let _ = window.show();
                                let _ = window.set_focus();
                                shortcut::report_error(
                                    app,
                                    format!("Could not quit NEXUS: {error}"),
                                );
                            }
                        }
                    }
                }
            }
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
            help_center::open_help_center,
            help_center::help_center_ready,
            command_discovery::command_discovery_metadata,
            workflow_picker::pick_workflow_path,
            workflow_studio::open_workflow_studio,
            workflow_studio::studio_ready,
            workflow_studio::studio_pending,
            workflow_studio::studio_resolve,
            workflow_execution::start_workflow,
            workflow_execution::workflow_run_status,
            workflow_execution::list_workflow_runs,
            workflow_execution::cancel_workflow_run,
            workflow_execution::decide_workflow_action,
            workflows::list_workflows,
            workflows::get_workflow,
            workflows::save_workflow,
            workflows::delete_workflow,
            action_registry::list_workflow_actions,
            audio_feedback::audio_feedback_state,
            system_actions::execute_system_action,
            system_actions::cancel_system_confirmation,
            audio::execute_audio,
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
