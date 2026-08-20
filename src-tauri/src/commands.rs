use crate::{
    environment::{EnvironmentManager, EnvironmentStatus},
    error::Result,
    proxy::{self, ProxyCandidate},
};

#[tauri::command]
pub async fn get_environment_status() -> Result<EnvironmentStatus> {
    EnvironmentManager::status()
}

#[tauri::command]
pub async fn enable_proxy_environment() -> Result<EnvironmentStatus> {
    EnvironmentManager::enable()
}

#[tauri::command]
pub async fn disable_proxy_environment() -> Result<EnvironmentStatus> {
    EnvironmentManager::disable()
}

#[tauri::command]
pub async fn detect_proxies() -> Result<Vec<ProxyCandidate>> {
    proxy::detect()
}
