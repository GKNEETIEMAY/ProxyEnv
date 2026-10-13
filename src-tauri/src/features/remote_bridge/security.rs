//! Policy facts only; no credentials, paths or connection identifiers.
use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecuritySnapshot {
    pub general_proxy_auth: GeneralProxyAuth,
    pub ai_route_auth: AiRouteAuth,
    pub remote_bind_scope: BindScope,
    pub shell_scope: ShellScope,
}
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GeneralProxyAuth {
    #[default]
    None,
}
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiRouteAuth {
    #[default]
    Session,
}
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BindScope {
    #[default]
    Loopback,
}
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShellScope {
    #[default]
    SessionOnly,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_has_only_allowlisted_facts() {
        assert_eq!(
            serde_json::to_value(SecuritySnapshot::default()).unwrap(),
            serde_json::json!({
                "generalProxyAuth":"none", "aiRouteAuth":"session", "remoteBindScope":"loopback", "shellScope":"sessionOnly"
            })
        );
    }
}
