use crate::{
    error::{ProxyEnvError, Result},
    features::network_observation::{self, TunObservation},
};

#[tauri::command]
pub async fn get_tun_observation() -> Result<TunObservation> {
    tauri::async_runtime::spawn_blocking(network_observation::observe)
        .await
        .map_err(|error| ProxyEnvError::Detection(error.to_string()))
}
