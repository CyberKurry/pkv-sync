use base64::Engine;
use serde_json::{json, Value};

pub const MODERN_VERSION: &str = "2026-07-28";
pub const LEGACY_VERSIONS: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];
pub const SUPPORTED_VERSIONS: [&str; 2] = [MODERN_VERSION, "2025-06-18"];

pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";
pub const META_SUBSCRIPTION_ID: &str = "io.modelcontextprotocol/subscriptionId";
pub const SERVER_NAME: &str = "cyberkurry pkv sync";

// App-defined codes, kept outside the -32000..-32768 range the 2026-07-28
// spec freezes for the protocol / reserved error space.
pub const ERR_RATE_LIMIT: i64 = -63001;
pub const ERR_AUTH: i64 = -63002;

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

pub fn decorate_result(mut result: Value, cache: Option<(u64, &str)>) -> Value {
    let obj = result.as_object_mut().expect("result must be an object");
    obj.insert("resultType".into(), json!("complete"));
    if let Some((ttl, scope)) = cache {
        obj.insert("ttlMs".into(), json!(ttl));
        obj.insert("cacheScope".into(), json!(scope));
    }
    obj.entry("_meta").or_insert_with(|| json!({}));
    if let Some(meta) = obj.get_mut("_meta").and_then(Value::as_object_mut) {
        meta.insert(
            META_SERVER_INFO.into(),
            json!({
                "name": SERVER_NAME,
                "version": env!("CARGO_PKG_VERSION"),
            }),
        );
    }
    result
}

pub fn decode_header_value(raw: &str) -> String {
    if let Some(b64) = raw
        .strip_prefix("=?base64?")
        .and_then(|rest| rest.strip_suffix("?="))
    {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
            if let Ok(text) = String::from_utf8(bytes) {
                return text;
            }
        }
    }
    raw.to_string()
}

pub fn validate_modern_headers(
    headers: &axum::http::HeaderMap,
    request: &Value,
) -> Result<(), &'static str> {
    let header_version = headers
        .get("MCP-Protocol-Version")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing MCP-Protocol-Version header")?;
    let body_version = request
        .pointer("/params/_meta")
        .and_then(|meta| meta.get(META_VERSION))
        .and_then(Value::as_str)
        .ok_or("request body lacks protocolVersion")?;
    if header_version != body_version {
        return Err("MCP-Protocol-Version header does not match body _meta");
    }
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .ok_or("request lacks method")?;
    let header_method = headers
        .get("Mcp-Method")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing Mcp-Method header")?;
    if header_method != method {
        return Err("Mcp-Method header does not match body method");
    }
    if let Some(name) = request.pointer("/params/name").and_then(Value::as_str) {
        let header_name = headers
            .get("Mcp-Name")
            .and_then(|v| v.to_str().ok())
            .ok_or("missing Mcp-Name header")?;
        if decode_header_value(header_name) != name {
            return Err("Mcp-Name header does not match body params.name");
        }
    }
    Ok(())
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

    #[test]
    fn decorate_adds_result_type_server_info_and_cache() {
        let out = decorate_result(json!({"tools": []}), Some((300_000, "private")));
        assert_eq!(out["resultType"], "complete");
        assert_eq!(out["ttlMs"], 300_000);
        assert_eq!(out["cacheScope"], "private");
        assert_eq!(out["_meta"][META_SERVER_INFO]["name"], SERVER_NAME);
        assert!(out["_meta"][META_SERVER_INFO]["version"].is_string());
    }

    #[test]
    fn decorate_without_cache_omits_ttl_fields() {
        let out = decorate_result(json!({"content": []}), None);
        assert_eq!(out["resultType"], "complete");
        assert!(out.get("ttlMs").is_none());
        assert!(out.get("cacheScope").is_none());
    }

    #[test]
    fn header_validation_happy_path() {
        let headers = header_map(&[
            ("MCP-Protocol-Version", "2026-07-28"),
            ("Mcp-Method", "tools/call"),
            ("Mcp-Name", "list_vaults"),
        ]);
        let req = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"list_vaults","_meta":{
                "io.modelcontextprotocol/protocolVersion":"2026-07-28",
                "io.modelcontextprotocol/clientCapabilities":{}}}});
        assert!(validate_modern_headers(&headers, &req).is_ok());
    }

    #[test]
    fn header_version_must_match_meta() {
        let headers = header_map(&[
            ("MCP-Protocol-Version", "2025-06-18"),
            ("Mcp-Method", "tools/list"),
        ]);
        let req = json!({"jsonrpc":"2.0","id":1,"method":"tools/list",
            "params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientCapabilities":{}}}});
        assert!(validate_modern_headers(&headers, &req).is_err());
    }

    #[test]
    fn missing_mcp_method_is_error() {
        let headers = header_map(&[("MCP-Protocol-Version", "2026-07-28")]);
        let req = json!({"jsonrpc":"2.0","id":1,"method":"tools/list",
            "params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientCapabilities":{}}}});
        assert!(validate_modern_headers(&headers, &req).is_err());
    }

    #[test]
    fn base64_sentinel_name_is_decoded() {
        let encoded = base64_of("笔记库");
        let headers = header_map(&[
            ("MCP-Protocol-Version", "2026-07-28"),
            ("Mcp-Method", "tools/call"),
            ("Mcp-Name", &format!("=?base64?{encoded}?=")),
        ]);
        let req = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"笔记库","_meta":{
                "io.modelcontextprotocol/protocolVersion":"2026-07-28",
                "io.modelcontextprotocol/clientCapabilities":{}}}});
        assert!(validate_modern_headers(&headers, &req).is_ok());
    }

    fn header_map(entries: &[(&str, &str)]) -> axum::http::HeaderMap {
        let mut m = axum::http::HeaderMap::new();
        for (k, v) in entries {
            m.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                axum::http::HeaderValue::from_str(v).unwrap(),
            );
        }
        m
    }

    fn base64_of(s: &str) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
    }
}
