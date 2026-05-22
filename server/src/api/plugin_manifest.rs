use crate::api::error::ApiError;
use crate::auth::AuthenticatedUser;
use crate::service::AppState;
use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAIN_JS: &[u8] = include_bytes!("../../../plugin/main.js");
const MANIFEST_JSON: &[u8] = include_bytes!("../../../plugin/manifest.json");
const STYLES_CSS: &[u8] = include_bytes!("../../../plugin/styles.css");

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/plugin-manifest", get(plugin_manifest))
        .route("/api/plugin-assets/main.js", get(main_js))
        .route("/api/plugin-assets/manifest.json", get(manifest_json))
        .route("/api/plugin-assets/styles.css", get(styles_css))
}

#[derive(Deserialize)]
struct ObsidianManifest {
    version: String,
}

#[derive(Serialize)]
struct PluginManifestResponse {
    version: String,
    main_js_url: String,
    main_js_sha256: String,
    manifest_json_url: String,
    manifest_json_sha256: String,
    styles_css_url: Option<String>,
    styles_css_sha256: Option<String>,
}

async fn plugin_manifest(
    _user: AuthenticatedUser,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<PluginManifestResponse>, ApiError> {
    let plugin_manifest: ObsidianManifest =
        serde_json::from_slice(MANIFEST_JSON).map_err(|e| ApiError::internal(e.to_string()))?;
    let origin = request_origin(&uri, &headers)?;

    Ok(Json(PluginManifestResponse {
        version: plugin_manifest.version,
        main_js_url: format!("{origin}/api/plugin-assets/main.js"),
        main_js_sha256: sha256_hex(MAIN_JS),
        manifest_json_url: format!("{origin}/api/plugin-assets/manifest.json"),
        manifest_json_sha256: sha256_hex(MANIFEST_JSON),
        styles_css_url: Some(format!("{origin}/api/plugin-assets/styles.css")),
        styles_css_sha256: Some(sha256_hex(STYLES_CSS)),
    }))
}

async fn main_js(_user: AuthenticatedUser) -> Response {
    asset_response("application/javascript", MAIN_JS)
}

async fn manifest_json(_user: AuthenticatedUser) -> Response {
    asset_response("application/json", MANIFEST_JSON)
}

async fn styles_css(_user: AuthenticatedUser) -> Response {
    asset_response("text/css", STYLES_CSS)
}

fn asset_response(content_type: &'static str, bytes: &'static [u8]) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
        Body::from(bytes),
    )
        .into_response()
}

fn request_origin(uri: &Uri, headers: &HeaderMap) -> Result<String, ApiError> {
    if let Some(scheme) = uri.scheme_str() {
        if let Some(authority) = uri.authority() {
            return Ok(format!("{scheme}://{authority}"));
        }
    }

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("http");
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("missing_host", "missing Host header"))?;
    Ok(format!("{scheme}://{host}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
