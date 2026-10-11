use axum::extract::Request;
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

pub async fn middleware(req: Request, next: Next) -> Response {
    if requires_check(&req) && !same_origin(&req) {
        return (StatusCode::FORBIDDEN, "csrf validation failed").into_response();
    }
    next.run(req).await
}

fn requires_check(req: &Request) -> bool {
    let path = req.uri().path();
    (path == "/setup" || path == "/admin" || path.starts_with("/admin/"))
        && !matches!(
            req.method(),
            &Method::GET | &Method::HEAD | &Method::OPTIONS
        )
        && !matches!(path, "/admin/login" | "/setup")
}

pub(crate) fn same_origin(req: &Request) -> bool {
    // H-5 escape hatch: with [admin].allow_insecure_admin and no
    // public_host, treat the request as same-origin always. This is the
    // honest insecure mode — we do NOT derive an origin from the
    // request-controlled Host header (that would fake protection); the
    // origin check is simply off, and config validation limits the hatch
    // to loopback binds. The double-submit-cookie CSRF paths for
    // /admin/login and /setup stay active regardless. With public_host
    // configured there is nothing to escape: the strict check governs.
    let policy_has_public_host = req
        .extensions()
        .get::<crate::admin::handlers::AdminCookiePolicy>()
        .is_some_and(|p| {
            p.public_host
                .as_deref()
                .is_some_and(|h| !h.trim().is_empty())
        });
    if !policy_has_public_host
        && req
            .extensions()
            .get::<crate::admin::handlers::AdminInsecureHatch>()
            .is_some_and(|hatch| hatch.0)
    {
        return true;
    }
    same_origin_parts(
        req.headers(),
        req.extensions()
            .get::<crate::admin::handlers::AdminCookiePolicy>(),
        req.extensions()
            .get::<crate::middleware::real_ip::ForwardedFromTrustedProxy>()
            .is_some_and(|trusted| trusted.0),
    )
}

pub(crate) fn same_origin_parts(
    headers: &HeaderMap,
    policy: Option<&crate::admin::handlers::AdminCookiePolicy>,
    forwarded_from_trusted_proxy: bool,
) -> bool {
    let Some(expected) = expected_origins(headers, policy, forwarded_from_trusted_proxy) else {
        return false;
    };
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|h| h.to_str().ok()) {
        return expected.iter().any(|value| origin == value);
    }
    headers
        .get(header::REFERER)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|referer| {
            expected
                .iter()
                .any(|value| referer == value || referer.starts_with(&format!("{value}/")))
        })
}

fn expected_origins(
    headers: &HeaderMap,
    policy: Option<&crate::admin::handlers::AdminCookiePolicy>,
    forwarded_from_trusted_proxy: bool,
) -> Option<Vec<String>> {
    // Fail closed: a CSRF check that relies on the request's own Host header
    // for its expected origin is brittle (host header injection through
    // misconfigured proxies, ambiguous virtual-host setups). Require an
    // explicit public_host. Operators who haven't set one see admin POSTs
    // rejected with 403 and a clear log line telling them what to configure.
    let Some(host) = policy
        .and_then(|p| p.public_host.as_deref())
        .and_then(normalize_public_host)
    else {
        tracing::warn!(
            "admin CSRF rejected: public_host is not configured; set [server].public_host in config.toml"
        );
        return None;
    };
    let proto = if policy.is_some_and(|p| p.secure) {
        "https"
    } else {
        trusted_forwarded_proto(headers, forwarded_from_trusted_proxy).unwrap_or("http")
    };
    Some(vec![format!("{proto}://{host}")])
}

fn normalize_public_host(public_host: &str) -> Option<&str> {
    let host = public_host.trim().trim_end_matches('/');
    if host.is_empty() {
        return None;
    }
    Some(
        host.strip_prefix("https://")
            .or_else(|| host.strip_prefix("http://"))
            .unwrap_or(host),
    )
}

