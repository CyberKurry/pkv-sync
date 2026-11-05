//! Shared link-target resolution for MCP surfaces (MEL-01 groundwork).
//!
//! One producer for "which vault file does this link denote": the comparison
//! key is the storage layer's `case_fold_key` (NFD + Unicode lowercase), and
//! the old `extract_link_targets`/`resolve_target` semantics stay in
//! `mcp::tools` until Task 02/06/07 migrate their consumers onto the span
//! scanner. This module currently only re-exports the storage key so early
//! tasks can depend on it without a second algorithm appearing.

pub(crate) use crate::storage::git::case_fold_key;

#[cfg(test)]
mod tests {
    // MEL-01.1: pin that the resolver-visible key IS the storage key.
    // Reuses the storage test's Unicode premises (Kelvin, ẞ, NFC/NFD,
    // Ä/ä) without restating them as MCP-local rules.
    #[test]
    fn mel_comparison_key_matches_storage() {
        use super::case_fold_key;
        assert_eq!(case_fold_key("Ä.md"), case_fold_key("ä.md"));
        assert_eq!(
            case_fold_key("caf\u{00e9}.md"),
            case_fold_key("cafe\u{301}.md")
        );
        assert_eq!(case_fold_key("\u{212a}.md"), case_fold_key("k.md"));
        assert_ne!(case_fold_key("\u{1e9e}.md"), case_fold_key("ss.md"));
    }
}
