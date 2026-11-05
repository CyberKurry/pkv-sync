//! Identity-preserving link rewrite planning (MEL-17).
//!
//! Pure functions over already-scanned documents: given a move
//! `from -> to`, the OLD target index (resolution happens against the
//! world BEFORE the move) and a projected NEW index (identity map), plan
//! the per-file text patches. Nothing here performs I/O or commits.

use crate::mcp::link_resolver::{self, Resolution, TargetIndex};
use crate::mcp::link_span::{decode_link_path, encode_host_path, Host, LinkToken, Syntax};
use std::collections::BTreeMap;
use std::ops::Range;

pub(crate) struct RewriteRequest {
    pub from: String,
    pub to: String,
    pub max_rewrites: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct TextPatch {
    pub range: Range<usize>,
    pub replacement: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RewriteFile {
    pub original_path: String,
    pub final_path: String,
    pub original_size: usize,
    pub final_size: usize,
    pub patches: Vec<TextPatch>,
}

#[derive(Debug, Default)]
pub(crate) struct RewritePlan {
    pub files: Vec<RewriteFile>,
    /// The moved source itself (Text(to) with rewrites applied).
    pub source_rewritten: bool,
}

/// Map one canonical path through the move (identity except from -> to).
pub(crate) fn mapped_target<'a>(old: &'a str, from: &str, to: &'a str) -> &'a str {
    if old == from {
        to
    } else {
        old
    }
}

/// Build the projected post-move index: same entries, `from` replaced by
/// `to` (I_new = I_old - from + to).
pub(crate) fn projected_index(index: &TargetIndex, from: &str, to: &str) -> TargetIndex {
    // Reconstruct canonical paths from the by_path values.
    let mut paths: Vec<String> = index
        .canonical_paths()
        .into_iter()
        .map(|p| mapped_target(&p, from, to).to_string())
        .collect();
    paths.sort();
    paths.dedup();
    TargetIndex::new(paths)
}

/// Plan patches for ONE document against the old index. `containing` is
/// the file's path BEFORE the move (its final path is the caller's map).
/// Returns the patches for that document (empty when no link changes).
pub(crate) fn plan_document(
    source: &str,
    containing: &str,
    tokens: &[LinkToken],
    old_index: &TargetIndex,
    req: &RewriteRequest,
) -> anyhow::Result<Vec<TextPatch>> {
    let new_index = projected_index(old_index, &req.from, &req.to);
    let mut patches: Vec<TextPatch> = Vec::new();
    for token in tokens {
        let resolution = link_resolver::resolve(old_index, containing, token, source);
        let target = match resolution {
            Resolution::Resolved(path) => path,
            // Self references, missing, ambiguous, external: never rewritten.
            _ => continue,
        };
        if target != req.from {
            continue;
        }
        // This link resolves to `from` in the old world. Regenerate its
        // path component for the new world.
        let new_dest = regenerate_destination(source, token, containing, &req.to, &new_index)?;
        let replacement_span = token.path.clone();
        if &source[replacement_span.clone()] == new_dest {
            continue; // byte-identical: no rewrite recorded
        }
        patches.push(TextPatch {
            range: replacement_span,
            replacement: new_dest,
        });
    }
    // Sort by range, dedupe, refuse overlaps.
    patches.sort_by(|a, b| a.range.start.cmp(&b.range.start));
    for pair in patches.windows(2) {
        anyhow::ensure!(
            pair[0].range.end <= pair[1].range.start,
            "link_rewrite_unsupported: overlapping patches"
        );
    }
    Ok(patches)
}

/// Regenerate the path component for a link that pointed at `from` and
/// must now point at `to`: wiki links use the full vault path (no .md);
/// markdown links use an explicit relative path from the CONTAINING
/// file's directory, percent-encoded. Subpath/display stay untouched
/// (they live outside the patched range).
fn regenerate_destination(
    _source: &str,
    token: &LinkToken,
    containing: &str,
    to: &str,
    new_index: &TargetIndex,
) -> anyhow::Result<String> {
    match token.syntax {
        Syntax::Wiki => {
            // Vault-root full path; extension rules: .md omitted, other
            // extensions kept (resolve handles both).
            let dest = if to.ends_with(".md") {
                to.trim_end_matches(".md").to_string()
            } else {
                to.to_string()
            };
            host_encode(&dest, token)
        }
        Syntax::Markdown => {
            // Explicit relative path from the containing file's directory.
            let rel = relative_path(containing, to)?;
            let encoded = percent_encode_path(&rel);
            let _ = new_index;
            host_encode(&encoded, token)
        }
    }
}

fn host_encode(dest: &str, token: &LinkToken) -> anyhow::Result<String> {
    match &token.host {
        Host::Body => Ok(dest.to_string()),
        host @ Host::Yaml { .. } => encode_host_path(dest, host),
    }
}

/// Path from `containing`'s directory to `target`, with ./ or ../ prefix
/// so it resolves file-relatively.
pub(crate) fn relative_path(containing: &str, target: &str) -> anyhow::Result<String> {
    let mut base: Vec<&str> = containing
        .rsplit_once('/')
        .map(|(dir, _)| dir.split('/').filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();
    let mut down: Vec<&str> = target.split('/').filter(|p| !p.is_empty()).collect();
    // Strip common prefix.
    while let (Some(b), Some(d)) = (base.first(), down.first()) {
        if b == d {
            base.remove(0);
            down.remove(0);
        } else {
            break;
        }
    }
    let mut parts: Vec<String> = Vec::new();
    for _ in base {
        parts.push("..".to_string());
    }
    if parts.is_empty() {
        parts.push(".".to_string());
    }
    for part in down {
        parts.push(part.to_string());
    }
    Ok(parts.join("/"))
}

/// Percent-encode the characters Markdown destinations require: space
/// and the usual reserved set. Slashes stay literal.
pub(crate) fn percent_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b' ' => out.push_str("%20"),
            b'(' | b')' | b'<' | b'>' | b'\"' | b'#' | b'%' | b'^' | b'\\' => {
                out.push_str(&format!("%{byte:02X}"));
            }
            _ => out.push(byte as char),
        }
    }
    out
}

