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
    #[error("percent-encoding nests too deeply")]
    PercentEncodingTooDeep,
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

/// Maximum number of percent-decoding rounds applied to a path before it
/// must reach a fixed point. Every accepted path normalizes to itself
/// (idempotency), so anything still changing after this many rounds is a
/// pathological `%2525...` chain, not a usable filename.
const MAX_DECODE_ROUNDS: usize = 4;

/// Percent-decode `input` until decoding no longer changes it.
///
/// Decoding to a fixed point (instead of exactly once) is what makes
/// `normalize` idempotent: `normalize(normalize(p)) == normalize(p)` for
/// every accepted `p`. Decoding once left a gap where a double-encoded
/// wire path such as `%252E%252E/foo.md` decoded to the harmless-looking
/// literal `%2E%2E/foo.md` and passed validation, yet every later
/// normalize call decoded that stored form again into `../foo.md` and
/// rejected it — a file that could be written once but never read or
/// updated again (ultrareview M-6). With a fixed point, both calls see
/// the same fully decoded path, so writes and reads agree by construction.
/// the same fully decoded path, so writes and reads agree by construction.
///
/// R10-1: escapes may encode CHARACTERS but never STRUCTURE. The client
/// percent-encodes each path SEGMENT (unicode filenames travel as %XX),
/// and its own scan-side check refuses local literals whose decoded
/// shape would differ. A wire path that would grow or shed a `/` or `\`
/// when decoded (`a%2Fb.md` → `a/b.md`, `a%5Cb.md`, `a%252Fb.md`) is a
/// disagreement about which file is meant, not a filename: accepting it
/// made the server store `a/b.md` while the client's vault holds the
/// single literal `a%2Fb.md`. The separator check runs before EVERY
/// decode round (a raw-input scan alone misses `%25`-wrapped forms),
/// and a refusal maps to [`PathError::WindowsUnsafe`] in [`normalize`].
/// Why [`decode_to_fixed_point`] refused an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodeFail {
    /// A percent escape would decode into a path separator (R10-1).
    EscapedSeparator,
    /// The decode chain never stabilized within the round budget.
    TooDeep,
}

fn decode_to_fixed_point(input: &str) -> Result<String, DecodeFail> {
    let mut current = input.to_string();
    for _ in 0..MAX_DECODE_ROUNDS {
        if introduces_escaped_separator(&current) {
            return Err(DecodeFail::EscapedSeparator);
        }
        let next = percent_decode_str(&current)
            .decode_utf8_lossy()
            .into_owned();
        if next == current {
            return Ok(current);
        }
        current = next;
    }
    // One final check keeps the round budget honest: a value that just
    // became stable is fine, anything still mutating is a chain.
    if introduces_escaped_separator(&current) {
        return Err(DecodeFail::EscapedSeparator);
    }
    let next = percent_decode_str(&current)
        .decode_utf8_lossy()
        .into_owned();
    if next == current {
        Ok(current)
    } else {
        Err(DecodeFail::TooDeep)
    }
}

/// Whether any percent escape in `input` decodes (in any round) into a
/// `/` or `\`. Covers `%2F`/`%5C` and their `%25`-wrapped re-encodings:
/// checked before EVERY decode round, so a `%252F` that resurfaces as
/// `%2F` after one round is caught by the next round's check — a single
/// raw-input scan would miss it (`%252F` does not contain `%2F`).
fn introduces_escaped_separator(input: &str) -> bool {
    let upper = input.to_ascii_uppercase();
    upper.contains("%2F") || upper.contains("%5C")
}

