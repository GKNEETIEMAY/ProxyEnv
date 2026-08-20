mod commands;
mod environment;
mod error;
mod proxy;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_environment_status,
            commands::enable_proxy_environment,
            commands::disable_proxy_environment,
            commands::detect_proxies,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ProxyEnv");
}
