use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use std::sync::LazyLock;

pub const MAX_PASSWORD_BYTES: usize = 4096;
static DEFAULT_ARGON2: LazyLock<Argon2<'static>> = LazyLock::new(Argon2::default);

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("password too short ({len} chars, min 8)")]
    TooShort { len: usize },
    #[error("password too long ({len} bytes, max {max})")]
    TooLong { len: usize, max: usize },
    #[error("password must be at least 12 characters and include uppercase, lowercase, and digit")]
    TooWeak,
    #[error("argon2: {0}")]
    Argon2(String),
}

impl From<argon2::password_hash::Error> for PasswordError {
    fn from(e: argon2::password_hash::Error) -> Self {
        PasswordError::Argon2(e.to_string())
    }
}

/// Hash a plaintext password with Argon2id. Returns the encoded hash string.
pub fn hash(plaintext: &str) -> Result<String, PasswordError> {
    if plaintext.len() > MAX_PASSWORD_BYTES {
        return Err(PasswordError::TooLong {
            len: plaintext.len(),
            max: MAX_PASSWORD_BYTES,
        });
    }
    let char_len = plaintext.chars().count();
    if char_len < 8 {
        return Err(PasswordError::TooShort { len: char_len });
    }
    let salt = SaltString::generate(&mut OsRng);
    let phc = DEFAULT_ARGON2.hash_password(plaintext.as_bytes(), &salt)?;
    Ok(phc.to_string())
}

pub fn validate_strong(plaintext: &str) -> Result<(), PasswordError> {
    if plaintext.len() > MAX_PASSWORD_BYTES {
        return Err(PasswordError::TooLong {
            len: plaintext.len(),
            max: MAX_PASSWORD_BYTES,
        });
    }
    let mut char_len = 0usize;
    let mut has_lower = false;
    let mut has_upper = false;
    let mut has_digit = false;
    for c in plaintext.chars() {
        char_len += 1;
        has_lower |= c.is_ascii_lowercase();
        has_upper |= c.is_ascii_uppercase();
        has_digit |= c.is_ascii_digit();
    }
    if char_len < 12 || !has_lower || !has_upper || !has_digit {
        return Err(PasswordError::TooWeak);
    }
    Ok(())
}

/// Verify a plaintext against a stored encoded hash. Returns true on match.
pub fn verify(plaintext: &str, encoded_hash: &str) -> Result<bool, PasswordError> {
    if plaintext.len() > MAX_PASSWORD_BYTES {
        return Err(PasswordError::TooLong {
            len: plaintext.len(),
            max: MAX_PASSWORD_BYTES,
        });
    }
    let parsed = PasswordHash::new(encoded_hash)?;
    Ok(DEFAULT_ARGON2
        .verify_password(plaintext.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_roundtrip() {
        let h = hash("correct horse battery staple").unwrap();
        assert!(verify("correct horse battery staple", &h).unwrap());
        assert!(!verify("wrong", &h).unwrap());
    }

    #[test]
    fn rejects_short_password() {
        let err = hash("short").unwrap_err();
        assert!(matches!(err, PasswordError::TooShort { len: 5 }));
    }

    #[test]
    fn rejects_password_short_by_character_count() {
        let err = hash("密码密码ab").unwrap_err();
        assert!(matches!(err, PasswordError::TooShort { len: 6 }));
    }

    #[test]
    fn rejects_too_long_password() {
        assert!(hash(&"a".repeat(4097)).is_err());
    }

    #[test]
    fn strong_password_policy_matches_setup_requirements() {
        assert!(validate_strong("Passw0rdStrong").is_ok());
        assert!(matches!(
            validate_strong("passw0rd!!").unwrap_err(),
            PasswordError::TooWeak
        ));
        assert!(matches!(
            validate_strong("PASSWORD1234").unwrap_err(),
            PasswordError::TooWeak
        ));
    }

    #[test]
    fn strong_password_validation_scans_chars_once() {
        let source = include_str!("password.rs");
        let fn_start = source
            .find("pub fn validate_strong")
            .expect("validate_strong exists");
        let fn_end = source[fn_start + 1..]
            .find("\npub fn verify")
            .map(|idx| fn_start + 1 + idx)
            .expect("verify follows validate_strong");
        let implementation = &source[fn_start..fn_end];

        assert_eq!(implementation.matches("plaintext.chars()").count(), 1);
    }

    #[test]
    fn verify_rejects_too_long_password_before_argon2() {
        let h = hash("correct horse battery staple").unwrap();
        let err = verify(&"a".repeat(MAX_PASSWORD_BYTES + 1), &h).unwrap_err();
        assert!(matches!(
            err,
            PasswordError::TooLong {
                len,
                max: MAX_PASSWORD_BYTES
            } if len == MAX_PASSWORD_BYTES + 1
        ));
    }

    #[test]
    fn each_hash_uses_unique_salt() {
        let h1 = hash("password1234").unwrap();
        let h2 = hash("password1234").unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn rejects_malformed_hash() {
        let err = verify("anything", "not-a-phc-string").unwrap_err();
        assert!(matches!(err, PasswordError::Argon2(_)));
    }

    #[test]
    fn argon2_parameters_are_cached() {
        let source = include_str!("password.rs");
        let hash_start = source.find("pub fn hash").expect("hash function exists");
        let tests_start = source
            .find("#[cfg(test)]")
            .expect("tests follow implementation");
        let implementation = &source[hash_start..tests_start];

        assert!(source.contains("static DEFAULT_ARGON2"));
        assert!(!implementation.contains("Argon2::default()"));
    }
}