fn trusted_forwarded_proto(
    headers: &HeaderMap,
    forwarded_from_trusted_proxy: bool,
) -> Option<&'static str> {
    if !forwarded_from_trusted_proxy {
        return None;
    }
    headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(str::trim)
        .and_then(|v| match v {
            "http" => Some("http"),
            "https" => Some("https"),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn with_public_host(mut req: Request, public_host: &str, secure: bool) -> Request {
        req.extensions_mut()
            .insert(crate::admin::handlers::AdminCookiePolicy {
                secure,
                public_host: Some(public_host.into()),
            });
        req
    }

    fn req(method: Method, origin: Option<&str>, referer: Option<&str>) -> Request {
        let mut builder = Request::builder()
            .method(method)
            .uri("/admin/gc")
            .header(header::HOST, "example.test");
        if let Some(origin) = origin {
            builder = builder.header(header::ORIGIN, origin);
        }
        if let Some(referer) = referer {
            builder = builder.header(header::REFERER, referer);
        }
        with_public_host(builder.body(Body::empty()).unwrap(), "example.test", false)
    }

    fn req_with_forwarded_proto(origin: &str, proto: &str) -> Request {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/gc")
            .header(header::HOST, "example.test")
            .header(header::ORIGIN, origin)
            .header("x-forwarded-proto", proto)
            .body(Body::empty())
            .unwrap();
        with_public_host(req, "example.test", false)
    }

    fn req_with_host(host: &str, origin: &str) -> Request {
        Request::builder()
            .method(Method::POST)
            .uri("/admin/gc")
            .header(header::HOST, host)
            .header(header::ORIGIN, origin)
            .body(Body::empty())
            .unwrap()
    }

    #[test]
    fn accepts_matching_origin() {
        assert!(same_origin(&req(
            Method::POST,
            Some("http://example.test"),
            None
        )));
    }

    #[test]
    fn accepts_matching_referer() {
        assert!(same_origin(&req(
            Method::POST,
            None,
            Some("http://example.test/admin")
        )));
    }

    #[test]
    fn rejects_missing_origin_and_referer() {
        assert!(!same_origin(&req(Method::POST, None, None)));
    }

    #[test]
    fn rejects_cross_origin() {
        assert!(!same_origin(&req(
            Method::POST,
            Some("http://attacker.test"),
            None
        )));
    }

    #[test]
    fn rejects_untrusted_forwarded_proto_spoof() {
        assert!(!same_origin(&req_with_forwarded_proto(
            "https://example.test",
            "https"
        )));
    }

    #[test]
    fn accepts_forwarded_proto_from_trusted_proxy() {
        let mut req = req_with_forwarded_proto("https://example.test", "https");
        req.extensions_mut()
            .insert(crate::middleware::real_ip::ForwardedFromTrustedProxy(true));
        assert!(same_origin(&req));
    }

    #[test]
    fn trusted_forwarded_proto_uses_proxy_appended_value() {
        let mut req = req_with_forwarded_proto("https://example.test", "http, https");
        req.extensions_mut()
            .insert(crate::middleware::real_ip::ForwardedFromTrustedProxy(true));
        assert!(same_origin(&req));
    }

    /// Regression: when `public_host` is not configured,
    /// CSRF must fail closed — never derive the expected origin from the
    /// request-controlled Host header, even if Origin happens to match Host.
    #[test]
    fn rejects_when_public_host_unconfigured_even_if_origin_matches_host() {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/gc")
            .header(header::HOST, "anything.test")
            .header(header::ORIGIN, "http://anything.test")
            .body(Body::empty())
            .unwrap();
        // No AdminCookiePolicy extension inserted → public_host is None.
        assert!(
            !same_origin(&req),
            "must fail closed when public_host is unconfigured"
        );
    }

    /// Regression: even with AdminCookiePolicy inserted
    /// but its public_host left as None, fail closed.
    #[test]
    fn rejects_when_policy_has_no_public_host() {
        let mut req = Request::builder()
            .method(Method::POST)
            .uri("/admin/gc")
            .header(header::HOST, "anything.test")
            .header(header::ORIGIN, "http://anything.test")
            .body(Body::empty())
            .unwrap();
        req.extensions_mut()
            .insert(crate::admin::handlers::AdminCookiePolicy {
                secure: false,
                public_host: None,
            });
        assert!(!same_origin(&req), "policy without public_host must reject");
    }

    #[test]
    fn public_host_policy_overrides_request_host() {
        let mut req = req_with_host("attacker.test", "https://attacker.test");
        req.extensions_mut()
            .insert(crate::admin::handlers::AdminCookiePolicy {
                secure: true,
                public_host: Some("example.test".into()),
            });
        assert!(!same_origin(&req));

        let mut req = req_with_host("attacker.test", "https://example.test");
        req.extensions_mut()
            .insert(crate::admin::handlers::AdminCookiePolicy {
                secure: true,
                public_host: Some("example.test".into()),
            });
        assert!(same_origin(&req));
    }

    #[test]
    fn normalizes_public_host_scheme_and_trailing_slash() {
        assert_eq!(
            normalize_public_host(" https://example.test/ "),
            Some("example.test")
        );
        assert_eq!(
            normalize_public_host("http://example.test"),
            Some("example.test")
        );
        assert_eq!(normalize_public_host("example.test"), Some("example.test"));
        assert_eq!(normalize_public_host("   "), None);
    }

    // ------------------------------------------------------------------
    // [admin].allow_insecure_admin (H-5 escape hatch)
    // ------------------------------------------------------------------

    fn req_no_origin() -> Request {
        Request::builder()
            .method(Method::POST)
            .uri("/admin/users")
            .header(header::HOST, "127.0.0.1:6710")
            .body(Body::empty())
            .unwrap()
    }

    fn with_policy(mut req: Request, secure: bool, public_host: Option<&str>) -> Request {
        req.extensions_mut()
            .insert(crate::admin::handlers::AdminCookiePolicy {
                secure,
                public_host: public_host.map(str::to_string),
            });
        req
    }

    /// H-5 red test: with the hatch on and no public_host, an admin POST
    /// with no Origin/Referer must pass (today it 403s — admin is unusable
    /// out of the box). The hatch means "treat as same-origin always".
    #[test]
    fn hatch_passes_admin_post_without_origin_when_public_host_unset() {
        let mut req = with_policy(req_no_origin(), false, None);
        req.extensions_mut()
            .insert(crate::admin::handlers::AdminInsecureHatch(true));
        assert!(
            same_origin(&req),
            "allow_insecure_admin must bypass the origin check"
        );
    }

    /// The hatch also accepts any Origin (documented risk: cross-site
    /// requests against the admin UI are no longer rejected by this layer).
    #[test]
    fn hatch_accepts_any_origin_when_public_host_unset() {
        let mut req = req_no_origin();
        req.headers_mut()
            .insert(header::ORIGIN, "https://attacker.test".parse().unwrap());
        let mut req = with_policy(req, false, None);
        req.extensions_mut()
            .insert(crate::admin::handlers::AdminInsecureHatch(true));
        assert!(same_origin(&req));
    }

    /// Fail-closed default is unchanged: no public_host and no hatch still
    /// rejects, even when Origin happens to match Host.
    #[test]
    fn no_hatch_and_no_public_host_still_fails_closed() {
        let mut req = req_no_origin();
        req.headers_mut()
            .insert(header::ORIGIN, "http://127.0.0.1:6710".parse().unwrap());
        let mut req = with_policy(req, false, None);
        req.extensions_mut()
            .insert(crate::admin::handlers::AdminInsecureHatch(false));
        assert!(!same_origin(&req));
    }

    /// The hatch is an escape hatch for the missing-public_host case only;
    /// with public_host configured the strict check still governs (and can
    /// still reject).
    #[test]
    fn hatch_does_not_weaken_check_when_public_host_is_set() {
        let mut req = req_no_origin();
        req.headers_mut()
            .insert(header::ORIGIN, "https://attacker.test".parse().unwrap());
        let mut req = with_policy(req, true, Some("example.test"));
        req.extensions_mut()
            .insert(crate::admin::handlers::AdminInsecureHatch(true));
        assert!(
            !same_origin(&req),
            "public_host set → strict origin check must still reject cross-origin"
        );
    }

    /// No hatch extension present at all (unit-scoped router tests) keeps
    /// the fail-closed default.
    #[test]
    fn missing_hatch_extension_keeps_fail_closed_default() {
        let req = with_policy(req_no_origin(), false, None);
        assert!(!same_origin(&req));
    }
}
