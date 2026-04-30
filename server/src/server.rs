use crate::auth::{password, LoginRateLimiter};
use crate::config::Config;
use crate::db::pool;
use crate::db::repos::{NewUser, UserRepo};
use crate::middleware::{deployment_key, real_ip, request_id, ua_filter};
use crate::service::AppState;
use crate::{admin, api};
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::Router;
use once_cell::sync::Lazy;
use rand::RngCore;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

static START: Lazy<Instant> = Lazy::new(Instant::now);

/// Initialize the global start time. Idempotent.
pub fn mark_start() {
    Lazy::force(&START);
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

/// Construct the fully-stacked axum Router for production use.
pub fn build_app(state: AppState, cfg: &Config, limiter: LoginRateLimiter) -> Router {
    let trusted = real_ip::TrustedProxies::from_vec(cfg.network.trusted_proxies.clone());
    let dep_key = deployment_key::DeploymentKey::new(cfg.server.deployment_key.clone());

    let api_routes = api::router()
        .layer(axum::middleware::from_fn_with_state(
            dep_key,
            deployment_key::middleware,
        ))
        .layer(axum::middleware::from_fn(ua_filter::middleware));
    let admin_cookie_policy = admin::handlers::AdminCookiePolicy {
        secure: cfg.server.public_host.is_some(),
    };
    let admin_routes = admin::handlers::router()
        .layer(tower_cookies::CookieManagerLayer::new())
        .layer(axum::extract::Extension(admin_cookie_policy));

    Router::new()
        .merge(api_routes)
        .merge(admin_routes)
        .with_state(state)
        .layer(axum::middleware::from_fn(access_log_middleware))
        .layer(axum::extract::Extension(limiter))
        .layer(axum::middleware::from_fn_with_state(
            trusted,
            real_ip::middleware,
        ))
        .layer(axum::middleware::from_fn(request_id::middleware))
}

async fn access_log_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
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
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;

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

/// If no admin exists, create one with a random password and print it once.
pub async fn bootstrap_admin_if_needed(state: &AppState) -> crate::Result<()> {
    if state.users.count_admins().await? > 0 {
        return Ok(());
    }

    let mut buf = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let password_plaintext: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    let password_hash =
        password::hash(&password_plaintext).map_err(|e| crate::Error::Internal(e.to_string()))?;
    state
        .users
        .create(NewUser {
            username: "admin".into(),
            password_hash,
            is_admin: true,
        })
        .await?;

    eprintln!();
    eprintln!("============================================================");
    eprintln!(" FIRST-RUN ADMIN CREATED");
    eprintln!(" username: admin");
    eprintln!(" password: {password_plaintext}");
    eprintln!();
    eprintln!(" Save this now. It will not be displayed again.");
    eprintln!(" Change it with: pkvsyncd user passwd admin");
    eprintln!("============================================================");
    eprintln!();
    Ok(())
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
    let state = AppState::new(pool, cfg.storage.data_dir.clone(), default_name).await?;
    bootstrap_admin_if_needed(&state).await?;

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
                "periodic cleanup completed"
            );
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
    Ok(())
}

/// Run the server until shutdown.
pub async fn run(cfg: Arc<Config>) -> crate::Result<()> {
    mark_start();
    let (state, limiter) = prepare_state_and_limiter(&cfg).await?;

    let url = format_share_url(
        cfg.server.public_host.as_deref(),
        &cfg.server.bind_addr,
        &cfg.server.deployment_key,
    );
    tracing::info!(
        bind = %cfg.server.bind_addr,
        share_url = %url,
        "PKV Sync server starting"
    );
    eprintln!();
    eprintln!("PKV Sync server started.");
    eprintln!("Public URL (share this with users):");
    eprintln!("  {url}");
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

    #[test]
    fn uptime_is_non_negative() {
        mark_start();
        let _ = uptime_seconds();
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
}

#[cfg(test)]
mod bootstrap_tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::UserRepo;
    use crate::service::AppState;

    #[tokio::test]
    async fn creates_admin_when_none() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into())
            .await
            .unwrap();
        bootstrap_admin_if_needed(&state).await.unwrap();
        assert_eq!(state.users.count_admins().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn noop_when_admin_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into())
            .await
            .unwrap();
        bootstrap_admin_if_needed(&state).await.unwrap();
        bootstrap_admin_if_needed(&state).await.unwrap();
        assert_eq!(state.users.count_admins().await.unwrap(), 1);
    }
}
