#[derive(Debug, Clone, serde::Serialize)]
pub struct McpNotification {
    pub method: String,
    pub params: serde_json::Value,
}

pub fn vault_changed(commit: String, event: crate::service::events::VaultEvent) -> McpNotification {
    McpNotification {
        method: "notifications/vault_changed".into(),
        params: serde_json::json!({
            "commit": commit,
            "event": event,
        }),
    }
}

pub fn resource_updated(
    vault_id: &str,
    commit: String,
    event: crate::service::events::VaultEvent,
    subscription_id: &serde_json::Value,
) -> McpNotification {
    McpNotification {
        method: "notifications/resources/updated".into(),
        params: serde_json::json!({
            "uri": format!("pkv://vault/{vault_id}"),
            "commit": commit,
            "event": event,
            "_meta": {
                "io.modelcontextprotocol/subscriptionId": subscription_id,
            },
        }),
    }
}

/// Non-standard signal that events were missed on a subscription and the
/// client must re-read the resource: neither transport replays the skipped
/// range, so silence would hide the gap entirely.
pub fn resources_dropped(vault_id: &str, subscription_id: &serde_json::Value) -> McpNotification {
    McpNotification {
        method: "notifications/resources/dropped".into(),
        params: serde_json::json!({
            "uri": format!("pkv://vault/{vault_id}"),
            "_meta": {
                "io.modelcontextprotocol/subscriptionId": subscription_id,
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
