use rand::{rngs::OsRng, RngCore};

/// Generate a deployment key: `k_` + 32 hex chars (128 bits of entropy).
pub fn generate_deployment_key() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let hex = hex::encode(bytes);
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

    #[test]
    fn deployment_key_uses_hex_encoder() {
        let source = include_str!("keygen.rs");
        let hex_encoder_call = concat!("hex", "::", "encode");
        let per_byte_format_call = concat!("format!", "(\"", "{b:02x}", "\")");
        assert!(source.contains(hex_encoder_call));
        assert!(!source.contains(per_byte_format_call));
    }
}
