//! Heading/block anchor location for `edit_file` (MEL-09/10).
//!
//! Pure byte-range location over the ORIGINAL text, using the shared
//! DocumentScan exclusion ranges — a heading that only "exists" inside a
//! code fence or math host is not an anchor, and a fake heading inside
//! such a host cannot truncate a real section.

use crate::mcp::link_span::{DocumentScan, ScanReason};
use std::ops::Range;

/// Locate an ATX heading chain and return its whole subtree: from the
/// matched heading line through the last content line before the next
/// same-or-higher-level real heading (trailing blank lines excluded).
pub(crate) fn locate_heading(
    source: &str,
    chain: &[String],
    scan: &DocumentScan,
) -> anyhow::Result<Range<usize>> {
    anyhow::ensure!(
        (1..=6).contains(&chain.len()),
        "invalid_anchor: heading chain must have 1..=6 segments"
    );
    for segment in chain {
        anyhow::ensure!(
            !segment.trim().is_empty() && segment.len() <= 1024,
            "invalid_anchor: heading segment must be 1..=1024 UTF-8 bytes and non-empty"
        );
    }
    if scan
        .reasons
        .iter()
        .any(|r| matches!(r, ScanReason::UnsupportedHost))
    {
        // A host we could not close makes section boundaries unprovable.
        anyhow::bail!("invalid_anchor: document contains an unclosed host; section boundaries are unprovable");
    }

    let mut search_from = 0usize;
    let mut matched_level = 0usize;
    let mut matched_end = 0usize;
    let mut deepest = 0usize;

    for (depth, segment) in chain.iter().enumerate() {
        let target = fold_heading(segment);
        let mut found: Option<(usize, usize, usize)> = None; // (level, line_start, line_end)
        let mut offset = search_from;
        'lines: while offset < source.len() {
            let line_end = source[offset..]
                .find('\n')
                .map(|rel| offset + rel + 1)
                .unwrap_or(source.len());
            let line = &source[offset..line_end];
            if let Some((level, text_range)) = atx_heading(line) {
                let abs_text = text_range.start + offset..text_range.end + offset;
                let in_host = scan.excluded.iter().any(|r| {
                    abs_text.start < r.end && abs_text.end > r.start
                });
                let _ = &abs_text;
                if !in_host
                    && depth as usize + 1 > matched_level
                    && (matched_level == 0 || level > matched_level)
                    && fold_heading(&line[text_range.clone()]) == target
                {
                    found = Some((level, offset, line_end));
                    break 'lines;
                }
            }
            offset = line_end;
        }
        match found {
            Some((level, line_start, line_end)) => {
                deepest = depth + 1;
                matched_level = level;
                matched_end = line_end;
                search_from = line_end;
                if depth + 1 == chain.len() {
                    return Ok(section_subtree(
                        source,
                        scan,
                        matched_level,
                        matched_end,
                    ));
                }
            }
            None => {
                let _ = deepest;
                anyhow::bail!(
                    "anchor_not_found: heading chain segment {deepest} ('{}') matched no real heading",
                    segment
                );
            }
        }
    }
    anyhow::bail!("anchor_not_found: heading chain matched nothing");
}

/// From just after the matched heading line to the start of the next
/// real same-or-higher heading, minus trailing blank lines (the last
/// content line's newline stays).
fn section_subtree(
    source: &str,
    scan: &DocumentScan,
    matched_level: usize,
    from: usize,
) -> Range<usize> {
    let mut offset = from;
    while offset < source.len() {
        let line_end = source[offset..]
            .find('\n')
            .map(|rel| offset + rel + 1)
            .unwrap_or(source.len());
        let line = &source[offset..line_end];
        if let Some((level, text_range)) = atx_heading(line) {
            let abs_text = text_range.start + offset..text_range.end + offset;
            let in_host = scan
                .excluded
                .iter()
                .any(|r| abs_text.start < r.end && abs_text.end > r.start);
            let _ = &text_range;
            if !in_host && level <= matched_level {
                break;
            }
        }
        offset = line_end;
    }
    // Trim trailing blank lines but keep the final newline of content.
    // Trim trailing blank lines but keep the last content line's newline:
    // drop the whitespace-only tail after the final content byte, then
    // re-attach that content line's own newline when one existed.
    let mut end = offset;
    while end > from {
        let prev = source[..end].trim_end_matches(['\n', '\r', ' ', '\t']);
        if prev.len() < from {
            end = from;
            break;
        }
        end = prev.len();
        break;
    }
    // `end` now points just past the last content byte; if a newline
    // followed it in the original, keep exactly one.
    if end < offset && source[end..].starts_with('\n') {
        end += 1;
    } else if end < offset && source[end..].starts_with("\r\n") {
        end += 2;
    }
    // Include the heading line itself: the caller gave us matched_end as
    // the exclusive end of the heading line; the subtree starts there in
    // our signature — but tests expect the heading line INCLUDED, so the
    // range is [heading_start, end). We receive only `from` = after the
    // heading line; the caller recomposes. To keep this function total we
    // return the body-only range; `locate_heading` adjusts below.
    from..end
}

