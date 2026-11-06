//! Shared link-target resolution (MEL-06).
//!
//! One producer for "which vault file does this link denote", keyed by
//! the storage comparison key (`case_fold_key`). Resolution semantics
//! (vault-root-first, explicit-relative only, unique-basename fallback)
//! are pinned by the `mel_resolve_*` discriminators.

use crate::mcp::link_span::{Host, LinkToken, Syntax};
use crate::storage::path;
use std::collections::HashMap;
use std::path::Path as StdPath;

pub(crate) use crate::storage::git::case_fold_key;

/// Complete vault file index: canonical paths plus case-folded lookup.
pub(crate) struct TargetIndex {
    by_path: HashMap<String, String>, // fold_key -> canonical path
    by_basename: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Resolution {
    /// Resolved to this canonical vault path.
    Resolved(String),
    /// `[[#heading]]` / `[[#^id]]` — same-file reference, not a backlink.
    SelfReference,
    /// Target not in the index.
    Missing,
    /// Basename matched multiple distinct files.
    Ambiguous,
    /// Scheme/absolute URL — never a vault file.
    External,
}

impl TargetIndex {
    pub(crate) fn new(paths: impl IntoIterator<Item = String>) -> TargetIndex {
        let mut index = TargetIndex {
            by_path: HashMap::new(),
            by_basename: HashMap::new(),
        };
        for canonical in paths {
            let folded = case_fold_key(&canonical);
            index.by_path.insert(folded, canonical.clone());
            if let Some(stem) = StdPath::new(&canonical)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                let stem = case_fold_key(stem);
                index.by_basename.entry(stem).or_default().push(canonical);
            }
        }
        index
    }

    /// Every canonical path in the index (MEL-17 projected indexes).
    pub(crate) fn canonical_paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = self.by_path.values().cloned().collect();
        paths.sort();
        paths.dedup();
        paths
    }
}

/// Resolve one token against the index. `containing` is the canonical
/// path of the file the link lives in (needed for explicit-relative and
/// self-reference classification). `source` is the whole document text
/// (token ranges index into it).
pub(crate) fn resolve(
    index: &TargetIndex,
    containing: &str,
    token: &LinkToken,
    source: &str,
) -> Resolution {
    let raw_path = &source[token.path.clone()];
    if token.syntax == Syntax::Wiki && raw_path.is_empty() {
        // `[[#heading]]`: no path component at all.
        return match token.subpath.as_ref() {
            Some(_) => Resolution::SelfReference,
            None => Resolution::Missing,
        };
    }

    let decoded = decode_for_resolve(token, source);

    if token.syntax == Syntax::Markdown {
        let _ = &decoded;
        // External/absolute forms were already filtered at scan time.
        // Split the fragment on the RAW destination BEFORE percent-decode:
        // a literal '#' in a filename is encoded %23 precisely so it can
        // survive this split (`a%23b.md#H` -> file `a#b.md`, fragment H).
        let raw_dest = &source[token.path.clone()];
        let (raw_file, _fragment) = match raw_dest.find('#') {
            Some(at) => (&raw_dest[..at], &raw_dest[at + 1..]),
            None => (raw_dest, ""),
        };
        let decoded_file = crate::mcp::link_span::percent_decode_path(raw_file)
            .unwrap_or_else(|_| raw_file.to_string());
        let dest = decoded_file.trim().to_string();
        if dest.is_empty() {
            return Resolution::Missing;
        }
        let explicit_relative = dest.starts_with("./") || dest.starts_with("../");
        if explicit_relative {
            return resolve_explicit_relative(index, containing, &dest);
        }
        return resolve_plain(index, &dest);
    }

    // Wiki: `[[/abs/path]]` forces absolute; otherwise plain rules. An
    // explicit path spec may omit only the `.md` suffix — the directory
    // component is never dropped and basename fallback never applies.
    let raw = decoded.trim();
    if let Some(abs) = raw.strip_prefix('/') {
        let with_md = if abs.ends_with(".md") {
            abs.to_string()
        } else {
            format!("{abs}.md")
        };
        for candidate in [abs, with_md.as_str()] {
            if let Some(hit) = index.by_path.get(&case_fold_key(candidate)) {
                return Resolution::Resolved(hit.clone());
            }
        }
        return Resolution::Missing;
    }
    resolve_plain(index, raw)
}

