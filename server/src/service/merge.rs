use serde::Serialize;
use similar::{DiffTag, TextDiff};
use std::borrow::Cow;

const MAX_MERGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_MERGE_LINES: usize = 100_000;
const MAX_MERGE_LINE_BYTES: usize = 64 * 1024;
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

    if merge_input_too_large(base, &base_lines)
        || merge_input_too_large(local, &local_lines)
        || merge_input_too_large(remote, &remote_lines)
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

fn merge_input_too_large(text: &str, lines: &[&str]) -> bool {
    text.len() > MAX_MERGE_BYTES
        || lines.len() > MAX_MERGE_LINES
        || lines.iter().any(|line| line.len() > MAX_MERGE_LINE_BYTES)
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
    let mut merged: Vec<Cow<'a, str>> = Vec::new();
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
                } else if let Some(merged_line) =
                    try_subline_char_merge(&base_segment, &local_segment, &remote_segment)
                {
                    merged.extend(merged_line.into_iter().map(Cow::Owned));
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
    (
        merged.iter().map(|line| line.as_ref()).collect::<String>(),
        has_conflict,
    )
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

fn append_conflict<'a>(merged: &mut Vec<Cow<'a, str>>, local: &[&'a str], remote: &[&'a str]) {
    merged.push(Cow::Borrowed("<<<<<<< local\n"));
    append_lines(merged, local);
    ensure_newline(merged);
    merged.push(Cow::Borrowed("=======\n"));
    append_lines(merged, remote);
    ensure_newline(merged);
    merged.push(Cow::Borrowed(">>>>>>> remote\n"));
}

fn append_lines<'a>(merged: &mut Vec<Cow<'a, str>>, lines: &[&'a str]) {
    merged.extend(lines.iter().map(|line| Cow::Borrowed(*line)));
}

fn ensure_newline<'a, T: AsRef<str> + From<&'a str>>(merged: &mut Vec<T>) {
    if merged
        .last()
        .is_some_and(|line| !line.as_ref().ends_with('\n'))
    {
        merged.push(T::from("\n"));
    }
}

// Character-level three-way merge primitives (Part A), wired into `merge_hunks`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CharHunk {
    base_start: usize, // char index into base
    base_end: usize,   // exclusive
    replacement: String,
}

fn char_diff_hunks(base: &str, changed: &str) -> Vec<CharHunk> {
    let changed_chars: Vec<char> = changed.chars().collect();
    let mut hunks = Vec::new();
    let diff = TextDiff::from_chars(base, changed);
    for op in diff.ops() {
        let (tag, base_range, changed_range) = op.as_tag_tuple();
        if tag == DiffTag::Equal {
            continue;
        }
        hunks.push(CharHunk {
            base_start: base_range.start,
            base_end: base_range.end,
            replacement: changed_chars[changed_range].iter().collect(),
        });
    }
    hunks
}

fn char_hunks_overlap(local: &[CharHunk], remote: &[CharHunk]) -> bool {
    for l in local {
        for r in remote {
            let l_ins = l.base_start == l.base_end;
            let r_ins = r.base_start == r.base_end;
            let conflict = if l_ins && r_ins {
                l.base_start == r.base_start
            } else if l_ins {
                r.base_start < l.base_start && l.base_start < r.base_end
            } else if r_ins {
                l.base_start < r.base_start && r.base_start < l.base_end
            } else {
                l.base_start < r.base_end && r.base_start < l.base_end
            };
            if conflict {
                return true;
            }
        }
    }
    false
}

fn apply_char_hunks(base: &str, local: &[CharHunk], remote: &[CharHunk]) -> String {
    let base_chars: Vec<char> = base.chars().collect();
    let mut all: Vec<&CharHunk> = local.iter().chain(remote.iter()).collect();
    all.sort_by_key(|h| (h.base_start, h.base_end));
    let mut out = String::new();
    let mut cursor = 0usize;
    for h in all {
        if h.base_start > cursor {
            out.extend(&base_chars[cursor..h.base_start]);
        }
        out.push_str(&h.replacement);
        cursor = h.base_end.max(cursor);
    }
    if cursor < base_chars.len() {
        out.extend(&base_chars[cursor..]);
    }
    out
}

