//! Shared parsing for `subscriptions/listen` requests. Both transports
//! receive the same filter shape (resource subscription URIs plus
//! list-changed switches), so the parsing and the ack envelope live here;
//! each transport keeps only its own vault-ownership validation and
//! receiver registration.

use crate::mcp::notifications::McpNotification;
use crate::mcp::protocol::META_SUBSCRIPTION_ID;
use serde_json::{json, Map, Value};

pub(crate) struct ListenRequest {
    /// Deduplicated subscription URIs in first-occurrence order.
    pub accepted_uris: Vec<String>,
    /// List-changed switches echoed back only when the client set them to
    /// `true` (the lists are static, so nothing is ever emitted for them).
    pub switches: Map<String, Value>,
}

/// Parse a `subscriptions/listen` request's `params.notifications` filter.
/// Duplicate URIs are collapsed to their first occurrence; unknown URIs are
/// the transport's problem to drop (it knows which vaults the token may see).
pub(crate) fn parse_listen_request(filter_json: &Value) -> ListenRequest {
    let requested: Vec<String> = filter_json
        .get("resourceSubscriptions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    let accepted_uris = requested
        .into_iter()
        .filter(|uri| seen.insert(uri.clone()))
        .collect();
    let mut switches = Map::new();
    for switch in [
        ("toolsListChanged", "toolsListChanged"),
        ("promptsListChanged", "promptsListChanged"),
        ("resourcesListChanged", "resourcesListChanged"),
    ] {
        if filter_json.get(switch.1).and_then(Value::as_bool) == Some(true) {
            switches.insert(switch.0.into(), Value::Bool(true));
        }
    }
    ListenRequest {
        accepted_uris,
        switches,
    }
}

/// Build the `notifications/subscriptions/acknowledged` envelope for a
/// parsed filter: the transport supplies the URIs it actually accepted.
pub(crate) fn acknowledged(id: &Value, lr: &ListenRequest) -> McpNotification {
    let mut notifications = lr.switches.clone();
    if !lr.accepted_uris.is_empty() {
        notifications.insert("resourceSubscriptions".into(), json!(&lr.accepted_uris));
    }
    McpNotification::new(
        "notifications/subscriptions/acknowledged",
        json!({
            "notifications": notifications,
            "_meta": {META_SUBSCRIPTION_ID: id},
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_deduplicates_uris_keeping_first_occurrence_order() {
        let lr = parse_listen_request(&json!({
            "resourceSubscriptions": ["pkv://vault/b", "pkv://vault/a", "pkv://vault/b"]
        }));
        assert_eq!(
            lr.accepted_uris,
            vec!["pkv://vault/b".to_string(), "pkv://vault/a".to_string()]
        );
    }

    #[test]
    fn parse_echoes_only_true_switches() {
        let lr = parse_listen_request(&json!({
            "resourceSubscriptions": [],
            "toolsListChanged": true,
            "promptsListChanged": false
        }));
        assert_eq!(lr.switches.len(), 1);
        assert_eq!(lr.switches["toolsListChanged"], json!(true));
    }

    #[test]
    fn ack_for_empty_filter_has_no_notifications() {
        let lr = parse_listen_request(&json!({}));
        let ack = acknowledged(&json!(1), &lr);
        assert_eq!(ack.method, "notifications/subscriptions/acknowledged");
        assert_eq!(ack.params["notifications"], json!({}));
        assert_eq!(ack.params["_meta"][META_SUBSCRIPTION_ID], json!(1));
    }
}
