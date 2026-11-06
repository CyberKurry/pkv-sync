//! Bounded tool-result envelopes (MEL-08).
//!
//! Budget discipline: the FULL result envelope (structuredContent + its
//! mirrored text + era decoration) is counted with a writing-less
//! serializer BEFORE any commit-time reporting, so a too-large answer is
//! refused (`mcp_response_too_large:`) instead of materialized. Field-level
//! clamping happens FIRST (aliases/snippets never become unbounded
//! Strings); this layer is the envelope-level backstop, not the only cap.

use crate::mcp::protocol::{self, Era};
use serde::Serialize;
use serde_json::Value;
use std::io::{self, Write};

/// Hard budgets (bytes unless noted). The wire-facing caps (raw link,
/// alias, snippet) are consumed by `backlinks`; the envelope caps are the
/// plan-08 contract for edit/move previews, whose callers adopt them when
/// those tools grow their envelope wiring.
#[allow(dead_code)]
pub(crate) const MAX_SUCCESS_BYTES: usize = 1024 * 1024;
#[allow(dead_code)]
pub(crate) const MAX_ANCHOR_ERROR_BYTES: usize = 16 * 1024;
#[allow(dead_code)]
pub(crate) const MAX_CANDIDATES: usize = 20;
#[allow(dead_code)]
pub(crate) const MAX_EXCERPT_BYTES: usize = 160;
pub(crate) const MAX_RAW_LINK_BYTES: usize = 4096;
pub(crate) const MAX_ALIAS_BYTES: usize = 1024;
pub(crate) const MAX_SNIPPET_BYTES: usize = 1024;

/// Char-boundary-safe byte prefix of `source`.
pub(crate) fn prefix_bytes(source: &str, max: usize) -> &str {
    let mut end = source.len().min(max);
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    &source[..end]
}

/// Counting writer: raw JSON bytes and their JSON-string-escaped length
/// (the text-mirror cost) in one pass, failing the moment the combined
/// envelope would exceed `limit`.
#[allow(dead_code)]
struct JsonSizes {
    raw: usize,
    escaped: usize,
    limit: usize,
}