/// Normalize a vault-relative path for protocol/Git storage.
///
/// Percent-escapes are decoded to a fixed point before validation, and the
/// fully decoded form is the canonical stored path. The result of a
/// successful `normalize` is itself a fixed point of decoding, so feeding
/// it back through `normalize` is the identity function: a path accepted
/// by a write is always addressable by later reads.
pub fn normalize(input: &str) -> Result<String, PathError> {
    let decoded = match decode_to_fixed_point(input) {
        Ok(decoded) => decoded,
        // R10-1: an escaped separator is a structure change — the caller
        // must resubmit the real path. A chain that never stabilizes is
        // pathological depth.
        Err(DecodeFail::EscapedSeparator) => return Err(PathError::WindowsUnsafe),
        Err(DecodeFail::TooDeep) => return Err(PathError::PercentEncodingTooDeep),
    };
    let s = if decoded.contains('\\') {
        std::borrow::Cow::Owned(decoded.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(&decoded)
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
    fn rejects_escaped_path_separators() {
        // R10-1: percent escapes may encode CHARACTERS (unicode filenames
        // travel as %XX because the wire is a URL path segment), but never
        // STRUCTURE. `a%2Fb.md` decodes to `a/b.md` — a DIFFERENT path
        // shape than the literal single-segment filename the client's
        // vault contains, so accepting it made the server store one file
        // where the client sees another (unresolvable conflict noise, or
        // path_conflict against the real directory). The client encodes
        // per SEGMENT and its own hasUnsafeDecodedShape check exists
        // precisely so a local literal `a%2Fb.md` is never pushed — the
        // server must agree by rejecting any escape that would introduce
        // or remove a path separator when decoded.
        assert_eq!(normalize("a%2Fb.md").unwrap_err(), PathError::WindowsUnsafe);
        assert_eq!(normalize("a%5Cb.md").unwrap_err(), PathError::WindowsUnsafe);
        // Structure-preserving escapes still decode normally.
        assert_eq!(normalize("%E6%96%87.md").unwrap(), "文.md");
        assert_eq!(normalize("%E6%96%87/note.md").unwrap(), "文/note.md");
        // A double-encoded separator (decodes to the literal string
        // `%2F`, which itself decodes to a separator) is also structure.
        assert_eq!(
            normalize("a%252Fb.md").unwrap_err(),
            PathError::WindowsUnsafe
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
    fn rejects_double_encoded_parent_traversal() {
        // The wire form `%252E%252E` decodes fully to `..`: accepting it
        // used to store a literal `%2E%2E` path that every later read
        // decoded again into `../` and rejected (writable but never
        // readable, ultrareview M-6).
        assert_eq!(
            normalize("%252E%252E/foo.md").unwrap_err(),
            PathError::ParentTraversal
        );
        assert_eq!(
            normalize("notes/%252E%252E/foo.md").unwrap_err(),
            PathError::ParentTraversal
        );
    }

    #[test]
    fn rejects_double_encoded_absolute_and_git_dir() {
        // R10-1 tightened `%252F` to WindowsUnsafe (escaped separator)
        // before it could decode far enough to become a leading `/`;
        // the acceptance question (reject) is unchanged.
        assert_eq!(
            normalize("%252Fetc%252Fpasswd.md").unwrap_err(),
            PathError::WindowsUnsafe
        );
        assert_eq!(normalize("%252Egit/config").unwrap_err(), PathError::GitDir);
    }

    #[test]
    fn rejects_triple_encoded_parent_traversal() {
        assert_eq!(
            normalize("%25252E%25252E/foo.md").unwrap_err(),
            PathError::ParentTraversal
        );
    }

    #[test]
    fn rejects_paths_that_never_reach_a_decode_fixed_point() {
        // Six layers of `%25` wrapping cannot stabilize within the decode
        // round budget, so it is rejected as too deep regardless of what
        // the innermost escape would eventually decode to.
        assert_eq!(
            normalize("%25252525252541.md").unwrap_err(),
            PathError::PercentEncodingTooDeep
        );
    }

    #[test]
    fn normalize_is_idempotent_on_accepted_paths() {
        // `%2F` no longer appears here: R10-1 rejects escapes that would
        // decode into a path separator, so the whole-path-encoded wire
        // form of `文字/note.md` is now WindowsUnsafe. The underlying
        // acceptance (a decoded unicode path round-trips) is pinned by
        // decodes_single_encoded_unicode_filename below.
        for raw in [
            "note.md",
            "folder/note.md",
            "folder\\note.md",
            "a/./b.md",
            "folder%20name/note.md",
            "%2D%2Djust-dashes.md",
        ] {
            let once = normalize(raw)
                .unwrap_or_else(|err| panic!("normalize({raw:?}) should be accepted: {err:?}"));
            let twice = normalize(&once)
                .unwrap_or_else(|err| panic!("normalize({once:?}) should be accepted: {err:?}"));
            assert_eq!(once, twice, "normalize must be idempotent for {raw:?}");
        }
    }

    #[test]
    fn decodes_single_encoded_unicode_filename() {
        assert_eq!(
            normalize("%E6%96%87%E5%AD%97/%E3%81%BF%E3%81%9F%E3%81%84.md").unwrap(),
            "文字/みたい.md"
        );
    }

    #[test]
    fn accepts_literal_percent_in_undecoded_filename() {
        // A literal `100%.md` on disk has no percent-escape shape; it must
        // survive a write/read roundtrip unchanged.
        assert_eq!(normalize("100%.md").unwrap(), "100%.md");
        assert_eq!(
            normalize("procent%2Emd").unwrap(),
            "procent.Md".to_string().replace('M', "m")
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
