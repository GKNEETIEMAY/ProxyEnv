mod commands;
mod desktop;
mod environment;
mod error;
mod proxy;
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
            commands::environment::get_environment_status,
            commands::environment::enable_proxy_environment,
            commands::environment::sync_proxy_environment,
            commands::environment::disable_proxy_environment,
            commands::proxy::detect_proxies,
            commands::settings::get_app_settings,
            commands::settings::save_app_settings,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ProxyEnv");
}
