use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("password too short ({len} chars, min 8)")]
    TooShort { len: usize },
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
    if plaintext.len() < 8 {
        return Err(PasswordError::TooShort {
            len: plaintext.len(),
        });
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
