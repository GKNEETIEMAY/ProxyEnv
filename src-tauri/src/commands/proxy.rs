use crate::{
    error::Result,
    features::proxy::{
        self, active,
        connectivity::{self, ProxyConnectivityResult},
        ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection, ProxyEnvironmentStatus,
    },
};

#[tauri::command]
pub async fn detect_proxies() -> Result<Vec<ProxyCandidate>> {
    proxy::detect()
}

#[tauri::command]
pub async fn inspect_proxy_endpoint(endpoint: ProxyEndpoint) -> Result<ProxyEndpointInspection> {
    proxy::inspect_endpoint(&endpoint)
}

#[tauri::command]
pub async fn select_active_proxy(candidate_id: String) -> Result<ProxyEnvironmentStatus> {
    let settings = crate::services::settings::load()?;
    active::select(&candidate_id, &settings.proxy_variables)
}

#[tauri::command]
pub async fn get_active_proxy_context() -> Result<active::ActiveProxyContext> {
    active::context()
}

#[tauri::command]
pub async fn test_current_proxy_connectivity(
    expected_revision: u64,
) -> Result<ProxyConnectivityResult> {
    let candidate =
        active::with_current(Some(expected_revision), |candidate| Ok(candidate.clone()))?;
    let result = connectivity::test_current_proxy(&candidate).await?;
    // The request uses a captured target; never present a completed old-target test as current.
    active::with_current(Some(expected_revision), |_| Ok(result))
}

#[tauri::command]
pub async fn get_last_proxy_connectivity_result() -> Result<Option<ProxyConnectivityResult>> {
    let context = active::context()?;
    Ok(connectivity::get_last_result(context.available_candidate()))
}