/// Checked final size of applying sorted, non-overlapping patches.
/// Refuses overlap/out-of-range and budget overflow (MEL-17.7).
pub(crate) fn projected_patch_size(
    source: &str,
    patches: &[TextPatch],
    max: usize,
) -> anyhow::Result<usize> {
    let mut end = 0;
    let mut size = source.len();
    for patch in patches {
        anyhow::ensure!(
            patch.range.start >= end
                && patch.range.end >= patch.range.start
                && source.get(patch.range.clone()).is_some(),
            "link_rewrite_unsupported: overlapping or invalid patch range"
        );
        size = size
            .checked_sub(patch.range.len())
            .and_then(|n| n.checked_add(patch.replacement.len()))
            .ok_or_else(|| anyhow::anyhow!("link_rewrite_too_large: size overflow"))?;
        end = patch.range.end;
    }
    anyhow::ensure!(size <= max, "link_rewrite_too_large: file byte budget");
    Ok(size)
}

/// Apply sorted patches to produce the rewritten document.
pub(crate) fn apply_patches(source: &str, patches: &[TextPatch], expected_len: usize) -> String {
    let mut out = String::with_capacity(expected_len);
    let mut last = 0;
    for patch in patches {
        out.push_str(&source[last..patch.range.start]);
        out.push_str(&patch.replacement);
        last = patch.range.end;
    }
    out.push_str(&source[last..]);
    out
}

/// Validate that a previously-resolved OTHER link's identity survives in
/// the new index (MEL-17.5): every still-resolved relation must map to
/// the same logical file.
pub(crate) fn identity_preserved(
    old_index: &TargetIndex,
    new_index: &TargetIndex,
    from: &str,
    to: &str,
    source: &str,
    containing: &str,
    tokens: &[LinkToken],
) -> anyhow::Result<()> {
    for token in tokens {
        if let Resolution::Resolved(old_target) =
            link_resolver::resolve(old_index, containing, token, source)
        {
            if old_target == from {
                continue; // the moved link itself
            }
            // Re-resolve in the new index from the FINAL containing path.
            let final_containing = mapped_target(containing, from, to);
            let new_res = link_resolver::resolve(new_index, final_containing, token, source);
            match new_res {
                Resolution::Resolved(new_target) => {
                    anyhow::ensure!(
                        new_target == old_target,
                        "link_rewrite_unsupported: rename changes another link's resolved identity ({} -> {})",
                        old_target,
                        new_target
                    );
                }
                _ => {
                    anyhow::bail!(
                        "link_rewrite_unsupported: rename breaks another link's resolution ({old_target})"
                    );
                }
            }
        }
    }
    Ok(())
}

