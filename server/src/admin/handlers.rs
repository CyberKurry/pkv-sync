use crate::admin::session::{self, AdminSession};
use crate::admin::templates::{
    ActivityTemplate, ActivityView, DashboardTemplate, InvitesTemplate, LoginTemplate,
    SettingsTemplate, UserDetailTemplate, UsersTemplate, VaultAdminView, VaultsTemplate,
};
use crate::api::error::ApiError;
use crate::auth::LoginRateLimiter;
use crate::auth::{password, token};
use crate::db::repos::{
    InviteRepo, NewToken, NewUser, RegistrationMode, RuntimeConfigRepo, TokenRepo, UserRepo,
    VaultRepo,
};
use crate::middleware::real_ip::ClientIp;
use crate::service::AppState;
use askama::Template;
use axum::extract::{Extension, Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::HashMap;
use tower_cookies::Cookies;

#[derive(Clone, Copy)]
pub struct AdminCookiePolicy {
    pub secure: bool,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/static/admin.css", get(crate::admin::admin_css))
        .route("/admin/language/:lang", get(set_language))
        .route("/admin/login", get(login_page).post(login_post))
        .route("/admin/logout", post(logout))
        .route("/admin", get(dashboard))
        .route("/admin/users", get(users_page).post(create_user_form))
        .route("/admin/users/:id", get(user_detail))
        .route("/admin/users/:id/password", post(reset_password_form))
        .route("/admin/users/:id/active", post(set_active_form))
        .route("/admin/users/:id/admin", post(set_admin_form))
        .route("/admin/users/:id/tokens", post(create_token_form))
        .route(
            "/admin/users/:id/tokens/:tid/revoke",
            post(revoke_token_form),
        )
        .route("/admin/vaults", get(vaults_page).post(create_vault_form))
        .route("/admin/vaults/:id/delete", post(delete_vault_form))
        .route("/admin/vaults/:id/reconcile", post(reconcile_vault_form))
        .route("/admin/invites", get(invites_page).post(create_invite_form))
        .route("/admin/invites/:code/delete", post(delete_invite_form))
        .route("/admin/settings", get(settings_page).post(settings_post))
        .route("/admin/activity", get(activity_page))
        .route("/admin/gc", post(run_gc_form))
        .layer(axum::middleware::from_fn(crate::admin::csrf::middleware))
}

fn admin_text(headers: &HeaderMap, cookies: &Cookies) -> crate::admin::i18n::AdminText {
    crate::admin::i18n::detect(headers, cookies).text()
}

async fn login_page(headers: HeaderMap, cookies: Cookies) -> Html<String> {
    Html(
        LoginTemplate {
            t: admin_text(&headers, &cookies),
            error: None,
        }
        .render()
        .unwrap(),
    )
}

async fn set_language(
    Extension(cookie_policy): Extension<AdminCookiePolicy>,
    cookies: Cookies,
    Path(lang): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Redirect {
    if let Some(lang) = crate::admin::i18n::AdminLang::parse(&lang) {
        cookies.add(crate::admin::i18n::language_cookie(
            lang,
            cookie_policy.secure,
        ));
    }
    let next = params
        .get("next")
        .filter(|value| value.starts_with("/admin") && !value.starts_with("//"))
        .map(String::as_str)
        .unwrap_or("/admin");
    Redirect::to(next)
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

async fn login_post(
    State(state): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    Extension(limiter): Extension<LoginRateLimiter>,
    Extension(cookie_policy): Extension<AdminCookiePolicy>,
    headers: HeaderMap,
    cookies: Cookies,
    Form(form): Form<LoginForm>,
) -> Result<Response, ApiError> {
    let t = crate::admin::i18n::detect(&headers, &cookies).text();
    if let Err(remaining) = limiter.check(ip) {
        return Err(ApiError::too_many(format!(
            "locked for {}s",
            remaining.as_secs()
        )));
    }

    let user = match crate::service::auth::verify_credentials(
        &state,
        &form.username,
        &form.password,
    )
    .await
    {
        Ok(u) => u,
        Err(e) if e.status == StatusCode::UNAUTHORIZED || e.status == StatusCode::FORBIDDEN => {
            limiter.record_failure(ip);
            return Ok(login_error(
                t,
                if e.status == StatusCode::FORBIDDEN {
                    "Account disabled"
                } else {
                    "Invalid credentials"
                },
                e.status,
            ));
        }
        Err(e) => return Err(e),
    };
    if !user.is_admin {
        limiter.record_failure(ip);
        return Ok(login_error(
            t,
            "Admin access required",
            StatusCode::FORBIDDEN,
        ));
    }

    state
        .users
        .touch_last_login(&user.id, chrono::Utc::now().timestamp())
        .await?;
    let session_id = session::create_session(&state, &user.id).await?;
    cookies.add(session::make_cookie(session_id, cookie_policy.secure));
    limiter.record_success(ip);
    Ok(Redirect::to("/admin").into_response())
}

fn login_error(
    t: crate::admin::i18n::AdminText,
    message: &'static str,
    status: StatusCode,
) -> Response {
    (
        status,
        Html(
            LoginTemplate {
                t,
                error: Some(message),
            }
            .render()
            .unwrap(),
        ),
    )
        .into_response()
}

async fn logout(
    State(state): State<AppState>,
    Extension(cookie_policy): Extension<AdminCookiePolicy>,
    cookies: Cookies,
    session: AdminSession,
) -> Result<Redirect, ApiError> {
    session::delete_session(&state, &session.session_id).await?;
    cookies.add(session::expired_cookie(cookie_policy.secure));
    Ok(Redirect::to("/admin/login"))
}

async fn dashboard(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
) -> Result<Html<String>, ApiError> {
    let (users,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;
    let (vaults,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM vaults")
        .fetch_one(&state.pool)
        .await?;
    let metrics = crate::admin::system::collect();
    Ok(Html(
        DashboardTemplate {
            t: admin_text(&headers, &cookies),
            username: session.user.username,
            users,
            vaults,
            cpu_percent: metrics.cpu_percent,
            memory_used_mb: metrics.memory_used_mb,
            memory_total_mb: metrics.memory_total_mb,
            disk_used_gb: metrics.disk_used_gb,
            disk_total_gb: metrics.disk_total_gb,
            uptime_seconds: crate::server::uptime_seconds(),
        }
        .render()
        .unwrap(),
    ))
}

async fn users_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Html<String>, ApiError> {
    let users = state.users.list().await?;
    Ok(Html(
        UsersTemplate {
            t: admin_text(&headers, &cookies),
            users,
            message: None,
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Deserialize)]
struct CreateUserForm {
    username: String,
    password: String,
    is_admin: Option<String>,
}

async fn create_user_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Form(form): Form<CreateUserForm>,
) -> Result<Redirect, ApiError> {
    if state
        .users
        .find_by_username(&form.username)
        .await?
        .is_some()
    {
        return Err(ApiError::conflict("username_taken", "username exists"));
    }
    let password_hash = password::hash(&form.password).map_err(|e| match e {
        password::PasswordError::TooShort { .. } => {
            ApiError::bad_request("weak_password", e.to_string())
        }
        _ => ApiError::internal(e.to_string()),
    })?;
    state
        .users
        .create(NewUser {
            username: form.username,
            password_hash,
            is_admin: form.is_admin.is_some(),
        })
        .await?;
    Ok(Redirect::to("/admin/users"))
}

async fn user_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
    Path(id): Path<String>,
) -> Result<Html<String>, ApiError> {
    let user = state
        .users
        .find_by_id(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let tokens = state.tokens.list_for_user(&id).await?;
    Ok(Html(
        UserDetailTemplate {
            t: admin_text(&headers, &cookies),
            user,
            tokens,
            message: None,
            created_token: None,
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Deserialize)]
struct TokenForm {
    device_name: String,
}

async fn create_token_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
    Path(id): Path<String>,
    Form(form): Form<TokenForm>,
) -> Result<Html<String>, ApiError> {
    let user = state
        .users
        .find_by_id(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let device_name = form.device_name.trim();
    if device_name.is_empty() || device_name.len() > 128 {
        return Err(ApiError::bad_request(
            "invalid_device_name",
            "device name length must be 1-128",
        ));
    }
    let raw = token::generate();
    state
        .tokens
        .create(NewToken {
            user_id: &id,
            token_hash: &token::hash(&raw),
            device_name,
        })
        .await?;
    tracing::info!(user_id = %id, device_name = %device_name, "admin created device token");
    let tokens = state.tokens.list_for_user(&id).await?;
    Ok(Html(
        UserDetailTemplate {
            t: admin_text(&headers, &cookies),
            user,
            tokens,
            message: Some("Device token created".into()),
            created_token: Some(raw),
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Deserialize)]
struct PasswordForm {
    password: String,
}

async fn reset_password_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Path(id): Path<String>,
    Form(form): Form<PasswordForm>,
) -> Result<Redirect, ApiError> {
    let password_hash = password::hash(&form.password).map_err(|e| match e {
        password::PasswordError::TooShort { .. } => {
            ApiError::bad_request("weak_password", e.to_string())
        }
        _ => ApiError::internal(e.to_string()),
    })?;
    state.users.update_password(&id, &password_hash).await?;
    state
        .tokens
        .revoke_all_for_user(&id, chrono::Utc::now().timestamp(), None)
        .await?;
    Ok(Redirect::to(&format!("/admin/users/{id}")))
}

#[derive(Deserialize)]
struct ActiveForm {
    active: bool,
}

async fn set_active_form(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
    Form(form): Form<ActiveForm>,
) -> Result<Redirect, ApiError> {
    if session.user.id == id && !form.active {
        return Err(ApiError::bad_request("self_disable", "cannot disable self"));
    }
    state.users.set_active(&id, form.active).await?;
    Ok(Redirect::to(&format!("/admin/users/{id}")))
}

#[derive(Deserialize)]
struct AdminForm {
    admin: bool,
}

async fn set_admin_form(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
    Form(form): Form<AdminForm>,
) -> Result<Redirect, ApiError> {
    if session.user.id == id && !form.admin && state.users.count_admins().await? <= 1 {
        return Err(ApiError::bad_request(
            "last_admin",
            "cannot demote the last admin",
        ));
    }
    state.users.set_admin(&id, form.admin).await?;
    Ok(Redirect::to(&format!("/admin/users/{id}")))
}

async fn revoke_token_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Path((id, token_id)): Path<(String, String)>,
) -> Result<Redirect, ApiError> {
    state
        .tokens
        .revoke(&token_id, chrono::Utc::now().timestamp())
        .await?;
    tracing::info!(user_id = %id, token_id = %token_id, "admin revoked device token");
    Ok(Redirect::to(&format!("/admin/users/{id}")))
}

type VaultAdminRow = (String, String, String, String, i64, Option<i64>, i64, i64);

async fn vaults_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Html<String>, ApiError> {
    Ok(Html(
        VaultsTemplate {
            t: admin_text(&headers, &cookies),
            vaults: list_admin_vaults(&state).await?,
            users: state.users.list().await?,
            message: None,
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Deserialize)]
struct CreateVaultForm {
    user_id: String,
    name: String,
}

async fn create_vault_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Form(form): Form<CreateVaultForm>,
) -> Result<Redirect, ApiError> {
    state
        .users
        .find_by_id(&form.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let vault = crate::service::vault::create_vault(&state, &form.user_id, &form.name).await?;
    tracing::info!(
        vault_id = %vault.id,
        user_id = %vault.user_id,
        name = %vault.name,
        "admin created vault"
    );
    Ok(Redirect::to("/admin/vaults"))
}

async fn delete_vault_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Path(id): Path<String>,
) -> Result<Redirect, ApiError> {
    let vault = state
        .vaults
        .find_by_id(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("vault not found"))?;
    state.vaults.delete_for_user(&vault.user_id, &id).await?;
    tracing::warn!(
        vault_id = %id,
        user_id = %vault.user_id,
        name = %vault.name,
        "admin deleted vault"
    );
    Ok(Redirect::to("/admin/vaults"))
}

async fn reconcile_vault_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Path(id): Path<String>,
) -> Result<Redirect, ApiError> {
    state
        .vaults
        .find_by_id(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("vault not found"))?;
    let report = crate::service::sync::reconcile_vault_metadata(&state, &id).await?;
    tracing::info!(
        vault_id = %id,
        head = report.head.as_deref(),
        file_count = report.file_count,
        size_bytes = report.size_bytes,
        "admin reconciled vault metadata"
    );
    Ok(Redirect::to("/admin/vaults"))
}

async fn list_admin_vaults(state: &AppState) -> Result<Vec<VaultAdminView>, ApiError> {
    let rows: Vec<VaultAdminRow> = sqlx::query_as(
        "SELECT v.id, v.user_id, u.username, v.name, v.created_at, v.last_sync_at,
                v.size_bytes, v.file_count
         FROM vaults v
         JOIN users u ON u.id = v.user_id
         ORDER BY u.username, v.name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                user_id,
                owner_username,
                name,
                created_at,
                last_sync_at,
                size_bytes,
                file_count,
            )| VaultAdminView {
                id,
                user_id,
                owner_username,
                name,
                created_at,
                last_sync_at,
                size_bytes,
                file_count,
            },
        )
        .collect())
}

async fn invites_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Html<String>, ApiError> {
    let invites = state
        .invites
        .list_active(chrono::Utc::now().timestamp())
        .await?;
    Ok(Html(
        InvitesTemplate {
            t: admin_text(&headers, &cookies),
            invites,
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Deserialize)]
struct InviteForm {
    expires_at: Option<String>,
}

async fn create_invite_form(
    State(state): State<AppState>,
    session: AdminSession,
    Form(form): Form<InviteForm>,
) -> Result<Redirect, ApiError> {
    let expires_at = match form.expires_at.as_deref().map(str::trim) {
        Some("") | None => None,
        Some(value) => Some(
            value
                .parse::<i64>()
                .map_err(|_| ApiError::bad_request("bad_expires_at", "invalid unix seconds"))?,
        ),
    };
    state.invites.create(&session.user.id, expires_at).await?;
    Ok(Redirect::to("/admin/invites"))
}

async fn delete_invite_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Path(code): Path<String>,
) -> Result<Redirect, ApiError> {
    state.invites.delete(&code).await?;
    Ok(Redirect::to("/admin/invites"))
}

async fn settings_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Html<String>, ApiError> {
    Ok(Html(
        SettingsTemplate {
            t: admin_text(&headers, &cookies),
            cfg: state.runtime_cfg.snapshot().await,
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Deserialize)]
struct SettingsForm {
    server_name: String,
    registration_mode: String,
    login_failure_threshold: u32,
    login_window_seconds: u64,
    login_lock_seconds: u64,
}

type ActivityRow = (
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

async fn settings_post(
    State(state): State<AppState>,
    session: AdminSession,
    Extension(limiter): Extension<LoginRateLimiter>,
    Form(form): Form<SettingsForm>,
) -> Result<Redirect, ApiError> {
    let server_name = form.server_name.trim();
    if server_name.is_empty() {
        return Err(ApiError::bad_request(
            "bad_server_name",
            "server name cannot be blank",
        ));
    }
    let mode = RegistrationMode::parse(&form.registration_mode)
        .ok_or_else(|| ApiError::bad_request("bad_mode", "invalid registration mode"))?;
    state
        .runtime_cfg_repo
        .set_server_name(server_name, Some(&session.user.id))
        .await?;
    state
        .runtime_cfg_repo
        .set_registration_mode(mode, Some(&session.user.id))
        .await?;
    state
        .runtime_cfg_repo
        .set_login_rate_limit(
            form.login_failure_threshold,
            form.login_window_seconds,
            form.login_lock_seconds,
            Some(&session.user.id),
        )
        .await?;
    let cfg = state.runtime_cfg_repo.load().await?;
    limiter.update_config(
        cfg.login_failure_threshold,
        std::time::Duration::from_secs(cfg.login_window_seconds),
        std::time::Duration::from_secs(cfg.login_lock_seconds),
    );
    state.runtime_cfg.replace(cfg).await;
    Ok(Redirect::to("/admin/settings"))
}

async fn activity_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Html<String>, ApiError> {
    let rows: Vec<ActivityRow> = sqlx::query_as(
        "SELECT a.timestamp, u.username, a.action, a.vault_id, a.client_ip, a.user_agent
         FROM sync_activity a
         JOIN users u ON u.id = a.user_id
         ORDER BY a.timestamp DESC
         LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;
    let activities = rows
        .into_iter()
        .map(
            |(timestamp, username, action, vault_id, client_ip, user_agent)| ActivityView {
                timestamp,
                username,
                action,
                vault_id,
                client_ip,
                user_agent,
            },
        )
        .collect();
    Ok(Html(
        ActivityTemplate {
            t: admin_text(&headers, &cookies),
            activities,
        }
        .render()
        .unwrap(),
    ))
}

async fn run_gc_form(
    State(state): State<AppState>,
    _session: AdminSession,
) -> Result<Redirect, ApiError> {
    let _ = crate::service::gc::run_blob_gc(&state).await?;
    Ok(Redirect::to("/admin"))
}
