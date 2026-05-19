use crate::api::error::ApiError;
use crate::auth::AuthenticatedUser;
use crate::service::exclude::EffectiveExcludes;
use crate::service::{vault, vault_settings, AppState};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/vaults/:id/settings",
        get(get_settings).put(put_settings),
    )
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
