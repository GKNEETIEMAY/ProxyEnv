//! Read-only CLI observations. Remote output is never used as a command or path.
use super::tool_adapter::{self, RemoteToolId, RemoteToolInstallation};
use super::BridgeResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteInstallCapabilities {
    pub curl: bool,
    pub npm: bool,
    pub brew: bool,
    pub node_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteToolDiscoverySnapshot {
    pub tools: Vec<RemoteToolInstallation>,
    pub capabilities: RemoteInstallCapabilities,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTool {
    tool: RemoteToolId,
    found: bool,
    runnable: bool,
    version: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDiscovery {
    tools: Vec<RawTool>,
    capabilities: RemoteInstallCapabilities,
}

pub(super) fn project_extensions(
    tools: &mut [RemoteToolInstallation],
    inspection: &super::extension::Inspection,
) {
    use tool_adapter::RemoteToolInstallState as S;
    for tool in tools {
        let capability = inspection
            .extensions
            .iter()
            .find(|entry| entry.tool == tool.tool.as_str());
        tool.extension_state = match capability {
            Some(entry) if !entry.detected => S::Missing,
            Some(_) if inspection.vscode.status != "detected" => S::Unknown,
            Some(_) if super::extension::profile_version(inspection, tool.tool).is_ok() => {
                S::Installed
            }
            Some(_) => S::Unsupported,
            None => S::Unknown,
        };
        tool.extension_version = capability.and_then(|entry| {
            tool_adapter::valid_version(&entry.version).then(|| entry.version.clone())
        });
    }
}

pub(super) fn parse(value: serde_json::Value) -> BridgeResult<RemoteToolDiscoverySnapshot> {
    if value["tools"].as_array().is_none_or(|tools| {
        tools.iter().any(|tool| {
            !tool
                .as_object()
                .is_some_and(|object| object.contains_key("version"))
        })
    }) || !value["capabilities"]
        .as_object()
        .is_some_and(|object| object.contains_key("nodeVersion"))
    {
        return Err("remoteUnsupported".into());
    }
    let raw: RawDiscovery = serde_json::from_value(value).map_err(|_| "remoteUnsupported")?;
    if raw.tools.len() != 2
        || raw.tools[0].tool != RemoteToolId::Codex
        || raw.tools[1].tool != RemoteToolId::Claude
        || raw
            .capabilities
            .node_version
            .as_deref()
            .is_some_and(|value| !tool_adapter::valid_version(value))
        || raw.tools.iter().any(|tool| {
            tool.version
                .as_deref()
                .is_some_and(|version| !tool_adapter::valid_version(version))
                || !tool.found && (tool.runnable || tool.version.is_some())
                || !tool.runnable && tool.version.is_some()
        })
    {
        return Err("remoteUnsupported".into());
    }
    Ok(RemoteToolDiscoverySnapshot {
        tools: raw
            .tools
            .iter()
            .map(|tool| {
                tool_adapter::project_installation(
                    tool.tool,
                    tool.found,
                    tool.runnable,
                    tool.version.as_deref(),
                )
            })
            .collect(),
        capabilities: raw.capabilities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tool_adapter::{project_installation, RemoteToolInstallState as S};
    #[test]
    fn compatibility_and_execution_states_are_independent_from_configuration() {
        for (tool, version, state) in [
            (RemoteToolId::Codex, "0.133.9", S::Unsupported),
            (RemoteToolId::Codex, "0.134.0", S::Installed),
            (RemoteToolId::Codex, "0.200.0", S::Installed),
            (RemoteToolId::Codex, "1.0.0", S::Unsupported),
            (RemoteToolId::Claude, "2.1.226", S::Unsupported),
            (RemoteToolId::Claude, "2.1.227", S::Installed),
            (RemoteToolId::Claude, "2.2.0", S::Installed),
            (RemoteToolId::Claude, "3.0.0", S::Unsupported),
        ] {
            assert_eq!(
                project_installation(tool, true, true, Some(version)).state,
                state
            );
        }
        assert_eq!(
            project_installation(RemoteToolId::Codex, false, false, None).state,
            S::Missing
        );
        assert_eq!(
            project_installation(RemoteToolId::Codex, true, false, None).state,
            S::Broken
        );
        assert_eq!(
            project_installation(RemoteToolId::Codex, true, true, Some("/home/private/cli")).state,
            S::Broken
        );
    }
    #[test]
    fn discovery_schema_rejects_extra_fields_duplicates_and_sensitive_output() {
        let valid = serde_json::json!({"tools":[{"tool":"codex","found":false,"runnable":false,"version":null},{"tool":"claude","found":true,"runnable":true,"version":"2.1.227"}],"capabilities":{"curl":true,"npm":false,"brew":false,"nodeVersion":null}});
        let snapshot = parse(valid.clone()).unwrap();
        assert_eq!(snapshot.tools[0].state, S::Missing);
        assert_eq!(snapshot.tools[1].state, S::Installed);
        for location in ["path", "HOME", "PATH", "token"] {
            let mut changed = valid.clone();
            changed["tools"][0][location] = "secret".into();
            assert!(parse(changed).is_err());
        }
        let mut changed = valid.clone();
        changed["tools"][1]["tool"] = "codex".into();
        assert!(parse(changed).is_err());
        let mut changed = valid.clone();
        changed["tools"][1]["version"] = "/home/private".into();
        assert!(parse(changed).is_err());
        let mut changed = valid.clone();
        changed["capabilities"]["nodeVersion"] = "v22.1.0".into();
        assert!(parse(changed).is_err());
        assert!(parse(serde_json::json!({})).is_err());
    }
}
