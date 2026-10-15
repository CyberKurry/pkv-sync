//! Minimal frontmatter reader for MCP filtering tools.
//!
//! Deliberately not a YAML parser: only top-level scalars (`key: value`),
//! top-level block lists (`^\s*- ` items, matching the howto's two-space
//! indent) and inline lists (`[a, b]`). Anything else — nesting, anchors,
//! quotes, multiline blocks — is skipped silently, and a file whose header
//! is missing, unclosed or larger than 4 KiB has no frontmatter at all. A
//! surprised caller gets "no match", never an error.

use std::collections::HashMap;

const MAX_HEADER_BYTES: usize = 4 * 1024;

#[derive(Debug, Default, PartialEq)]
pub struct Frontmatter {
    pub scalars: HashMap<String, String>,
    pub lists: HashMap<String, Vec<String>>,
}

impl Frontmatter {
    pub fn parse(text: &str) -> Option<Frontmatter> {
        let mut lines = text.split_inclusive('\n');
        let first = lines.next()?;
        if first.trim_end() != "---" {
            return None;
        }
        let mut header = String::new();
        for line in lines {
            if line.trim_end() == "---" {
                return Some(Self::parse_header(&header));
            }
            header.push_str(line);
            if header.len() > MAX_HEADER_BYTES {
                return None;
            }
        }
        None // unclosed
    }

    fn parse_header(header: &str) -> Frontmatter {
        let mut fm = Frontmatter::default();
        let mut current_list: Option<(String, Vec<String>)> = None;
        for raw in header.lines() {
            let line = raw.trim_end();
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            // List item: leading whitespace + "- " (the howto's indented
            // block shape). Items only attach to the key that opened a list.
            if let Some(item) = trimmed.strip_prefix("- ") {
                if let Some((_, items)) = current_list.as_mut() {
                    let item = unquote(item.trim());
                    items.push(item.to_string());
                }
                continue;
            }
            // A non-item line closes the list that was being collected.
            if let Some((key, items)) = current_list.take() {
                if !items.is_empty() {
                    fm.lists.insert(key, items);
                }
            }
            // Only column-0 keys are top level; an indented `key: value`
            // belongs to a nested mapping and must not become a scalar.
            if line.starts_with(' ') || line.starts_with('\t') {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim().to_string();
            let value = value.trim();
            if key.is_empty() {
                continue;
            }
            if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                let items: Vec<String> = inner
                    .split(',')
                    .map(|s| unquote(s.trim()).to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !items.is_empty() {
                    fm.lists.insert(key, items);
                }
            } else if value.is_empty() {
                // May open an indented block list; if no "- " items follow,
                // the key is neither a scalar nor a list.
                current_list = Some((key, Vec::new()));
            } else {
                fm.scalars.insert(key, value.to_string());
            }
        }
        if let Some((key, items)) = current_list {
            if !items.is_empty() {
                fm.lists.insert(key, items);
            }
        }
        fm
    }

    pub fn has(&self, key: &str) -> bool {
        self.scalars.contains_key(key) || self.lists.contains_key(key)
    }

    pub fn scalar(&self, key: &str) -> Option<&str> {
        self.scalars.get(key).map(String::as_str)
    }

    pub fn list(&self, key: &str) -> Option<&[String]> {
        self.lists.get(key).map(Vec::as_slice)
    }
}

fn unquote(value: &str) -> &str {
    value.trim_matches('"').trim_matches('\'')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wiki_page() -> String {
        // The exact shape documented in public-docs/llm-wiki-howto.md
        // (indented block list under `sources:`).
        "---\nkind: wiki\nsources:\n  - sources/meeting.md\n  - sources/spec.md\nupdated: 2026-06-08\n---\n\n# Page\nbody\n".to_string()
    }

    #[test]
    fn parses_howto_shaped_frontmatter() {
        let fm = Frontmatter::parse(&wiki_page()).expect("howto format must parse");
        assert_eq!(fm.scalar("kind"), Some("wiki"));
        assert_eq!(fm.scalar("updated"), Some("2026-06-08"));
        assert_eq!(
            fm.list("sources"),
            Some(
                &[
                    "sources/meeting.md".to_string(),
                    "sources/spec.md".to_string()
                ][..]
            )
        );
        assert!(fm.has("sources"));
    }

    #[test]
    fn parses_inline_list() {
        let fm = Frontmatter::parse("---\nkind: wiki\ntags: [a, b]\n---\nbody");
        assert_eq!(
            fm.unwrap().list("tags"),
            Some(&["a".to_string(), "b".to_string()][..])
        );
    }

    #[test]
    fn none_when_no_frontmatter_or_unclosed_or_oversized() {
        assert!(Frontmatter::parse("just body").is_none());
        assert!(Frontmatter::parse("---\nkind: wiki\n").is_none()); // unclosed
        let oversized = format!("---\n{}\n---\n", "x".repeat(5000));
        assert!(Frontmatter::parse(&oversized).is_none()); // > 4 KiB
        assert!(Frontmatter::parse("-- \nkind: wiki\n---\n").is_none()); // first line must be ---
    }

    #[test]
    fn nested_keys_skipped_not_fatal() {
        let fm = Frontmatter::parse("---\nkind: wiki\nmeta:\n  deep: value\n---\nbody").unwrap();
        assert_eq!(fm.scalar("kind"), Some("wiki"));
        assert!(fm.scalar("meta").is_none()); // non-scalar, non-recognised list
        assert!(!fm.has("deep")); // nested keys are invisible
    }

    #[test]
    fn scalar_matches_are_trimmed_whole_strings() {
        let fm = Frontmatter::parse("---\nkind:   wiki  \n---\nbody").unwrap();
        assert_eq!(fm.scalar("kind"), Some("wiki"));
        assert!(fm.has("kind"));
    }

    #[test]
    fn crlf_headers_parse() {
        let fm = Frontmatter::parse("---\r\nkind: wiki\r\n---\r\nbody");
        assert_eq!(fm.unwrap().scalar("kind"), Some("wiki"));
    }

    #[test]
    fn quoted_list_items_unquoted() {
        let fm = Frontmatter::parse("---\nsources:\n  - \"a.md\"\n  - 'b.md'\n---\nbody").unwrap();
        assert_eq!(
            fm.list("sources"),
            Some(&["a.md".to_string(), "b.md".to_string()][..])
        );
    }

    #[test]
    fn key_with_empty_value_and_no_items_is_neither_scalar_nor_list() {
        let fm = Frontmatter::parse("---\nkind: wiki\nempty:\nupdated: x\n---\nbody").unwrap();
        assert!(!fm.has("empty"));
        assert!(fm.has("updated"));
    }
}