fn try_subline_char_merge(base: &[&str], local: &[&str], remote: &[&str]) -> Option<Vec<String>> {
    // Constraint 1: single line on every side.
    if base.len() != 1 || local.len() != 1 || remote.len() != 1 {
        return None;
    }
    let b = base[0];
    let l = local[0];
    let r = remote[0];

    let local_hunks = char_diff_hunks(b, l);
    let remote_hunks = char_diff_hunks(b, r);
    if local_hunks.is_empty() || remote_hunks.is_empty() {
        // one side identical to base — the line-level chain already handled this.
        return None;
    }

    // Constraint 2: overlapping char edits → fall back to line conflict.
    if char_hunks_overlap(&local_hunks, &remote_hunks) {
        return None;
    }

    Some(vec![apply_char_hunks(b, &local_hunks, &remote_hunks)])
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

    #[test]
    fn char_hunks_capture_single_word_replacement() {
        let hunks = char_diff_hunks("the quick fox", "the slow fox");
        assert_eq!(hunks.len(), 1);
        let h = &hunks[0];
        assert_eq!(&"the quick fox"[h.base_start..h.base_end], "quick");
        assert_eq!(h.replacement, "slow");
    }

    #[test]
    fn char_hunks_overlap_detection() {
        let a = CharHunk {
            base_start: 4,
            base_end: 9,
            replacement: "slow".into(),
        };
        let b_overlap = CharHunk {
            base_start: 6,
            base_end: 9,
            replacement: "x".into(),
        };
        let b_disjoint = CharHunk {
            base_start: 10,
            base_end: 13,
            replacement: "y".into(),
        };
        assert!(char_hunks_overlap(std::slice::from_ref(&a), &[b_overlap]));
        assert!(!char_hunks_overlap(&[a], &[b_disjoint]));
    }

    #[test]
    fn apply_char_hunks_combines_disjoint_edits() {
        let local = char_diff_hunks("the quick brown fox", "the slow brown fox");
        let remote = char_diff_hunks("the quick brown fox", "the quick red fox");
        let merged = apply_char_hunks("the quick brown fox", &local, &remote);
        assert_eq!(merged, "the slow red fox");
    }

    #[test]
    fn subline_merges_disjoint_same_line_edits() {
        let base = vec!["the quick brown fox\n"];
        let local = vec!["the slow brown fox\n"];
        let remote = vec!["the quick red fox\n"];
        let merged = try_subline_char_merge(&base, &local, &remote);
        assert_eq!(merged, Some(vec!["the slow red fox\n".to_string()]));
    }

    #[test]
    fn subline_rejects_overlapping_edits() {
        let base = vec!["alpha\n"];
        let local = vec!["BETA\n"];
        let remote = vec!["gamma\n"];
        assert_eq!(try_subline_char_merge(&base, &local, &remote), None);
    }

    #[test]
    fn subline_rejects_multi_line_segments() {
        let base = vec!["a\n", "b\n"];
        let local = vec!["A\n", "b\n"];
        let remote = vec!["a\n", "B\n"];
        assert_eq!(try_subline_char_merge(&base, &local, &remote), None);
    }

    #[test]
    fn subline_merges_cjk_disjoint_edits() {
        // base 数据传送类; local edits chars 2-3 (传送→转移), remote edits char 4 (类→物) — disjoint.
        let base = vec!["数据传送类\n"];
        let local = vec!["数据转移类\n"];
        let remote = vec!["数据传送物\n"];
        let merged = try_subline_char_merge(&base, &local, &remote);
        assert_eq!(merged, Some(vec!["数据转移物\n".to_string()]));
    }

    #[test]
    fn three_way_auto_merges_disjoint_same_line_edits() {
        let base = "one\nthe quick brown fox\nthree\n";
        let local = "one\nthe slow brown fox\nthree\n";
        let remote = "one\nthe quick red fox\nthree\n";
        let merged = three_way_merge(base, local, remote);
        assert_eq!(
            merged,
            MergeOutcome::Clean("one\nthe slow red fox\nthree\n".to_string())
        );
    }

    #[test]
    fn three_way_conflicts_on_overlapping_same_line_edits() {
        let base = "one\nalpha\nthree\n";
        let local = "one\nBETA\nthree\n";
        let remote = "one\nGAMMA\nthree\n";
        let MergeOutcome::Conflicted(text) = three_way_merge(base, local, remote) else {
            panic!("expected conflict on overlapping same-line edits");
        };
        assert!(text.contains("<<<<<<< local"));
        assert!(text.contains("BETA"));
        assert!(text.contains("GAMMA"));
    }

    #[test]
    fn multi_line_conflict_still_emits_line_markers() {
        // A genuine multi-line conflict must still produce line markers,
        // untouched by the single-line char-merge path.
        let base = "a\nb\nc\n";
        let local = "a\nX\nY\n";
        let remote = "a\nP\nQ\n";
        let MergeOutcome::Conflicted(text) = three_way_merge(base, local, remote) else {
            panic!("expected conflict");
        };
        assert!(text.contains("<<<<<<< local"));
        assert!(text.contains(">>>>>>> remote"));
    }

    #[test]
    fn char_merge_is_deterministic() {
        let base = "the quick brown fox\n";
        let local = "the slow brown fox\n";
        let remote = "the quick red fox\n";
        let a = three_way_merge(base, local, remote);
        let b = three_way_merge(base, local, remote);
        assert_eq!(a, b);
    }

    #[test]
    fn random_single_line_merges_total_deterministic_conflict_complete() {
        // Deterministic LCG (no dependency). A naive "every input char survives"
        // invariant is FALSE: a side may legitimately delete a char, or
        // base == remote may short-circuit to the other side. So we pin the
        // properties that always hold and catch real char-merge regressions:
        // (1) no panic, (2) determinism, (3) a conflict fallback emits both
        // sides verbatim (no silent loss on the conflict path).
        let alphabet = ['a', 'b', 'c', 'd', '数', '据', ' '];
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as u32
        };
        let make_line = |next: &mut dyn FnMut() -> u32| -> String {
            let len = (next() as usize % 6) + 1;
            let mut s: String = (0..len)
                .map(|_| alphabet[next() as usize % alphabet.len()])
                .collect();
            s.push('\n');
            s
        };

        for _ in 0..500 {
            let base = make_line(&mut next);
            let local = make_line(&mut next);
            let remote = make_line(&mut next);

            let first = three_way_merge(&base, &local, &remote);
            let second = three_way_merge(&base, &local, &remote);
            assert_eq!(
                first, second,
                "non-deterministic: base={base:?} local={local:?} remote={remote:?}"
            );

            if let MergeOutcome::Conflicted(text) = &first {
                let local_line = local.trim_end_matches('\n');
                let remote_line = remote.trim_end_matches('\n');
                assert!(
                    text.contains(local_line),
                    "conflict dropped local {local_line:?}: {text:?}"
                );
                assert!(
                    text.contains(remote_line),
                    "conflict dropped remote {remote_line:?}: {text:?}"
                );
            }
        }
    }
}
