use axum::http::HeaderValue;
use base64::Engine;

pub fn extract_token_from_basic(header: &HeaderValue) -> Option<String> {
    let s = header.to_str().ok()?;
    let b64 = s.strip_prefix("Basic ")?;
    let decoded = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let s = String::from_utf8(decoded).ok()?;
    let (_user, token) = s.split_once(':')?;
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header(value: &str) -> HeaderValue {
        HeaderValue::from_str(value).unwrap()
    }

    #[test]
    fn extracts_token_from_valid_basic_header() {
        let encoded = base64::engine::general_purpose::STANDARD.encode("user:pks_abc123");
        let header = make_header(&format!("Basic {encoded}"));
        assert_eq!(
            extract_token_from_basic(&header),
            Some("pks_abc123".to_string())
        );
    }

    #[test]
    fn extracts_token_with_empty_username() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(":pks_abc123");
        let header = make_header(&format!("Basic {encoded}"));
        assert_eq!(
            extract_token_from_basic(&header),
            Some("pks_abc123".to_string())
        );
    }

    #[test]
    fn returns_none_for_empty_token() {
        let encoded = base64::engine::general_purpose::STANDARD.encode("user:");
        let header = make_header(&format!("Basic {encoded}"));
        assert_eq!(extract_token_from_basic(&header), None);
    }

    #[test]
    fn returns_none_for_missing_basic_prefix() {
        let header = make_header("Bearer some_token");
        assert_eq!(extract_token_from_basic(&header), None);
    }

    #[test]
    fn returns_none_for_invalid_base64() {
        let header = make_header("Basic !!!not-base64!!!");
        assert_eq!(extract_token_from_basic(&header), None);
    }

    #[test]
    fn returns_none_for_no_colon() {
        let encoded = base64::engine::general_purpose::STANDARD.encode("nocolonhere");
        let header = make_header(&format!("Basic {encoded}"));
        assert_eq!(extract_token_from_basic(&header), None);
    }
}
