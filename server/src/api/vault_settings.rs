use crate::api::error::ApiError;
use crate::auth::AuthenticatedUser;
use crate::middleware::rate_limit;
use crate::service::exclude::EffectiveExcludes;
use crate::service::{vault, vault_settings, AppState};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    router_with_rate_limiter(rate_limit::RequestRateLimiter::sync_api())
}

fn router_with_rate_limiter(limiter: rate_limit::RequestRateLimiter) -> Router<AppState> {
    Router::new()
        .route(
            "/api/vaults/:id/settings",
            get(get_settings).put(put_settings),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            limiter,
            rate_limit::rest_middleware,
        ))
}

#[derive(Debug, Deserialize, Serialize)]
struct VaultSettingsBody {
    extra_sync_globs: Vec<String>,
}

async fn get_settings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Json<VaultSettingsBody>, ApiError> {
    let _vault = vault::ensure_user_vault(&state, &user.user_id, &id).await?;
    let settings = vault_settings::load(&state, &id).await?;
    Ok(Json(VaultSettingsBody {
        extra_sync_globs: settings.extra_sync_globs,
    }))
}

async fn put_settings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    Json(req): Json<VaultSettingsBody>,
) -> Result<StatusCode, ApiError> {
    let _vault = vault::ensure_user_vault(&state, &user.user_id, &id).await?;
    validate_extra_sync_globs(&req.extra_sync_globs)?;
    vault_settings::save(
        &state,
        &id,
        &vault_settings::VaultSettings {
            extra_sync_globs: req.extra_sync_globs,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn validate_extra_sync_globs(globs: &[String]) -> Result<(), ApiError> {
    EffectiveExcludes::compile(globs)
        .map(|_| ())
        .map_err(|e| ApiError::bad_request("invalid_glob", format!("invalid extra sync glob: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{password, token};
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
    use crate::service::vault as vault_service;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn setup_with_limiter(
        limiter: rate_limit::RequestRateLimiter,
    ) -> (axum::Router, String, String) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        let h = password::hash("passw0rd!!").unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "alice".into(),
                password_hash: h,
                is_admin: false,
            })
            .await
            .unwrap();
        let raw = token::generate();
        state
            .tokens
            .create(NewToken {
                user_id: &user.id,
                token_hash: &token::hash(&raw),
                device_id: "device-settings",
                device_name: "d",
            })
            .await
            .unwrap();
        let vault = vault_service::create_vault(&state, &user.id, "main")
            .await
            .unwrap();
        (
            router_with_rate_limiter(limiter).with_state(state),
            raw,
            vault.id,
        )
    }

    fn auth_request(method: &str, uri: impl Into<String>, raw: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri.into())
            .header("authorization", format!("Bearer {raw}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn vault_settings_routes_are_rate_limited() {
        let (app, raw, vault_id) = setup_with_limiter(rate_limit::RequestRateLimiter::new(
            1,
            std::time::Duration::from_secs(60),
        ))
        .await;

        let uri = format!("/api/vaults/{vault_id}/settings");
        let first = app
            .clone()
            .oneshot(auth_request("GET", &uri, &raw))
            .await
            .unwrap();
        let second = app.oneshot(auth_request("GET", &uri, &raw)).await.unwrap();

        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
