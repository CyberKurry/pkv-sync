use crate::auth::LoginRateLimiter;
use crate::config::Config;
use crate::db::pool;
use crate::db::repos::{RuntimeConfigRepo, SqliteRuntimeConfigRepo};
use crate::middleware::{deployment_key, rate_limit, real_ip, request_id, ua_filter};
use crate::service::AppState;
use crate::{admin, api, mcp};
use axum::extract::{MatchedPath, Request, State};
use axum::http::{HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum::Router;
use std::net::SocketAddr;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

static START: LazyLock<Instant> = LazyLock::new(Instant::now);

const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; base-uri 'self'; frame-ancestors 'none'; object-src 'none'; form-action 'self'; img-src 'self' data:; style-src 'self'";

#[derive(Clone)]
struct SecurityHeadersConfig {
    hsts: bool,
}

/// Initialize the global start time. Idempotent.
pub fn mark_start() {
    LazyLock::force(&START);
}

/// Seconds since `mark_start()` was first called.
pub fn uptime_seconds() -> u64 {
    START.elapsed().as_secs()
}

/// Format the share URL admins distribute to users.
///
/// Returns `https://<host>/<key>/` for HTTPS-deployed servers.
/// For raw HTTP (dev/local), returns `http://<bind>/<key>/`.
pub fn format_share_url(
    public_host: Option<&str>,
    bind: &SocketAddr,
    deployment_key: &str,
) -> String {
    if let Some(host) = public_host {
        format!("https://{host}/{deployment_key}/")
    } else {
        format!("http://{bind}/{deployment_key}/")
    }
}

fn format_public_origin(public_host: Option<&str>, bind: &SocketAddr) -> String {
    if let Some(host) = public_host {
        format!("https://{host}")
    } else {
        format!("http://{bind}")
    }
}

/// Construct the fully-stacked axum Router for production use.
pub fn build_app(state: AppState, cfg: &Config, limiter: LoginRateLimiter) -> Router {
    let trusted = real_ip::TrustedProxies::from_vec(cfg.network.trusted_proxies.clone());
    let dep_key = deployment_key::DeploymentKey::new(cfg.server.deployment_key.clone());
    let metrics_state = state.clone();
    let security_headers = SecurityHeadersConfig {
        hsts: cfg.server.public_host.is_some(),
    };

    let api_routes = api::router()
        .layer(axum::extract::Extension(
            api::plugin_manifest::PluginAssetOrigin::from_public_host(
                cfg.server.public_host.clone(),
            ),
        ))
        .layer(axum::middleware::from_fn_with_state(
            dep_key,
            deployment_key::middleware,
        ))
        .layer(axum::middleware::from_fn(ua_filter::middleware))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            setup_gate,
        ));
    let admin_cookie_policy = admin::handlers::AdminCookiePolicy {
        secure: cfg.server.public_host.is_some(),
        public_host: cfg.server.public_host.clone(),
    };
    let admin_routes = admin::handlers::router()
        .layer(tower_cookies::CookieManagerLayer::new())
        .layer(axum::extract::Extension(admin_cookie_policy))
        .layer(axum::middleware::from_fn_with_state(
            rate_limit::RequestRateLimiter::admin_web(),
            rate_limit::admin_web_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            admin::handlers::setup_redirect_middleware,
        ));

    let mut app = Router::new()
        .merge(api_routes)
        .merge(admin_routes)
        .with_state(state.clone());
    if cfg.mcp.embed_in_serve {
        app = app.merge(mcp::transport_http::router(
            state.clone(),
            cfg.server.deployment_key.clone(),
        ));
    }

    app.layer(axum::middleware::from_fn_with_state(
        security_headers,
        security_headers_middleware,
    ))
    .layer(axum::middleware::from_fn_with_state(
        metrics_state,
        access_log_middleware,
    ))
    .layer(axum::extract::Extension(limiter))
    .layer(axum::middleware::from_fn_with_state(
        trusted,
        real_ip::middleware,
    ))
    .layer(axum::middleware::from_fn(request_id::middleware))
}

