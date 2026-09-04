use crate::{
    error::{ProxyEnvError, Result},
    features::diagnostic_report::{self, DiagnosticReportData},
};

#[tauri::command]
pub async fn generate_diagnostic_report(
    application_id: Option<String>,
) -> Result<DiagnosticReportData> {
    tauri::async_runtime::spawn_blocking(move || {
        diagnostic_report::generate(application_id.as_deref())
    })
    .await
    .map_err(|_| ProxyEnvError::Detection("diagnostic report unavailable".into()))?
}
