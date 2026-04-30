use std::collections::HashSet;

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
            .map(|(_, ext)| self.extensions.contains(&ext.to_ascii_lowercase()))
            .unwrap_or(false)
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
}