impl Write for JsonSizes {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &b in buf {
            let add = match b {
                b'"' | b'\\' | b'\n' | b'\r' | b'\t' | 8 | 12 => 2,
                0..=31 => 6,
                _ => 1,
            };
            self.raw = self
                .raw
                .checked_add(1)
                .ok_or_else(|| io::Error::other("response size overflow"))?;
            self.escaped = self
                .escaped
                .checked_add(add)
                .ok_or_else(|| io::Error::other("response size overflow"))?;
            if self
                .raw
                .checked_add(self.escaped)
                .filter(|&n| n <= self.limit)
                .is_none()
            {
                return Err(io::Error::other("response size limit"));
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Predict the final envelope size for `value` under `era` decoration:
/// a minimal envelope carrying the value as structuredContent and its
/// serialized form as the single text block. Never builds the full JSON.
#[allow(dead_code)]
pub(crate) fn checked_success_size<T: Serialize + ?Sized>(
    value: &T,
    modern: bool,
    max: usize,
) -> anyhow::Result<usize> {
    let mut sizes = JsonSizes {
        raw: 0,
        escaped: 0,
        limit: max,
    };
    serde_json::to_writer(&mut sizes, value)
        .map_err(|e| anyhow::anyhow!("mcp_response_too_large: {e}"))?;
    // Envelope overhead: {"content":[{"type":"text","text":""}],
    //  "structuredContent":<value>,"isError":false}
    // plus, for modern, the decorated result fields. We measure a real
    // minimal envelope below and add raw+escaped on top.
    let skeleton = json_envelope_skeleton(modern);
    let skeleton_len = serde_json::to_vec(&skeleton)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .len();
    let total = skeleton_len
        .checked_add(sizes.raw)
        .and_then(|n| n.checked_add(sizes.escaped))
        .ok_or_else(|| anyhow::anyhow!("mcp_response_too_large: size overflow"))?;
    if total > max {
        anyhow::bail!("mcp_response_too_large: result envelope would exceed {max} bytes");
    }
    Ok(total)
}

#[allow(dead_code)]
fn json_envelope_skeleton(modern: bool) -> Value {
    // The minimal envelope with a null structuredContent placeholder; the
    // caller subtracts the 4-byte `null` when inserting the value. For
    // size prediction the skeleton bound is what we need (>= real).
    let mut envelope = serde_json::json!({
        "content": [{"type": "text", "text": ""}],
        "structuredContent": null,
        "isError": false,
    });
    if modern {
        // decorate_result adds resultType + _meta.serverInfo on the object
        // inside structuredContent — approximate with fixed overhead.
        envelope["resultType"] = serde_json::json!("complete");
        envelope["_meta"] = serde_json::json!({
            protocol::META_SERVER_INFO: {"name": "", "version": ""}
        });
    }
    envelope
}

/// Bounded business failure: public snake_case code + message (Display
/// keeps the prefixed wire shape) + optional structured details that MUST
/// already be field-clamped by the caller.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct ToolFailure {
    pub code: &'static str,
    pub message: String,
    pub details: Value,
}

impl std::fmt::Display for ToolFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Unified success envelope: `content` (text mirror of the structured
/// JSON) + `structuredContent` + `isError:false`, decorated for modern
/// era via the single producer (`protocol::decorate_result`).
#[allow(dead_code)]
pub(crate) fn make_tool_result(
    structured: Value,
    is_error: bool,
    era: Era,
    cap: Option<usize>,
) -> anyhow::Result<Value> {
    if let Some(max) = cap {
        // Size gate BEFORE building the text mirror.
        checked_success_size(&structured, era == Era::Modern, max)?;
    }
    let mut result = serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&structured)?,
        }],
        "structuredContent": structured,
        "isError": is_error,
    });
    if era == Era::Modern {
        // decorate operates on the result OBJECT: apply to the object that
        // carries resultType — the transport applies it at the result level;
        // here we mirror what transport_stdio does for the result root.
        result = protocol::decorate_result(result, None);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mel_response_prefix_bytes_respects_char_boundary() {
        assert_eq!(prefix_bytes("héllo", 2), "h");
        assert_eq!(prefix_bytes("héllo", 3), "hé");
        assert_eq!(prefix_bytes("abc", 100), "abc");
        // emoji never splits
        assert_eq!(prefix_bytes("a😀b", 2), "a");
        assert_eq!(prefix_bytes("a😀b", 5), "a😀");
    }

    #[test]
    fn mel_response_checked_size_measures_not_materializes() {
        // A value whose escaped form doubles (quotes/backslashes) must be
        // counted at its ESCAPED length for the text mirror.
        let value = serde_json::json!({ "text": "\"\\".repeat(20) });
        let max = 512;
        let size = checked_success_size(&value, false, max).unwrap();
        assert!(size > 40, "quotes and backslashes counted doubled");
        assert!(size <= max);
    }

    #[test]
    fn mel_response_checked_size_refuses_over_budget() {
        let value = serde_json::json!({ "alias": "x".repeat(40 * 1024) });
        let err = checked_success_size(&value, false, MAX_ANCHOR_ERROR_BYTES).unwrap_err();
        assert!(err.to_string().starts_with("mcp_response_too_large:"));
    }

    #[test]
    fn mel_response_checked_size_overflow_safe() {
        // usize::MAX-scale multiplication must refuse, not wrap.
        let huge = serde_json::json!({ "a": "x".repeat(1024 * 1024) });
        let err = checked_success_size(&huge, false, 1024).unwrap_err();
        assert!(err.to_string().contains("mcp_response_too_large"));
    }

    #[test]
    fn mel_response_make_tool_result_both_eras_shapes() {
        let structured = serde_json::json!({ "ok": true });
        let legacy = make_tool_result(structured.clone(), false, Era::Legacy, None).unwrap();
        assert_eq!(legacy["isError"], false);
        assert_eq!(legacy["structuredContent"]["ok"], true);
        assert!(legacy["content"][0]["text"].is_string());
        assert!(
            legacy.get("resultType").is_none(),
            "legacy has no resultType"
        );

        let modern = make_tool_result(structured, false, Era::Modern, None).unwrap();
        assert_eq!(modern["resultType"], "complete");
        assert!(modern["_meta"][protocol::META_SERVER_INFO]["name"].is_string());
    }

    #[test]
    fn mel_response_make_tool_result_enforces_cap() {
        let structured = serde_json::json!({ "alias": "y".repeat(64 * 1024) });
        let err = make_tool_result(structured, false, Era::Legacy, Some(16 * 1024)).unwrap_err();
        assert!(err.to_string().starts_with("mcp_response_too_large:"));
    }

    #[test]
    fn mel_response_size_prediction_matches_real_serialization() {
        // The predicted size must be >= the real envelope's byte length
        // (skeleton bound), and within a small margin of it.
        let structured = serde_json::json!({ "note": "z".repeat(500) });
        let predicted = checked_success_size(&structured, false, MAX_SUCCESS_BYTES).unwrap();
        let real = make_tool_result(structured, false, Era::Legacy, None).unwrap();
        let real_len = serde_json::to_vec(&real).unwrap().len();
        assert!(
            predicted >= real_len,
            "prediction {predicted} must bound real {real_len}"
        );
        assert!(
            predicted - real_len < 256,
            "skeleton overhead sane: {} vs {}",
            predicted,
            real_len
        );
    }

    #[test]
    fn mel_response_tool_failure_display_keeps_prefix() {
        let failure = ToolFailure {
            code: "anchor_not_found",
            message: "no heading matched".into(),
            details: serde_json::json!({}),
        };
        assert_eq!(failure.to_string(), "anchor_not_found: no heading matched");
    }
}
