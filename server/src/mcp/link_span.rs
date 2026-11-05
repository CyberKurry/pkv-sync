//! Span-aware Markdown link scanner (MEL-02).
//!
//! The single producer of "which byte ranges of this document are links":
//! tokens carry ranges into the ORIGINAL text (never resolved targets that
//! would go stale as the index changes), plus the exclusion ranges the
//! anchors module reuses. Consumers resolve identity through
//! `mcp::link_resolver`, never here.

use std::ops::Range;

pub(crate) mod markdown;
pub(crate) mod math;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Syntax {
    Wiki,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuoteStyle {
    Single,
    Double,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Host {
    Body,
    Yaml {
        scalar: Range<usize>,
        style: QuoteStyle,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct LinkToken {
    /// Whole link construct, `[[…]]` / `![[…]]` / `[label](dest)`.
    pub whole: Range<usize>,
    /// Path component only (before `#` / `|`; destination for Markdown).
    pub path: Range<usize>,
    /// `#h1#h2` / `#^id` as raw bytes (no decode).
    pub subpath: Option<Range<usize>>,
    /// `|alias` raw bytes including the leading pipe (and `\|` escapes).
    pub display: Option<Range<usize>>,
    pub syntax: Syntax,
    pub embed: bool,
    pub host: Host,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanReason {
    FileCap,
    ByteCap,
    LinkCap,
    UnsupportedHost,
}

#[derive(Debug)]
pub(crate) struct DocumentScan {
    pub tokens: Vec<LinkToken>,
    pub excluded: Vec<Range<usize>>,
    pub reasons: Vec<ScanReason>,
}

/// Scan `source` for link tokens outside host-owned exclusion zones.
///
/// `max_links` caps token count; the (max_links+1)-th link only records
/// `ScanReason::LinkCap` — the document is then NOT complete
/// (`reasons` non-empty is the completeness contract).
pub(crate) fn scan_document(source: &str, max_links: usize) -> DocumentScan {
    markdown::scan(source, max_links)
}

/// Percent-decode the path component of `token` with a byte budget.
///
/// Only the path is decoded; aliases never become unbounded Strings here.
pub(crate) fn decode_link_path(
    source: &str,
    token: &LinkToken,
    max_bytes: usize,
) -> anyhow::Result<String> {
    markdown::decode_path(source, token, max_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_of<'a>(scan: &DocumentScan, source: &'a str) -> Vec<&'a str> {
        scan.tokens
            .iter()
            .map(|t| &source[t.path.clone()])
            .collect()
    }

    #[test]
    fn mel_scan_math_owns_its_content() {
        for source in [
            "$\\text{[[Old]]}$ [[Live]]",
            "$$\n\\text{[[Old]]}\n$$\n[[Live]]",
            "$$x ^id$$ [[Live]]",
        ] {
            let scan = scan_document(source, 100);
            assert!(scan.reasons.is_empty(), "{source:?}");
            let paths = paths_of(&scan, source);
            assert_eq!(paths, ["Live"], "{source:?}");
        }
        let incomplete = scan_document("$$\n[[Old]]\n", 100);
        assert!(incomplete.reasons.contains(&ScanReason::UnsupportedHost));
    }

    #[test]
    fn mel_scan_markdown_exclusions() {
        let cases: &[(&str, &[&str])] = &[
            ("[[Old]] [x](./Old.md)", &["Old", "./Old.md"]),
            ("\u{60}[[Old]]\u{60} [[Live]]", &["Live"]),
            ("~~~md\n[[Old]]\n~~~\n[[Live]]", &["Live"]),
            ("    [[Old]]\n\n[[Live]]", &["Live"]),
            ("%% [[Old]] %% [[Live]]", &["Live"]),
            ("<!-- [[Old]] --> [[Live]]", &["Live"]),
            ("\\[x](Old.md) [[Live]]", &["Live"]),
        ];
        for (source, expected) in cases {
            let scan = scan_document(source, 100);
            assert!(scan.reasons.is_empty(), "{source:?}");
            let paths = paths_of(&scan, source);
            assert_eq!(&paths, expected, "{source:?}");
        }
    }

    #[test]
    fn mel_scan_wikilink_components() {
        // path / subpath / display ranges point into the original bytes;
        // embed flag travels with the token.
        let source = "![[note#^blk|My Alias]] and [[plain]]";
        let scan = scan_document(source, 100);
        assert!(scan.reasons.is_empty());
        assert_eq!(paths_of(&scan, source), ["note", "plain"]);
        let embed = &scan.tokens[0];
        assert!(embed.embed);
        assert_eq!(&source[embed.subpath.clone().unwrap()], "#^blk");
        assert_eq!(&source[embed.display.clone().unwrap()], "|My Alias");
        assert!(!scan.tokens[1].embed);
    }

    #[test]
    fn mel_scan_markdown_destination_forms() {
        let source = "[a](b.md) [c](<d e.md>) [e](f%20g.md) [h](sub/i.md)";
        let scan = scan_document(source, 100);
        assert!(scan.reasons.is_empty());
        assert_eq!(
            paths_of(&scan, source),
            ["b.md", "d e.md", "f%20g.md", "sub/i.md"]
        );
    }

    #[test]
    fn mel_scan_ignores_external_and_absolute_markdown_links() {
        let source = "[x](https://e.x/a.md) [y](/abs/a.md) [[Live]]";
        let scan = scan_document(source, 100);
        assert!(scan.reasons.is_empty());
        assert_eq!(paths_of(&scan, source), ["Live"]);
    }

    #[test]
    fn mel_scan_link_cap_records_reason_without_clone() {
        let source = "[[a]] [[b]] [[c]]";
        let scan = scan_document(source, 2);
        assert_eq!(scan.tokens.len(), 2);
        assert!(scan.reasons.contains(&ScanReason::LinkCap));
    }

    #[test]
    fn mel_scan_unclosed_constructs_are_not_links() {
        let source = "[[unclosed [[ok]] [x](unclosed.md [y](ok.md)";
        let scan = scan_document(source, 100);
        assert!(scan.reasons.is_empty());
        assert_eq!(paths_of(&scan, source), ["ok", "ok.md"]);
    }

    #[test]
    fn mel_scan_decode_link_path_respects_budget() {
        let source = "[[café]]";
        let scan = scan_document(source, 100);
        let token = &scan.tokens[0];
        assert_eq!(
            decode_link_path(source, token, 512).unwrap(),
            "café".to_string()
        );
        // Budget below the decoded length is a public refusal, not a panic.
        assert!(decode_link_path(source, token, 3).is_err());
    }

    #[test]
    fn mel_scan_excluded_ranges_cover_code_fence() {
        let source = "```\n[[Old]]\n```\n[[Live]]";
        let scan = scan_document(source, 100);
        // The fenced block's inner bytes are one contiguous excluded range.
        let inner = source.find("[[Old]]").unwrap()..source.find("[[Old]]").unwrap() + 7;
        assert!(scan
            .excluded
            .iter()
            .any(|r| r.start <= inner.start && r.end >= inner.end));
    }

    /// MEL-02.5: the span scanner and the legacy byte-extractor in
    /// `mcp::tools::extract_links` must agree on WHICH links exist for the
    /// body-host shapes both support. Runs only under cfg(test) of the
    /// tools module; mirrored here would duplicate the fixture.
    #[test]
    fn mel_scan_agrees_with_legacy_extractor_on_body_links() {
        let cases = [
            "[[Note]]",
            "[[dir/Note|alias]]",
            "[[Note#Heading]]",
            "![[Note]]",
            "[x](rel.md)",
            "[x](./rel.md)",
            "[x](../up.md)",
            "[x](sub/a.md)",
            "[[A]] [[B]] middle [[C]]",
            "text with [x](https://ext.example/a.md) external [[D]]",
        ];
        for source in cases {
            let scan = scan_document(source, 1000);
            assert!(scan.reasons.is_empty(), "{source:?}");
            let mut spanned: Vec<String> = scan
                .tokens
                .iter()
                .map(|t| match t.syntax {
                    Syntax::Wiki => source[t.path.clone()].to_string(),
                    Syntax::Markdown => decode_link_path(source, t, 512).unwrap(),
                })
                .collect();
            spanned.sort();
            let mut legacy = crate::mcp::tools::extract_link_targets_for_tests(source);
            legacy.sort();
            assert_eq!(spanned, legacy, "divergence on {source:?}");
        }
    }
}
