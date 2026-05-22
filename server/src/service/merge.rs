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
struct Hunk {
    base_start: usize,
    base_end: usize,
    replacement: Vec<String>,
}

impl Hunk {
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

fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut start = 0;
    for (idx, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            lines.push(text[start..=idx].to_string());
            start = idx + 1;
        }
    }
    if start < text.len() {
        lines.push(text[start..].to_string());
    }
    lines
}

fn diff_hunks(base: &[String], changed: &[String]) -> Vec<Hunk> {
    let base_refs: Vec<&str> = base.iter().map(String::as_str).collect();
    let changed_refs: Vec<&str> = changed.iter().map(String::as_str).collect();

    TextDiff::from_slices(&base_refs, &changed_refs)
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

fn merge_hunks(base: &[String], local_hunks: &[Hunk], remote_hunks: &[Hunk]) -> (String, bool) {
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
    local_hunks: &[Hunk],
    remote_hunks: &[Hunk],
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

fn apply_region(base: &[String], start: usize, end: usize, hunks: &[Hunk]) -> Vec<String> {
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

fn hunks_overlap(left: &Hunk, right: &Hunk) -> bool {
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

fn hunk_overlaps_region(hunk: &Hunk, start: usize, end: usize) -> bool {
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

fn append_conflict(merged: &mut Vec<String>, local: &[String], remote: &[String]) {
    merged.push("<<<<<<< local\n".to_string());
    append_lines(merged, local);
    ensure_newline(merged);
    merged.push("=======\n".to_string());
    append_lines(merged, remote);
    ensure_newline(merged);
    merged.push(">>>>>>> remote\n".to_string());
}

fn append_lines(merged: &mut Vec<String>, lines: &[String]) {
    merged.extend_from_slice(lines);
}

fn ensure_newline(merged: &mut Vec<String>) {
    if merged.last().is_some_and(|line| !line.ends_with('\n')) {
        merged.push("\n".to_string());
    }
}