fn decode_for_resolve(token: &LinkToken, source: &str) -> String {
    let raw = &source[token.path.clone()];
    match (&token.host, token.syntax) {
        (Host::Yaml { style, .. }, _) => {
            crate::mcp::link_span::yaml::decode_scalar_path(raw, *style)
                .unwrap_or_else(|_| raw.to_string())
        }
        (_, Syntax::Markdown) => {
            crate::mcp::link_span::percent_decode_path(raw).unwrap_or_else(|_| raw.to_string())
        }
        _ => raw.to_string(),
    }
}

fn resolve_plain(index: &TargetIndex, dest: &str) -> Resolution {
    // A destination carrying a directory component must match that path
    // exactly (Obsidian: bare destinations are vault-absolute first);
    // dropping the directory to fall back to basename would redirect.
    let key = case_fold_key(dest);
    if let Some(hit) = index.by_path.get(&key) {
        return Resolution::Resolved(hit.clone());
    }
    // A `.md` suffix is optional in wiki destinations, path or bare alike.
    let key_md = case_fold_key(&format!("{dest}.md"));
    if let Some(hit) = index.by_path.get(&key_md) {
        return Resolution::Resolved(hit.clone());
    }
    if dest.contains('/') {
        return Resolution::Missing;
    }
    let dest_no_md = dest.strip_suffix(".md").unwrap_or(dest);
    // Bare name: unique basename wins; multiple -> Ambiguous; extension
    // must participate (`report.png` never matches `report.canvas`).
    match index
        .by_basename
        .get(&case_fold_key(dest_no_md))
        .map(|v| v.as_slice())
    {
        Some([only]) => Resolution::Resolved(only.clone()),
        Some(&[]) | None => Resolution::Missing,
        Some(_) => Resolution::Ambiguous,
    }
}

fn resolve_explicit_relative(index: &TargetIndex, containing: &str, dest: &str) -> Resolution {
    match normalize_relative(containing, dest) {
        Some(normalized) => {
            let no_md = normalized.strip_suffix(".md").unwrap_or(&normalized);
            for candidate in [normalized.as_str(), no_md] {
                if let Some(hit) = index.by_path.get(&case_fold_key(candidate)) {
                    return Resolution::Resolved(hit.clone());
                }
            }
            Resolution::Missing
        }
        None => Resolution::External, // escapes the vault root
    }
}

