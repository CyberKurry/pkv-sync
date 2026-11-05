//! Math host boundaries for `link_span` (MEL-R2-02).
//!
//! Obsidian-style math (`$…$` inline, `$$…$$` block) owns its content:
//! no links, headings, or block IDs inside. We only find the DELIMITERS
//! — TeX is never executed and never validated beyond pairing rules:
//!
//! - `$$` takes precedence: opening and closing runs must match in
//!   length; block math may span lines.
//! - Single `$` does not cross a blank line (paragraph boundary).
//! - An odd backslash escapes a delimiter (`\$` is not one).
//! - An unpaired single `$` returns the bytes to the body; an unclosed
//!   `$$` marks the document `UnsupportedHost` (we cannot prove where
//!   the host ends, so no statement inside it may be trusted).

/// Returns the end offset (exclusive) of the math host starting at
/// `open` (which points at the first `$`), or `None` when it cannot be
/// closed on the current rules.
pub(super) fn math_host_end(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let dollars = bytes[open..]
        .iter()
        .take_while(|&&b| b == b'$')
        .count();
    if dollars >= 2 {
        // Block math: find a closing $$ run of the same length.
        let want = dollars;
        let mut i = open + dollars;
        while i < bytes.len() {
            if bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'$' {
                let run = bytes[i..]
                    .iter()
                    .take_while(|&&b| b == b'$')
                    .count();
                if run == want {
                    return Some(i + run);
                }
                // A different-length run cannot close this host; skip it.
                i += run;
                continue;
            }
            i += 1;
        }
        None
    } else {
        // Inline math: single $, same line, no blank line between.
        let mut i = open + 1;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += 2,
                b'\n' => {
                    // A blank line (next line empty) ends the paragraph —
                    // the single $ goes back to the body.
                    let rest = &source[i..];
                    let next_line_end = rest.find('\n').map(|r| i + r + 1).unwrap_or(source.len());
                    let next = source[i + 1..next_line_end].trim();
                    if next.is_empty() {
                        return None;
                    }
                    i += 1;
                }
                b'$' => return Some(i + 1),
                _ => i += 1,
            }
        }
        None
    }
}
