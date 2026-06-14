use crate::admin::password::hash_admin_password;
use crate::admin::session::{self, AdminSession};
use crate::admin::templates::{
    avatar_label, ActivityFilterUser, ActivityTemplate, ActivityView, DashboardTemplate,
    DeviceTokenAdminView, DevicesTemplate, DiffRowView, InviteAdminView, InvitesTemplate,
    LoginTemplate, SettingsTemplate, SetupTemplate, TokenAdminView, UserAdminView,
    UserDetailTemplate, UserOptionView, UsersTemplate, VaultAdminView, VaultBrowserView,
    VaultDiffTemplate, VaultFileEntryView, VaultFileViewTemplate, VaultFilesTemplate,
    VaultHistoryEntryView, VaultHistoryTemplate, VaultSettingsTemplate, VaultsTemplate,
};
use crate::api::error::ApiError;
use crate::auth::LoginRateLimiter;
use crate::auth::{password, token};
use crate::db::repos::{
    InviteRepo, NewActivity, NewToken, NewUser, RegistrationMode, RuntimeConfigRepo,
    RuntimeConfigSettingsUpdate, SyncActivityRepo, TokenRepo, TokenRow, User, UserOption, UserRepo,
    Vault, VaultRepo,
};
use crate::middleware::real_ip::{ClientIp, ForwardedFromTrustedProxy};
use crate::service::auth::validate_username;
use crate::service::AppState;
use crate::storage::git::{GitVaultStore, StoredFile, TreeEntry};
use axum::extract::{Extension, Form, Path, Query, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use chrono::{NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use rand::{rngs::OsRng, RngCore};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path as FsPath, PathBuf};
use subtle::ConstantTimeEq;
use tower_cookies::{Cookie, Cookies};

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
const ADMIN_DEVICE_TOKEN_DISPLAY_LIMIT: i64 = 500;
const SETUP_CSRF_COOKIE: &str = "pkv_setup_csrf";
const LOGIN_CSRF_COOKIE: &str = "pkv_login_csrf";

#[derive(Clone)]
pub struct AdminCookiePolicy {
    pub secure: bool,
    pub public_host: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/static/admin.css", get(crate::admin::admin_css))
        .route("/admin/static/admin.js", get(crate::admin::admin_js))
        .route(
            "/admin/static/lucide-icons.svg",
            get(crate::admin::admin_icons),
        )
        .route("/admin/language", get(set_language_query))
        .route("/admin/language/:lang", get(set_language))
        .route("/admin/login", get(login_page).post(login_post))
        .route("/setup", get(setup_get).post(setup_post))
        .route("/admin/logout", post(logout))
        .route("/admin", get(dashboard))
        .route("/admin/upgrade/request", post(request_upgrade_form))
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
        .route(
            "/admin/vaults/:id/settings",
            get(vault_settings_page).post(vault_settings_post),
        )
        .route("/admin/vaults/:id/files", get(vault_files_page))
        .route("/admin/vaults/:id/files/*path", get(vault_file_view_page))
        .route(
            "/admin/vaults/:id/history/*path",
            get(vault_file_history_page),
        )
        .route("/admin/vaults/:id/diff", get(vault_diff_page))
        .route("/admin/vaults/:id/rollback", post(rollback_vault_form))
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

fn render_html<T: askama::Template>(template: T) -> Response {
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "admin template render failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

fn render_html_with_status<T: askama::Template>(status: StatusCode, template: T) -> Response {
    let mut response = render_html(template);
    if response.status() == StatusCode::OK {
        *response.status_mut() = status;
    }
    response
}

fn fmt_ts(timestamp: i64, timezone: &str) -> String {
    crate::time::format_unix_seconds(timestamp, timezone)
}

fn fmt_opt_ts(timestamp: Option<i64>, timezone: &str) -> Option<String> {
    timestamp.map(|ts| fmt_ts(ts, timezone))
}

fn token_fingerprint(id: &str) -> String {
    let trimmed = id.trim();
    if trimmed.chars().count() <= 12 {
        return trimmed.to_string();
    }
    let prefix: String = trimmed.chars().take(8).collect();
    let suffix: String = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}...{suffix}")
}

struct UserAdminStats {
    vault_count: i64,
    last_sync_at: Option<i64>,
}

async fn user_admin_stats(state: &AppState, user_id: &str) -> Result<UserAdminStats, ApiError> {
    let (vault_count, last_sync_at): (i64, Option<i64>) =
        sqlx::query_as("SELECT COUNT(id), MAX(last_sync_at) FROM vaults WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await?;
    Ok(UserAdminStats {
        vault_count,
        last_sync_at,
    })
}

fn user_view(user: User, timezone: &str, stats: UserAdminStats) -> UserAdminView {
    UserAdminView {
        id: user.id,
        avatar_label: avatar_label(&user.username),
        username: user.username,
        is_admin: user.is_admin,
        is_active: user.is_active,
        created_at: fmt_ts(user.created_at, timezone),
        vault_count: stats.vault_count,
        last_sync_at: fmt_opt_ts(stats.last_sync_at, timezone),
    }
}

fn user_option_view(user: UserOption) -> UserOptionView {
    UserOptionView {
        id: user.id,
        username: user.username,
    }
}

fn token_view(token: TokenRow, timezone: &str) -> TokenAdminView {
    TokenAdminView {
        fingerprint: token_fingerprint(&token.id),
        id: token.id,
        device_name: token.device_name,
        created_at: fmt_ts(token.created_at, timezone),
        last_used_at: fmt_opt_ts(token.last_used_at, timezone),
        revoked_at: fmt_opt_ts(token.revoked_at, timezone),
    }
}

async fn login_page(
    State(state): State<AppState>,
    Extension(cookie_policy): Extension<AdminCookiePolicy>,
    headers: HeaderMap,
    cookies: Cookies,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let t = admin_text(&headers, &cookies);
    let login_csrf = generate_csrf_token("lc");
    cookies.add(csrf_cookie(
        LOGIN_CSRF_COOKIE,
        login_csrf.clone(),
        cookie_policy.secure,
        "/admin/login",
    ));
    let success = params
        .get("setup")
        .filter(|value| value.as_str() == "complete")
        .map(|_| t.setup_success);
    render_html(LoginTemplate {
        t,
        error: None,
        success,
        setup_required: state.is_setup_pending().await,
        username_value: String::new(),
        login_csrf,
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub async fn setup_redirect_middleware(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if req.method() == Method::GET && req.uri().path() == "/admin" && state.is_setup_pending().await
    {
        return Redirect::to("/setup").into_response();
    }
    next.run(req).await
}

async fn set_language(
    Extension(cookie_policy): Extension<AdminCookiePolicy>,
    cookies: Cookies,
    _session: AdminSession,
    Path(lang): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Redirect {
    set_language_cookie(&cookie_policy, &cookies, &lang);
    redirect_to_safe_admin_next(&params)
}

async fn set_language_query(
    Extension(cookie_policy): Extension<AdminCookiePolicy>,
    cookies: Cookies,
    _session: AdminSession,
    Query(params): Query<HashMap<String, String>>,
) -> Redirect {
    if let Some(lang) = params.get("lang") {
        set_language_cookie(&cookie_policy, &cookies, lang);
    }
    redirect_to_safe_admin_next(&params)
}

fn set_language_cookie(policy: &AdminCookiePolicy, cookies: &Cookies, lang: &str) {
    if let Some(lang) = crate::admin::i18n::AdminLang::parse(lang) {
        cookies.add(crate::admin::i18n::language_cookie(lang, policy.secure));
    }
}

fn redirect_to_safe_admin_next(params: &HashMap<String, String>) -> Redirect {
    let next = params
        .get("next")
        .filter(|value| is_safe_admin_next(value))
        .map(String::as_str)
        .unwrap_or("/admin");
    Redirect::to(next)
}

fn is_safe_admin_next(value: &str) -> bool {
    let decoded = decode_admin_next_path(value);
    let value = decoded.as_str();
    if value.starts_with("//") || value.contains('\\') || value.bytes().any(|b| b < 0x20) {
        return false;
    }
    let path_end = value.find(['?', '#']).unwrap_or(value.len());
    let path = &value[..path_end];
    match path.strip_prefix("/admin") {
        Some("") => true,
        Some(rest) if rest.starts_with('/') => path
            .split('/')
            .all(|segment| segment != "." && segment != ".."),
        Some(rest) => rest.starts_with('?') || rest.starts_with('#'),
        None => false,
    }
}

fn decode_admin_next_path(value: &str) -> String {
    let mut current = value.to_string();
    for _ in 0..4 {
        let decoded = percent_decode_str(&current)
            .decode_utf8_lossy()
            .into_owned();
        if decoded == current {
            break;
        }
        current = decoded;
    }
    current
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
    login_csrf: Option<String>,
}

#[derive(Deserialize)]
struct SetupForm {
    username: String,
    password: String,
    confirm: String,
    setup_csrf: Option<String>,
}

fn generate_csrf_token(prefix: &str) -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("{prefix}_{}", hex::encode(bytes))
}

fn generate_setup_csrf() -> String {
    generate_csrf_token("sc")
}

fn csrf_cookie(
    name: &'static str,
    token: String,
    secure: bool,
    path: &'static str,
) -> Cookie<'static> {
    let mut cookie = Cookie::new(name, token);
    cookie.set_http_only(true);
    cookie.set_secure(secure);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    cookie.set_path(path);
    cookie
}

fn setup_csrf_cookie(token: String, secure: bool) -> Cookie<'static> {
    csrf_cookie(SETUP_CSRF_COOKIE, token, secure, "/setup")
}

fn expired_setup_csrf_cookie(secure: bool) -> Cookie<'static> {
    let mut cookie = Cookie::new(SETUP_CSRF_COOKIE, "");
    cookie.set_secure(secure);
    cookie.set_path("/setup");
    cookie.make_removal();
    cookie
}

fn csrf_matches(cookies: &Cookies, cookie_name: &str, form_token: &str) -> bool {
    let Some(cookie_token) = cookies.get(cookie_name) else {
        return false;
    };
    let cookie_value = cookie_token.value();
    if cookie_value.is_empty() || form_token.is_empty() || cookie_value.len() != form_token.len() {
        return false;
    }
    cookie_value.as_bytes().ct_eq(form_token.as_bytes()).into()
}

fn setup_csrf_matches(cookies: &Cookies, form_token: &str) -> bool {
    csrf_matches(cookies, SETUP_CSRF_COOKIE, form_token)
}

fn login_csrf_matches(cookies: &Cookies, form_token: &str) -> bool {
    csrf_matches(cookies, LOGIN_CSRF_COOKIE, form_token)
}

async fn setup_get(
    State(state): State<AppState>,
    Extension(cookie_policy): Extension<AdminCookiePolicy>,
    headers: HeaderMap,
    cookies: Cookies,
) -> Response {
    if !state.is_setup_pending().await {
        return StatusCode::NOT_FOUND.into_response();
    }
    let setup_csrf = generate_setup_csrf();
    cookies.add(setup_csrf_cookie(setup_csrf.clone(), cookie_policy.secure));
    render_html(SetupTemplate {
        t: admin_text(&headers, &cookies),
        error: None,
        username_value: String::new(),
        setup_csrf,
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn setup_post(
    State(state): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    Extension(cookie_policy): Extension<AdminCookiePolicy>,
    _forwarded_from_trusted_proxy: Option<Extension<ForwardedFromTrustedProxy>>,
    headers: HeaderMap,
    cookies: Cookies,
    Form(form): Form<SetupForm>,
) -> Result<Response, ApiError> {
    let t = admin_text(&headers, &cookies);
    if !state.is_setup_pending().await {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }
    if state.setup_limiter.check(format!("setup:{ip}")).is_err() {
        return Err(ApiError::too_many("too many setup attempts"));
    }
    if !setup_csrf_matches(&cookies, form.setup_csrf.as_deref().unwrap_or("")) {
        return Ok((StatusCode::FORBIDDEN, "csrf validation failed").into_response());
    }

    let username = match validate_setup_username(&form.username) {
        Ok(username) => username,
        Err(SetupUsernameValidationError::Invalid) => {
            return Ok(setup_error(
                t,
                &cookies,
                cookie_policy.secure,
                t.setup_username_invalid,
                form.username,
                StatusCode::BAD_REQUEST,
            ));
        }
    };
    if let Err(err) = validate_setup_password(&form.password, &form.confirm) {
        let message = match err {
            SetupPasswordValidationError::Mismatch => t.setup_password_mismatch,
            SetupPasswordValidationError::TooWeak => t.setup_password_too_weak,
        };
        return Ok(setup_error(
            t,
            &cookies,
            cookie_policy.secure,
            message,
            username,
            StatusCode::BAD_REQUEST,
        ));
    }
    if state.users.count_admins().await? > 0 {
        state.mark_setup_complete().await;
        return Ok(StatusCode::NOT_FOUND.into_response());
    }

    let password_hash = hash_admin_password(&form.password).await?;
    let created = state
        .users
        .create_first_admin(NewUser {
            username: username.clone(),
            password_hash,
            is_admin: true,
        })
        .await?;
    if created.is_none() {
        // Atomic race lost: another concurrent setup_post created the first
        // admin between our pre-check and INSERT. Seal setup and respond with
        // 404 so the loser falls through to the login page like any other
        // post-setup request.
        state.mark_setup_complete().await;
        return Ok(StatusCode::NOT_FOUND.into_response());
    }
    state.mark_setup_complete().await;
    cookies.add(expired_setup_csrf_cookie(cookie_policy.secure));
    tracing::info!(username = %username, "first admin created via setup wizard");
    Ok(Redirect::to("/admin/login?setup=complete").into_response())
}

fn setup_error(
    t: crate::admin::i18n::AdminText,
    cookies: &Cookies,
    secure_cookie: bool,
    message: &'static str,
    username_value: String,
    status: StatusCode,
) -> Response {
    let setup_csrf = generate_setup_csrf();
    cookies.add(setup_csrf_cookie(setup_csrf.clone(), secure_cookie));
    render_html_with_status(
        status,
        SetupTemplate {
            t,
            error: Some(message),
            username_value,
            setup_csrf,
            version: env!("CARGO_PKG_VERSION"),
        },
    )
}

enum SetupUsernameValidationError {
    Invalid,
}

enum SetupPasswordValidationError {
    TooWeak,
    Mismatch,
}

fn validate_setup_username(value: &str) -> Result<String, SetupUsernameValidationError> {
    let trimmed = value.trim();
    if trimmed.len() < 3 || trimmed.len() > 32 {
        return Err(SetupUsernameValidationError::Invalid);
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(SetupUsernameValidationError::Invalid);
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn validate_setup_password(
    password: &str,
    confirm: &str,
) -> Result<(), SetupPasswordValidationError> {
    if password != confirm {
        return Err(SetupPasswordValidationError::Mismatch);
    }
    if password::validate_strong(password).is_err() {
        return Err(SetupPasswordValidationError::TooWeak);
    }
    Ok(())
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
    if state.is_setup_pending().await {
        return Ok(setup_required_login(t, StatusCode::SERVICE_UNAVAILABLE));
    }
    if !login_csrf_matches(&cookies, form.login_csrf.as_deref().unwrap_or("")) {
        return Ok((StatusCode::FORBIDDEN, "csrf validation failed").into_response());
    }
    let reservation = match limiter.try_acquire(ip) {
        Ok(r) => r,
        Err(remaining) => {
            return Err(ApiError::too_many(format!(
                "locked for {}s",
                remaining.as_secs()
            )));
        }
    };

    let user = match crate::service::auth::verify_credentials(
        &state,
        &form.username,
        &form.password,
    )
    .await
    {
        Ok(u) => u,
        Err(e) if e.status == StatusCode::UNAUTHORIZED || e.status == StatusCode::FORBIDDEN => {
            reservation.failure();
            return Ok(login_error(
                t,
                &cookies,
                cookie_policy.secure,
                "Invalid credentials",
                StatusCode::UNAUTHORIZED,
            ));
        }
        Err(e) => {
            reservation.release();
            return Err(e);
        }
    };
    if !user.is_admin {
        reservation.failure();
        return Ok(login_error(
            t,
            &cookies,
            cookie_policy.secure,
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
    reservation.success();
    Ok(Redirect::to("/admin").into_response())
}

fn login_error(
    t: crate::admin::i18n::AdminText,
    cookies: &Cookies,
    secure_cookie: bool,
    message: &'static str,
    status: StatusCode,
) -> Response {
    let login_csrf = generate_csrf_token("lc");
    cookies.add(csrf_cookie(
        LOGIN_CSRF_COOKIE,
        login_csrf.clone(),
        secure_cookie,
        "/admin/login",
    ));
    render_html_with_status(
        status,
        LoginTemplate {
            t,
            error: Some(message),
            success: None,
            setup_required: false,
            username_value: String::new(),
            login_csrf,
            version: env!("CARGO_PKG_VERSION"),
        },
    )
}

fn setup_required_login(t: crate::admin::i18n::AdminText, status: StatusCode) -> Response {
    render_html_with_status(
        status,
        LoginTemplate {
            t,
            error: None,
            success: None,
            setup_required: true,
            username_value: String::new(),
            login_csrf: String::new(),
            version: env!("CARGO_PKG_VERSION"),
        },
    )
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
) -> Result<Response, ApiError> {
    let dashboard_summary = dashboard_summary(&state).await?;
    let metrics = collect_dashboard_metrics(state.data_dir.clone()).await?;
    let t = admin_text(&headers, &cookies);
    let uptime_seconds = crate::server::uptime_seconds();
    let recent_activities = list_admin_activities(&state, 5, &ActivityFilters::default()).await?;
    let update_status = state.update_status.read().await.clone();
    let last_update_check_at = *state.last_update_check_at.read().await;
    let last_sync_activity_at = dashboard_summary.last_sync_activity_at;
    let sse_subscribers = state.events.total_subscribers();
    let now = chrono::Utc::now().timestamp();
    let last_update_check_display = last_update_check_at
        .map(|ts| crate::human::format_duration_seconds((now - ts).max(0) as u64, t.html_lang))
        .unwrap_or_default();
    let last_sync_activity_display = last_sync_activity_at
        .map(|ts| crate::human::format_duration_seconds((now - ts).max(0) as u64, t.html_lang))
        .unwrap_or_default();
    let sync_status_state: &'static str = if sse_subscribers > 0 {
        "live"
    } else if last_sync_activity_at
        .map(|ts| (now - ts).max(0) < 24 * 60 * 60)
        .unwrap_or(false)
    {
        "idle"
    } else {
        "quiet"
    };
    Ok(render_html(DashboardTemplate {
        disk_used_display: crate::human::format_bytes(metrics.disk_used_bytes),
        disk_total_display: crate::human::format_bytes(metrics.disk_total_bytes),
        uptime_display: crate::human::format_duration_seconds(uptime_seconds, t.html_lang),
        t,
        username: session.user.username,
        users: dashboard_summary.users,
        vaults: dashboard_summary.vaults,
        cpu_percent: metrics.cpu_percent,
        cpu_display: format!("{:.0}", metrics.cpu_percent),
        cpu_cores_display: crate::admin::system::format_cpu_cores(metrics.cpu_cores),
        memory_display: crate::human::format_bytes(
            metrics.memory_used_mb.saturating_mul(1024 * 1024),
        ),
        memory_total_display: crate::human::format_bytes(
            metrics.memory_total_mb.saturating_mul(1024 * 1024),
        ),
        update_status,
        current_version: env!("CARGO_PKG_VERSION"),
        last_update_check_display,
        sse_subscribers,
        last_sync_activity_display,
        sync_status_state,
        recent_activities,
    }))
}

/// One-click "Upgrade now": if update-check has found a newer stable release,
/// write a device-local upgrade-request marker for the opt-in privileged updater
/// to apply. The server itself never upgrades; it only records the request.
async fn request_upgrade_form(State(state): State<AppState>, _session: AdminSession) -> Redirect {
    if let Some(status) = state.update_status.read().await.as_ref() {
        let now = chrono::Utc::now().timestamp().max(0) as u64;
        if let Err(err) = crate::service::upgrade_signal::request_upgrade(
            &state.data_dir,
            env!("CARGO_PKG_VERSION"),
            &status.latest_version,
            now,
        ) {
            tracing::warn!("failed to write upgrade-request marker: {err}");
        }
    }
    Redirect::to("/admin")
}

#[derive(Debug, PartialEq, Eq)]
struct DashboardSummary {
    users: i64,
    vaults: i64,
    last_sync_activity_at: Option<i64>,
}

async fn dashboard_summary(state: &AppState) -> Result<DashboardSummary, sqlx::Error> {
    let (users, vaults, last_sync_activity_at): (i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM users),
            (SELECT COUNT(*) FROM vaults),
            (SELECT MAX(timestamp) FROM sync_activity)",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(DashboardSummary {
        users,
        vaults,
        last_sync_activity_at,
    })
}

async fn collect_dashboard_metrics(
    data_dir: PathBuf,
) -> Result<crate::admin::system::SystemMetrics, ApiError> {
    collect_dashboard_metrics_with(data_dir, crate::admin::system::collect).await
}

async fn collect_dashboard_metrics_with<F>(
    data_dir: PathBuf,
    collect: F,
) -> Result<crate::admin::system::SystemMetrics, ApiError>
where
    F: FnOnce(&FsPath) -> crate::admin::system::SystemMetrics + Send + 'static,
{
    tokio::task::spawn_blocking(move || collect(&data_dir))
        .await
        .map_err(|_| ApiError::internal("dashboard metrics task panicked"))
}

type UserAdminRow = (String, String, bool, bool, i64, i64, Option<i64>);

async fn users_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
    Query(filters): Query<UserFilters>,
) -> Result<Response, ApiError> {
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let query = filters.q.unwrap_or_default().trim().to_string();
    let status = match filters.status.as_deref() {
        Some("active" | "inactive" | "admin") => filters.status.unwrap_or_default(),
        _ => String::new(),
    };
    let users = list_admin_users(&state, &timezone, &query, &status).await?;
    Ok(render_html(UsersTemplate {
        t: admin_text(&headers, &cookies),
        users,
        query,
        status,
        message: None,
    }))
}

async fn list_admin_users(
    state: &AppState,
    timezone: &str,
    query: &str,
    status: &str,
) -> Result<Vec<UserAdminView>, ApiError> {
    let query = query.trim();
    let has_query = !query.is_empty();
    let filter_sql = admin_user_filter_sql(has_query, status);
    let sql = format!(
        "SELECT u.id, u.username, u.is_admin, u.is_active, u.created_at,
                COUNT(v.id) AS vault_count, MAX(v.last_sync_at) AS last_sync_at
         FROM users u
         LEFT JOIN vaults v ON v.user_id = u.id
         {filter_sql}
         GROUP BY u.id, u.username, u.is_admin, u.is_active, u.created_at
         ORDER BY u.username"
    );
    let mut sql_query = sqlx::query_as::<_, UserAdminRow>(&sql);
    if has_query {
        sql_query = sql_query.bind(admin_user_search_pattern(query));
    }
    let rows: Vec<UserAdminRow> = sql_query.fetch_all(&state.pool).await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, username, is_admin, is_active, created_at, vault_count, last_sync_at)| {
                UserAdminView {
                    id,
                    avatar_label: avatar_label(&username),
                    username,
                    is_admin,
                    is_active,
                    created_at: fmt_ts(created_at, timezone),
                    vault_count,
                    last_sync_at: fmt_opt_ts(last_sync_at, timezone),
                }
            },
        )
        .collect())
}

fn admin_user_filter_sql(has_query: bool, status: &str) -> String {
    let mut filters = Vec::new();
    if has_query {
        filters.push("LOWER(u.username) LIKE ? ESCAPE '\\'");
    }
    match status {
        "active" => filters.push("u.is_active = 1"),
        "inactive" => filters.push("u.is_active = 0"),
        "admin" => filters.push("u.is_admin = 1"),
        _ => {}
    }
    if filters.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", filters.join(" AND "))
    }
}

fn admin_user_search_pattern(query: &str) -> String {
    let mut pattern = String::with_capacity(query.len() + 2);
    pattern.push('%');
    for ch in query.trim().to_lowercase().chars() {
        match ch {
            '%' | '_' | '\\' => {
                pattern.push('\\');
                pattern.push(ch);
            }
            _ => pattern.push(ch),
        }
    }
    pattern.push('%');
    pattern
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
    let password_hash = hash_admin_password(&form.password).await?;
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
) -> Result<Response, ApiError> {
    let user = state
        .users
        .find_by_id(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let stats = user_admin_stats(&state, &id).await?;
    let tokens = state
        .tokens
        .list_for_user(&id)
        .await?
        .into_iter()
        .map(|token| token_view(token, &timezone))
        .collect();
    Ok(render_html(UserDetailTemplate {
        t: admin_text(&headers, &cookies),
        user: user_view(user, &timezone, stats),
        tokens,
        message: None,
        error: None,
        created_token: None,
    }))
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
    if device_name.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "invalid_device_name",
            "device name cannot contain control characters",
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
) -> Result<Response, ApiError> {
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
    let stats = user_admin_stats(&state, &id).await?;
    let tokens = state
        .tokens
        .list_for_user(&id)
        .await?
        .into_iter()
        .map(|token| token_view(token, &timezone))
        .collect();
    Ok(render_html(UserDetailTemplate {
        t: admin_text(&headers, &cookies),
        user: user_view(user, &timezone, stats),
        tokens,
        message: Some("Device token created".into()),
        error: None,
        created_token: Some(raw),
    }))
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
    let password_hash = hash_admin_password(&form.password).await?;
    state.users.update_password(&id, &password_hash).await?;
    state
        .tokens
        .revoke_all_for_user(&id, chrono::Utc::now().timestamp(), None)
        .await?;
    session::delete_sessions_for_user(&state, &id).await?;
    Ok(Redirect::to(&format!("/admin/users/{id}")))
}

#[derive(Deserialize)]
struct ActiveForm {
    active: bool,
}

async fn set_active_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    Path(id): Path<String>,
    Form(form): Form<ActiveForm>,
) -> Result<Response, ApiError> {
    if session.user.id == id && !form.active {
        return user_detail_error(
            &state,
            &headers,
            &cookies,
            &id,
            admin_text(&headers, &cookies).cannot_disable_self,
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    state.users.set_active(&id, form.active).await?;
    Ok(Redirect::to(&format!("/admin/users/{id}")).into_response())
}

#[derive(Deserialize)]
struct AdminForm {
    admin: bool,
}

async fn set_admin_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    Path(id): Path<String>,
    Form(form): Form<AdminForm>,
) -> Result<Response, ApiError> {
    if !state
        .users
        .set_admin_preserving_last_admin(&id, form.admin)
        .await?
    {
        return user_detail_error(
            &state,
            &headers,
            &cookies,
            &id,
            admin_text(&headers, &cookies).cannot_demote_last_admin,
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    if session.user.id == id && !form.admin {
        session::delete_sessions_for_user(&state, &id).await?;
    }
    Ok(Redirect::to(&format!("/admin/users/{id}")).into_response())
}

async fn user_detail_error(
    state: &AppState,
    headers: &HeaderMap,
    cookies: &Cookies,
    id: &str,
    error: &'static str,
    status: StatusCode,
) -> Result<Response, ApiError> {
    let user = state
        .users
        .find_by_id(id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let timezone = state.runtime_cfg.snapshot().await.timezone;
    let stats = user_admin_stats(state, id).await?;
    let tokens = state
        .tokens
        .list_for_user(id)
        .await?
        .into_iter()
        .map(|token| token_view(token, &timezone))
        .collect();
    Ok(render_html_with_status(
        status,
        UserDetailTemplate {
            t: admin_text(headers, cookies),
            user: user_view(user, &timezone, stats),
            tokens,
            message: None,
            error: Some(error),
            created_token: None,
        },
    ))
}

async fn revoke_token_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Path((id, token_fingerprint)): Path<(String, String)>,
) -> Result<Redirect, ApiError> {
    let tokens = state.tokens.list_for_user(&id).await?;
    let token_id = resolve_token_fingerprint(
        tokens.iter().map(|token| token.id.as_str()),
        &token_fingerprint,
    )?;
    state
        .tokens
        .revoke(token_id, chrono::Utc::now().timestamp())
        .await?;
    tracing::info!(user_id = %id, token_id = %token_id, "admin revoked device token");
    Ok(Redirect::to(&format!("/admin/users/{id}")))
}

fn resolve_token_fingerprint<'a>(
    token_ids: impl Iterator<Item = &'a str>,
    fingerprint: &str,
) -> Result<&'a str, ApiError> {
    let mut matches = token_ids.filter(|id| token_fingerprint(id) == fingerprint);
    let Some(token_id) = matches.next() else {
        return Err(ApiError::not_found("token not found"));
    };
    if matches.next().is_some() {
        return Err(ApiError::conflict(
            "ambiguous_token_fingerprint",
            "token fingerprint is ambiguous",
        ));
    }
    Ok(token_id)
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
) -> Result<Response, ApiError> {
    Ok(render_html(DevicesTemplate {
        t: admin_text(&headers, &cookies),
        users: list_user_options(&state).await?,
        tokens: list_admin_device_tokens(&state).await?,
        created_token: None,
    }))
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
) -> Result<Response, ApiError> {
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
    Ok(render_html(DevicesTemplate {
        t: admin_text(&headers, &cookies),
        users: list_user_options(&state).await?,
        tokens: list_admin_device_tokens(&state).await?,
        created_token: Some(raw),
    }))
}

async fn revoke_device_token_form(
    State(state): State<AppState>,
    _session: AdminSession,
    Path(token_fingerprint): Path<String>,
) -> Result<Redirect, ApiError> {
    let token_ids = list_admin_device_token_ids(&state).await?;
    let token_id =
        resolve_token_fingerprint(token_ids.iter().map(String::as_str), &token_fingerprint)?;
    state
        .tokens
        .revoke(token_id, chrono::Utc::now().timestamp())
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
         ORDER BY tok.revoked_at IS NOT NULL, tok.created_at DESC, tok.id DESC
         LIMIT ?",
    )
    .bind(ADMIN_DEVICE_TOKEN_DISPLAY_LIMIT)
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
            )| {
                let fingerprint = token_fingerprint(&id);
                DeviceTokenAdminView {
                    id,
                    fingerprint,
                    user_id,
                    username,
                    device_id,
                    device_name,
                    created_at: fmt_ts(created_at, &timezone),
                    last_used_at: fmt_opt_ts(last_used_at, &timezone),
                    revoked_at: fmt_opt_ts(revoked_at, &timezone),
                }
            },
        )
        .collect())
}

