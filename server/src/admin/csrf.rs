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
    let Some(expected) = expected_origin(req) else {
        return false;
    };
    if let Some(origin) = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok())
    {
        return origin == expected;
    }
    req.headers()
        .get(header::REFERER)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|referer| referer == expected || referer.starts_with(&format!("{expected}/")))
}

fn expected_origin(req: &Request) -> Option<String> {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .or_else(|| req.uri().authority().map(|a| a.as_str()))?;
    let proto = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| matches!(*v, "http" | "https"))
        .unwrap_or("http");
    Some(format!("{proto}://{host}"))
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
}
