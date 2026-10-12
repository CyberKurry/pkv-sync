//! Single `Authorization: Bearer` parser shared by the REST extractor and
//! the MCP transport (ultrareview 2026-10-11, R-12): the two used to carry
//! separate implementations whose scheme handling disagreed.

use axum::http::{header, HeaderMap};

/// Extracts the credential from an `Authorization: Bearer <token>` header.
///
/// RFC 9110 §11.1 makes the auth-scheme case-insensitive, so `Bearer`,
/// `bearer` and `BeArEr` all parse (the MCP transport behaved that way
/// after L-1; unifying here gives the REST extractor the same handling).
/// Everything after the first space is returned verbatim — whether the
/// token is well-formed is the caller's business (`token::looks_valid`).
pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())?;
    let (scheme, rest) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    // RFC 7235 allows `1*SP` between the scheme and the credentials; a
    // single split leaves the extra spaces glued to the token, which then
    // fails `token::looks_valid` (R2-5).
    let credential = rest.trim_start_matches(' ');
    (!credential.is_empty()).then_some(credential)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, value.parse().unwrap());
        headers
    }

    #[test]
    fn scheme_is_case_insensitive_and_credential_is_verbatim() {
        for value in ["Bearer abc", "bearer abc", "BEARER abc", "BeArEr abc"] {
            assert_eq!(bearer_token(&headers(value)), Some("abc"), "{value}");
        }
        assert_eq!(bearer_token(&headers("Basic abc")), None);
        // No credential after the scheme, or no space at all: not a bearer
        // header the transport can use.
        assert_eq!(bearer_token(&headers("Bearer")), None);
        assert_eq!(bearer_token(&HeaderMap::new()), None);
    }

    #[test]
    fn multiple_spaces_after_scheme_are_skipped() {
        // RFC 7235 `1*SP`: the credential itself is returned verbatim, so
        // only the separating spaces are trimmed (R2-5).
        assert_eq!(bearer_token(&headers("Bearer  abc")), Some("abc"));
        assert_eq!(bearer_token(&headers("bearer   abc")), Some("abc"));
        // A credential that genuinely starts with spaces cannot be
        // expressed — but tokens are hex, so this is not a real input.
        assert_eq!(bearer_token(&headers("Bearer ")), None);
    }
}
