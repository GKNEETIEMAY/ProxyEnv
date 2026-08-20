use crate::{
    environment::{EnvironmentManager, EnvironmentStatus},
    error::Result,
    proxy::{self, ProxyCandidate, ProxyProtocol},
};

#[tauri::command]
pub async fn get_environment_status() -> Result<EnvironmentStatus> {
    EnvironmentManager::status()
}

#[tauri::command]
pub async fn enable_proxy_environment(
    host: Option<String>,
    port: Option<u16>,
    protocol: Option<ProxyProtocol>,
) -> Result<EnvironmentStatus> {
    match (host, port, protocol) {
        (Some(host), Some(port), Some(protocol)) => {
            EnvironmentManager::enable_for_proxy(&host, port, protocol)
        }
        _ => EnvironmentManager::enable(),
    }
}

#[tauri::command]
pub async fn sync_proxy_environment(
    host: String,
    port: u16,
    protocol: ProxyProtocol,
) -> Result<EnvironmentStatus> {
    let status = EnvironmentManager::status()?;
    if !status.enabled {
        return Ok(status);
    }
    EnvironmentManager::enable_for_proxy(&host, port, protocol)
}

#[tauri::command]
pub async fn disable_proxy_environment() -> Result<EnvironmentStatus> {
    EnvironmentManager::disable()
}

#[tauri::command]
pub async fn detect_proxies() -> Result<Vec<ProxyCandidate>> {
    proxy::detect()
}
