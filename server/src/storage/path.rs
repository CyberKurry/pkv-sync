use percent_encoding::percent_decode_str;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PathError {
    #[error("empty path")]
    Empty,
    #[error("absolute paths are not allowed")]
    Absolute,
    #[error("parent traversal is not allowed")]
    ParentTraversal,
    #[error("NUL byte is not allowed")]
    Nul,
    #[error(".git paths are not allowed")]
    GitDir,
}

/// Normalize a vault-relative path for protocol/Git storage.
pub fn normalize(input: &str) -> Result<String, PathError> {
    let decoded = percent_decode_str(input).decode_utf8_lossy();
    let s = decoded.replace('\\', "/");
    if s.is_empty() {
        return Err(PathError::Empty);
    }
    if s.starts_with('/') {
        return Err(PathError::Absolute);
    }
    if s.as_bytes().contains(&0) {
        return Err(PathError::Nul);
    }

    let mut out = Vec::new();
    for part in s.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(PathError::ParentTraversal);
        }
        if part.eq_ignore_ascii_case(".git") {
            return Err(PathError::GitDir);
        }
        out.push(part);
    }
    if out.is_empty() {
        return Err(PathError::Empty);
    }
    Ok(out.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_backslashes() {
        assert_eq!(normalize("folder\\note.md").unwrap(), "folder/note.md");
    }

    #[test]
    fn strips_dot_components() {
        assert_eq!(normalize("a/./b.md").unwrap(), "a/b.md");
    }

    #[test]
    fn rejects_parent_traversal() {
        assert_eq!(
            normalize("a/../b.md").unwrap_err(),
            PathError::ParentTraversal
        );
    }

    #[test]
    fn rejects_absolute() {
        assert_eq!(normalize("/etc/passwd").unwrap_err(), PathError::Absolute);
    }

    #[test]
    fn rejects_git_dir() {
        assert_eq!(normalize(".git/config").unwrap_err(), PathError::GitDir);
        assert_eq!(normalize("x/.git/config").unwrap_err(), PathError::GitDir);
    }

    #[test]
    fn decodes_percent_encoding() {
        assert_eq!(
            normalize("folder%20name/note.md").unwrap(),
            "folder name/note.md"
        );
    }

    #[test]
    fn rejects_percent_encoded_parent() {
        assert_eq!(
            normalize("a/%2E%2E/b").unwrap_err(),
            PathError::ParentTraversal
        );
    }
}