async fn access_log_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());
    let request_id = req
        .headers()
        .get(request_id::HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let client_ip = req
        .extensions()
        .get::<real_ip::ClientIp>()
        .map(|ip| ip.0.to_string());
    let started = Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16();
    let latency = started.elapsed().as_secs_f64();
    let latency_ms = latency * 1000.0;
    let code = status.to_string();
    let method_for_metrics = method.as_str().to_string();
    state
        .metrics
        .http_requests_total
        .with_label_values(&[&route, &method_for_metrics, &code])
        .inc();
    state
        .metrics
        .http_request_duration_seconds
        .with_label_values(&[&route, &method_for_metrics])
        .observe(latency);

    if status >= 500 {
        tracing::error!(
            method = %method,
            path = %path,
            status,
            latency_ms,
            request_id = request_id.as_deref(),
            client_ip = client_ip.as_deref(),
            "request completed"
        );
    } else if status >= 400 {
        tracing::warn!(
            method = %method,
            path = %path,
            status,
            latency_ms,
            request_id = request_id.as_deref(),
            client_ip = client_ip.as_deref(),
            "request completed"
        );
    } else {
        tracing::info!(
            method = %method,
            path = %path,
            status,
            latency_ms,
            request_id = request_id.as_deref(),
            client_ip = client_ip.as_deref(),
            "request completed"
        );
    }

    response
}

async fn setup_gate(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if req.uri().path().starts_with("/api/")
        && req.method() != Method::OPTIONS
        && state.is_setup_pending().await
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "code": "setup_required",
                    "message": "PKV server requires first-run setup; see /setup"
                }
            })),
        )
            .into_response();
    }
    next.run(req).await
}

async fn security_headers_middleware(
    State(config): State<SecurityHeadersConfig>,
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    // `same-origin`, not `no-referrer`: under `no-referrer`, browsers serialize
    // the `Origin` header of same-origin POSTs as the literal string `null`
    // (Fetch spec, "determine request's origin"). That breaks the admin/setup
    // CSRF check, which requires Origin == public_host. `same-origin` keeps
    // referrer info from leaking cross-origin while preserving Origin/Referer
    // on the admin UI's own form submissions.
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    if config.hsts {
        headers.insert(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    response
}

pub async fn log_setup_state(state: &AppState, public_host: Option<&str>) {
    if !state.is_setup_pending().await {
        return;
    }
    let url = public_host
        .map(|host| format!("https://{host}/setup"))
        .unwrap_or_else(|| "<your-server-url>/setup".into());
    eprintln!();
    eprintln!("============================================================");
    eprintln!(" PKV SYNC FIRST-RUN SETUP REQUIRED");
    eprintln!(" Open this URL in a browser to create the admin account:");
    eprintln!("   {url}");
    eprintln!("============================================================");
    eprintln!();
}

async fn prepare_state_and_limiter(cfg: &Config) -> crate::Result<(AppState, LoginRateLimiter)> {
    std::fs::create_dir_all(&cfg.storage.data_dir)
        .map_err(|e| crate::Error::Io(cfg.storage.data_dir.clone(), e))?;

    let pool = pool::connect(&cfg.storage.db_path).await?;
    pool::migrate_up(&pool).await?;
    let default_name = cfg
        .server
        .public_host
        .clone()
        .unwrap_or_else(|| "PKV Sync".into());

    let git_available = std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !git_available {
        tracing::warn!(
            "`git` binary not found in PATH; smart HTTP endpoints will be disabled regardless of runtime config"
        );
    }

    SqliteRuntimeConfigRepo::new(pool.clone())
        .seed_update_check_from_static_config(
            cfg.update_check.enabled,
            cfg.update_check.interval_seconds,
        )
        .await?;

    let state = AppState::new(
        pool,
        cfg.storage.data_dir.clone(),
        default_name,
        git_available,
    )
    .await?;
    log_setup_state(&state, cfg.server.public_host.as_deref()).await;

    let runtime_cfg = state.runtime_cfg.snapshot().await;
    let limiter = LoginRateLimiter::new(
        runtime_cfg.login_failure_threshold,
        Duration::from_secs(runtime_cfg.login_window_seconds),
        Duration::from_secs(runtime_cfg.login_lock_seconds),
    );
    Ok((state, limiter))
}

/// Run the server on an already-bound listener.
pub async fn run_with_listener(
    cfg: Arc<Config>,
    listener: tokio::net::TcpListener,
) -> crate::Result<()> {
    mark_start();
    let (state, limiter) = prepare_state_and_limiter(&cfg).await?;
    run_with_listener_and_state(cfg, listener, state, limiter).await
}

pub async fn run_with_listener_and_state(
    cfg: Arc<Config>,
    listener: tokio::net::TcpListener,
    state: AppState,
    limiter: LoginRateLimiter,
) -> crate::Result<()> {
    crate::service::update_check::spawn_update_check(state.clone(), cfg.update_check.clone());
    let cleanup_state = state.clone();
    let cleanup_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(6 * 60 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            interval.tick().await;
            let report = crate::service::cleanup::run_scheduled_cleanup(&cleanup_state).await;
            tracing::info!(
                sessions = report.sessions_deleted,
                tokens = report.tokens_deleted,
                activity = report.activity_deleted,
                idempotency = report.idempotency_deleted,
                blobs = report.blobs_deleted,
                git_gc = report.git_gc_pruned,
                git_gc_failed = report.git_gc_failed,
                "periodic cleanup completed"
            );
        }
    });
    let cleanup_limiter = limiter.clone();
    let cleanup_limiter_state = state.clone();
    let limiter_cleanup_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            interval.tick().await;
            let (login_removed, auth_failure_removed, mcp_auth_removed, mcp_write_removed) =
                prune_stale_limiters_blocking(
                    cleanup_limiter.clone(),
                    cleanup_limiter_state.clone(),
                )
                .await;
            if login_removed > 0 {
                tracing::debug!(
                    removed = login_removed,
                    "pruned stale login limiter entries"
                );
            }
            if auth_failure_removed > 0 {
                tracing::debug!(
                    removed = auth_failure_removed,
                    "pruned stale API auth failure limiter entries"
                );
            }
            if mcp_auth_removed > 0 {
                tracing::debug!(
                    removed = mcp_auth_removed,
                    "pruned stale MCP auth limiter entries"
                );
            }
            if mcp_write_removed > 0 {
                tracing::debug!(
                    removed = mcp_write_removed,
                    "pruned stale MCP write limiter entries"
                );
            }
        }
    });

    let app = build_app(state, &cfg, limiter);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|e| crate::Error::Internal(format!("server error: {e}")))?;
    cleanup_handle.abort();
    limiter_cleanup_handle.abort();
    Ok(())
}

