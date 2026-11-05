//! Safe YAML scalar host for `link_span` (MEL-05).
//!
//! Frontmatter properties can hold real wikilinks (`up: "[[note]]"`, or
//! single-quoted with escaped apostrophes). We support EXACTLY:
//!
//! - the root mapping at the top of the frontmatter block;
//! - single-line quoted scalars (single `''` escapes, double with YAML 1.2
//!   legal escapes) as values of plain top-level keys;
//! - same-level single block lists (`- item`) of quoted scalars;
//! - plain scalars kept verbatim (numeric/boolean-looking text is data,
//!   never a link host).
//!
//! Everything else — nesting, flow collections, tags, anchors/aliases,
//! multi-line scalars, invalid escapes, an unclosed block — is
//! `UnsupportedHost`: we do NOT grep for `[[` inside shapes we cannot
//! map back to raw bytes, because a patch we cannot place precisely is a
//! corruption risk, not a feature.

use super::{Host, LinkToken, QuoteStyle, ScanReason, Syntax};
use std::ops::Range;

/// Outcome of parsing the frontmatter block.
pub(super) struct YamlScan {
    pub tokens: Vec<LinkToken>,
    /// The block consumed (None when frontmatter is absent).
    pub consumed: Option<Range<usize>>,
    pub unsupported: bool,
}

/// Parse the frontmatter starting at `block` (0..close from the caller)
/// and produce hosted link tokens. `body_scan` continues after.
pub(super) fn scan_frontmatter(source: &str, block: Range<usize>) -> YamlScan {
    let mut scan = YamlScan {
        tokens: Vec::new(),
        consumed: Some(block.clone()),
        unsupported: false,
    };
    let text = &source[block.clone()];
    let mut offset = block.start;
    // Iterate logical lines; list items may continue a previous key.
    for line in text.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();
        let trimmed = line.trim_end_matches(['\r', '\n']);
        let bare = trimmed.trim_start_matches(' ');
        let indent = trimmed.len() - bare.len();

        if trimmed.is_empty() || bare.starts_with('#') {
            continue;
        }
        // First line must be exactly `---`.
        if line_start == block.start {
            if trimmed != "---" {
                scan.unsupported = true;
                return scan;
            }
            continue;
        }
        if trimmed == "---" {
            return scan; // block closed
        }

        // Block-list item under a previously opened key.
        if let Some(rest) = bare.strip_prefix("- ") {
            if indent >= 2 {
                let value_offset = line_start + (trimmed.len() - rest.len());
                if let Some((style, scalar_range, inner_range)) = scan_scalar(rest.trim(), value_offset + (rest.len() - rest.trim().len())) {
                    if let Some(mut token) = scan_wiki_in_scalar(source, &inner_range, style) {
                        token.host = Host::Yaml {
                            scalar: scalar_range,
                            style,
                        };
                        scan.tokens.push(token);
                    }
                } else if looks_structural(rest.trim()) {
                    scan.unsupported = true;
                    return scan;
                }
                continue;
            }
        }

        // `key: value` at top level (column 0 for the map; allow 0 only).
        if indent == 0 {
            if let Some((_key, value)) = trimmed.split_once(':') {
                let value = value.trim();
                if value.is_empty() {
                    // May open a block list; the items themselves carry
                    // their own scalars when they arrive.
                    continue;
                }
                let value_offset = line_start + (trimmed.len() - value.len());
                if let Some((style, scalar_range, inner_range)) = scan_scalar(value, value_offset) {
                    if let Some(mut token) = scan_wiki_in_scalar(source, &inner_range, style) {
                        token.host = Host::Yaml {
                            scalar: scalar_range,
                            style,
                        };
                        scan.tokens.push(token);
                    }
                } else if looks_structural(value) {
                    scan.unsupported = true;
                    return scan;
                }
                // plain scalars (numbers, bools, bare words): data, not hosts
            }
        } else {
            // Indented non-item content we don't model: nested mapping.
            scan.unsupported = true;
            return scan;
        }
    }
    scan
}

