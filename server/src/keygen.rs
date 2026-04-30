use rand::{rngs::OsRng, RngCore};

/// Generate a deployment key: `k_` + 32 hex chars (128 bits of entropy).
pub fn generate_deployment_key() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("k_{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn key_has_expected_format() {
        let k = generate_deployment_key();
        assert!(k.starts_with("k_"));
        assert_eq!(k.len(), 2 + 32);
        assert!(k[2..].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn keys_are_unique() {
        let mut set = HashSet::new();
        for _ in 0..100 {
            assert!(set.insert(generate_deployment_key()));
        }
    }
}
