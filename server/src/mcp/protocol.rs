use serde_json::{json, Value};

pub const MODERN_VERSION: &str = "2026-07-28";
pub const LEGACY_VERSIONS: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];
pub const SUPPORTED_VERSIONS: [&str; 2] = [MODERN_VERSION, "2025-06-18"];

const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CLIENT_CAPS: &str = "io.modelcontextprotocol/clientCapabilities";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    Modern,
    Legacy,
}

#[derive(Debug)]
pub enum ProtocolError {
    MissingMeta(&'static str),
    UnsupportedVersion { requested: String },
}

pub fn classify_request(request: &Value) -> Result<Era, ProtocolError> {
    if request.get("method").and_then(Value::as_str) == Some("initialize") {
        return Ok(Era::Legacy);
    }
    let Some(meta) = request.pointer("/params/_meta").and_then(Value::as_object) else {
        return Ok(Era::Legacy);
    };
    let modern_intent = meta
        .keys()
        .any(|k| k.starts_with("io.modelcontextprotocol/"));
    if !modern_intent {
        return Ok(Era::Legacy);
    }
    let Some(version) = meta.get(META_VERSION).and_then(Value::as_str) else {
        // _meta present with modern intent but no version key: a legacy client
        // never sends these keys, so treat as malformed modern → error.
        return Err(ProtocolError::MissingMeta(META_VERSION));
    };
    if version == MODERN_VERSION {
        if !meta.contains_key(META_CLIENT_CAPS) {
            return Err(ProtocolError::MissingMeta(META_CLIENT_CAPS));
        }
        return Ok(Era::Modern);
    }
    Err(ProtocolError::UnsupportedVersion {
        requested: version.to_string(),
    })
}

pub fn error_response(id: &Value, err: ProtocolError) -> Value {
    match err {
        ProtocolError::MissingMeta(key) => json!({
            "jsonrpc": "2.0", "id": id,
            "error": {"code": -32602,
                "message": format!("missing required _meta key: {key}")}
        }),
        ProtocolError::UnsupportedVersion { requested } => json!({
            "jsonrpc": "2.0", "id": id,
            "error": {"code": -32022, "message": "Unsupported protocol version",
                "data": {"supported": SUPPORTED_VERSIONS, "requested": requested}}
        }),
    }
}

pub fn legacy_negotiate_initialize(requested: &str) -> &'static str {
    LEGACY_VERSIONS
        .iter()
        .find(|v| **v == requested)
        .copied()
        .unwrap_or("2025-06-18")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_is_always_legacy_even_with_modern_version() {
        let req = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"x","version":"1"}}});
        assert!(matches!(classify_request(&req), Ok(Era::Legacy)));
    }

    #[test]
    fn modern_meta_classifies_modern() {
        let req = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list",
            "params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientCapabilities":{}}}});
        assert!(matches!(classify_request(&req), Ok(Era::Modern)));
    }

    #[test]
    fn no_meta_and_no_initialize_is_legacy() {
        let req = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}});
        assert!(matches!(classify_request(&req), Ok(Era::Legacy)));
    }

    #[test]
    fn modern_version_without_capabilities_is_missing_meta() {
        let req = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list",
            "params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}});
        assert!(matches!(
            classify_request(&req),
            Err(ProtocolError::MissingMeta(_))
        ));
    }

    #[test]
    fn unsupported_modern_version_is_unsupported() {
        let req = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list",
            "params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2099-01-01",
            "io.modelcontextprotocol/clientCapabilities":{}}}});
        assert!(matches!(
            classify_request(&req),
            Err(ProtocolError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn unsupported_version_error_carries_supported_list() {
        let req = serde_json::json!({"jsonrpc":"2.0","id":7,"method":"tools/list",
            "params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2099-01-01",
            "io.modelcontextprotocol/clientCapabilities":{}}}});
        let err = classify_request(&req).unwrap_err();
        let resp = error_response(&serde_json::json!(7), err);
        assert_eq!(resp["error"]["code"], -32022);
        assert_eq!(
            resp["error"]["data"]["supported"],
            serde_json::json!(SUPPORTED_VERSIONS)
        );
        assert_eq!(resp["error"]["data"]["requested"], "2099-01-01");
    }

    #[test]
    fn legacy_negotiate_echoes_known_and_defaults() {
        assert_eq!(legacy_negotiate_initialize("2024-11-05"), "2024-11-05");
        assert_eq!(legacy_negotiate_initialize("1999-01-01"), "2025-06-18");
    }

    #[test]
    fn progress_token_only_meta_stays_legacy() {
        let req = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"list_vaults","_meta":{"progressToken":1}}});
        assert!(matches!(classify_request(&req), Ok(Era::Legacy)));
    }
}