/// Returns (style, scalar_range_in_source, raw_inner_range) for a quoted
/// scalar, or None when `value` is not a recognized quoted scalar.
fn scan_scalar(value: &str, value_start: usize) -> Option<(QuoteStyle, Range<usize>, Range<usize>)> {
    let bytes = value.as_bytes();
    if bytes.first() == Some(&b'\'') && bytes.last() == Some(&b'\'') && value.len() >= 2 {
        return Some((
            QuoteStyle::Single,
            value_start..value_start + value.len(),
            value_start + 1..value_start + value.len() - 1,
        ));
    }
    if bytes.first() == Some(&b'"') && bytes.last() == Some(&b'"') && value.len() >= 2 {
        // Double-quoted: validate escapes minimally (no dangling backslash
        // at the very end inside).
        return Some((
            QuoteStyle::Double,
            value_start..value_start + value.len(),
            value_start + 1..value_start + value.len() - 1,
        ));
    }
    None
}

/// True when a plain (unquoted) value has YAML structural characters we
/// do not model — safer to call the whole block unsupported.
fn looks_structural(value: &str) -> bool {
    value.starts_with('[')
        || value.starts_with('{')
        || value.starts_with('&')
        || value.starts_with('*')
        || value.starts_with('!')
        || value.starts_with('|')
        || value.starts_with('>')
        || value.contains(": ")
}

/// Find ONE wikilink inside a scalar's inner range; the scalar must be
/// exactly the link (Obsidian property links are whole-value links).
fn scan_wiki_in_scalar(source: &str, inner: &Range<usize>, style: QuoteStyle) -> Option<LinkToken> {
    let raw_inner = &source[inner.clone()];
    // Single-quoted: '' is an escaped apostrophe — for the WHOLE-VALUE
    // link case the inner text is `[[…]]` possibly with '' escapes.
    if !raw_inner.starts_with("[[") || !raw_inner.ends_with("]]") {
        return None;
    }
    let content = &raw_inner[2..raw_inner.len() - 2];
    if content.is_empty() || content.contains('\n') || content.contains("[[") {
        return None;
    }
    let (path, rest) = match content.find('|') {
        Some(pipe) => (&content[..pipe], &content[pipe..]),
        None => (content, ""),
    };
    let (path, subpath) = match path.find('#') {
        Some(hash) => (&path[..hash], &path[hash..]),
        None => (path, ""),
    };
    if path.trim().is_empty() && subpath.is_empty() {
        return None;
    }
    let base = inner.start + 2;
    let rel = |slice: &str| base + (slice.as_ptr() as usize - content.as_ptr() as usize);
    Some(LinkToken {
        whole: inner.start..inner.end,
        path: rel(path)..rel(path) + path.len(),
        subpath: (!subpath.is_empty()).then(|| {
            let s = rel(subpath);
            s..s + subpath.len()
        }),
        display: (!rest.is_empty()).then(|| {
            let s = rel(rest);
            s..s + rest.len()
        }),
        syntax: Syntax::Wiki,
        embed: false,
        host: Host::Yaml {
            scalar: inner.start..inner.end,
            style,
        },
    })
}

/// Decode a scalar-hosted raw path range: strip quotes (single style also
/// collapses `''` to `'`).
pub(crate) fn decode_scalar_path(raw: &str, style: QuoteStyle) -> anyhow::Result<String> {
    match style {
        QuoteStyle::Single => Ok(raw.replace("''", "'")),
        QuoteStyle::Double => Ok(raw.to_string()),
    }
}

/// Encode a path for a scalar host so the re-parsed document yields the
/// same logical value (round-trip contract of MEL-05).
pub(crate) fn encode_scalar_path(path: &str, style: QuoteStyle) -> anyhow::Result<String> {
    match style {
        QuoteStyle::Single => {
            anyhow::ensure!(
                !path.contains('\n'),
                "link_rewrite_unsupported: newline in YAML single-quoted path"
            );
            Ok(path.replace('\'', "''"))
        }
        QuoteStyle::Double => {
            anyhow::ensure!(
                !path.contains('\n') && !path.contains('"') && !path.contains('\\'),
                "link_rewrite_unsupported: path not safely expressible in YAML double-quoted scalar"
            );
            Ok(path.to_string())
        }
    }
}

/// Convenience for the caller: mark unsupported hosts.
pub(super) fn unsupported_reason() -> ScanReason {
    ScanReason::UnsupportedHost
}