async fn list_admin_device_token_ids(state: &AppState) -> Result<Vec<String>, ApiError> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT id FROM tokens ORDER BY created_at DESC, id DESC")
            .fetch_all(&state.pool)
            .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

type VaultAdminRow = (String, String, String, String, i64, Option<i64>, i64, i64);

async fn vaults_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Response, ApiError> {
    let cfg = state.runtime_cfg.snapshot().await;
    let vaults = list_admin_vaults(&state).await?;
    let total_size: u64 = vaults.iter().map(|v| v.size_bytes.max(0) as u64).sum();
    let synced_today = count_vaults_synced_today(&state).await?;
    Ok(render_html(VaultsTemplate {
        t: admin_text(&headers, &cookies),
        total_vaults: vaults.len(),
        total_size_display: crate::human::format_bytes(total_size),
        synced_today,
        vaults,
        users: list_user_options(&state).await?,
        message: None,
        enable_history_ui: cfg.enable_history_ui,
    }))
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

struct AdminViewRequest {
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    client_ip: IpAddr,
}

async fn vault_files_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    ensure_admin_history_enabled(&state).await?;
    let (_vault, vault_view) = admin_vault(&state, &id).await?;
    let store = state.git_store();
    let entries = store
        .list_tree(&id, None)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let files = entries
        .into_iter()
        .map(|entry| file_entry_view(&id, entry))
        .collect();
    Ok(render_html(VaultFilesTemplate {
        t: admin_text(&headers, &cookies),
        vault: vault_view,
        files,
    }))
}

