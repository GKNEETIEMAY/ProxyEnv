use crate::{
    error::Result,
    features::proxy::{self, ProxyCandidate},
};

#[tauri::command]
pub async fn detect_proxies() -> Result<Vec<ProxyCandidate>> {
    proxy::detect()
}
