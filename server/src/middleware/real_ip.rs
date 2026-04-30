use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use ipnet::IpNet;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

/// IP determined to belong to the actual client after honoring trusted proxies.
#[derive(Clone, Debug)]
pub struct ClientIp(pub IpAddr);

#[derive(Clone)]
pub struct TrustedProxies(pub Arc<Vec<IpNet>>);

impl TrustedProxies {
    pub fn from_vec(v: Vec<IpNet>) -> Self {
        Self(Arc::new(v))
    }

    fn contains(&self, ip: IpAddr) -> bool {
        self.0.iter().any(|net| net.contains(&ip))
    }
}

pub async fn middleware(
    ConnectInfo(socket_addr): ConnectInfo<SocketAddr>,
    State(trusted): State<TrustedProxies>,
    mut req: Request,
    next: Next,
) -> Response {
    let socket_ip = socket_addr.ip();
    let client_ip = if trusted.contains(socket_ip) {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(str::trim)
            .and_then(|s| s.parse::<IpAddr>().ok())
            .unwrap_or(socket_ip)
    } else {
        socket_ip
    };
    req.extensions_mut().insert(ClientIp(client_ip));
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::{ConnectInfo, Extension};
    use axum::http::{HeaderValue, Request as HttpRequest};
    use axum::routing::get;
    use axum::Router;
    use std::net::SocketAddr;
    use tower::ServiceExt;

    async fn handler(Extension(ClientIp(ip)): Extension<ClientIp>) -> String {
        ip.to_string()
    }

    fn app(trusted: Vec<IpNet>) -> Router {
        let trusted = TrustedProxies::from_vec(trusted);
        Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(trusted, middleware))
    }

    fn req_with(ip: &str, x_forwarded: Option<&str>) -> HttpRequest<Body> {
        let mut b = HttpRequest::builder().uri("/");
        if let Some(xff) = x_forwarded {
            b = b.header("x-forwarded-for", HeaderValue::from_str(xff).unwrap());
        }
        let mut r = b.body(Body::empty()).unwrap();
        let socket: SocketAddr = format!("{ip}:9999").parse().unwrap();
        r.extensions_mut().insert(ConnectInfo(socket));
        r
    }

    #[tokio::test]
    async fn trusts_xff_from_trusted_proxy() {
        let app = app(vec!["127.0.0.1/32".parse().unwrap()]);
        let resp = app
            .oneshot(req_with("127.0.0.1", Some("203.0.113.42")))
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 100).await.unwrap();
        assert_eq!(&body[..], b"203.0.113.42");
    }

    #[tokio::test]
    async fn ignores_xff_from_untrusted() {
        let app = app(vec!["127.0.0.1/32".parse().unwrap()]);
        let resp = app
            .oneshot(req_with("203.0.113.99", Some("1.2.3.4")))
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 100).await.unwrap();
        assert_eq!(&body[..], b"203.0.113.99");
    }

    #[tokio::test]
    async fn falls_back_when_no_xff() {
        let app = app(vec!["127.0.0.1/32".parse().unwrap()]);
        let resp = app.oneshot(req_with("127.0.0.1", None)).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 100).await.unwrap();
        assert_eq!(&body[..], b"127.0.0.1");
    }

    #[tokio::test]
    async fn picks_leftmost_xff_value() {
        let app = app(vec!["127.0.0.1/32".parse().unwrap()]);
        let resp = app
            .oneshot(req_with("127.0.0.1", Some("203.0.113.42, 10.0.0.1")))
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 100).await.unwrap();
        assert_eq!(&body[..], b"203.0.113.42");
    }
}
