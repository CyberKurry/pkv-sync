/// JSON-RPC 2.0 requires the version field on every message, including
/// notifications; strict MCP clients (rmcp, the TS SDK) fail deserialization
/// and silently drop lines without it. Serialized first so the wire prefix
/// is byte-stable for line-scanning clients.
const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpNotification {
    jsonrpc: &'static str,
    pub method: String,
    pub params: serde_json::Value,
}

impl McpNotification {
    pub(crate) fn new(method: &str, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            method: method.into(),
            params,
        }
    }
}

pub fn vault_changed(commit: String, event: crate::service::events::VaultEvent) -> McpNotification {
    McpNotification::new(
        "notifications/vault_changed",
        serde_json::json!({
            "commit": commit,
            "event": event,
        }),
    )
}

pub fn resource_updated(
    vault_id: &str,
    commit: String,
    event: crate::service::events::VaultEvent,
    subscription_id: &serde_json::Value,
) -> McpNotification {
    McpNotification::new(
        "notifications/resources/updated",
        serde_json::json!({
            "uri": format!("pkv://vault/{vault_id}"),
            "commit": commit,
            "event": event,
            "_meta": {
                "io.modelcontextprotocol/subscriptionId": subscription_id,
            },
        }),
    )
}

/// Non-standard signal that events were missed on a subscription and the
/// client must re-read the resource: neither transport replays the skipped
/// range, so silence would hide the gap entirely.
pub fn resources_dropped(vault_id: &str, subscription_id: &serde_json::Value) -> McpNotification {
    McpNotification::new(
        "notifications/resources/dropped",
        serde_json::json!({
            "uri": format!("pkv://vault/{vault_id}"),
            "_meta": {
                "io.modelcontextprotocol/subscriptionId": subscription_id,
            },
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_notification_serializes_with_jsonrpc_2_0_first() {
        // M-1: strict MCP clients validate each inbound message as
        // JSON-RPC 2.0 and drop ones without the version field, so every
        // constructor must emit it (and, for byte-stable framing, first).
        let event = crate::service::events::VaultEvent {
            commit: "c1".into(),
            parent: None,
            source_device_id: "dev".into(),
            at: 1,
            kind: crate::service::events::EventKind::Commit,
            changes: vec![],
        };
        let notifications = vec![
            vault_changed("c1".into(), event.clone()),
            resource_updated("vault-1", "c1".into(), event, &serde_json::json!(5)),
            resources_dropped("vault-1", &serde_json::json!(5)),
        ];
        for notification in notifications {
            let line = serde_json::to_string(&notification).unwrap();
            assert!(
                line.starts_with("{\"jsonrpc\":\"2.0\""),
                "notification must lead with the version field: {line}"
            );
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(value["jsonrpc"], "2.0", "line: {line}");
        }
    }

    #[test]
    fn resources_dropped_carries_uri_and_subscription_id() {
        let n = resources_dropped("vault-1", &serde_json::json!(5));
        assert_eq!(n.method, "notifications/resources/dropped");
        assert_eq!(n.params["uri"], "pkv://vault/vault-1");
        assert_eq!(
            n.params["_meta"]["io.modelcontextprotocol/subscriptionId"],
            5
        );
    }

    #[test]
    fn resource_updated_carries_uri_commit_and_subscription_id() {
        let event = crate::service::events::VaultEvent {
            commit: "c1".into(),
            parent: None,
            source_device_id: "dev".into(),
            at: 1,
            kind: crate::service::events::EventKind::Commit,
            changes: vec![],
        };
        let n = resource_updated("vault-1", "c1".into(), event, &serde_json::json!(5));
        assert_eq!(n.method, "notifications/resources/updated");
        assert_eq!(n.params["uri"], "pkv://vault/vault-1");
        assert_eq!(n.params["commit"], "c1");
        assert_eq!(
            n.params["_meta"]["io.modelcontextprotocol/subscriptionId"],
            5
        );
    }
}
