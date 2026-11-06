//! Inline/block HTML host boundaries for `link_span` (MEL-R2-03).
//!
//! HTML elements own their tags and (for raw-text elements) their
//! content: no links inside. We track:
//!
//! - ASCII tag names, case-insensitive;
//! - attribute values in quotes — a `>` inside quotes never closes a tag;
//! - open/close/void elements and a nesting stack, so `<span><b>…</b>
//!   </span>` excludes one contiguous range;
//! - raw-text elements (`script`/`style`/`textarea`/`title`) whose
//!   content is not rescanned for pseudo-tags;
//! - `<!-- … -->` comments (already handled at block level; the inline
//!   pass reuses that range).
//!
//! No browser error-correction: an element we cannot prove closed marks
//! the document `UnsupportedHost` via the caller, not a guess.

/// Raw-text HTML elements whose content must not be parsed as markup.
const RAW_TEXT: [&str; 4] = ["script", "style", "textarea", "title"];
/// Void elements: no closing tag, own only their own tag bytes.
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Returns (end_of_open_tag, tag_name, is_void) for the tag starting at
/// `open` (points at `<`). `end_of_open_tag` is just past `>` (or `/>`).
pub(super) fn open_tag(source: &str, open: usize) -> Option<(usize, String, bool)> {
    let bytes = source.as_bytes();
    if bytes.get(open + 1).map(|b| b.is_ascii_alphabetic()) != Some(true) {
        return None; // </close>, <!--, <!doctype>, <?…> are not open tags
    }
    let mut i = open + 1;
    let name_start = i;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
        i += 1;
    }
    let name = source[name_start..i].to_ascii_lowercase();
    // Walk attributes until the tag's `>`, honoring quotes. A '<' inside
    // the tag body means this is actually nested markup (our caller only
    // hands us well-formed single tags); treat as not-an-open-tag rather
    // than consuming the nested element.
    while i < bytes.len() {
        match bytes[i] {
            b'<' => return None,
            b'"' | b'\'' => {
                let quote = bytes[i];
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != quote {
                    if bytes[j] == b'\\' {
                        j += 1;
                    }
                    j += 1;
                }
                if j >= bytes.len() {
                    return None; // unterminated attribute value
                }
                i = j + 1;
            }
            b'/' => {
                if bytes.get(i + 1) == Some(&b'>') {
                    let void = VOID.contains(&name.as_str());
                    // Self-closing non-void still only owns this tag (the
                    // caller decides completeness; XHTML-style is accepted
                    // as written).
                    return Some((i + 2, name, void));
                }
                i += 1;
            }
            b'>' => {
                // Raw-text elements are NOT void here: they own content
                // until </name>. The caller consults is_raw_text separately.
                let void = VOID.contains(&name.as_str());
                return Some((i + 1, name, void));
            }
            _ => i += 1,
        }
    }
    None
}

/// Returns the offset of the closing `</name …>` (just past `>`), or None.
pub(super) fn find_close_tag(source: &str, from: usize, name: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'<' && bytes.get(i + 1) == Some(&b'/') {
            let mut j = i + 2;
            let name_start = j;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
                j += 1;
            }
            if source[name_start..j].eq_ignore_ascii_case(name) {
                // Skip to the `>` of this close tag (attributes illegal but
                // tolerated through whitespace/attr bytes).
                while j < bytes.len() && bytes[j] != b'>' {
                    j += 1;
                }
                if j < bytes.len() {
                    return Some(j + 1);
                }
                return None;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    None
}

pub(super) fn is_raw_text(name: &str) -> bool {
    RAW_TEXT.contains(&name)
}
