use crate::{
    error::Result,
    proxy::{self, ProxyCandidate},
};

#[tauri::command]
pub async fn detect_proxies() -> Result<Vec<ProxyCandidate>> {
    proxy::detect()
}