fn prune_stale_limiters(
    limiter: &LoginRateLimiter,
    state: &AppState,
) -> (usize, usize, usize, usize) {
    (
        limiter.prune_stale(),
        state.auth_failure_limiter.prune_stale(),
        state.mcp_auth_limiter.prune_stale(),
        state.mcp_write_limiter.prune_stale(),
    )
}

async fn prune_stale_limiters_blocking(
    limiter: LoginRateLimiter,
    state: AppState,
) -> (usize, usize, usize, usize) {
    tokio::task::spawn_blocking(move || prune_stale_limiters(&limiter, &state))
        .await
        .unwrap_or_else(|err| {
            tracing::warn!(error = %err, "failed to prune stale limiter entries");
            (0, 0, 0, 0)
        })
}

/// Run the server until shutdown.
pub async fn run(cfg: Arc<Config>) -> crate::Result<()> {
    mark_start();
    let (state, limiter) = prepare_state_and_limiter(&cfg).await?;

    let public_origin =
        format_public_origin(cfg.server.public_host.as_deref(), &cfg.server.bind_addr);
    tracing::info!(
        bind = %cfg.server.bind_addr,
        public_origin = %public_origin,
        "PKV Sync server starting"
    );
    eprintln!();
    eprintln!("PKV Sync server started.");
    eprintln!("Public origin:");
    eprintln!("  {public_origin}");
    eprintln!(
        "Deployment key is configured; copy it from the server config when onboarding users."
    );
    eprintln!();

    let listener = tokio::net::TcpListener::bind(cfg.server.bind_addr)
        .await
        .map_err(|e| crate::Error::Io(std::path::PathBuf::from("(bind)"), e))?;
    run_with_listener_and_state(cfg, listener, state, limiter).await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;

    #[test]
    fn uptime_is_non_negative() {
        mark_start();
        let _ = uptime_seconds();
    }

    #[tokio::test]
    async fn limiter_cleanup_prunes_stale_entries_off_runtime_thread() {
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        let limiter = LoginRateLimiter::new(1, Duration::from_millis(5), Duration::from_millis(5));
        limiter
            .try_acquire("127.0.0.1".parse().unwrap())
            .unwrap()
            .failure();
        state.auth_failure_limiter.update_config(
            1,
            Duration::from_millis(5),
            Duration::from_millis(5),
        );
        state
            .auth_failure_limiter
            .try_acquire("api-auth")
            .unwrap()
            .failure();
        state
            .mcp_write_limiter
            .update_config(1, Duration::from_millis(5));
        state
            .mcp_write_limiter
            .try_record("token", "vault")
            .unwrap();
        state
            .mcp_auth_limiter
            .update_config(1, Duration::from_millis(5), Duration::from_millis(5));
        state
            .mcp_auth_limiter
            .try_acquire("mcp-auth")
            .unwrap()
            .failure();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let (login_removed, auth_failure_removed, mcp_auth_removed, mcp_write_removed) =
            prune_stale_limiters_blocking(limiter, state).await;

        assert_eq!(login_removed, 1);
        assert_eq!(auth_failure_removed, 1);
        assert_eq!(mcp_auth_removed, 1);
        assert_eq!(mcp_write_removed, 1);
    }

    #[tokio::test]
    async fn prepare_state_seeds_update_check_runtime_config_from_static_config() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = Config {
            server: crate::config::ServerConfig {
                bind_addr: "127.0.0.1:0".parse().unwrap(),
                deployment_key: "k_test".into(),
                public_host: None,
            },
            storage: crate::config::StorageConfig {
                data_dir: tmp.path().join("data"),
                db_path: tmp.path().join("pkv.db"),
            },
            network: crate::config::NetworkConfig {
                trusted_proxies: vec![],
            },
            logging: crate::config::LoggingConfig::default(),
            update_check: crate::config::UpdateCheckConfig {
                enabled: false,
                interval_seconds: 7200,
                repo: "cyberkurry/pkv-sync".into(),
            },
            mcp: crate::config::McpConfig::default(),
        };

        let (state, _limiter) = prepare_state_and_limiter(&cfg).await.unwrap();
        let runtime = state.runtime_cfg.snapshot().await;
        assert!(!runtime.update_check_enabled);
        assert_eq!(runtime.update_check_interval_seconds, 7200);
    }
}

