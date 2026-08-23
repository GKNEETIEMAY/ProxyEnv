mod commands;
mod desktop;
mod environment;
mod error;
mod features;
mod services;

use desktop::tray;
use services::settings;

use tauri::Emitter;
use tauri_plugin_autostart::MacosLauncher;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
            let _ = app.emit("second-instance-opened", ());
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| {
            let settings = settings::load().unwrap_or_default();
            tray::setup(app.handle(), &settings)?;
            if tray::should_start_silent() {
                tray::hide_main_window(app.handle());
            } else {
                tray::show_main_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if tray::should_hide_on_close() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::application_assistant::list_running_applications,
            commands::application_assistant::diagnose_application,
            commands::application_assistant::preview_application_rule_fix,
            commands::application_assistant::apply_application_rule_fix,
            commands::application_assistant::restore_application_rule_change,
            commands::application_assistant::launch_application_with_current_proxy,
            commands::application_assistant::launch_application_without_proxy,
            commands::environment::get_environment_status,
            commands::environment::enable_proxy_environment,
            commands::environment::sync_proxy_environment,
            commands::environment::sync_manual_proxy_environment,
            commands::environment::disable_proxy_environment,
            commands::environment::restore_proxy_environment,
            commands::network_observation::get_tun_observation,
            commands::proxy::detect_proxies,
            commands::proxy::inspect_proxy_endpoint,
            commands::proxy::test_current_proxy_connectivity,
            commands::proxy::get_last_proxy_connectivity_result,
            commands::settings::get_app_settings,
            commands::settings::save_app_settings,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ProxyEnv");
}
