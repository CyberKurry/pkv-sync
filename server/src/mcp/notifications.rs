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