async fn vault_file_view_page(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    Path((id, path)): Path<(String, String)>,
    Query(query): Query<FileViewQuery>,
) -> Result<Response, ApiError> {
    ensure_admin_history_enabled(&state).await?;
    vault_file_view_html(
        state,
        AdminViewRequest {
            headers,
            cookies,
            session,
            client_ip,
        },
        id,
        path,
        query,
    )
    .await
}

async fn vault_file_history_page(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    Path((id, path)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    ensure_admin_history_enabled(&state).await?;
    vault_file_history_html(
        state,
        AdminViewRequest {
            headers,
            cookies,
            session,
            client_ip,
        },
        id,
        &path,
    )
    .await
}

async fn vault_file_view_html(
    state: AppState,
    request: AdminViewRequest,
    id: String,
    path: String,
    query: FileViewQuery,
) -> Result<Response, ApiError> {
    let cfg = state.runtime_cfg.snapshot().await;
    let (_vault, vault_view) = admin_vault(&state, &id).await?;
    let path = crate::storage::path::normalize(&path)
        .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
    let store = state.git_store();
    let file = store
        .read_file(&id, &path, query.at.as_deref())
        .await
        .map_err(|e| {
            tracing::warn!(
                error = %e,
                vault_id = %id,
                file_path = %path,
                commit = query.at.as_deref().unwrap_or("HEAD"),
                "admin vault file read failed"
            );
            ApiError::bad_request("bad_commit", "invalid commit")
        })?
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
    record_admin_view(
        &state,
        &request.session,
        &id,
        "view_commit",
        Some(&path),
        &request.headers,
        request.client_ip,
    )
    .await?;
    Ok(render_html(VaultFileViewTemplate {
        t: admin_text(&request.headers, &request.cookies),
        vault: vault_view,
        path,
        at: query.at,
        size_display: crate::human::format_bytes(size_bytes),
        binary,
        content,
        history_url,
        diff_url,
        enable_diff_endpoint: cfg.enable_diff_endpoint,
    }))
}

async fn vault_file_history_html(
    state: AppState,
    request: AdminViewRequest,
    id: String,
    path: &str,
) -> Result<Response, ApiError> {
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
    record_admin_view(
        &state,
        &request.session,
        &id,
        "view_history",
        Some(&path),
        &request.headers,
        request.client_ip,
    )
    .await?;
    Ok(render_html(VaultHistoryTemplate {
        t: admin_text(&request.headers, &request.cookies),
        vault: vault_view,
        path,
        entries,
    }))
}

async fn vault_diff_page(
    State(state): State<AppState>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    cookies: Cookies,
    session: AdminSession,
    Path(id): Path<String>,
    Query(query): Query<DiffQuery>,
) -> Result<Response, ApiError> {
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
        client_ip,
    )
    .await?;
    Ok(render_html(VaultDiffTemplate {
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
    }))
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

#[derive(Deserialize)]
struct VaultSettingsForm {
    extra_sync_globs: String,
    apply_starter: Option<String>,
}

#[derive(Deserialize)]
struct VaultRollbackForm {
    commit: String,
    path: Option<String>,
}

async fn vault_settings_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let (_vault, vault_view) = admin_vault(&state, &id).await?;
    let settings = crate::service::vault_settings::load(&state, &id).await?;
    Ok(render_html(VaultSettingsTemplate {
        t: admin_text(&headers, &cookies),
        vault: vault_view,
        extra_sync_globs_display: settings.extra_sync_globs.join("\n"),
    }))
}

