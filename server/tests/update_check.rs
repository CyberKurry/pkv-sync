use axum::routing::get;
use axum::Router;
use pkv_sync_server::service::update_check::{check_once, UpdateStatus};
use std::net::SocketAddr;
use std::time::Duration;

struct MockReleaseServer {
    url: String,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for MockReleaseServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn mock_release(status: axum::http::StatusCode, body: &'static str) -> MockReleaseServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/repos/cyberkurry/pkv-sync/releases/latest",
        get(move || async move { (status, body) }),
    );
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    MockReleaseServer {
        url: format!("http://{addr}/repos/cyberkurry/pkv-sync/releases/latest"),
        handle,
    }
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

#[tokio::test]
async fn detects_newer_version() {
    let server = mock_release(
        axum::http::StatusCode::OK,
        r#"{
            "tag_name": "v0.9.0",
            "html_url": "https://github.com/cyberkurry/pkv-sync/releases/tag/v0.9.0",
            "body": "Release notes here"
        }"#,
    )
    .await;

    let status = check_once("0.8.0", &server.url, &http_client())
        .await
        .unwrap();

    assert_eq!(
        status,
        Some(UpdateStatus {
            latest_version: "0.9.0".into(),
            current_version: "0.8.0".into(),
            release_url: "https://github.com/cyberkurry/pkv-sync/releases/tag/v0.9.0".into(),
            notes_excerpt: "Release notes here".into(),
        })
    );
}

#[tokio::test]
async fn no_update_when_current_is_latest() {
    let server = mock_release(
        axum::http::StatusCode::OK,
        r#"{
            "tag_name": "v0.8.0",
            "html_url": "https://github.com/cyberkurry/pkv-sync/releases/tag/v0.8.0",
            "body": ""
        }"#,
    )
    .await;

    let status = check_once("0.8.0", &server.url, &http_client())
        .await
        .unwrap();

    assert!(status.is_none());
}

#[tokio::test]
async fn handles_github_429_gracefully() {
    let server = mock_release(axum::http::StatusCode::TOO_MANY_REQUESTS, "").await;

    let status = check_once("0.8.0", &server.url, &http_client())
        .await
        .unwrap();

    assert!(status.is_none());
}

#[tokio::test]
async fn ignores_prerelease_tags() {
    let server = mock_release(
        axum::http::StatusCode::OK,
        r#"{
            "tag_name": "v0.9.0-rc.1",
            "html_url": "https://github.com/cyberkurry/pkv-sync/releases/tag/v0.9.0-rc.1",
            "body": "Release candidate"
        }"#,
    )
    .await;

    let status = check_once("0.8.0", &server.url, &http_client())
        .await
        .unwrap();

    assert!(status.is_none());
}
