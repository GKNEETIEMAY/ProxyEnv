// SHA-256 snapshots captured from the pre-adapter implementation at 9679c7d.
use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};

#[test]
fn linux_wire_bytes_match_pre_adapter_baseline() {
    let mut rows = Vec::new();
    let mut snapshot = |name: &str, value: &str| {
        rows.push(format!(
            "{name} {}",
            hex::encode(Sha256::digest(value.as_bytes()))
        ));
    };
    snapshot("stdin-command", SCRIPT_COMMAND);
    for operation in [
        "check",
        "verify",
        "internet",
        "test",
        "preview",
        "status",
        "apply",
        "restore",
        "restore-preview",
        "tool-verify",
        "tool-discover",
        "session-env-apply",
        "session-env-remove",
    ] {
        let request = json!({
            "operation":operation, "port":17897, "ports":[17897,25721],
            "protocol":"socks5", "expectedHash":"a".repeat(64), "backupHash":"b".repeat(64),
            "repairPermissions":true, "profileModelBase64":"YQ==", "profileCatalogBase64":"e30=",
            "profileHash":"c".repeat(64), "sessionToken":"d".repeat(64), "sessionId":"e".repeat(32),
        });
        let (payload, actual_operation) = remote_payload(&request).unwrap();
        assert_eq!(actual_operation, operation);
        snapshot(operation, &payload);
    }
    let (payload, _) = remote_payload(&json!({
        "operation":"apply", "tool":"claude", "profileSettingsBase64":"e30=",
        "profileHash":"a".repeat(64), "sessionToken":"b".repeat(64), "extensionVersion":"2.1.227",
    }))
    .unwrap();
    snapshot("claude-extension", &payload);
    for operation in ["status", "prepare", "apply", "remove", "cleanup"] {
        let payload = skill_payload(&json!({
            "operation":operation, "tool":"codex", "skillName":"sample-skill",
            "skillHash":"f".repeat(64), "manifestBase64":"YQ==", "directoriesBase64":"Yg==",
            "fileCount":1, "totalSize":1,
        }))
        .unwrap();
        snapshot(&format!("skill-{operation}"), &payload);
    }
    let extension = json!({"operation":"network-apply","sessionId":"e".repeat(32),"proxyPort":17897,"proxyProtocol":"mixed"});
    snapshot("extension", &extension_payload(&extension));
    snapshot(
        "interactive-check",
        &interactive_check_remote_command(&[], "__PROXYENV_SSH_AUTH_fixture__"),
    );
    snapshot(
        "interactive-tunnel",
        &tunnel_remote_command(&[17897, 25721], "__PROXYENV_SSH_AUTH_fixture__"),
    );
    snapshot(
        "managed-terminal",
        &managed_terminal_remote_command(&"e".repeat(32)).unwrap(),
    );
    snapshot(
        "environment-source",
        &session_environment_source_command(&"e".repeat(32)),
    );
    snapshot(
        "skill-scp-path",
        &skill_upload_path("codex", "sample-skill", &"f".repeat(64), "sub/skill.md").unwrap(),
    );
    let expected: Vec<_> = include_str!("linux-wire.sha256").lines().collect();
    assert_eq!(rows, expected);
}

#[test]
fn rejects_invalid_parameters_before_producing_a_linux_payload() {
    let valid = json!({
        "operation":"apply", "tool":"claude", "port":25721,
        "ports":[17897], "protocol":"http", "expectedHash":"absent", "backupHash":"absent",
        "profileSettingsBase64":"e30=", "profileHash":"a".repeat(64),
        "sessionToken":"b".repeat(64), "sessionId":"c".repeat(32),
    });
    assert!(remote_payload(&valid).is_ok());
    for (field, value, error) in [
        ("operation", json!("unknown"), "invalidRequest"),
        ("tool", json!("unsupported"), "invalidRequest"),
        ("port", json!(65536), "invalidPort"),
        ("ports", json!([1023]), "invalidPort"),
        ("ports", json!(["17897"]), "invalidPort"),
        ("protocol", json!("unsupported"), "invalidRequest"),
        ("expectedHash", json!("invalid"), "invalidRequest"),
        ("backupHash", json!("invalid"), "invalidRequest"),
        (
            "profileSettingsBase64",
            json!("invalid value"),
            "invalidRequest",
        ),
        ("profileHash", json!("invalid"), "invalidRequest"),
        ("sessionToken", json!(""), "invalidRequest"),
        ("sessionId", json!("invalid"), "invalidRequest"),
    ] {
        let mut request = valid.clone();
        request[field] = value;
        assert_eq!(remote_payload(&request).unwrap_err(), error, "{field}");
    }
    assert!(managed_terminal_remote_command("invalid").is_err());
    for relative_path in [
        "",
        "/absolute",
        "../file",
        "sub//file",
        ".proxyenv-owner",
        "sub/file name",
    ] {
        assert!(
            skill_upload_path("codex", "sample-skill", &"a".repeat(64), relative_path).is_err()
        );
    }
}