async fn vault_settings_post(
    State(state): State<AppState>,
    _session: AdminSession,
    Path(id): Path<String>,
    Form(form): Form<VaultSettingsForm>,
) -> Result<Redirect, ApiError> {
    let (_vault, _vault_view) = admin_vault(&state, &id).await?;
    let extra_sync_globs = if form.apply_starter.is_some() {
        crate::service::vault_settings::starter_extra_sync_globs()
    } else {
        parse_glob_lines(&form.extra_sync_globs)?
    };
    crate::service::vault_settings::save(
        &state,
        &id,
        &crate::service::vault_settings::VaultSettings { extra_sync_globs },
    )
    .await?;
    Ok(Redirect::to(&format!("/admin/vaults/{id}/settings")))
}

async fn rollback_vault_form(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
    Form(form): Form<VaultRollbackForm>,
) -> Result<Redirect, ApiError> {
    ensure_admin_history_enabled(&state).await?;
    let (_vault, _vault_view) = admin_vault(&state, &id).await?;
    let device_id = format!("admin-web-{}", session.user.id);
    let actor = crate::service::vault::RollbackActor {
        user_id: &session.user.id,
        is_admin: session.user.is_admin,
        token_id: None,
        device_id: &device_id,
    };
    crate::service::vault::rollback_to_commit_as(&state, actor, &id, &form.commit)
        .await
        .map_err(rollback_error_to_api)?;

    let next = form
        .path
        .as_deref()
        .and_then(|path| crate::storage::path::normalize(path).ok())
        .map(|path| {
            format!(
                "/admin/vaults/{}/history/{}",
                url_path(&id),
                url_path(&path)
            )
        })
        .unwrap_or_else(|| format!("/admin/vaults/{}/files", url_path(&id)));
    Ok(Redirect::to(&next))
}