#[cfg(test)]
mod url_tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn formats_with_public_host() {
        let bind: SocketAddr = "127.0.0.1:6710".parse().unwrap();
        let s = format_share_url(Some("sync.example.com"), &bind, "k_abc");
        assert_eq!(s, "https://sync.example.com/k_abc/");
    }

    #[test]
    fn formats_without_public_host() {
        let bind: SocketAddr = "127.0.0.1:6710".parse().unwrap();
        let s = format_share_url(None, &bind, "k_xyz");
        assert_eq!(s, "http://127.0.0.1:6710/k_xyz/");
    }

    #[test]
    fn public_origin_for_logs_omits_deployment_key() {
        let bind: SocketAddr = "127.0.0.1:6710".parse().unwrap();

        let origin = format_public_origin(Some("sync.example.com"), &bind);

        assert_eq!(origin, "https://sync.example.com");
        assert!(!origin.contains("k_secret"));
    }
}

#[cfg(test)]
mod bootstrap_tests {
    use crate::db::pool;
    use crate::db::repos::UserRepo;
    use crate::service::AppState;

    #[tokio::test]
    async fn creates_admin_when_none() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        assert!(state.is_setup_pending().await);
        assert_eq!(state.users.count_admins().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn setup_completed_when_admin_exists_before_state_initializes() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let users = crate::db::repos::SqliteUserRepo::new(pool.clone());
        users
            .create(crate::db::repos::NewUser {
                username: "admin".into(),
                password_hash: crate::auth::password::hash("passw0rd!!").unwrap(),
                is_admin: true,
            })
            .await
            .unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        assert_eq!(state.users.count_admins().await.unwrap(), 1);
        assert!(!state.is_setup_pending().await);
    }
}
