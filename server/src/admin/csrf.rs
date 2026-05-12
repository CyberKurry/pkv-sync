use axum::extract::Request;
use axum::http::{header, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

pub async fn middleware(req: Request, next: Next) -> Response {
    if requires_check(&req) && !same_origin(&req) {
        return (StatusCode::FORBIDDEN, "csrf validation failed").into_response();
    }
    next.run(req).await
}

fn requires_check(req: &Request) -> bool {
    !matches!(
        req.method(),
        &Method::GET | &Method::HEAD | &Method::OPTIONS
    ) && req.uri().path() != "/admin/login"
}

fn same_origin(req: &Request) -> bool {
    let Some(expected) = expected_origins(req) else {
        return false;
    };
    if let Some(origin) = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok())
    {
        return expected.iter().any(|value| origin == value);
    }
    req.headers()
        .get(header::REFERER)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|referer| {
            expected
                .iter()
                .any(|value| referer == value || referer.starts_with(&format!("{value}/")))
        })
}

fn expected_origins(req: &Request) -> Option<Vec<String>> {
    let policy = req
        .extensions()
        .get::<crate::admin::handlers::AdminCookiePolicy>();
    let host = policy
        .and_then(|p| p.public_host.as_deref())
        .or_else(|| {
            req.headers()
                .get(header::HOST)
                .and_then(|h| h.to_str().ok())
        })
        .or_else(|| req.uri().authority().map(|a| a.as_str()))?;
    let proto = trusted_forwarded_proto(req).unwrap_or(if policy.is_some_and(|p| p.secure) {
        "https"
    } else {
        "http"
    });
    Some(vec![format!("{proto}://{host}")])
}

fn trusted_forwarded_proto(req: &Request) -> Option<&'static str> {
    if !req
        .extensions()
        .get::<crate::middleware::real_ip::ForwardedFromTrustedProxy>()
        .is_some_and(|trusted| trusted.0)
    {
        return None;
    }
    req.headers()
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.split(',').next())
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
        builder.body(Body::empty()).unwrap()
    }

    fn req_with_forwarded_proto(origin: &str, proto: &str) -> Request {
        Request::builder()
            .method(Method::POST)
            .uri("/admin/gc")
            .header(header::HOST, "example.test")
            .header(header::ORIGIN, origin)
            .header("x-forwarded-proto", proto)
            .body(Body::empty())
            .unwrap()
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
}
