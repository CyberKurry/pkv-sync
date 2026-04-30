use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

/// Plaintext token format: `pks_` + 64 hex (256 bits of entropy).
pub const PREFIX: &str = "pks_";
const RAW_BYTES: usize = 32;

/// Generate a fresh plaintext token. Caller stores the result on the client side
/// and the SHA256 hash in the DB.
pub fn generate() -> String {
    let mut buf = [0u8; RAW_BYTES];
    OsRng.fill_bytes(&mut buf);
    let hex: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    format!("{PREFIX}{hex}")
}

/// SHA-256 hash of the plaintext token, lowercase hex. This is what's stored.
pub fn hash(plaintext: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    let digest = hasher.finalize();
    hex::encode(digest)
}

/// Validate that a string looks like a token (cheap precheck before DB lookup).
pub fn looks_valid(s: &str) -> bool {
    s.starts_with(PREFIX)
        && s.len() == PREFIX.len() + RAW_BYTES * 2
        && s[PREFIX.len()..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generate_format() {
        let t = generate();
        assert!(t.starts_with(PREFIX));
        assert_eq!(t.len(), PREFIX.len() + 64);
    }

    #[test]
    fn generate_unique() {
        let mut set = HashSet::new();
        for _ in 0..200 {
            assert!(set.insert(generate()));
        }
    }

    #[test]
    fn hash_is_deterministic() {
        let t = "pks_abc";
        assert_eq!(hash(t), hash(t));
    }

    #[test]
    fn hash_is_different_per_input() {
        assert_ne!(hash("a"), hash("b"));
    }

    #[test]
    fn looks_valid_accepts_real() {
        assert!(looks_valid(&generate()));
    }

    #[test]
    fn looks_valid_rejects_garbage() {
        assert!(!looks_valid("foo"));
        assert!(!looks_valid(&format!("{PREFIX}xyz")));
        assert!(!looks_valid(&format!("{PREFIX}{}", "0".repeat(63))));
    }
}
