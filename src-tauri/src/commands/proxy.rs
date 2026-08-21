use crate::{
    error::Result,
    features::proxy::{self, ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection},
};

#[tauri::command]
pub async fn detect_proxies() -> Result<Vec<ProxyCandidate>> {
    proxy::detect()
}

#[tauri::command]
pub async fn inspect_proxy_endpoint(endpoint: ProxyEndpoint) -> ProxyEndpointInspection {
    proxy::inspect_endpoint(&endpoint)
}
