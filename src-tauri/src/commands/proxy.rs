use crate::{
    error::{ProxyEnvError, Result},
    features::proxy::{
        self,
        connectivity::{self, ProxyConnectivityResult},
        ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection,
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
pub async fn test_current_proxy_connectivity() -> Result<ProxyConnectivityResult> {
    let candidates = proxy::detect()?;
    let candidate = active_candidate(&candidates).ok_or(ProxyEnvError::ActiveProxyMissing)?;
    connectivity::test_current_proxy(candidate).await
}

#[tauri::command]
pub async fn get_last_proxy_connectivity_result() -> Result<Option<ProxyConnectivityResult>> {
    let candidates = proxy::detect()?;
    Ok(connectivity::get_last_result(active_candidate(&candidates)))
}

fn active_candidate(candidates: &[ProxyCandidate]) -> Option<&ProxyCandidate> {
    candidates.iter().find(|candidate| candidate.listening)
}