fn normalize_relative(containing: &str, dest: &str) -> Option<String> {
    let mut parts: Vec<String> = containing
        .rsplit_once('/')
        .map(|(dir, _)| {
            dir.split('/')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let dest = dest.replace('\\', "/");
    for part in dest.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            parts.pop()?;
        } else {
            parts.push(part.to_string());
        }
    }
    let joined = parts.join("/");
    path::normalize(&joined).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::link_span::scan_document;

    fn resolve_str(index: &TargetIndex, containing: &str, source: &str) -> Resolution {
        let scan = scan_document(source, 10);
        assert!(scan.reasons.is_empty());
        let token = &scan.tokens[0];
        resolve(index, containing, token, source)
    }

    #[test]
    fn mel_resolve_root_before_relative() {
        // root A.md and docs/A.md both exist; docs/I.md says [x](A.md):
        // the bare destination is vault-absolute-first -> root A.md.
        let index = TargetIndex::new([
            "A.md".to_string(),
            "docs/A.md".to_string(),
            "docs/I.md".to_string(),
        ]);
        assert_eq!(
            resolve_str(&index, "docs/I.md", "[x](A.md)"),
            Resolution::Resolved("A.md".into())
        );
    }

    #[test]
    fn mel_resolve_explicit_relative_only() {
        let index = TargetIndex::new([
            "A.md".to_string(),
            "docs/A.md".to_string(),
            "docs/I.md".to_string(),
        ]);
        assert_eq!(
            resolve_str(&index, "docs/I.md", "[x](./A.md)"),
            Resolution::Resolved("docs/A.md".into())
        );
        // Missing target does NOT fall back to the root file.
        let index2 = TargetIndex::new(["A.md".to_string(), "docs/I.md".to_string()]);
        assert_eq!(
            resolve_str(&index2, "docs/I.md", "[x](./A.md)"),
            Resolution::Missing
        );
    }

    #[test]
    fn mel_resolve_keeps_directory() {
        let index = TargetIndex::new(["A.md".to_string()]);
        // [[missing/A]] with only root A.md present: directory component
        // must not be dropped to reach the root file. (The original fixture
        // also indexed missing/A.md, which masked the intent — resolving to
        // it is correct and now has its own case below.)
        assert_eq!(
            resolve_str(&index, "I.md", "[[missing/A]]"),
            Resolution::Missing
        );
        // With the file present, the path destination resolves through the
        // optional-.md rule.
        let full = TargetIndex::new(["A.md".to_string(), "missing/A.md".to_string()]);
        assert_eq!(
            resolve_str(&full, "I.md", "[[missing/A]]"),
            Resolution::Resolved("missing/A.md".into())
        );
    }

    #[test]
    fn mel_resolve_non_md_extension() {
        let index = TargetIndex::new(["report.canvas".to_string()]);
        // ![[report.png]] must not resolve to report.canvas: extensions
        // other than .md participate exactly.
        assert_eq!(
            resolve_str(&index, "I.md", "![[report.png]]"),
            Resolution::Missing
        );
    }

    #[test]
    fn mel_resolve_folded_ambiguity() {
        let index = TargetIndex::new([
            "notes/A.md".to_string(),
            "archive/a.md".to_string(), // case-folded same basename
        ]);
        assert_eq!(resolve_str(&index, "I.md", "[[A]]"), Resolution::Ambiguous);
    }

    #[test]
    fn mel_resolve_fragment_after_split() {
        let index = TargetIndex::new(["a#b.md".to_string()]);
        // [x](a%23b.md#H): the %23 decodes to '#' — the FIRST '#' after
        // decode is the real filename's; fragment is what follows.
        // Note: our scan keeps the raw destination range; decode happens
        // here. `a%23b.md#H` -> dest `a%23b.md`... wait: percent decode
        // happens on the whole destination first, so `a#b.md#H` splits at
        // the FIRST '#': dest `a`, fragment `b.md#H`. The spec table wants
        // file a#b.md. Correct handling: split fragment BEFORE decode.
        let r = resolve_str(&index, "I.md", "[x](a%23b.md#H)");
        assert_eq!(r, Resolution::Resolved("a#b.md".into()));
    }

    #[test]
    fn mel_resolve_self_reference() {
        let index = TargetIndex::new(["I.md".to_string()]);
        assert_eq!(
            resolve_str(&index, "I.md", "[[#H]]"),
            Resolution::SelfReference
        );
        assert_eq!(
            resolve_str(&index, "I.md", "[[#^id]]"),
            Resolution::SelfReference
        );
        // Self-reference is NOT counted as a file-level backlink target.
        assert_ne!(
            resolve_str(&index, "I.md", "[[#H]]"),
            Resolution::Resolved("I.md".into())
        );
    }

    #[test]
    fn mel_resolve_dotted_basename_wikilink_md_equivalence() {
        let index = TargetIndex::new(["N.md".to_string()]);
        assert_eq!(
            resolve_str(&index, "I.md", "[[N]]"),
            Resolution::Resolved("N.md".into())
        );
        assert_eq!(
            resolve_str(&index, "I.md", "[[N.md]]"),
            Resolution::Resolved("N.md".into())
        );
    }

    #[test]
    fn mel_resolve_wikilink_absolute_prefix() {
        let index = TargetIndex::new(["dir/N.md".to_string(), "N.md".to_string()]);
        assert_eq!(
            resolve_str(&index, "I.md", "[[/dir/N]]"),
            Resolution::Resolved("dir/N.md".into())
        );
    }
}