/// Decode helper exposed for planning: the resolved path of a token.
pub(crate) fn decoded_path(source: &str, token: &LinkToken) -> String {
    decode_link_path(source, token, 4096).unwrap_or_default()
}

#[allow(dead_code)]
pub(crate) type _SizeMap = BTreeMap<String, usize>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::link_resolver::TargetIndex;
    use crate::mcp::link_span::scan_document;

    fn req() -> RewriteRequest {
        RewriteRequest {
            from: "ideas/brainstorm.md".into(),
            to: "archive/brainstorm-2026.md".into(),
            max_rewrites: 98,
        }
    }

    fn plan_str(
        source: &str,
        containing: &str,
        index: &TargetIndex,
        r: &RewriteRequest,
    ) -> anyhow::Result<String> {
        let scan = scan_document(source, 1000);
        let patches = plan_document(source, containing, &scan.tokens, index, r)?;
        let size = projected_patch_size(source, &patches, 1 << 20)?;
        Ok(apply_patches(source, &patches, size))
    }

    #[test]
    fn mel_rewrite_old_identity() {
        // Old index still contains `from`: inbound links get rewritten.
        let index = TargetIndex::new(["ideas/brainstorm.md".to_string(), "a.md".to_string()]);
        let out = plan_str(
            "see [[ideas/brainstorm]] and [[brainstorm|alias]] and [[brainstorm#Sec]]",
            "a.md",
            &index,
            &req(),
        )
        .unwrap();
        assert!(out.contains("[[archive/brainstorm-2026]]"));
        assert!(out.contains("[[archive/brainstorm-2026|alias]]"));
        assert!(out.contains("[[archive/brainstorm-2026#Sec]]"));
    }

    #[test]
    fn mel_rewrite_self_reference_moves_source() {
        let index = TargetIndex::new(["ideas/brainstorm.md".to_string()]);
        // The moved file's own self-link rewrites to the NEW path.
        let out = plan_str(
            "# Brainstorm\n[[ideas/brainstorm]] self",
            "ideas/brainstorm.md",
            &index,
            &req(),
        )
        .unwrap();
        assert!(out.contains("[[archive/brainstorm-2026]]"));
    }

    #[test]
    fn mel_rewrite_relative_outgoing() {
        let index = TargetIndex::new(["ideas/brainstorm.md".to_string(), "notes/B.md".to_string()]);
        // The source file links ./B relatively; its identity must survive
        // even though archive/B also exists in the projected index.
        let index2 = TargetIndex::new([
            "ideas/brainstorm.md".to_string(),
            "notes/B.md".to_string(),
            "archive/B.md".to_string(),
        ]);
        let scan_src = "[x](./B.md)";
        let scan = scan_document(scan_src, 10);
        identity_preserved(
            &index2,
            &projected_index(&index2, "ideas/brainstorm.md", "archive/brainstorm-2026.md"),
            "ideas/brainstorm.md",
            "archive/brainstorm-2026.md",
            scan_src,
            "ideas/brainstorm.md",
            &scan.tokens,
        )
        .unwrap();
        // ...and the relative link itself is not rewritten (target != from).
        let out = plan_str(scan_src, "ideas/brainstorm.md", &index2, &req()).unwrap();
        assert_eq!(out, scan_src);
        let _ = index;
    }

    #[test]
    fn mel_rewrite_self_fragment_untouched() {
        let index = TargetIndex::new(["ideas/brainstorm.md".to_string()]);
        let out = plan_str(
            "see [[#Section]] here",
            "ideas/brainstorm.md",
            &index,
            &req(),
        )
        .unwrap();
        assert_eq!(out, "see [[#Section]] here");
    }

    #[test]
    fn mel_rewrite_unrelated_target_untouched() {
        let index = TargetIndex::new(["ideas/brainstorm.md".to_string(), "other.md".to_string()]);
        let out = plan_str("[[other]] stays", "a.md", &index, &req()).unwrap();
        assert_eq!(out, "[[other]] stays");
    }

    #[test]
    fn mel_rewrite_new_ambiguity_refused() {
        // Moving to a name that makes a previously-unique basename
        // ambiguous changes ANOTHER link's identity -> whole plan refuses.
        let index = TargetIndex::new([
            "ideas/brainstorm.md".to_string(),
            "a.md".to_string(),
            "keep/other.md".to_string(),
            "other.md".to_string(),
        ]);
        let scan_src = "[[other]]";
        let scan = scan_document(scan_src, 10);
        // In old index "other" resolves to keep/other.md (root other.md also
        // exists -> ambiguous actually). Build a clean case:
        // Old world: [[a]] resolves by unique basename to ideas/a.md.
        // The move CREATES archive/a.md -> [[a]] becomes ambiguous.
        let clean = TargetIndex::new([
            "ideas/brainstorm.md".to_string(),
            "ideas/a.md".to_string(),
            "notes/n.md".to_string(),
        ]);
        let r = RewriteRequest {
            from: "ideas/brainstorm.md".into(),
            to: "archive/a.md".into(),
            max_rewrites: 98,
        };
        let src2 = "[[a]] resolves uniquely today";
        let scan2 = scan_document(src2, 10);
        assert!(matches!(
            crate::mcp::link_resolver::resolve(&clean, "notes/n.md", &scan2.tokens[0], src2),
            Resolution::Resolved(_)
        ));
        let err = identity_preserved(
            &clean,
            &projected_index(&clean, &r.from, &r.to),
            &r.from,
            &r.to,
            src2,
            "notes/n.md",
            &scan2.tokens,
        )
        .unwrap_err();
        assert!(err.to_string().starts_with("link_rewrite_unsupported:"));
        let _ = index;
    }

    #[test]
    fn mel_rewrite_markdown_and_embed_forms() {
        let index = TargetIndex::new([
            "ideas/brainstorm.md".to_string(),
            "docs/guide.md".to_string(),
        ]);
        let out = plan_str(
            "[x](../ideas/brainstorm.md) ![[ideas/brainstorm]]",
            "docs/guide.md",
            &index,
            &req(),
        )
        .unwrap();
        // Markdown regenerated relative to the containing dir.
        assert!(out.contains("[x](../archive/brainstorm-2026.md)"), "{out}");
        assert!(out.contains("![[archive/brainstorm-2026]]"));
    }

    #[test]
    fn mel_rewrite_host_boundaries() {
        let index = TargetIndex::new(["ideas/brainstorm.md".to_string(), "a.md".to_string()]);
        let out = plan_str(
            "$[[ideas/brainstorm]]$ [[ideas/brainstorm]]",
            "a.md",
            &index,
            &req(),
        )
        .unwrap();
        // Only the second (real) link changed.
        assert!(out.starts_with("$[[ideas/brainstorm]]$"));
        assert!(out.ends_with("[[archive/brainstorm-2026]]"));
    }

    #[test]
    fn mel_rewrite_projected_size_checks() {
        let source = "a [[ideas/brainstorm]] b";
        let patches = vec![TextPatch {
            range: 2..21,
            replacement: "x".repeat(100),
        }];
        assert!(projected_patch_size(source, &patches, 50).is_err());
        let expected = source.len() - 19 + 100;
        assert_eq!(
            projected_patch_size(source, &patches, 200).unwrap(),
            expected
        );
        // Overlap refused.
        let bad = vec![
            TextPatch {
                range: 0..5,
                replacement: "a".into(),
            },
            TextPatch {
                range: 3..8,
                replacement: "b".into(),
            },
        ];
        assert!(projected_patch_size(source, &bad, 100).is_err());
    }

    #[test]
    fn mel_rewrite_yaml_host_path() {
        let source = "---\nup: \"[[ideas/brainstorm]]\"\n---\nbody";
        let index = TargetIndex::new(["ideas/brainstorm.md".to_string(), "a.md".to_string()]);
        let scan = scan_document(source, 100);
        assert_eq!(scan.tokens.len(), 1, "yaml-hosted link found");
        let patches = plan_document(source, "a.md", &scan.tokens, &index, &req()).unwrap();
        assert_eq!(patches.len(), 1);
        // Replacement carries the new path; quotes live outside the range.
        assert_eq!(patches[0].replacement, "archive/brainstorm-2026");
    }

    #[test]
    fn mel_rewrite_percent_encode_path() {
        assert_eq!(percent_encode_path("a b/c#d.md"), "a%20b/c%23d.md");
        assert_eq!(
            relative_path("docs/guide.md", "archive/x.md").unwrap(),
            "../archive/x.md"
        );
        assert_eq!(
            relative_path("docs/guide.md", "docs/a.md").unwrap(),
            "./a.md"
        );
    }
}
