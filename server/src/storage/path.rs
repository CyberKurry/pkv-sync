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
    #[error("path is too long")]
    TooLong,
    #[error("path is not representable on Windows")]
    WindowsUnsafe,
}

const MAX_PATH_LEN: usize = 512;
const MAX_PATH_COMPONENT_LEN: usize = 255;

/// DOS device names that Windows resolves specially regardless of directory,
/// including when used with an extension (`CON.txt`).
const WINDOWS_RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub(crate) fn is_windows_unsafe_component(part: &str) -> bool {
    if part.contains(':') {
        return true;
    }
    if part.ends_with('.') || part.ends_with(' ') {
        return true;
    }
    let stem = part.split('.').next().unwrap_or(part);
    WINDOWS_RESERVED_NAMES.contains(&stem.to_ascii_uppercase().as_str())
}

/// Normalize a vault-relative path for protocol/Git storage.
pub fn normalize(input: &str) -> Result<String, PathError> {
    let decoded = percent_decode_str(input).decode_utf8_lossy();
    let s = if decoded.contains('\\') {
        std::borrow::Cow::Owned(decoded.replace('\\', "/"))
    } else {
        decoded
    };
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
        if is_windows_unsafe_component(part) {
            return Err(PathError::WindowsUnsafe);
        }
        if part.len() > MAX_PATH_COMPONENT_LEN {
            return Err(PathError::TooLong);
        }
        out.push(part);
    }
    if out.is_empty() {
        return Err(PathError::Empty);
    }
    let normalized = out.join("/");
    if normalized.len() > MAX_PATH_LEN {
        return Err(PathError::TooLong);
    }
    Ok(normalized)
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

    #[test]
    fn rejects_overlong_paths_and_components() {
        assert_eq!(
            normalize(&format!("{}.md", "a".repeat(6000))).unwrap_err(),
            PathError::TooLong
        );
        assert_eq!(
            normalize(&format!("{}/note.md", "a".repeat(256))).unwrap_err(),
            PathError::TooLong
        );
    }

    #[test]
    fn rejects_windows_drive_letter_components() {
        assert_eq!(normalize("C:/x.md").unwrap_err(), PathError::WindowsUnsafe);
        assert_eq!(normalize("c:").unwrap_err(), PathError::WindowsUnsafe);
        assert_eq!(
            normalize("notes/C:evil.md").unwrap_err(),
            PathError::WindowsUnsafe
        );
        assert_eq!(
            normalize(r"\\srv\share\file.md").unwrap_err(),
            PathError::Absolute
        );
    }

    #[test]
    fn rejects_windows_reserved_device_names() {
        assert_eq!(normalize("CON").unwrap_err(), PathError::WindowsUnsafe);
        assert_eq!(normalize("con.txt").unwrap_err(), PathError::WindowsUnsafe);
        assert_eq!(
            normalize("notes/NUL.md").unwrap_err(),
            PathError::WindowsUnsafe
        );
        assert_eq!(normalize("note.md ").unwrap_err(), PathError::WindowsUnsafe);
        assert_eq!(normalize("lpt1.png").unwrap_err(), PathError::WindowsUnsafe);
        assert_eq!(normalize("notes/com10.md").unwrap(), "notes/com10.md");
    }

    #[test]
    fn rejects_trailing_dot_or_space_components() {
        assert_eq!(normalize("note.md.").unwrap_err(), PathError::WindowsUnsafe);
        assert_eq!(normalize("note.md ").unwrap_err(), PathError::WindowsUnsafe);
        assert_eq!(
            normalize("dir /note.md").unwrap_err(),
            PathError::WindowsUnsafe
        );
        assert_eq!(normalize("note.md"), Ok("note.md".into()));
    }
}
