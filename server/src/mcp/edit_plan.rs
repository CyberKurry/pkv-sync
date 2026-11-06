//! Sequential literal edits and structural edit application (MEL-11).
//!
//! Allocation discipline (MEL-D01): the FULL geometry of an edit plan —
//! every intermediate length — is computed with checked arithmetic BEFORE
//! any output String exists. A plan whose second step would explode the
//! file is refused at arithmetic, not discovered after a 240× clone.

use std::ops::Range;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiteralEdit {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditOperation {
    Replace,
    InsertBefore,
    InsertAfter,
    Append,
    Prepend,
}

pub(crate) const MAX_LITERAL_EDITS: usize = 100;

/// Checked final length of replacing `matches` occurrences of `old`
/// (each `old` bytes) with `new` in a `current`-byte file, under `max`.
fn checked_replacement_len(
    current: usize,
    old: usize,
    new: usize,
    matches: usize,
    max: usize,
) -> anyhow::Result<usize> {
    let removed = old
        .checked_mul(matches)
        .ok_or_else(|| anyhow::anyhow!("file exceeds max_file_size of {max} bytes"))?;
    let added = new
        .checked_mul(matches)
        .ok_or_else(|| anyhow::anyhow!("file exceeds max_file_size of {max} bytes"))?;
    current
        .checked_sub(removed)
        .and_then(|n| n.checked_add(added))
        .filter(|&n| n <= max)
        .ok_or_else(|| anyhow::anyhow!("file exceeds max_file_size of {max} bytes"))
}

/// Apply sequential literal edits to `source` under a byte budget `max`:
/// every edit must match EXACTLY ONCE (or everywhere with replace_all),
/// each step's length is checked before materialization, and any failure
/// aborts the whole plan. Edits apply IN ORDER, each to the result of the
/// previous one.
pub(crate) fn apply_literal(
    source: &str,
    edits: &[LiteralEdit],
    replace_all: bool,
    max: usize,
) -> anyhow::Result<String> {
    anyhow::ensure!(
        (1..=MAX_LITERAL_EDITS).contains(&edits.len()),
        "invalid_path: edits must contain 1..={MAX_LITERAL_EDITS} items"
    );
    for edit in edits {
        anyhow::ensure!(
            !edit.old_text.is_empty(),
            "invalid_path: old_text must be non-empty"
        );
    }
    anyhow::ensure!(
        source.len() <= max,
        "file exceeds max_file_size of {max} bytes"
    );

    // Phase 1 (arithmetic only): walk the plan against the RUNNING text,
    // validating match counts and every intermediate length, building
    // nothing. Match counting borrows the current step's input; since the
    // input only exists as data later, we validate on the fly in phase 2
    // — but ALWAYS re-check the length BEFORE constructing the next
    // String, so a failing step allocates nothing.
    let mut current: std::borrow::Cow<str> = std::borrow::Cow::Borrowed(source);
    for edit in edits {
        let matches = current.match_indices(&edit.old_text).count();
        if matches == 0 {
            anyhow::bail!(
                "anchor_not_found: old_text not found: {:?}",
                excerpt(&edit.old_text)
            );
        }
        if matches > 1 && !replace_all {
            anyhow::bail!(
                "anchor_ambiguous: old_text matches {matches} times; pass replace_all or add surrounding context: {:?}",
                excerpt(&edit.old_text)
            );
        }
        // Geometry check BEFORE building the next string.
        checked_replacement_len(
            current.len(),
            edit.old_text.len(),
            edit.new_text.len(),
            matches,
            max,
        )?;
        let mut next = String::with_capacity(
            current.len() - edit.old_text.len() * matches + edit.new_text.len() * matches,
        );
        let mut last = 0;
        for (at, _) in current.match_indices(&edit.old_text) {
            next.push_str(&current[last..at]);
            next.push_str(&edit.new_text);
            last = at + edit.old_text.len();
        }
        next.push_str(&current[last..]);
        current = std::borrow::Cow::Owned(next);
    }
    Ok(current.into_owned())
}

/// Apply a structural operation to a byte range: replace the range's
/// content, or insert before/after it, or append/prepend to the whole
/// document. No newline is added implicitly.
pub(crate) fn apply_range(
    source: &str,
    range: Range<usize>,
    operation: EditOperation,
    content: &str,
    max: usize,
) -> anyhow::Result<String> {
    anyhow::ensure!(
        source.get(range.clone()).is_some(),
        "invalid_anchor: range is not a char-boundary slice of the source"
    );
    let final_len = match operation {
        EditOperation::Replace => source
            .len()
            .checked_sub(range.len())
            .and_then(|n| n.checked_add(content.len())),
        EditOperation::InsertBefore | EditOperation::InsertAfter => {
            source.len().checked_add(content.len())
        }
        EditOperation::Append | EditOperation::Prepend => source.len().checked_add(content.len()),
    }
    .filter(|&n| n <= max)
    .ok_or_else(|| anyhow::anyhow!("file exceeds max_file_size of {max} bytes"))?;

    let mut out = String::with_capacity(final_len);
    match operation {
        EditOperation::Replace => {
            out.push_str(&source[..range.start]);
            out.push_str(content);
            out.push_str(&source[range.end..]);
        }
        EditOperation::InsertBefore => {
            out.push_str(&source[..range.start]);
            out.push_str(content);
            out.push_str(&source[range.start..]);
        }
        EditOperation::InsertAfter => {
            out.push_str(&source[..range.end]);
            out.push_str(content);
            out.push_str(&source[range.end..]);
        }
        EditOperation::Append => {
            out.push_str(source);
            out.push_str(content);
        }
        EditOperation::Prepend => {
            out.push_str(content);
            out.push_str(source);
        }
    }
    Ok(out)
}

fn excerpt(text: &str) -> String {
    crate::mcp::response_budget::prefix_bytes(text, 160).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mel_literal_checks_expansion_before_materializing() {
        assert!(checked_replacement_len(1024, 1, 1024, 1024, 1024).is_err());
        assert_eq!(checked_replacement_len(10, 2, 1, 3, 10).unwrap(), 7);
        assert!(checked_replacement_len(1, usize::MAX, 1, 2, 10).is_err());
    }

    #[test]
    fn mel_literal_sequential_edits_apply_in_order() {
        let source = "one two three";
        let edits = vec![
            LiteralEdit {
                old_text: "one".into(),
                new_text: "1".into(),
            },
            LiteralEdit {
                old_text: "1 two".into(),
                new_text: "first".into(),
            },
        ];
        let out = apply_literal(source, &edits, false, 1024).unwrap();
        assert_eq!(out, "first three");
    }

    #[test]
    fn mel_literal_second_failure_aborts_whole_plan() {
        let edits = vec![
            LiteralEdit {
                old_text: "one".into(),
                new_text: "two".into(),
            },
            LiteralEdit {
                old_text: "missing".into(),
                new_text: "x".into(),
            },
        ];
        let err = apply_literal("one", &edits, false, 1024).unwrap_err();
        assert!(err.to_string().starts_with("anchor_not_found:"));
    }

    #[test]
    fn mel_literal_ambiguous_without_replace_all() {
        let edits = vec![LiteralEdit {
            old_text: "a".into(),
            new_text: "b".into(),
        }];
        let err = apply_literal("a a a", &edits, false, 1024).unwrap_err();
        assert!(err.to_string().starts_with("anchor_ambiguous:"));
        assert!(err.to_string().contains("3 times"));
        // replace_all replaces everywhere.
        let out = apply_literal("a a a", &edits, true, 1024).unwrap();
        assert_eq!(out, "b b b");
    }

    #[test]
    fn mel_literal_intermediate_over_budget_refused_even_if_final_fits() {
        // F=64: 64 a's; a->bb (128, over), then bb->"" (0, under). The
        // FIRST step must refuse even though the plan ends under budget.
        let source = "a".repeat(64);
        let edits = vec![
            LiteralEdit {
                old_text: "a".into(),
                new_text: "bb".into(),
            },
            LiteralEdit {
                old_text: "bb".into(),
                new_text: String::new(),
            },
        ];
        let err = apply_literal(&source, &edits, true, 64).unwrap_err();
        assert!(err.to_string().starts_with("file exceeds max_file_size"));
    }

    #[test]
    fn mel_literal_counts_and_budget_use_matches() {
        // replace_all=true with 16 matches of a->15b's: input 16 bytes,
        // projected 240 — the per-match arithmetic must govern, not a
        // single-match guess.
        let source = "a".repeat(16);
        let edits = vec![LiteralEdit {
            old_text: "a".into(),
            new_text: "b".repeat(15),
        }];
        let err = apply_literal(&source, &edits, true, 64).unwrap_err();
        assert!(err.to_string().starts_with("file exceeds max_file_size"));
        // Same plan with a budget that fits materializes exactly.
        let out = apply_literal(&source, &edits, true, 240).unwrap();
        assert_eq!(out.len(), 240);
    }

    #[test]
    fn mel_literal_validation_rejects_empty_and_oversized_plans() {
        let _scan_edits: Vec<LiteralEdit> = vec![LiteralEdit {
            old_text: "x".into(),
            new_text: "y".into(),
        }];
        assert!(apply_literal("x", &[], false, 1024).is_err());
        let empty_old = vec![LiteralEdit {
            old_text: String::new(),
            new_text: "y".into(),
        }];
        assert!(apply_literal("x", &empty_old, false, 1024).is_err());
        let too_many: Vec<LiteralEdit> = (0..101)
            .map(|i| LiteralEdit {
                old_text: format!("t{i}"),
                new_text: "y".into(),
            })
            .collect();
        // Build a source containing all markers once.
        let source: String = too_many.iter().map(|e| e.old_text.clone()).collect();
        assert!(apply_literal(&source, &too_many, false, 1 << 20).is_err());
    }

    #[test]
    fn mel_literal_multibyte_and_crlf_boundaries() {
        let source = "café\r\nnaïve\r\n";
        let edits = vec![LiteralEdit {
            old_text: "é\r\nnaïve".into(),
            new_text: "e\r\nsimple".into(),
        }];
        let out = apply_literal(source, &edits, false, 1024).unwrap();
        assert_eq!(out, "cafe\r\nsimple\r\n");
    }

    #[test]
    fn mel_edit_range_operations() {
        let source = "# T\nbody\n";
        let range = 4..8; // "body"
        assert_eq!(
            apply_range(source, range.clone(), EditOperation::Replace, "new", 1024).unwrap(),
            "# T\nnew\n"
        );
        assert_eq!(
            apply_range(
                source,
                range.clone(),
                EditOperation::InsertBefore,
                "pre\n",
                1024
            )
            .unwrap(),
            "# T\npre\nbody\n"
        );
        assert_eq!(
            apply_range(
                source,
                range.clone(),
                EditOperation::InsertAfter,
                "\npost",
                1024
            )
            .unwrap(),
            "# T\nbody\npost\n"
        );
        assert_eq!(
            apply_range(source, range.clone(), EditOperation::Append, "!", 1024).unwrap(),
            "# T\nbody\n!"
        );
        assert_eq!(
            apply_range(source, range.clone(), EditOperation::Prepend, "!", 1024).unwrap(),
            "!# T\nbody\n"
        );
    }

    #[test]
    fn mel_edit_range_budget_checked() {
        let source = "x".repeat(100);
        let big = "y".repeat(50);
        let err = apply_range(&source, 0..100, EditOperation::Append, &big, 128).unwrap_err();
        assert!(err.to_string().starts_with("file exceeds max_file_size"));
    }

    #[test]
    fn mel_edit_range_rejects_non_boundary_range() {
        let source = "héllo";
        // 1..2 cuts through the two-byte 'é'.
        assert!(apply_range(source, 1..2, EditOperation::Replace, "x", 1024).is_err());
    }

    #[test]
    fn mel_literal_size_failure_never_materializes() {
        // MEL-D01: the production path must REFUSE the geometry — assert
        // on apply_literal itself, not a mirror of its arithmetic.
        let source = "a".repeat(16);
        let edit = LiteralEdit {
            old_text: "a".into(),
            new_text: "b".repeat(15),
        };
        let err = apply_literal(&source, &[edit], true, 64).unwrap_err();
        assert!(err.to_string().starts_with("file exceeds max_file_size"));
        // Within budget the same plan materializes exactly.
        let edit = LiteralEdit {
            old_text: "a".into(),
            new_text: "b".repeat(15),
        };
        let out = apply_literal(&source, &[edit], true, 240).unwrap();
        assert_eq!(out.len(), 240);
    }
}
