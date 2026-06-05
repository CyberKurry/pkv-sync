use serde::Serialize;
use similar::{DiffTag, TextDiff};

const MAX_MERGE_LINES: usize = 100_000;
const NUL: u8 = 0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MergeOutcome {
    Clean(String),
    Conflicted(String),
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Hunk<'a> {
    base_start: usize,
    base_end: usize,
    replacement: Vec<&'a str>,
}

impl<'a> Hunk<'a> {
    fn is_insert(&self) -> bool {
        self.base_start == self.base_end
    }
}

pub fn three_way_merge_bytes(base: &[u8], local: &[u8], remote: &[u8]) -> MergeOutcome {
    if base.contains(&NUL) || local.contains(&NUL) || remote.contains(&NUL) {
        return MergeOutcome::Binary;
    }

    let (Ok(base), Ok(local), Ok(remote)) = (
        std::str::from_utf8(base),
        std::str::from_utf8(local),
        std::str::from_utf8(remote),
    ) else {
        return MergeOutcome::Binary;
    };

    three_way_merge(base, local, remote)
}

pub fn three_way_merge(base: &str, local: &str, remote: &str) -> MergeOutcome {
    let base_lines = split_lines(base);
    let local_lines = split_lines(local);
    let remote_lines = split_lines(remote);

    if [base_lines.len(), local_lines.len(), remote_lines.len()]
        .iter()
        .any(|line_count| *line_count > MAX_MERGE_LINES)
    {
        return MergeOutcome::Binary;
    }

    if local == remote {
        return MergeOutcome::Clean(local.to_string());
    }
    if base == local {
        return MergeOutcome::Clean(remote.to_string());
    }
    if base == remote {
        return MergeOutcome::Clean(local.to_string());
    }

    let local_hunks = diff_hunks(&base_lines, &local_lines);
    let remote_hunks = diff_hunks(&base_lines, &remote_lines);
    let (merged, has_conflict) = merge_hunks(&base_lines, &local_hunks, &remote_hunks);
    if has_conflict {
        MergeOutcome::Conflicted(merged)
    } else {
        MergeOutcome::Clean(merged)
    }
}

fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut start = 0;
    for (idx, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            lines.push(&text[start..=idx]);
            start = idx + 1;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

fn diff_hunks<'a>(base: &[&str], changed: &'a [&'a str]) -> Vec<Hunk<'a>> {
    TextDiff::from_slices(base, changed)
        .ops()
        .iter()
        .filter_map(|op| {
            let (tag, base_range, changed_range) = op.as_tag_tuple();
            if tag == DiffTag::Equal {
                return None;
            }

            Some(Hunk {
                base_start: base_range.start,
                base_end: base_range.end,
                replacement: changed[changed_range].to_vec(),
            })
        })
        .collect()
}

fn merge_hunks<'a>(
    base: &'a [&'a str],
    local_hunks: &[Hunk<'a>],
    remote_hunks: &[Hunk<'a>],
) -> (String, bool) {
    let mut merged = Vec::new();
    let mut has_conflict = false;
    let mut base_cursor = 0;
    let mut local_idx = 0;
    let mut remote_idx = 0;

    while local_idx < local_hunks.len() || remote_idx < remote_hunks.len() {
        let next_local = local_hunks.get(local_idx);
        let next_remote = remote_hunks.get(remote_idx);
        let next_start = match (next_local, next_remote) {
            (Some(local), Some(remote)) => local.base_start.min(remote.base_start),
            (Some(local), None) => local.base_start,
            (None, Some(remote)) => remote.base_start,
            (None, None) => break,
        };

        append_lines(&mut merged, &base[base_cursor..next_start]);
        base_cursor = next_start;

        match (next_local, next_remote) {
            (Some(local), Some(remote)) if hunks_overlap(local, remote) => {
                let (start, end, next_local_idx, next_remote_idx) =
                    collect_overlapping_region(local_hunks, remote_hunks, local_idx, remote_idx);
                let local_segment =
                    apply_region(base, start, end, &local_hunks[local_idx..next_local_idx]);
                let remote_segment =
                    apply_region(base, start, end, &remote_hunks[remote_idx..next_remote_idx]);
                let base_segment = base[start..end].to_vec();

                if local_segment == remote_segment {
                    append_lines(&mut merged, &local_segment);
                } else if local_segment == base_segment {
                    append_lines(&mut merged, &remote_segment);
                } else if remote_segment == base_segment {
                    append_lines(&mut merged, &local_segment);
                } else if let Some(segment) =
                    merge_trailing_append(&base_segment, &local_segment, &remote_segment)
                {
                    append_lines(&mut merged, &segment);
                } else {
                    has_conflict = true;
                    append_conflict(&mut merged, &local_segment, &remote_segment);
                }

                base_cursor = end;
                local_idx = next_local_idx;
                remote_idx = next_remote_idx;
            }
            (Some(local), Some(remote)) if local.base_start <= remote.base_start => {
                append_lines(&mut merged, &local.replacement);
                base_cursor = local.base_end;
                local_idx += 1;
            }
            (Some(_), Some(remote)) => {
                append_lines(&mut merged, &remote.replacement);
                base_cursor = remote.base_end;
                remote_idx += 1;
            }
            (Some(local), None) => {
                append_lines(&mut merged, &local.replacement);
                base_cursor = local.base_end;
                local_idx += 1;
            }
            (None, Some(remote)) => {
                append_lines(&mut merged, &remote.replacement);
                base_cursor = remote.base_end;
                remote_idx += 1;
            }
            (None, None) => break,
        }
    }

    append_lines(&mut merged, &base[base_cursor..]);
    (merged.concat(), has_conflict)
}

fn collect_overlapping_region(
    local_hunks: &[Hunk<'_>],
    remote_hunks: &[Hunk<'_>],
    local_idx: usize,
    remote_idx: usize,
) -> (usize, usize, usize, usize) {
    let mut start = local_hunks[local_idx]
        .base_start
        .min(remote_hunks[remote_idx].base_start);
    let mut end = local_hunks[local_idx]
        .base_end
        .max(remote_hunks[remote_idx].base_end);
    let mut next_local_idx = local_idx;
    let mut next_remote_idx = remote_idx;

    loop {
        let mut changed = false;

        while let Some(hunk) = local_hunks.get(next_local_idx) {
            if !hunk_overlaps_region(hunk, start, end) {
                break;
            }
            start = start.min(hunk.base_start);
            end = end.max(hunk.base_end);
            next_local_idx += 1;
            changed = true;
        }

        while let Some(hunk) = remote_hunks.get(next_remote_idx) {
            if !hunk_overlaps_region(hunk, start, end) {
                break;
            }
            start = start.min(hunk.base_start);
            end = end.max(hunk.base_end);
            next_remote_idx += 1;
            changed = true;
        }

        if !changed {
            break;
        }
    }

    (start, end, next_local_idx, next_remote_idx)
}

fn apply_region<'a>(
    base: &'a [&'a str],
    start: usize,
    end: usize,
    hunks: &[Hunk<'a>],
) -> Vec<&'a str> {
    let mut lines = Vec::new();
    let mut cursor = start;

    for hunk in hunks {
        if hunk.base_start > cursor {
            lines.extend_from_slice(&base[cursor..hunk.base_start]);
        }
        lines.extend_from_slice(&hunk.replacement);
        cursor = hunk.base_end.max(cursor);
    }

    if cursor < end {
        lines.extend_from_slice(&base[cursor..end]);
    }

    lines
}

fn merge_trailing_append<'a>(
    base: &[&'a str],
    local: &[&'a str],
    remote: &[&'a str],
) -> Option<Vec<&'a str>> {
    merge_one_sided_trailing_append(base, local, remote)
        .or_else(|| merge_one_sided_trailing_append(base, remote, local))
}

fn merge_one_sided_trailing_append<'a>(
    base: &[&'a str],
    edited: &[&'a str],
    appended: &[&'a str],
) -> Option<Vec<&'a str>> {
    if base.len() != 1 || edited.is_empty() || appended.len() < 2 || base[0].ends_with('\n') {
        return None;
    }
    if appended[0].strip_suffix('\n') != Some(base[0]) {
        return None;
    }

    let mut merged = edited.to_vec();
    ensure_newline(&mut merged);
    merged.extend_from_slice(&appended[1..]);
    Some(merged)
}

fn hunks_overlap(left: &Hunk<'_>, right: &Hunk<'_>) -> bool {
    if left.is_insert() && right.is_insert() {
        return left.base_start == right.base_start;
    }
    if left.is_insert() {
        return right.base_start <= left.base_start && left.base_start < right.base_end;
    }
    if right.is_insert() {
        return left.base_start <= right.base_start && right.base_start < left.base_end;
    }
    left.base_start < right.base_end && right.base_start < left.base_end
}

fn hunk_overlaps_region(hunk: &Hunk<'_>, start: usize, end: usize) -> bool {
    if hunk.is_insert() {
        if start == end {
            hunk.base_start == start
        } else {
            start <= hunk.base_start && hunk.base_start < end
        }
    } else if start == end {
        hunk.base_start <= start && start < hunk.base_end
    } else {
        hunk.base_start < end && start < hunk.base_end
    }
}

fn append_conflict<'a>(merged: &mut Vec<&'a str>, local: &[&'a str], remote: &[&'a str]) {
    merged.push("<<<<<<< local\n");
    append_lines(merged, local);
    ensure_newline(merged);
    merged.push("=======\n");
    append_lines(merged, remote);
    ensure_newline(merged);
    merged.push(">>>>>>> remote\n");
}

fn append_lines<'a>(merged: &mut Vec<&'a str>, lines: &[&'a str]) {
    merged.extend_from_slice(lines);
}

fn ensure_newline(merged: &mut Vec<&str>) {
    if merged.last().is_some_and(|line| !line.ends_with('\n')) {
        merged.push("\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanly_merges_independent_edits() {
        let base = "one\ntwo\nthree\n";
        let local = "one\nTWO\nthree\n";
        let remote = "one\ntwo\nTHREE\n";

        let merged = three_way_merge(base, local, remote);

        assert_eq!(merged, MergeOutcome::Clean("one\nTWO\nTHREE\n".to_string()));
    }

    #[test]
    fn marks_conflicting_edits() {
        let merged = three_way_merge("one\ntwo\n", "one\nlocal\n", "one\nremote\n");

        let MergeOutcome::Conflicted(text) = merged else {
            panic!("expected conflict");
        };
        assert!(text.contains("<<<<<<< local\nlocal\n=======\nremote\n>>>>>>> remote\n"));
    }

    #[test]
    fn preserves_final_line_without_newline() {
        let merged = three_way_merge("one\ntwo", "one\nlocal", "one\ntwo\nremote");

        assert_eq!(
            merged,
            MergeOutcome::Clean("one\nlocal\nremote".to_string())
        );
    }

    #[test]
    fn preserves_final_line_without_newline_when_append_is_local() {
        let merged = three_way_merge("one\ntwo", "one\ntwo\nlocal", "one\nremote");

        assert_eq!(
            merged,
            MergeOutcome::Clean("one\nremote\nlocal".to_string())
        );
    }

    #[test]
    fn split_lines_borrows_input_lines() {
        let source = include_str!("merge.rs");
        let fn_start = source
            .find("fn split_lines")
            .expect("split_lines implementation exists");
        let next_fn = source[fn_start + 1..]
            .find("\nfn ")
            .map(|idx| fn_start + 1 + idx)
            .expect("following function exists");
        let implementation = &source[fn_start..next_fn];

        assert!(implementation.contains("Vec<&str>"));
        assert!(!implementation.contains(".to_string()"));
    }
}
