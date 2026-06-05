use std::collections::HashSet;
use std::sync::LazyLock;

static DEFAULT_TEXT_CLASSIFIER: LazyLock<TextClassifier> = LazyLock::new(TextClassifier::default);

#[derive(Clone, Debug)]
pub struct TextClassifier {
    extensions: HashSet<String>,
}

impl Default for TextClassifier {
    fn default() -> Self {
        Self::new(["md", "canvas", "base", "json", "txt", "css"])
    }
}

impl TextClassifier {
    pub fn new<I, S>(exts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let extensions = exts
            .into_iter()
            .map(|s| s.as_ref().trim_start_matches('.').to_ascii_lowercase())
            .collect();
        Self { extensions }
    }

    pub fn is_text_path(&self, path: &str) -> bool {
        path.rsplit_once('.')
            .map(|(_, ext)| {
                self.extensions
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(ext))
            })
            .unwrap_or(false)
    }

    pub fn default_ref() -> &'static Self {
        &DEFAULT_TEXT_CLASSIFIER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_text_extensions() {
        let c = TextClassifier::default();
        assert!(c.is_text_path("note.md"));
        assert!(c.is_text_path("canvas.canvas"));
        assert!(c.is_text_path("theme.CSS"));
        assert!(!c.is_text_path("img.png"));
        assert!(!c.is_text_path("README"));
    }

    #[test]
    fn custom_exts() {
        let c = TextClassifier::new(["foo"]);
        assert!(c.is_text_path("x.foo"));
        assert!(!c.is_text_path("x.md"));
    }

    #[test]
    fn text_path_lookup_does_not_allocate_lowercase_extension() {
        let source = include_str!("text_kind.rs");
        let fn_start = source
            .find("pub fn is_text_path")
            .expect("is_text_path implementation exists");
        let test_start = source.find("#[cfg(test)]").expect("test module exists");
        let implementation = &source[fn_start..test_start];

        assert!(!implementation.contains("ext.to_ascii_lowercase()"));
        assert!(implementation.contains("eq_ignore_ascii_case"));
    }

    #[test]
    fn default_ref_reuses_single_classifier_instance() {
        let first = TextClassifier::default_ref() as *const TextClassifier;
        let second = TextClassifier::default_ref() as *const TextClassifier;

        assert_eq!(first, second);
        assert!(TextClassifier::default_ref().is_text_path("note.md"));
    }
}