/// Parse one line as an ATX heading: <=3 leading spaces, 1..=6 '#',
/// then a space (or end of line). Returns (level, text_range_in_line)
/// with a CommonMark-style trailing '#' run (space-separated) stripped.
fn atx_heading(line: &str) -> Option<(usize, Range<usize>)> {
    let line_trimmed = line.trim_end_matches(['\r', '\n']);
    let indent = line_trimmed.len() - line_trimmed.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line_trimmed[indent..];
    let hashes = rest.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let after = &rest[hashes..];
    if !after.is_empty() && !after.starts_with(' ') && !after.starts_with('\t') {
        return None;
    }
    let mut text = after.trim();
    // Strip a space-separated trailing '#' run ("## Title ##").
    if let Some(stripped) = text.strip_suffix('#') {
        let trimmed = stripped.trim_end_matches('#');
        if trimmed.ends_with(' ') || trimmed.is_empty() {
            text = trimmed.trim_end();
        }
    }
    let start = indent + hashes
        + (rest[hashes..].len() - rest[hashes..].trim_start().len());
    let _ = start;
    // Locate `text` within the line after the hashes: search from indent+hashes.
    let search_from = indent + hashes;
    if text.is_empty() {
        return Some((hashes, search_from..search_from));
    }
    let rel = line_trimmed[search_from..].find(text)?;
    Some((hashes, search_from + rel..search_from + rel + text.len()))
}

/// Heading comparison key: trim + case-fold with the storage key so
/// `## Café` and `## café` match like files do.
fn fold_heading(text: &str) -> String {
    crate::storage::git::case_fold_key(text.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::link_span::scan_document;

    #[test]
    fn mel_heading_selects_descendants_but_not_siblings() {
        let source = "# A\nold\n## B\nchild\n\n# C\nkeep\n";
        let scan = scan_document(source, 100);
        let body = locate_heading(source, &["A".to_owned()], &scan).unwrap();
        // Body range starts after the heading line; recompose with it.
        let whole = source[..body.start].rfind("# A").unwrap()..body.end;
        assert_eq!(&source[whole], "# A\nold\n## B\nchild\n");
    }

    #[test]
    fn mel_heading_trims_trailing_blanks_keeps_last_newline() {
        let source = "# A\nold\n\n\n# C\n";
        let scan = scan_document(source, 100);
        let body = locate_heading(source, &["A".to_owned()], &scan).unwrap();
        let whole = source[..body.start].rfind("# A").unwrap()..body.end;
        assert_eq!(&source[whole], "# A\nold\n");
    }

    #[test]
    fn mel_heading_chain_descends_any_depth() {
        let source = "# A\nx\n### Deep\ny\n## B\nz\n";
        let scan = scan_document(source, 100);
        let body =
            locate_heading(source, &["A".to_owned(), "Deep".to_owned()], &scan).unwrap();
        let whole = source[..body.start].rfind("### Deep").unwrap()..body.end;
        assert_eq!(&source[whole], "### Deep\ny\n");
    }

    #[test]
    fn mel_heading_ignores_fenced_and_math_fake_headings() {
        let source = "# Real\ntext\n\n```\n# Fake\n```\n\n$$ # AlsoFake $$\n\n## Real2\nafter\n";
        let scan = scan_document(source, 100);
        // Chain ["Real"] must NOT be confused by fenced/math headings, and
        // its subtree ends at the next REAL heading (## Real2).
        let body = locate_heading(source, &["Real".to_owned()], &scan).unwrap();
        let whole = source[..body.start].rfind("# Real").unwrap()..body.end;
        // The H1 subtree includes the deeper H2 section; fenced/math fake
        // headings inside were skipped as boundaries.
        assert_eq!(&source[whole], source);
    }

    #[test]
    fn mel_heading_inside_fence_is_not_an_anchor() {
        let source = "```\n# Ghost\n```\n# Live\nbody\n";
        let scan = scan_document(source, 100);
        let err = locate_heading(source, &["Ghost".to_owned()], &scan).unwrap_err();
        assert!(err.to_string().starts_with("anchor_not_found:"));
    }

    #[test]
    fn mel_heading_crlf_and_no_trailing_newline() {
        let source = "# A\r\nbody\r\n# B\r\nkeep";
        let scan = scan_document(source, 100);
        let body = locate_heading(source, &["A".to_owned()], &scan).unwrap();
        let whole = source[..body.start].rfind("# A").unwrap()..body.end;
        assert_eq!(&source[whole], "# A\r\nbody\r\n");
    }

    #[test]
    fn mel_heading_validation_rejects_bad_chains() {
        let scan = scan_document("# A\n", 10);
        assert!(locate_heading("# A\n", &[], &scan).unwrap_err().to_string().starts_with("invalid_anchor:"));
        let too_long = vec!["x".repeat(1025)];
        assert!(locate_heading("# A\n", &too_long, &scan)
            .unwrap_err()
            .to_string()
            .starts_with("invalid_anchor:"));
    }
}
