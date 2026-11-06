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
        anyhow::bail!(
            "invalid_anchor: document contains an unclosed host; section boundaries are unprovable"
        );
    }

    let mut search_from = 0usize;
    let mut matched_level = 0usize;
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
                let host = in_host(scan, abs_text.start, abs_text.end);
                if !host
                    && depth + 1 > matched_level
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
                search_from = line_end;
                if depth + 1 == chain.len() {
                    let body = section_subtree(source, scan, matched_level, line_end);
                    // The subtree INCLUDES its own heading line. body.start
                    // sits just past the heading line's newline; the last
                    // '\n' strictly before body.start-1 opens the heading
                    // line. When no such newline exists the heading line
                    // starts at 0.
                    let heading_start = source[..body.start.saturating_sub(1)]
                        .rfind('\n')
                        .map(|p| p + 1)
                        .unwrap_or(0);
                    return Ok(heading_start.max(line_start)..body.end);
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
            let host = in_host(scan, abs_text.start, abs_text.end);
            if !host && level <= matched_level {
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
    if end > from {
        let prev = source[..end].trim_end_matches(['\n', '\r', ' ', '\t']);
        end = if prev.len() < from { from } else { prev.len() };
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
    let start = indent + hashes + (rest[hashes..].len() - rest[hashes..].trim_start().len());
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

/// Locate a `^block-id` target (MEL-10). Two shapes with DIFFERENT scope:
/// - item form: `... ^id` at the end of a LIST-ITEM line — the block is
///   that single item (marker through its continuation lines/sublist,
///   excluding sibling items);
/// - standalone form: `^id` on its own line — the block is the preceding
///   complete supported block (a whole list, paragraph, etc.) across the
///   blank lines that separate them, plus the id line itself.
pub(crate) fn locate_block(
    source: &str,
    id: &str,
    scan: &DocumentScan,
) -> anyhow::Result<Range<usize>> {
    anyhow::ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "invalid_anchor: block id must be 1..=128 bytes of [A-Za-z0-9-]"
    );

    let mut item_hits: Vec<Range<usize>> = Vec::new();
    let mut standalone_hits: Vec<Range<usize>> = Vec::new();
    let mut offset = 0usize;
    while offset < source.len() {
        let line_end = source[offset..]
            .find('\n')
            .map(|rel| offset + rel + 1)
            .unwrap_or(source.len());
        let line = source[offset..line_end].trim_end_matches(['\r', '\n']);
        let trimmed = line.trim_start_matches([' ', '\t']);
        let line_start = offset + (line.len() - trimmed.len());
        if !in_host(scan, line_start, line_end) {
            if trimmed == format!("^{id}") {
                standalone_hits.push(line_start..line_end);
            } else if trimmed.ends_with(&format!(" ^{id}")) {
                item_hits.push(line_start..line_end);
            }
        }
        offset = line_end;
    }
    let total = item_hits.len() + standalone_hits.len();
    if total == 0 {
        anyhow::bail!("anchor_not_found: no block carries id '{id}'");
    }
    if total > 1 {
        anyhow::bail!(
            "anchor_ambiguous: block id '{id}' appears {total} times; block ids must be unique"
        );
    }
    if let Some(hit) = item_hits.pop() {
        return Ok(item_block_range(source, scan, hit));
    }
    let hit = standalone_hits.pop().expect("total == 1");
    Ok(standalone_block_range(source, scan, hit))
}

/// O(log n) host-zone probe over the scan's sorted, coalesced exclusion
/// ranges (R13-4: the linear any() sweep made locate calls O(H*E) on
/// documents with many headings and many exclusion ranges).
fn in_host(scan: &DocumentScan, start: usize, end: usize) -> bool {
    let idx = scan.excluded.partition_point(|r| r.end <= start);
    scan.excluded
        .get(idx)
        .is_some_and(|r| start < r.end && end > r.start)
}

/// The item form: from the marker of the item carrying the id, through its
/// continuation lines and nested sublists, ending before the next sibling.
#[allow(clippy::while_let_loop)]
fn item_block_range(source: &str, scan: &DocumentScan, id_line: Range<usize>) -> Range<usize> {
    let id_line_text = &source[id_line.clone()];
    let id_indent = leading_spaces(id_line_text);
    let mut start = id_line.start;
    loop {
        let Some(prev_nl) = source[..start].rfind('\n') else {
            break;
        };
        if prev_nl == 0 {
            break;
        }
        let prev_start = source[..prev_nl].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let prev_trimmed = source[prev_start..prev_nl].trim_end_matches(['\r', '\n']);
        if prev_trimmed.trim().is_empty() {
            break;
        }
        let prev_indent = leading_spaces(prev_trimmed);
        if prev_indent > id_indent && !in_host(scan, prev_start, prev_nl) {
            start = prev_start;
            continue;
        }
        break;
    }
    let mut end = id_line.end;
    loop {
        let Some(next_nl) = source[end..].find('\n') else {
            break;
        };
        let next_start = end;
        let next_end = next_start + next_nl + 1;
        let next_line = &source[next_start..next_end];
        let next_trimmed = next_line.trim_end_matches(['\r', '\n']);
        if next_trimmed.trim().is_empty() {
            break;
        }
        let next_indent = leading_spaces(next_trimmed);
        if next_indent > id_indent && !in_host(scan, next_start, next_end) {
            end = next_end;
            continue;
        }
        break;
    }
    start..end
}

/// The standalone form: associate the id line with the previous complete
/// block across blank lines; return that block plus the id line. A whole
/// list never collapses to its last item.
#[allow(clippy::while_let_loop)]
fn standalone_block_range(
    source: &str,
    scan: &DocumentScan,
    id_line: Range<usize>,
) -> Range<usize> {
    let mut cursor = id_line.start;
    loop {
        let Some(prev_nl) = source[..cursor].rfind('\n') else {
            break;
        };
        if prev_nl == 0 {
            break;
        }
        let prev_start = source[..prev_nl].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let prev_trimmed = source[prev_start..prev_nl].trim_end_matches(['\r', '\n']);
        if prev_trimmed.trim().is_empty() {
            cursor = prev_start;
            continue;
        }
        if in_host(scan, prev_start, prev_nl) {
            break;
        }
        if is_list_item(prev_trimmed) {
            let list_start = list_start_above(source, scan, prev_start);
            return list_start..id_line.end;
        }
        // Paragraph: extend up through consecutive non-blank lines.
        let mut para_start = prev_start;
        loop {
            let Some(above_nl) = source[..para_start].rfind('\n') else {
                break;
            };
            if above_nl == 0 {
                break;
            }
            let above_start = source[..above_nl].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let above_line = source[above_start..above_nl].trim_end_matches(['\r', '\n']);
            if above_line.trim().is_empty() || in_host(scan, above_start, above_nl) {
                break;
            }
            para_start = above_start;
        }
        return para_start..id_line.end;
    }
    id_line.start..id_line.end
}

#[allow(clippy::while_let_loop)]
fn list_start_above(source: &str, scan: &DocumentScan, from: usize) -> usize {
    let mut start = from;
    loop {
        let Some(prev_nl) = source[..start].rfind('\n') else {
            break;
        };
        if prev_nl == 0 {
            break;
        }
        let prev_start = source[..prev_nl].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let prev_line = source[prev_start..prev_nl].trim_end_matches(['\r', '\n']);
        if prev_line.trim().is_empty() || in_host(scan, prev_start, prev_nl) {
            break;
        }
        if is_list_item(prev_line) || leading_spaces(prev_line) > 0 {
            start = prev_start;
            continue;
        }
        break;
    }
    start
}

fn is_list_item(line: &str) -> bool {
    let t = line.trim_start_matches([' ', '\t']);
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    digits > 0 && t[digits..].starts_with(". ")
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::link_span::scan_document;

    #[test]
    fn mel_heading_selects_descendants_but_not_siblings() {
        let source = "# A\nold\n## B\nchild\n\n# C\nkeep\n";
        let scan = scan_document(source, 100);
        let whole = locate_heading(source, &["A".to_owned()], &scan).unwrap();
        assert_eq!(&source[whole], "# A\nold\n## B\nchild\n");
    }

    #[test]
    fn mel_heading_trims_trailing_blanks_keeps_last_newline() {
        let source = "# A\nold\n\n\n# C\n";
        let scan = scan_document(source, 100);
        let whole = locate_heading(source, &["A".to_owned()], &scan).unwrap();
        assert_eq!(&source[whole], "# A\nold\n");
    }

    #[test]
    fn mel_heading_chain_descends_any_depth() {
        let source = "# A\nx\n### Deep\ny\n## B\nz\n";
        let scan = scan_document(source, 100);
        let whole = locate_heading(source, &["A".to_owned(), "Deep".to_owned()], &scan).unwrap();
        assert_eq!(&source[whole], "### Deep\ny\n");
    }

    #[test]
    fn mel_heading_ignores_fenced_and_math_fake_headings() {
        let source = "# Real\ntext\n\n```\n# Fake\n```\n\n$$ # AlsoFake $$\n\n## Real2\nafter\n";
        let scan = scan_document(source, 100);
        // Chain ["Real"] must NOT be confused by fenced/math headings, and
        // its subtree ends at the next REAL heading (## Real2).
        let whole = locate_heading(source, &["Real".to_owned()], &scan).unwrap();
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
        let whole = locate_heading(source, &["A".to_owned()], &scan).unwrap();
        assert_eq!(&source[whole], "# A\r\nbody\r\n");
    }

    #[test]
    fn mel_heading_validation_rejects_bad_chains() {
        let scan = scan_document("# A\n", 10);
        assert!(locate_heading("# A\n", &[], &scan)
            .unwrap_err()
            .to_string()
            .starts_with("invalid_anchor:"));
        let too_long = vec!["x".repeat(1025)];
        assert!(locate_heading("# A\n", &too_long, &scan)
            .unwrap_err()
            .to_string()
            .starts_with("invalid_anchor:"));
    }

    #[test]
    fn mel_block_item_does_not_select_siblings() {
        let source = "- keep\n- edit ^id\n- tail\n";
        let scan = scan_document(source, 100);
        let range = locate_block(source, "id", &scan).unwrap();
        assert_eq!(&source[range.clone()], "- edit ^id\n");
        let mut result = source.to_owned();
        result.replace_range(range, "- new\n");
        assert_eq!(result, "- keep\n- new\n- tail\n");
    }

    #[test]
    fn mel_block_standalone_selects_the_whole_list() {
        let source = "- a\n- b\n\n^list\n\nTail\n";
        let scan = scan_document(source, 100);
        let range = locate_block(source, "list", &scan).unwrap();
        let mut result = source.to_owned();
        result.replace_range(range, "new\n");
        assert_eq!(result, "new\n\nTail\n");
    }

    #[test]
    fn mel_block_standalone_paragraph_crosses_no_lists() {
        let source = "Intro line one.\nIntro line two.\n\n^para\n\nAfter\n";
        let scan = scan_document(source, 100);
        let range = locate_block(source, "para", &scan).unwrap();
        assert_eq!(
            &source[range],
            "Intro line one.\nIntro line two.\n\n^para\n"
        );
    }

    #[test]
    fn mel_block_duplicate_id_is_ambiguous() {
        let source = "- a ^x\n\n- b ^x\n";
        let scan = scan_document(source, 100);
        let err = locate_block(source, "x", &scan).unwrap_err();
        assert!(err.to_string().starts_with("anchor_ambiguous:"));
    }

    #[test]
    fn mel_block_id_inside_code_or_math_is_invisible() {
        let source = "```\n- a ^ghost\n```\n$$ ^spook $$\n- real ^id\n";
        let scan = scan_document(source, 100);
        let range = locate_block(source, "id", &scan).unwrap();
        assert_eq!(&source[range], "- real ^id\n");
        assert!(locate_block(source, "ghost", &scan)
            .unwrap_err()
            .to_string()
            .starts_with("anchor_not_found:"));
        assert!(locate_block(source, "spook", &scan)
            .unwrap_err()
            .to_string()
            .starts_with("anchor_not_found:"));
    }

    #[test]
    fn mel_block_id_validation() {
        let scan = scan_document("x\n", 10);
        assert!(locate_block("x\n", "", &scan)
            .unwrap_err()
            .to_string()
            .starts_with("invalid_anchor:"));
        assert!(locate_block("x\n", "bad_id", &scan)
            .unwrap_err()
            .to_string()
            .starts_with("invalid_anchor:"));
        assert!(locate_block("x\n", &"i".repeat(129), &scan)
            .unwrap_err()
            .to_string()
            .starts_with("invalid_anchor:"));
    }

    #[test]
    fn mel_block_item_includes_nested_sublist() {
        let source = "- sibling\n- item ^id\n  - nested\n  - more\n- next\n";
        let scan = scan_document(source, 100);
        let range = locate_block(source, "id", &scan).unwrap();
        assert_eq!(&source[range], "- item ^id\n  - nested\n  - more\n");
    }
}
