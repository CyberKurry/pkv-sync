use crate::api::error::ApiError;
use crate::auth::AuthenticatedUser;
use crate::service::AppState;
use axum::body::Body;
use axum::extract::Extension;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

const MAIN_JS: &[u8] = include_bytes!("../../../plugin/main.js");
const MANIFEST_JSON: &[u8] = include_bytes!("../../../plugin/manifest.json");
const STYLES_CSS: &[u8] = include_bytes!("../../../plugin/styles.css");
static OBSIDIAN_MANIFEST: LazyLock<ObsidianManifest> =
    LazyLock::new(|| serde_json::from_slice(MANIFEST_JSON).expect("bundled manifest.json parses"));
static MAIN_JS_SHA256: LazyLock<String> = LazyLock::new(|| sha256_hex(MAIN_JS));
static MANIFEST_JSON_SHA256: LazyLock<String> = LazyLock::new(|| sha256_hex(MANIFEST_JSON));
static STYLES_CSS_SHA256: LazyLock<String> = LazyLock::new(|| sha256_hex(STYLES_CSS));

#[derive(Clone)]
pub struct PluginAssetOrigin {
    origin: Option<String>,
}

impl PluginAssetOrigin {
    pub fn from_public_host(public_host: Option<String>) -> Self {
        let origin = public_host
            .map(|host| host.trim().trim_end_matches('/').to_string())
            .filter(|host| !host.is_empty())
            .map(|host| {
                if host.starts_with("http://") || host.starts_with("https://") {
                    host
                } else {
                    format!("https://{host}")
                }
            });
        Self { origin }
    }
}

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
    origin: Option<Extension<PluginAssetOrigin>>,
) -> Result<Json<PluginManifestResponse>, ApiError> {
    let origin = origin
        .and_then(|Extension(origin)| origin.origin)
        .map(Ok)
        .unwrap_or_else(|| request_origin(&uri, &headers))?;

    Ok(Json(PluginManifestResponse {
        version: OBSIDIAN_MANIFEST.version.clone(),
        main_js_url: format!("{origin}/api/plugin-assets/main.js"),
        main_js_sha256: MAIN_JS_SHA256.clone(),
        manifest_json_url: format!("{origin}/api/plugin-assets/manifest.json"),
        manifest_json_sha256: MANIFEST_JSON_SHA256.clone(),
        styles_css_url: Some(format!("{origin}/api/plugin-assets/styles.css")),
        styles_css_sha256: Some(STYLES_CSS_SHA256.clone()),
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

    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("missing_host", "missing Host header"))?;
    Ok(format!("http://{host}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    #[test]
    fn plugin_manifest_caches_static_manifest_and_asset_hashes() {
        let source = include_str!("plugin_manifest.rs");
        let handler_start = source
            .find("async fn plugin_manifest")
            .expect("plugin_manifest handler exists");
        let main_js_start = source
            .find("async fn main_js")
            .expect("asset handler follows manifest handler");
        let handler = &source[handler_start..main_js_start];

        assert!(source.contains("static OBSIDIAN_MANIFEST: LazyLock"));
        assert!(source.contains("static MAIN_JS_SHA256: LazyLock"));
        assert!(!handler.contains("serde_json::from_slice(MANIFEST_JSON)"));
        assert!(!handler.contains("sha256_hex(MAIN_JS)"));
    }
}