fn rollback_error_to_api(err: crate::service::vault::RollbackError) -> ApiError {
    match err {
        crate::service::vault::RollbackError::NotFound => ApiError::not_found("vault not found"),
        crate::service::vault::RollbackError::Forbidden => {
            ApiError::forbidden("vault access denied")
        }
        crate::service::vault::RollbackError::UnknownCommit { .. } => {
            ApiError::bad_request("unknown_commit", "commit is not reachable from vault head")
        }
        crate::service::vault::RollbackError::Internal(message) => ApiError::internal(message),
    }
}

async fn create_vault_form(
    State(state): State<AppState>,
    session: AdminSession,
    Form(form): Form<CreateVaultForm>,
) -> Result<Redirect, ApiError> {
    state
        .users
        .find_by_id(&form.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;
    let vault = crate::service::vault::create_vault(&state, &form.user_id, &form.name).await?;
    crate::service::vault::record_lifecycle_activity(
        &state,
        &session.user.id,
        None,
        "create_vault",
        &vault,
        None,
        None,
    )
    .await?;
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
    session: AdminSession,
    Path(id): Path<String>,
) -> Result<Redirect, ApiError> {
    let vault = state
        .vaults
        .find_by_id(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("vault not found"))?;
    crate::service::vault::delete_vault_for_user(&state, &vault.user_id, &id).await?;
    crate::service::vault::record_lifecycle_activity(
        &state,
        &session.user.id,
        None,
        "delete_vault",
        &vault,
        None,
        None,
    )
    .await?;
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
    let rollback_url = format!("/admin/vaults/{}/rollback", url_path(vault_id));
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
        rollback_url,
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

    struct FailingTemplate;

    impl std::fmt::Display for FailingTemplate {
        fn fmt(&self, _formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }

    impl askama::Template for FailingTemplate {
        fn render_into(&self, _writer: &mut (impl std::fmt::Write + ?Sized)) -> askama::Result<()> {
            Err(askama::Error::Fmt(std::fmt::Error))
        }

        const EXTENSION: Option<&'static str> = Some("html");
        const SIZE_HINT: usize = 0;
        const MIME_TYPE: &'static str = "text/html";
    }

    #[test]
    fn render_html_returns_internal_server_error_when_template_render_fails() {
        let response = render_html(FailingTemplate);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn masks_client_ips_for_admin_activity_views() {
        assert_eq!(mask_client_ip("203.0.113.42"), "203.0.113.*");
        assert_eq!(
            mask_client_ip("2001:db8:85a3::8a2e:370:7334"),
            "2001:db8:85a3:*:*:*:*:*"
        );
        assert_eq!(
            mask_client_ip("2001:db8:85a3:abcd::1"),
            "2001:db8:85a3:*:*:*:*:*"
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

    #[test]
    fn admin_user_filters_are_pushed_into_sql() {
        assert_eq!(
            admin_user_filter_sql(true, "active"),
            " WHERE LOWER(u.username) LIKE ? ESCAPE '\\' AND u.is_active = 1"
        );
        assert_eq!(
            admin_user_filter_sql(true, "inactive"),
            " WHERE LOWER(u.username) LIKE ? ESCAPE '\\' AND u.is_active = 0"
        );
        assert_eq!(
            admin_user_filter_sql(false, "admin"),
            " WHERE u.is_admin = 1"
        );
    }

    #[test]
    fn admin_user_search_escapes_like_wildcards() {
        assert_eq!(admin_user_search_pattern("Bo_B%"), "%bo\\_b\\%%");
    }

    #[test]
    fn admin_next_rejects_percent_encoded_dot_segments() {
        assert!(!is_safe_admin_next("/admin/%2e%2e/api/config"));
        assert!(!is_safe_admin_next("/admin/%252e%252e/api/config"));
        assert!(is_safe_admin_next("/admin/users?q=a%2Eb"));
    }

    #[test]
    fn admin_next_rejects_percent_decoded_control_characters() {
        assert!(!is_safe_admin_next("/admin/x%0dy"));
        assert!(!is_safe_admin_next("/admin/x%250ay"));
        assert!(!is_safe_admin_next("/admin/users?tab=a%09b"));
    }

    #[tokio::test]
    async fn dashboard_summary_combines_counts_and_last_sync_activity() {
        use crate::db::pool;
        use crate::db::repos::{NewUser, UserRepo, VaultRepo};

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
                password_hash: "h".into(),
                is_admin: true,
            })
            .await
            .unwrap();
        state.vaults.create(&user.id, "main").await.unwrap();
        state
            .activities
            .insert(NewActivity {
                user_id: &user.id,
                vault_id: None,
                token_id: None,
                action: "login",
                commit_hash: None,
                client_ip: None,
                user_agent: None,
                details: None,
            })
            .await
            .unwrap();

        let summary = dashboard_summary(&state).await.unwrap();

        assert_eq!(summary.users, 1);
        assert_eq!(summary.vaults, 1);
        assert!(summary.last_sync_activity_at.is_some());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_metrics_collection_runs_off_async_thread() {
        let caller_thread = std::thread::current().id();
        let worker_thread = std::sync::Arc::new(std::sync::Mutex::new(None));
        let observed_thread = worker_thread.clone();

        let metrics = collect_dashboard_metrics_with(PathBuf::from("."), move |_| {
            *observed_thread.lock().unwrap() = Some(std::thread::current().id());
            crate::admin::system::SystemMetrics {
                cpu_percent: 0.0,
                cpu_cores: 1.0,
                memory_used_mb: 1,
                memory_total_mb: 2,
                disk_used_bytes: 3,
                disk_total_bytes: 4,
            }
        })
        .await
        .unwrap();

        assert_eq!(metrics.disk_total_bytes, 4);
        assert_ne!(
            worker_thread
                .lock()
                .unwrap()
                .expect("worker thread recorded"),
            caller_thread
        );
    }

    #[test]
    fn settings_post_uses_batched_runtime_config_update() {
        let source = include_str!("handlers.rs");
        let start = source.find(concat!("async ", "fn settings_post(")).unwrap();
        let end = source[start..]
            .find(concat!("async ", "fn activity_page("))
            .unwrap();
        let settings_post = &source[start..start + end];

        assert!(settings_post.contains(".set_admin_settings"));
        for legacy_setter in [
            ".set_server_name(",
            ".set_timezone(",
            ".set_registration_mode(",
            ".set_login_rate_limit(",
            ".set_history_flags(",
            ".set_extra_exclude_globs(",
            ".set_enable_git_smart_http(",
            ".set_enable_metrics(",
            ".set_enable_auto_merge(",
            ".set_update_check_enabled(",
            ".set_update_check_interval_seconds(",
            ".set_sse_heartbeat_seconds(",
            ".set_push_debounce_ms(",
            ".set_inline_content_max_bytes(",
        ] {
            assert!(
                !settings_post.contains(legacy_setter),
                "settings_post should use the batch settings writer instead of {legacy_setter}"
            );
        }
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

    fn login_request_body(extra: &str) -> axum::body::Body {
        axum::body::Body::from(format!("username=admin&password=passw0rd%21%21{extra}"))
    }

    fn login_request() -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("POST")
            .uri("/admin/login")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(login_request_body(""))
            .unwrap()
    }

    async fn login_csrf(app: &axum::Router) -> (String, String) {
        use axum::body::to_bytes;
        use tower::ServiceExt;

        let page = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/admin/login")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(page.status(), axum::http::StatusCode::OK);
        let cookie = page
            .headers()
            .get(axum::http::header::SET_COOKIE)
            .expect("login csrf cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let body =
            String::from_utf8(to_bytes(page.into_body(), 32768).await.unwrap().to_vec()).unwrap();
        let marker = "name=\"login_csrf\" type=\"hidden\" value=\"";
        let start = body.find(marker).expect("login csrf hidden input") + marker.len();
        let end = body[start..].find('"').expect("login csrf value end");
        (body[start..start + end].to_string(), cookie)
    }

    async fn login_request_with_csrf(app: &axum::Router) -> axum::http::Request<axum::body::Body> {
        let (csrf, cookie) = login_csrf(app).await;
        axum::http::Request::builder()
            .method("POST")
            .uri("/admin/login")
            .header("content-type", "application/x-www-form-urlencoded")
            .header(axum::http::header::COOKIE, cookie)
            .body(login_request_body(&format!("&login_csrf={csrf}")))
            .unwrap()
    }

    #[tokio::test]
    async fn admin_login_rotates_existing_sessions_for_user() {
        use crate::admin::session;
        use tower::ServiceExt;

        let (app, state, user) = admin_login_test_app(true).await;
        let old_session = session::create_session(&state, &user.id).await.unwrap();

        let resp = app
            .clone()
            .oneshot(login_request_with_csrf(&app).await)
            .await
            .unwrap();

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
    async fn setup_post_requires_csrf_cookie_even_for_same_origin_requests() {
        use crate::db::pool;
        use crate::middleware::real_ip::ClientIp;
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
                public_host: Some("admin.example.test".into()),
            }))
            .layer(Extension(ClientIp("127.0.0.1".parse().unwrap())));

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/setup")
                    .header("host", "admin.example.test")
                    .header("origin", "http://admin.example.test")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "username=admin&password=Passw0rdStrong&confirm=Passw0rdStrong",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn inactive_admin_login_uses_generic_error() {
        use axum::body::to_bytes;
        use tower::ServiceExt;

        let (app, _state, _user) = admin_login_test_app(false).await;

        let resp = app
            .clone()
            .oneshot(login_request_with_csrf(&app).await)
            .await
            .unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
        let body =
            String::from_utf8(to_bytes(resp.into_body(), 16384).await.unwrap().to_vec()).unwrap();
        assert!(body.contains("Invalid credentials"));
        assert!(!body.contains("Account disabled"));
    }

    #[tokio::test]
    async fn login_post_without_csrf_token_does_not_consume_login_limiter() {
        use tower::ServiceExt;

        let (app, _state, _user) = admin_login_test_app(true).await;

        let missing_csrf = app.clone().oneshot(login_request()).await.unwrap();
        assert_eq!(missing_csrf.status(), axum::http::StatusCode::FORBIDDEN);

        let valid_login = app
            .clone()
            .oneshot(login_request_with_csrf(&app).await)
            .await
            .unwrap();
        assert_eq!(
            valid_login.status(),
            axum::http::StatusCode::SEE_OTHER,
            "a CSRF-rejected login attempt must not spend the login rate-limit budget"
        );
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
        let user = state
            .users
            .create(NewUser {
                username: "admin".into(),
                password_hash: "h".into(),
                is_admin: true,
            })
            .await
            .unwrap();
        let session_id = session::create_session(&state, &user.id).await.unwrap();
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
                    .header("cookie", format!("{}={}", session::COOKIE_NAME, session_id))
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
    client_ip: IpAddr,
) -> Result<(), ApiError> {
    let details = path.map(|path| serde_json::json!({ "path": path }).to_string());
    let client_ip = client_ip.to_string();
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
            client_ip: Some(&client_ip),
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

fn parse_glob_lines(value: &str) -> Result<Vec<String>, ApiError> {
    let globs: Vec<String> = value
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    for glob in &globs {
        globset::Glob::new(glob).map_err(|e| {
            ApiError::bad_request("invalid_glob", format!("invalid glob pattern: {}", e))
        })?;
    }
    Ok(globs)
}

async fn invites_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
) -> Result<Response, ApiError> {
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
    let used_invites = state.invites.count_used().await?;
    let pending_invites = invites.len();
    Ok(render_html(InvitesTemplate {
        t: admin_text(&headers, &cookies),
        invites,
        pending_invites,
        used_invites,
    }))
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
) -> Result<Response, ApiError> {
    let cfg = state.runtime_cfg.snapshot().await;
    Ok(render_html(SettingsTemplate {
        t: admin_text(&headers, &cookies),
        max_file_size_display: crate::human::format_bytes(cfg.max_file_size),
        text_extensions_display: cfg.text_extensions.join(", "),
        extra_exclude_globs_display: cfg.extra_exclude_globs.join("\n"),
        cfg,
        git_available: state.git_available,
    }))
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
    enable_metrics: Option<String>,
    enable_auto_merge: Option<String>,
    update_check_enabled: Option<String>,
    update_check_interval_seconds: u64,
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
                a.client_ip, a.user_agent, a.details
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
                details,
            )| {
                let (detail_vault_id, detail_vault_name) =
                    activity_detail_vault(details.as_deref());
                ActivityView {
                    timestamp: fmt_ts(timestamp, &timezone),
                    username,
                    action,
                    vault_id: vault_id.or(detail_vault_id),
                    vault_name: vault_name.or(detail_vault_name),
                    device_name,
                    client_ip: client_ip.map(|ip| mask_client_ip(&ip)),
                    user_agent,
                }
            },
        )
        .collect())
}

fn activity_detail_vault(details: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(details) = details else {
        return (None, None);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(details) else {
        return (None, None);
    };
    let vault_id = value
        .get("vault_id")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let vault_name = value
        .get("vault_name")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    (vault_id, vault_name)
}

fn mask_client_ip(value: &str) -> String {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(addr)) => {
            let octets = addr.octets();
            format!("{}.{}.{}.*", octets[0], octets[1], octets[2])
        }
        Ok(IpAddr::V6(addr)) => {
            let segments = addr.segments();
            format!(
                "{:x}:{:x}:{:x}:*:*:*:*:*",
                segments[0], segments[1], segments[2]
            )
        }
        Err(_) => "redacted".to_string(),
    }
}

async fn list_activity_filter_users(state: &AppState) -> Result<Vec<ActivityFilterUser>, ApiError> {
    Ok(state
        .users
        .list_options()
        .await?
        .into_iter()
        .map(|user| ActivityFilterUser {
            id: user.id,
            username: user.username,
        })
        .collect())
}

async fn list_user_options(state: &AppState) -> Result<Vec<UserOptionView>, ApiError> {
    Ok(state
        .users
        .list_options()
        .await?
        .into_iter()
        .map(user_option_view)
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
    let extra_exclude_globs: Vec<String> = form
        .extra_exclude_globs
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    crate::service::exclude::EffectiveExcludes::compile(&extra_exclude_globs).map_err(|e| {
        ApiError::bad_request("invalid_glob", format!("invalid glob pattern: {}", e))
    })?;
    const UPDATE_CHECK_INTERVAL_MIN: u64 = 60;
    const UPDATE_CHECK_INTERVAL_MAX: u64 = 30 * 24 * 60 * 60;
    if !(UPDATE_CHECK_INTERVAL_MIN..=UPDATE_CHECK_INTERVAL_MAX)
        .contains(&form.update_check_interval_seconds)
    {
        return Err(ApiError::bad_request(
            "update_check_interval_out_of_range",
            format!(
                "update_check_interval_seconds must be between {} and {} seconds",
                UPDATE_CHECK_INTERVAL_MIN, UPDATE_CHECK_INTERVAL_MAX
            ),
        ));
    }
    // Inline payload is shipped over SSE to every subscribed device; an
    // unbounded value lets one operator misconfiguration explode SSE frames
    // and starve subscribers. 64 KiB is the documented ceiling in Plan J.
    const INLINE_CONTENT_MAX_CAP: u32 = 64 * 1024;
    if form.inline_content_max_bytes > INLINE_CONTENT_MAX_CAP {
        return Err(ApiError::bad_request(
            "inline_content_max_bytes_too_large",
            format!(
                "inline_content_max_bytes must be ≤ {} bytes",
                INLINE_CONTENT_MAX_CAP
            ),
        ));
    }
    state
        .runtime_cfg_repo
        .set_admin_settings(
            RuntimeConfigSettingsUpdate {
                server_name: server_name.to_string(),
                timezone,
                registration_mode: mode,
                login_failure_threshold: form.login_failure_threshold,
                login_window_seconds: form.login_window_seconds,
                login_lock_seconds: form.login_lock_seconds,
                enable_history_ui: form.enable_history_ui.is_some(),
                enable_diff_endpoint: form.enable_diff_endpoint.is_some(),
                extra_exclude_globs,
                sse_heartbeat_seconds: form.sse_heartbeat_seconds,
                push_debounce_ms: form.push_debounce_ms,
                enable_git_smart_http: form.enable_git_smart_http.is_some(),
                enable_metrics: form.enable_metrics.is_some(),
                enable_auto_merge: form.enable_auto_merge.is_some(),
                update_check_enabled: form.update_check_enabled.is_some(),
                update_check_interval_seconds: form.update_check_interval_seconds,
                inline_content_max_bytes: form.inline_content_max_bytes,
            },
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
    state.notify_update_check_runtime_changed();
    Ok(Redirect::to("/admin/settings"))
}

async fn activity_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    _session: AdminSession,
    Query(filters): Query<ActivityFilters>,
) -> Result<Response, ApiError> {
    let selected_user_id = filters.user_id.clone().unwrap_or_default();
    let selected_action = filters.action.clone().unwrap_or_default();
    Ok(render_html(ActivityTemplate {
        t: admin_text(&headers, &cookies),
        activities: list_admin_activities(&state, ADMIN_ACTIVITY_LIMIT, &filters).await?,
        users: list_activity_filter_users(&state).await?,
        selected_user_id,
        selected_action,
    }))
}

async fn run_gc_form(
    State(state): State<AppState>,
    _session: AdminSession,
) -> Result<Redirect, ApiError> {
    let _ = crate::service::gc::run_blob_gc(&state).await?;
    Ok(Redirect::to("/admin"))
}
