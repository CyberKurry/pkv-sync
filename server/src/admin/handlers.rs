use crate::admin::session::{self, AdminSession};
use crate::admin::templates::{
    ActivityFilterUser, ActivityTemplate, ActivityView, DashboardTemplate, DeviceTokenAdminView,
    DevicesTemplate, DiffRowView, InviteAdminView, InvitesTemplate, LoginTemplate,
    SettingsTemplate, TokenAdminView, UserAdminView, UserDetailTemplate, UsersTemplate,
    VaultAdminView, VaultBrowserView, VaultDiffTemplate, VaultFileEntryView, VaultFileViewTemplate,
    VaultFilesTemplate, VaultHistoryEntryView, VaultHistoryTemplate, VaultsTemplate,
};
use crate::api::error::ApiError;
use crate::auth::LoginRateLimiter;
use crate::auth::{password, token};
use crate::db::repos::{
    InviteRepo, NewActivity, NewToken, NewUser, RegistrationMode, RuntimeConfigRepo,
    SyncActivityRepo, TokenRepo, TokenRow, User, UserRepo, Vault, VaultRepo,
};
use crate::middleware::real_ip::ClientIp;
use crate::service::auth::validate_username;
use crate::service::AppState;
use crate::storage::git::{Git2VaultStore, GitVaultStore, StoredFile, TreeEntry};
use askama::Template;
use axum::extract::{Extension, Form, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use chrono::{NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::IpAddr;
use tower_cookies::Cookies;

const URL_PATH: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');
const URL_QUERY: &AsciiSet = &URL_PATH.add(b'&').add(b'=').add(b'+');
const ADMIN_ACTIVITY_LIMIT: i64 = 30;

#[derive(Clone)]
pub struct AdminCookiePolicy {
    pub secure: bool,
    pub public_host: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/static/admin.css", get(crate::admin::admin_css))
        .route(
            "/admin/static/lucide-icons.svg",
            get(crate::admin::admin_icons),
        )
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
        .route(
            "/admin/devices",
            get(devices_page).post(create_device_token_form),
        )
        .route("/admin/devices/:tid/revoke", post(revoke_device_token_form))
        .route("/admin/vaults", get(vaults_page).post(create_vault_form))
        .route("/admin/vaults/:id/files", get(vault_files_page))
        .route("/admin/vaults/:id/files/*path", get(vault_file_view_page))
        .route(
            "/admin/vaults/:id/history/*path",
            get(vault_file_history_page),
        )
        .route("/admin/vaults/:id/diff", get(vault_diff_page))
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

fn fmt_ts(timestamp: i64, timezone: &str) -> String {
    crate::time::format_unix_seconds(timestamp, timezone)
}

fn fmt_opt_ts(timestamp: Option<i64>, timezone: &str) -> Option<String> {
    timestamp.map(|ts| fmt_ts(ts, timezone))
}

fn user_view(user: User, timezone: &str) -> UserAdminView {
    UserAdminView {
        id: user.id,
        username: user.username,
        is_admin: user.is_admin,
        is_active: user.is_active,
        created_at: fmt_ts(user.created_at, timezone),
    }
}

fn token_view(token: TokenRow, timezone: &str) -> TokenAdminView {
    TokenAdminView {
        id: token.id,
        device_name: token.device_name,
        created_at: fmt_ts(token.created_at, timezone),
        last_used_at: fmt_opt_ts(token.last_used_at, timezone),
        revoked_at: fmt_opt_ts(token.revoked_at, timezone),
    }
}

async fn login_page(headers: HeaderMap, cookies: Cookies) -> Html<String> {
    Html(
        LoginTemplate {
            t: admin_text(&headers, &cookies),
            error: None,
            version: env!("CARGO_PKG_VERSION"),
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
        .filter(|value| is_safe_admin_next(value))
        .map(String::as_str)
        .unwrap_or("/admin");
    Redirect::to(next)
}

fn is_safe_admin_next(value: &str) -> bool {
    if value.starts_with("//") || value.contains('\\') {
        return false;
    }
    match value.strip_prefix("/admin") {
        Some("") => true,
        Some(rest) => rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#'),
        None => false,
    }
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
                "Invalid credentials",
                StatusCode::UNAUTHORIZED,
            ));
        }
        Err(e) => return Err(e),
    };
    if !user.is_admin {
        limiter.record_failure(ip);
        return Ok(login_error(
            t,
            "Invalid credentials",
            StatusCode::UNAUTHORIZED,
        ));
    }

    state
        .users
        .touch_last_login(&user.id, chrono::Utc::now().timestamp())
        .await?;
    session::delete_sessions_for_user(&state, &user.id).await?;
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
                version: env!("CARGO_PKG_VERSION"),
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
    let metrics = crate::admin::system::collect(&state.data_dir);
    let t = admin_text(&headers, &cookies);
    let uptime_seconds = crate::server::uptime_seconds();
    let recent_activities = list_admin_activities(&state, 5, &ActivityFilters::default()).await?;
    Ok(Html(
        DashboardTemplate {
            disk_used_display: crate::human::format_bytes(metrics.disk_used_bytes),
            disk_total_display: crate::human::format_bytes(metrics.disk_total_bytes),
            uptime_display: crate::human::format_duration_seconds(uptime_seconds, t.html_lang),
            t,
            username: session.user.username,
            users,
            vaults,
            cpu_percent: metrics.cpu_percent,
            cpu_display: format!("{:.0}", metrics.cpu_percent),
            cpu_cores_display: crate::admin::system::format_cpu_cores(metrics.cpu_cores),
            memory_display: crate::human::format_bytes(
                metrics.memory_used_mb.saturating_mul(1024 * 1024),
            ),
            memory_total_display: crate::human::format_bytes(
                metrics.memory_total_mb.saturating_mul(1024 * 1024),
            ),
            recent_activities,
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
    Query(filters): Query<UserFilters>,
) -> Result<Html<String>, ApiError> {
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let query = filters.q.unwrap_or_default().trim().to_string();
    let status = match filters.status.as_deref() {
        Some("active" | "inactive" | "admin") => filters.status.unwrap_or_default(),
        _ => String::new(),
    };
    let query_lc = query.to_lowercase();
    let users: Vec<UserAdminView> = state
        .users
        .list()
        .await?
        .into_iter()
        .filter(|u| query_lc.is_empty() || u.username.to_lowercase().contains(&query_lc))
        .filter(|u| match status.as_str() {
            "active" => u.is_active,
            "inactive" => !u.is_active,
            "admin" => u.is_admin,
            _ => true,
        })
        .map(|u| user_view(u, &timezone))
        .collect();
    Ok(Html(
        UsersTemplate {
            t: admin_text(&headers, &cookies),
            users,
            query,
            status,
            message: None,
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Default, Deserialize)]
struct UserFilters {
    q: Option<String>,
    status: Option<String>,
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
    validate_username(&form.username)?;
    if state
        .users
        .find_by_username(&form.username)
        .await?
        .is_some()
    {
        return Err(ApiError::conflict("username_taken", "username exists"));
    }
    let password_hash = password::hash(&form.password).map_err(|e| match e {
        password::PasswordError::TooShort { .. } | password::PasswordError::TooLong { .. } => {
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
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let tokens = state
        .tokens
        .list_for_user(&id)
        .await?
        .into_iter()
        .map(|token| token_view(token, &timezone))
        .collect();
    Ok(Html(
        UserDetailTemplate {
            t: admin_text(&headers, &cookies),
            user: user_view(user, &timezone),
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

fn validated_device_name(device_name: &str) -> Result<&str, ApiError> {
    let device_name = device_name.trim();
    if device_name.is_empty() || device_name.len() > 128 {
        return Err(ApiError::bad_request(
            "invalid_device_name",
            "device name length must be 1-128",
        ));
    }
    Ok(device_name)
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
    let device_name = validated_device_name(&form.device_name)?;
    let raw = token::generate();
    let device_id = format!("admin_{}", uuid::Uuid::new_v4().simple());
    state
        .tokens
        .create(NewToken {
            user_id: &id,
            token_hash: &token::hash(&raw),
            device_id: &device_id,
            device_name,
        })
        .await?;
    tracing::info!(user_id = %id, device_name = %device_name, "admin created device token");
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let tokens = state
        .tokens
        .list_for_user(&id)
        .await?
        .into_iter()
        .map(|token| token_view(token, &timezone))
        .collect();
    Ok(Html(
        UserDetailTemplate {
            t: admin_text(&headers, &cookies),
            user: user_view(user, &timezone),
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
        password::PasswordError::TooShort { .. } | password::PasswordError::TooLong { .. } => {
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
    if !state
        .users
        .set_admin_preserving_last_admin(&id, form.admin)
        .await?
    {
        return Err(ApiError::bad_request(
            "last_admin",
            "cannot demote the last admin",
        ));
    }
    Ok(Redirect::to(&format!("/admin/users/{id}")))
}

async fn revoke_token_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Path((id, token_id)): Path<(String, String)>,
) -> Result<Redirect, ApiError> {
    let tokens = state.tokens.list_for_user(&id).await?;
    if !tokens.iter().any(|token| token.id == token_id) {
        return Err(ApiError::not_found("token not found"));
    }
    state
        .tokens
        .revoke(&token_id, chrono::Utc::now().timestamp())
        .await?;
    tracing::info!(user_id = %id, token_id = %token_id, "admin revoked device token");
    Ok(Redirect::to(&format!("/admin/users/{id}")))
}

type DeviceTokenAdminRow = (
    String,
    String,
    String,
    String,
    String,
    i64,
    Option<i64>,
    Option<i64>,
);

async fn devices_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Html<String>, ApiError> {
    Ok(Html(
        DevicesTemplate {
            t: admin_text(&headers, &cookies),
            users: state.users.list().await?,
            tokens: list_admin_device_tokens(&state).await?,
            created_token: None,
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Deserialize)]
struct CreateDeviceTokenForm {
    user_id: String,
    device_name: String,
}

async fn create_device_token_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
    Form(form): Form<CreateDeviceTokenForm>,
) -> Result<Html<String>, ApiError> {
    state
        .users
        .find_by_id(&form.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let device_name = validated_device_name(&form.device_name)?;
    let raw = token::generate();
    let device_id = format!("admin_{}", uuid::Uuid::new_v4().simple());
    state
        .tokens
        .create(NewToken {
            user_id: &form.user_id,
            token_hash: &token::hash(&raw),
            device_id: &device_id,
            device_name,
        })
        .await?;
    tracing::info!(
        user_id = %form.user_id,
        device_name = %device_name,
        "admin created device token from devices page"
    );
    Ok(Html(
        DevicesTemplate {
            t: admin_text(&headers, &cookies),
            users: state.users.list().await?,
            tokens: list_admin_device_tokens(&state).await?,
            created_token: Some(raw),
        }
        .render()
        .unwrap(),
    ))
}

async fn revoke_device_token_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Path(token_id): Path<String>,
) -> Result<Redirect, ApiError> {
    state
        .tokens
        .revoke(&token_id, chrono::Utc::now().timestamp())
        .await?;
    tracing::info!(token_id = %token_id, "admin revoked device token from devices page");
    Ok(Redirect::to("/admin/devices"))
}

async fn list_admin_device_tokens(state: &AppState) -> Result<Vec<DeviceTokenAdminView>, ApiError> {
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let rows: Vec<DeviceTokenAdminRow> = sqlx::query_as(
        "SELECT tok.id, tok.user_id, u.username, tok.device_id, tok.device_name,
                tok.created_at, tok.last_used_at, tok.revoked_at
         FROM tokens tok
         JOIN users u ON u.id = tok.user_id
         ORDER BY tok.revoked_at IS NOT NULL, tok.created_at DESC, tok.id DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                user_id,
                username,
                device_id,
                device_name,
                created_at,
                last_used_at,
                revoked_at,
            )| DeviceTokenAdminView {
                id,
                user_id,
                username,
                device_id,
                device_name,
                created_at: fmt_ts(created_at, &timezone),
                last_used_at: fmt_opt_ts(last_used_at, &timezone),
                revoked_at: fmt_opt_ts(revoked_at, &timezone),
            },
        )
        .collect())
}

type VaultAdminRow = (String, String, String, String, i64, Option<i64>, i64, i64);

async fn vaults_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Html<String>, ApiError> {
    let cfg = state.runtime_cfg.snapshot().await;
    let vaults = list_admin_vaults(&state).await?;
    let total_size: u64 = vaults.iter().map(|v| v.size_bytes.max(0) as u64).sum();
    let synced_today = count_vaults_synced_today(&state).await?;
    Ok(Html(
        VaultsTemplate {
            t: admin_text(&headers, &cookies),
            total_vaults: vaults.len(),
            total_size_display: crate::human::format_bytes(total_size),
            synced_today,
            vaults,
            users: state.users.list().await?,
            message: None,
            enable_history_ui: cfg.enable_history_ui,
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Deserialize)]
struct FileViewQuery {
    at: Option<String>,
}

#[derive(Deserialize)]
struct DiffQuery {
    path: String,
    to: String,
    from: Option<String>,
}

async fn vault_files_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
    Path(id): Path<String>,
) -> Result<Html<String>, ApiError> {
    ensure_admin_history_enabled(&state).await?;
    let (_vault, vault_view) = admin_vault(&state, &id).await?;
    let store = Git2VaultStore::new(state.default_vault_root());
    let entries = store
        .list_tree(&id, None)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let files = entries
        .into_iter()
        .map(|entry| file_entry_view(&id, entry))
        .collect();
    Ok(Html(
        VaultFilesTemplate {
            t: admin_text(&headers, &cookies),
            vault: vault_view,
            files,
        }
        .render()
        .unwrap(),
    ))
}

async fn vault_file_view_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    Path((id, path)): Path<(String, String)>,
    Query(query): Query<FileViewQuery>,
) -> Result<Html<String>, ApiError> {
    ensure_admin_history_enabled(&state).await?;
    vault_file_view_html(state, headers, cookies, session, id, path, query).await
}

async fn vault_file_history_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    Path((id, path)): Path<(String, String)>,
) -> Result<Html<String>, ApiError> {
    ensure_admin_history_enabled(&state).await?;
    vault_file_history_html(state, headers, cookies, session, id, &path).await
}

async fn vault_file_view_html(
    state: AppState,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    id: String,
    path: String,
    query: FileViewQuery,
) -> Result<Html<String>, ApiError> {
    let cfg = state.runtime_cfg.snapshot().await;
    let (_vault, vault_view) = admin_vault(&state, &id).await?;
    let path = crate::storage::path::normalize(&path)
        .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
    let store = Git2VaultStore::new(state.default_vault_root());
    let file = store
        .read_file(&id, &path, query.at.as_deref())
        .await
        .map_err(|e| ApiError::bad_request("bad_commit", e.to_string()))?
        .ok_or_else(|| ApiError::not_found("file not found"))?;
    let (binary, content, size_bytes) = file_preview(file);
    let to_commit = query.at.clone().or(store
        .head(&id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?);
    let history_url = format!(
        "/admin/vaults/{}/history/{}",
        url_path(&id),
        url_path(&path)
    );
    let diff_url = to_commit.as_ref().map(|to| {
        format!(
            "/admin/vaults/{}/diff?path={}&to={}",
            url_path(&id),
            url_query(&path),
            url_query(to)
        )
    });
    record_admin_view(&state, &session, &id, "view_commit", Some(&path), &headers).await?;
    Ok(Html(
        VaultFileViewTemplate {
            t: admin_text(&headers, &cookies),
            vault: vault_view,
            path,
            at: query.at,
            size_display: crate::human::format_bytes(size_bytes),
            binary,
            content,
            history_url,
            diff_url,
            enable_diff_endpoint: cfg.enable_diff_endpoint,
        }
        .render()
        .unwrap(),
    ))
}

async fn vault_file_history_html(
    state: AppState,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    id: String,
    path: &str,
) -> Result<Html<String>, ApiError> {
    ensure_admin_history_enabled(&state).await?;
    let (vault, vault_view) = admin_vault(&state, &id).await?;
    let path = crate::storage::path::normalize(path)
        .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let commits =
        crate::service::history::file_history(&state, &vault.user_id, &id, &path, 100).await?;
    let entries = commits
        .into_iter()
        .map(|commit| history_entry_view(&id, &path, commit, &timezone))
        .collect();
    record_admin_view(&state, &session, &id, "view_history", Some(&path), &headers).await?;
    Ok(Html(
        VaultHistoryTemplate {
            t: admin_text(&headers, &cookies),
            vault: vault_view,
            path,
            entries,
        }
        .render()
        .unwrap(),
    ))
}

async fn vault_diff_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    Path(id): Path<String>,
    Query(query): Query<DiffQuery>,
) -> Result<Html<String>, ApiError> {
    let cfg = state.runtime_cfg.snapshot().await;
    if !cfg.enable_history_ui || !cfg.enable_diff_endpoint {
        return Err(ApiError::not_found("history disabled"));
    }
    let (vault, vault_view) = admin_vault(&state, &id).await?;
    let diff = crate::service::diff::unified_diff(
        &state,
        &vault.user_id,
        &id,
        query.from.as_deref(),
        &query.to,
        &query.path,
    )
    .await?;
    let rows = diff_rows(&diff.patch);
    let from_label = diff
        .from
        .as_deref()
        .map(short_commit)
        .unwrap_or_else(|| "base".into());
    let to = diff.to.unwrap_or(query.to);
    let to_label = short_commit(&to);
    record_admin_view(
        &state,
        &session,
        &id,
        "view_diff",
        Some(&diff.path),
        &headers,
    )
    .await?;
    Ok(Html(
        VaultDiffTemplate {
            t: admin_text(&headers, &cookies),
            vault: vault_view,
            path: diff.path,
            from: diff.from,
            to,
            from_label,
            to_label,
            binary: diff.binary,
            truncated: diff.truncated,
            rows,
        }
        .render()
        .unwrap(),
    ))
}

async fn ensure_admin_history_enabled(state: &AppState) -> Result<(), ApiError> {
    if state.runtime_cfg.snapshot().await.enable_history_ui {
        Ok(())
    } else {
        Err(ApiError::not_found("history disabled"))
    }
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
    crate::service::vault::delete_vault_for_user(&state, &vault.user_id, &id).await?;
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
    let timezone = state.runtime_cfg.snapshot().await.timezone;
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
                created_at: fmt_ts(created_at, &timezone),
                last_sync_at: fmt_opt_ts(last_sync_at, &timezone),
                size_display: crate::human::format_i64_bytes(size_bytes),
                size_bytes,
                file_count,
            },
        )
        .collect())
}

async fn count_vaults_synced_today(state: &AppState) -> Result<usize, ApiError> {
    let now = chrono::Utc::now();
    let today_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default()
        .and_utc()
        .timestamp();
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM vaults WHERE last_sync_at IS NOT NULL AND last_sync_at >= ?",
    )
    .bind(today_start)
    .fetch_one(&state.pool)
    .await?;
    Ok(count.max(0) as usize)
}

async fn admin_vault(state: &AppState, id: &str) -> Result<(Vault, VaultBrowserView), ApiError> {
    let vault = state
        .vaults
        .find_by_id(id)
        .await?
        .ok_or_else(|| ApiError::not_found("vault not found"))?;
    let owner = state
        .users
        .find_by_id(&vault.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("owner not found"))?;
    let view = VaultBrowserView {
        id: vault.id.clone(),
        name: vault.name.clone(),
        owner_username: owner.username,
    };
    Ok((vault, view))
}

fn file_entry_view(vault_id: &str, entry: TreeEntry) -> VaultFileEntryView {
    VaultFileEntryView {
        name: file_name(&entry.path).to_string(),
        view_url: format!(
            "/admin/vaults/{}/files/{}",
            url_path(vault_id),
            url_path(&entry.path)
        ),
        size_display: crate::human::format_bytes(entry.size),
        kind: if entry.is_blob_pointer {
            "Binary".into()
        } else {
            "Text".into()
        },
        path: entry.path,
    }
}

fn file_preview(file: StoredFile) -> (bool, String, u64) {
    match file {
        StoredFile::Text { bytes } => {
            let size = bytes.len() as u64;
            (false, String::from_utf8_lossy(&bytes).into_owned(), size)
        }
        StoredFile::BlobPointer { hash, size, mime } => {
            let mime = mime.unwrap_or_else(|| "application/octet-stream".into());
            (true, format!("{mime}\n{hash}"), size)
        }
    }
}

fn history_entry_view(
    vault_id: &str,
    path: &str,
    commit: crate::service::history::CommitSummary,
    timezone: &str,
) -> VaultHistoryEntryView {
    let short_commit = short_commit(&commit.commit);
    let view_url = format!(
        "/admin/vaults/{}/files/{}?at={}",
        url_path(vault_id),
        url_path(path),
        url_query(&commit.commit)
    );
    let mut diff_url = format!(
        "/admin/vaults/{}/diff?path={}&to={}",
        url_path(vault_id),
        url_query(path),
        url_query(&commit.commit)
    );
    if let Some(parent) = &commit.parent {
        diff_url.push_str("&from=");
        diff_url.push_str(&url_query(parent));
    }
    VaultHistoryEntryView {
        commit: commit.commit,
        short_commit,
        parent: commit.parent,
        message: commit
            .message
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .to_string(),
        timestamp: fmt_ts(commit.timestamp, timezone),
        author_device: commit
            .author_device
            .unwrap_or_else(|| "Unknown device".into()),
        change_type: commit
            .change_type
            .map(|kind| change_type_label(&kind))
            .unwrap_or_else(|| "modified".into()),
        view_url,
        diff_url,
    }
}

fn diff_rows(patch: &str) -> Vec<DiffRowView> {
    let lines: Vec<&str> = patch.lines().collect();
    let mut rows = Vec::new();
    let mut left_line = 0usize;
    let mut right_line = 0usize;
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index];
        let class = diff_line_class(line);
        if class == "diff-meta" || class == "diff-hunk" {
            if class == "diff-hunk" {
                if let Some((left, right)) = parse_hunk_header(line) {
                    left_line = left;
                    right_line = right;
                }
            }
            rows.push(DiffRowView {
                class: class.into(),
                full_width: true,
                text: line.to_string(),
                left_line: None,
                right_line: None,
                left_class: String::new(),
                right_class: String::new(),
                left_text: String::new(),
                right_text: String::new(),
            });
            index += 1;
            continue;
        }

        if class == "diff-context" {
            rows.push(DiffRowView {
                class: class.into(),
                full_width: false,
                text: String::new(),
                left_line: Some(left_line),
                right_line: Some(right_line),
                left_class: "diff-context".into(),
                right_class: "diff-context".into(),
                left_text: strip_diff_prefix(line).into(),
                right_text: strip_diff_prefix(line).into(),
            });
            left_line += 1;
            right_line += 1;
            index += 1;
            continue;
        }

        if class == "diff-del" {
            let mut deleted = Vec::new();
            let mut added = Vec::new();
            let mut cursor = index;
            while lines
                .get(cursor)
                .map(|line| diff_line_class(line) == "diff-del")
                .unwrap_or(false)
            {
                deleted.push(lines[cursor]);
                cursor += 1;
            }
            while lines
                .get(cursor)
                .map(|line| diff_line_class(line) == "diff-add")
                .unwrap_or(false)
            {
                added.push(lines[cursor]);
                cursor += 1;
            }
            for offset in 0..deleted.len().max(added.len()) {
                match (deleted.get(offset), added.get(offset)) {
                    (Some(deleted_line), Some(added_line)) => {
                        rows.push(DiffRowView {
                            class: "diff-modify".into(),
                            full_width: false,
                            text: String::new(),
                            left_line: Some(left_line),
                            right_line: Some(right_line),
                            left_class: "diff-del".into(),
                            right_class: "diff-add".into(),
                            left_text: strip_diff_prefix(deleted_line).into(),
                            right_text: strip_diff_prefix(added_line).into(),
                        });
                        left_line += 1;
                        right_line += 1;
                    }
                    (Some(deleted_line), None) => {
                        rows.push(DiffRowView {
                            class: "diff-del".into(),
                            full_width: false,
                            text: String::new(),
                            left_line: Some(left_line),
                            right_line: None,
                            left_class: "diff-del".into(),
                            right_class: "diff-empty".into(),
                            left_text: strip_diff_prefix(deleted_line).into(),
                            right_text: String::new(),
                        });
                        left_line += 1;
                    }
                    (None, Some(added_line)) => {
                        rows.push(DiffRowView {
                            class: "diff-add".into(),
                            full_width: false,
                            text: String::new(),
                            left_line: None,
                            right_line: Some(right_line),
                            left_class: "diff-empty".into(),
                            right_class: "diff-add".into(),
                            left_text: String::new(),
                            right_text: strip_diff_prefix(added_line).into(),
                        });
                        right_line += 1;
                    }
                    (None, None) => {}
                }
            }
            index = cursor;
            continue;
        }

        rows.push(DiffRowView {
            class: "diff-add".into(),
            full_width: false,
            text: String::new(),
            left_line: None,
            right_line: Some(right_line),
            left_class: "diff-empty".into(),
            right_class: "diff-add".into(),
            left_text: String::new(),
            right_text: strip_diff_prefix(line).into(),
        });
        right_line += 1;
        index += 1;
    }
    rows
}

fn diff_line_class(line: &str) -> &'static str {
    if line.starts_with("@@") {
        "diff-hunk"
    } else if line.starts_with("+++") || line.starts_with("---") || line.starts_with('\\') {
        "diff-meta"
    } else if line.starts_with('+') {
        "diff-add"
    } else if line.starts_with('-') {
        "diff-del"
    } else {
        "diff-context"
    }
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "@@" {
        return None;
    }
    let left = parts.next()?.trim_start_matches('-');
    let right = parts.next()?.trim_start_matches('+');
    Some((parse_hunk_start(left)?, parse_hunk_start(right)?))
}

fn parse_hunk_start(value: &str) -> Option<usize> {
    value.split(',').next()?.parse().ok()
}

fn strip_diff_prefix(line: &str) -> &str {
    if line.starts_with(' ') || line.starts_with('+') || line.starts_with('-') {
        &line[1..]
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_client_ips_for_admin_activity_views() {
        assert_eq!(mask_client_ip("203.0.113.42"), "203.*.*.42");
        assert_eq!(
            mask_client_ip("2001:db8:85a3::8a2e:370:7334"),
            "2001:db8:*:*:*:*:370:7334"
        );
        assert_eq!(mask_client_ip("not-an-ip"), "redacted");
    }

    #[test]
    fn diff_rows_pair_deleted_and_added_lines_for_split_view() {
        let rows = diff_rows("--- c1\n+++ c2\n@@ -1,2 +1,2 @@\n keep\n-old\n+new");

        assert_eq!(rows[3].left_text, "keep");
        assert_eq!(rows[3].right_text, "keep");
        assert_eq!(rows[4].class, "diff-modify");
        assert_eq!(rows[4].left_line, Some(2));
        assert_eq!(rows[4].right_line, Some(2));
        assert_eq!(rows[4].left_text, "old");
        assert_eq!(rows[4].right_text, "new");
    }

    #[test]
    fn diff_rows_pair_grouped_deleted_and_added_blocks() {
        let rows =
            diff_rows("@@ -1,4 +1,4 @@\n-old title\n-old subtitle\n+new title\n+new subtitle");

        assert_eq!(rows[1].class, "diff-modify");
        assert_eq!(rows[1].left_text, "old title");
        assert_eq!(rows[1].right_text, "new title");
        assert_eq!(rows[2].class, "diff-modify");
        assert_eq!(rows[2].left_text, "old subtitle");
        assert_eq!(rows[2].right_text, "new subtitle");
    }

    async fn admin_login_test_app(
        active: bool,
    ) -> (axum::Router, AppState, crate::db::repos::User) {
        use crate::auth::LoginRateLimiter;
        use crate::db::pool;
        use crate::db::repos::{NewUser, UserRepo};
        use crate::middleware::real_ip::ClientIp;
        use axum::extract::Extension;
        use std::time::Duration;

        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "admin".into(),
                password_hash: crate::auth::password::hash("passw0rd!!").unwrap(),
                is_admin: true,
            })
            .await
            .unwrap();
        state.users.set_active(&user.id, active).await.unwrap();
        let app = router()
            .with_state(state.clone())
            .layer(tower_cookies::CookieManagerLayer::new())
            .layer(Extension(AdminCookiePolicy {
                secure: false,
                public_host: None,
            }))
            .layer(Extension(LoginRateLimiter::new(
                10,
                Duration::from_secs(60),
                Duration::from_secs(60),
            )))
            .layer(Extension(ClientIp("127.0.0.1".parse().unwrap())));
        (app, state, user)
    }

    fn login_request() -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("POST")
            .uri("/admin/login")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(axum::body::Body::from(
                "username=admin&password=passw0rd%21%21",
            ))
            .unwrap()
    }

    #[tokio::test]
    async fn admin_login_rotates_existing_sessions_for_user() {
        use crate::admin::session;
        use tower::ServiceExt;

        let (app, state, user) = admin_login_test_app(true).await;
        let old_session = session::create_session(&state, &user.id).await.unwrap();

        let resp = app.oneshot(login_request()).await.unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::SEE_OTHER);
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM admin_sessions WHERE user_id = ?")
                .bind(&user.id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        let (old_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM admin_sessions WHERE id = ?")
                .bind(old_session)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(count, 1);
        assert_eq!(old_count, 0);
    }

    #[tokio::test]
    async fn inactive_admin_login_uses_generic_error() {
        use axum::body::to_bytes;
        use tower::ServiceExt;

        let (app, _state, _user) = admin_login_test_app(false).await;

        let resp = app.oneshot(login_request()).await.unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
        let body =
            String::from_utf8(to_bytes(resp.into_body(), 16384).await.unwrap().to_vec()).unwrap();
        assert!(body.contains("Invalid credentials"));
        assert!(!body.contains("Account disabled"));
    }

    #[tokio::test]
    async fn language_redirect_rejects_admin_prefix_without_boundary() {
        use crate::db::pool;
        use crate::service::AppState;
        use axum::body::Body;
        use axum::extract::Extension;
        use axum::http::Request;
        use tower::ServiceExt;

        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        let app = router()
            .with_state(state)
            .layer(tower_cookies::CookieManagerLayer::new())
            .layer(Extension(AdminCookiePolicy {
                secure: false,
                public_host: None,
            }));

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/language/en?next=/admin@attacker.test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.headers()["location"].to_str().unwrap(), "/admin");
    }
}

fn change_type_label(kind: &crate::service::diff::ChangeType) -> String {
    match kind {
        crate::service::diff::ChangeType::Added => "added",
        crate::service::diff::ChangeType::Modified => "modified",
        crate::service::diff::ChangeType::Deleted => "deleted",
    }
    .into()
}

async fn record_admin_view(
    state: &AppState,
    session: &AdminSession,
    vault_id: &str,
    action: &str,
    path: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let details = path.map(|path| serde_json::json!({ "path": path }).to_string());
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok());
    state
        .activities
        .insert(NewActivity {
            user_id: &session.user.id,
            vault_id: Some(vault_id),
            token_id: None,
            action,
            commit_hash: None,
            client_ip: None,
            user_agent,
            details: details.as_deref(),
        })
        .await?;
    Ok(())
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn short_commit(commit: &str) -> String {
    commit.chars().take(7).collect()
}

fn url_path(value: &str) -> String {
    utf8_percent_encode(value, URL_PATH).to_string()
}

fn url_query(value: &str) -> String {
    utf8_percent_encode(value, URL_QUERY).to_string()
}

async fn invites_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Html<String>, ApiError> {
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let invites: Vec<InviteAdminView> = state
        .invites
        .list_active(chrono::Utc::now().timestamp())
        .await?
        .into_iter()
        .map(|invite| InviteAdminView {
            code: invite.code,
            created_at: fmt_ts(invite.created_at, &timezone),
            expires_at: fmt_opt_ts(invite.expires_at, &timezone),
            used_at: fmt_opt_ts(invite.used_at, &timezone),
        })
        .collect();
    let used_invites = invites
        .iter()
        .filter(|invite| invite.used_at.is_some())
        .count();
    let pending_invites = invites.len().saturating_sub(used_invites);
    Ok(Html(
        InvitesTemplate {
            t: admin_text(&headers, &cookies),
            invites,
            pending_invites,
            used_invites,
            revoked_invites: 0,
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
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let expires_at = parse_invite_expires_at(form.expires_at.as_deref(), &timezone)?;
    state.invites.create(&session.user.id, expires_at).await?;
    Ok(Redirect::to("/admin/invites"))
}

fn parse_invite_expires_at(value: Option<&str>, timezone: &str) -> Result<Option<i64>, ApiError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = if value.chars().all(|c| c.is_ascii_digit()) {
        value
            .parse::<i64>()
            .map_err(|_| ApiError::bad_request("bad_expires_at", "invalid expiry"))?
    } else {
        let dt = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M")
            .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S"))
            .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S"))
            .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M"))
            .map_err(|_| {
                ApiError::bad_request("bad_expires_at", "use a future date/time or unix seconds")
            })?;
        let tz = timezone.parse::<Tz>().unwrap_or(Tz::UTC);
        tz.from_local_datetime(&dt)
            .single()
            .ok_or_else(|| ApiError::bad_request("bad_expires_at", "invalid local time"))?
            .timestamp()
    };
    if parsed <= chrono::Utc::now().timestamp() {
        return Err(ApiError::bad_request(
            "bad_expires_at",
            "expiry must be in the future",
        ));
    }
    Ok(Some(parsed))
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
    let cfg = state.runtime_cfg.snapshot().await;
    Ok(Html(
        SettingsTemplate {
            t: admin_text(&headers, &cookies),
            max_file_size_display: crate::human::format_bytes(cfg.max_file_size),
            text_extensions_display: cfg.text_extensions.join(", "),
            extra_exclude_globs_display: cfg.extra_exclude_globs.join("\n"),
            cfg,
            git_available: state.git_available,
        }
        .render()
        .unwrap(),
    ))
}

#[derive(Deserialize)]
struct SettingsForm {
    server_name: String,
    timezone: String,
    registration_mode: String,
    login_failure_threshold: u32,
    login_window_seconds: u64,
    login_lock_seconds: u64,
    enable_history_ui: Option<String>,
    enable_diff_endpoint: Option<String>,
    enable_git_smart_http: Option<String>,
    extra_exclude_globs: String,
    sse_heartbeat_seconds: u64,
    push_debounce_ms: u32,
    inline_content_max_bytes: u32,
}

type ActivityRow = (
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

#[derive(Default, Deserialize)]
struct ActivityFilters {
    user_id: Option<String>,
    action: Option<String>,
}

async fn list_admin_activities(
    state: &AppState,
    limit: i64,
    filters: &ActivityFilters,
) -> Result<Vec<ActivityView>, ApiError> {
    let user_id = filters.user_id.as_deref().filter(|s| !s.is_empty());
    let action = filters.action.as_deref().filter(|s| !s.is_empty());
    let mut sql = String::from(
        "SELECT a.timestamp, u.username, a.action, a.vault_id, v.name, tok.device_name,
                a.client_ip, a.user_agent
         FROM sync_activity a
         JOIN users u ON u.id = a.user_id
         LEFT JOIN vaults v ON v.id = a.vault_id
         LEFT JOIN tokens tok ON tok.id = a.token_id
         WHERE 1 = 1",
    );
    if user_id.is_some() {
        sql.push_str(" AND a.user_id = ?");
    }
    if action.is_some() {
        sql.push_str(" AND a.action = ?");
    }
    sql.push_str(" ORDER BY a.timestamp DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, ActivityRow>(&sql);
    if let Some(user_id) = user_id {
        query = query.bind(user_id);
    }
    if let Some(action) = action {
        query = query.bind(action);
    }
    let rows: Vec<ActivityRow> = query.bind(limit).fetch_all(&state.pool).await?;
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    Ok(rows
        .into_iter()
        .map(
            |(
                timestamp,
                username,
                action,
                vault_id,
                vault_name,
                device_name,
                client_ip,
                user_agent,
            )| ActivityView {
                timestamp: fmt_ts(timestamp, &timezone),
                username,
                action,
                vault_id,
                vault_name,
                device_name,
                client_ip: client_ip.map(|ip| mask_client_ip(&ip)),
                user_agent,
            },
        )
        .collect())
}

fn mask_client_ip(value: &str) -> String {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(addr)) => {
            let octets = addr.octets();
            format!("{}.*.*.{}", octets[0], octets[3])
        }
        Ok(IpAddr::V6(addr)) => {
            let segments = addr.segments();
            format!(
                "{:x}:{:x}:*:*:*:*:{:x}:{:x}",
                segments[0], segments[1], segments[6], segments[7]
            )
        }
        Err(_) => "redacted".to_string(),
    }
}

async fn list_activity_filter_users(state: &AppState) -> Result<Vec<ActivityFilterUser>, ApiError> {
    Ok(state
        .users
        .list()
        .await?
        .into_iter()
        .map(|user| ActivityFilterUser {
            id: user.id,
            username: user.username,
        })
        .collect())
}

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
    let timezone = crate::time::normalize_timezone(&form.timezone)
        .ok_or_else(|| ApiError::bad_request("bad_timezone", "invalid timezone"))?;
    state
        .runtime_cfg_repo
        .set_server_name(server_name, Some(&session.user.id))
        .await?;
    state
        .runtime_cfg_repo
        .set_timezone(&timezone, Some(&session.user.id))
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
    state
        .runtime_cfg_repo
        .set_history_flags(
            form.enable_history_ui.is_some(),
            form.enable_diff_endpoint.is_some(),
            Some(&session.user.id),
        )
        .await?;
    let extra_exclude_globs: Vec<String> = form
        .extra_exclude_globs
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    crate::service::exclude::EffectiveExcludes::compile(&extra_exclude_globs).map_err(|e| {
        ApiError::bad_request("invalid_glob", format!("invalid glob pattern: {}", e))
    })?;
    state
        .runtime_cfg_repo
        .set_extra_exclude_globs(extra_exclude_globs, Some(&session.user.id))
        .await?;
    state
        .runtime_cfg_repo
        .set_enable_git_smart_http(form.enable_git_smart_http.is_some(), Some(&session.user.id))
        .await?;
    state
        .runtime_cfg_repo
        .set_sse_heartbeat_seconds(form.sse_heartbeat_seconds, Some(&session.user.id))
        .await?;
    state
        .runtime_cfg_repo
        .set_push_debounce_ms(form.push_debounce_ms, Some(&session.user.id))
        .await?;
    state
        .runtime_cfg_repo
        .set_inline_content_max_bytes(form.inline_content_max_bytes, Some(&session.user.id))
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
    Query(filters): Query<ActivityFilters>,
) -> Result<Html<String>, ApiError> {
    let selected_user_id = filters.user_id.clone().unwrap_or_default();
    let selected_action = filters.action.clone().unwrap_or_default();
    Ok(Html(
        ActivityTemplate {
            t: admin_text(&headers, &cookies),
            activities: list_admin_activities(&state, ADMIN_ACTIVITY_LIMIT, &filters).await?,
            users: list_activity_filter_users(&state).await?,
            selected_user_id,
            selected_action,
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
