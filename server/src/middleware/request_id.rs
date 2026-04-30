use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

pub const HEADER: &str = "x-request-id";

pub async fn middleware(mut req: Request, next: Next) -> Response {
    let id = Uuid::new_v4().simple().to_string();
    let value = HeaderValue::from_str(&id).expect("uuid hex is valid header");
    req.headers_mut().insert(HEADER, value.clone());
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(HEADER, value);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    #[tokio::test]
    async fn injects_request_id_header() {
        let app: Router = Router::new()
            .route("/", get(|| async { "hi" }))
            .layer(axum::middleware::from_fn(middleware));
        let resp = app
            .oneshot(HttpRequest::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let id = resp.headers().get(HEADER).expect("header set");
        assert_eq!(id.to_str().unwrap().len(), 32);
    }
}
