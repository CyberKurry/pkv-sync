use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

pub const MAX_PASSWORD_BYTES: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("password too short ({len} chars, min 8)")]
    TooShort { len: usize },
    #[error("password too long ({len} bytes, max {max})")]
    TooLong { len: usize, max: usize },
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
    let argon = Argon2::default();
    let phc = argon.hash_password(plaintext.as_bytes(), &salt)?;
    Ok(phc.to_string())
}

/// Verify a plaintext against a stored encoded hash. Returns true on match.
pub fn verify(plaintext: &str, encoded_hash: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(encoded_hash)?;
    Ok(Argon2::default()
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
}
