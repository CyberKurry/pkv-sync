use crate::api::error::ApiError;
use crate::auth::{password, token};
use crate::db::repos::{InviteRepo, NewToken, NewUser, RegistrationMode, TokenRepo, UserRepo};
use crate::service::AppState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterReq {
    pub username: String,
    pub password: String,
    pub device_name: String,
    pub invite_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
    pub device_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordReq {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResp {
    pub token: String,
    pub user_id: String,
    pub username: String,
    pub is_admin: bool,
}

const USERNAME_MIN: usize = 3;
const USERNAME_MAX: usize = 32;

fn validate_username(u: &str) -> Result<(), ApiError> {
    if u.len() < USERNAME_MIN || u.len() > USERNAME_MAX {
        return Err(ApiError::bad_request(
            "invalid_username",
            format!("username must be {USERNAME_MIN}-{USERNAME_MAX} chars"),
        ));
    }
    if !u
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(ApiError::bad_request(
            "invalid_username",
            "username may contain letters, digits, _ - .",
        ));
    }
    Ok(())
}

fn is_unique_error(e: &sqlx::Error) -> bool {
    e.to_string().to_lowercase().contains("unique")
}

pub async fn verify_credentials(
    state: &AppState,
    username: &str,
    password: &str,
) -> Result<crate::db::repos::User, ApiError> {
    let user = state.users.find_by_username(username).await?;
    let user = match user {
        Some(u) => u,
        None => {
            let _ = crate::auth::password::hash("dummy-password");
            return Err(ApiError::unauthorized("invalid credentials"));
        }
    };
    let ok = crate::auth::password::verify(password, &user.password_hash)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !ok {
        return Err(ApiError::unauthorized("invalid credentials"));
    }
    if !user.is_active {
        return Err(ApiError::forbidden("account disabled"));
    }
    Ok(user)
}

pub async fn register(state: &AppState, req: RegisterReq) -> Result<AuthResp, ApiError> {
    validate_username(&req.username)?;
    let cfg = state.runtime_cfg.snapshot().await;
    match cfg.registration_mode {
        RegistrationMode::Disabled => {
            return Err(ApiError::forbidden("registration is disabled"));
        }
        RegistrationMode::InviteOnly => {
            let code = req
                .invite_code
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("invite_required", "invite code required"))?;
            let now = chrono::Utc::now().timestamp();
            let inv = state
                .invites
                .find(code)
                .await?
                .ok_or_else(|| ApiError::bad_request("invalid_invite", "invite not found"))?;
            if inv.used_at.is_some() {
                return Err(ApiError::bad_request(
                    "invalid_invite",
                    "invite already used",
                ));
            }
            if let Some(exp) = inv.expires_at {
                if exp <= now {
                    return Err(ApiError::bad_request("invalid_invite", "invite expired"));
                }
            }
        }
        RegistrationMode::Open => {}
    }
    if state.users.find_by_username(&req.username).await?.is_some() {
        return Err(ApiError::conflict(
            "username_taken",
            "username already exists",
        ));
    }
    let pwd_hash = password::hash(&req.password).map_err(|e| match e {
        password::PasswordError::TooShort { .. } => {
            ApiError::bad_request("weak_password", e.to_string())
        }
        _ => ApiError::internal(e.to_string()),
    })?;
    let user = state
        .users
        .create(NewUser {
            username: req.username.clone(),
            password_hash: pwd_hash,
            is_admin: false,
        })
        .await
        .map_err(|e| {
            if is_unique_error(&e) {
                ApiError::conflict("username_taken", "username already exists")
            } else {
                ApiError::from(e)
            }
        })?;
    if let Some(code) = &req.invite_code {
        let now = chrono::Utc::now().timestamp();
        let claimed = state.invites.mark_used(code, &user.id, now).await?;
        if !claimed {
            let _ = state.users.delete(&user.id).await;
            return Err(ApiError::bad_request(
                "invalid_invite",
                "invite not available",
            ));
        }
    }
    issue_token(
        state,
        &user.id,
        &user.username,
        user.is_admin,
        &req.device_name,
    )
    .await
}

pub async fn login(state: &AppState, req: LoginReq) -> Result<AuthResp, ApiError> {
    let user = verify_credentials(state, &req.username, &req.password).await?;
    state
        .users
        .touch_last_login(&user.id, chrono::Utc::now().timestamp())
        .await?;
    issue_token(
        state,
        &user.id,
        &user.username,
        user.is_admin,
        &req.device_name,
    )
    .await
}

pub async fn change_password(
    state: &AppState,
    user_id: &str,
    current_token_id: &str,
    req: ChangePasswordReq,
) -> Result<(), ApiError> {
    let user = state
        .users
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("user not found"))?;
    let ok = password::verify(&req.current_password, &user.password_hash)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !ok {
        return Err(ApiError::unauthorized("current password incorrect"));
    }
    let new_hash = password::hash(&req.new_password).map_err(|e| match e {
        password::PasswordError::TooShort { .. } => {
            ApiError::bad_request("weak_password", e.to_string())
        }
        _ => ApiError::internal(e.to_string()),
    })?;
    state.users.update_password(user_id, &new_hash).await?;
    state
        .tokens
        .revoke_all_for_user(
            user_id,
            chrono::Utc::now().timestamp(),
            Some(current_token_id),
        )
        .await?;
    Ok(())
}

async fn issue_token(
    state: &AppState,
    user_id: &str,
    username: &str,
    is_admin: bool,
    device_name: &str,
) -> Result<AuthResp, ApiError> {
    if device_name.is_empty() || device_name.len() > 64 {
        return Err(ApiError::bad_request(
            "invalid_device",
            "device_name length must be 1-64",
        ));
    }
    let raw = token::generate();
    let h = token::hash(&raw);
    state
        .tokens
        .create(NewToken {
            user_id,
            token_hash: &h,
            device_name,
        })
        .await?;
    Ok(AuthResp {
        token: raw,
        user_id: user_id.into(),
        username: username.into(),
        is_admin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{RegistrationMode, RuntimeConfigRepo, TokenRepo};

    async fn make_state(mode: RegistrationMode) -> AppState {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into())
            .await
            .unwrap();
        state
            .runtime_cfg_repo
            .set_registration_mode(mode, None)
            .await
            .unwrap();
        let cfg = state.runtime_cfg_repo.load().await.unwrap();
        state.runtime_cfg.replace(cfg).await;
        state
    }

    #[tokio::test]
    async fn register_disabled_rejects() {
        let s = make_state(RegistrationMode::Disabled).await;
        let r = register(
            &s,
            RegisterReq {
                username: "alice".into(),
                password: "passw0rd!!".into(),
                device_name: "x".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(r.status, axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn register_open_allows_new() {
        let s = make_state(RegistrationMode::Open).await;
        let resp = register(
            &s,
            RegisterReq {
                username: "alice".into(),
                password: "passw0rd!!".into(),
                device_name: "x".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(resp.username, "alice");
        assert!(resp.token.starts_with("pks_"));
    }

    #[tokio::test]
    async fn register_duplicate_conflicts() {
        let s = make_state(RegistrationMode::Open).await;
        let _ = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "passw0rd!!".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let err = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "passw0rd!!".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn register_weak_password_rejected() {
        let s = make_state(RegistrationMode::Open).await;
        let err = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "short".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "weak_password");
    }

    #[tokio::test]
    async fn register_invalid_username_rejected() {
        let s = make_state(RegistrationMode::Open).await;
        let err = register(
            &s,
            RegisterReq {
                username: "ab".into(),
                password: "passw0rd!!".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "invalid_username");
    }

    #[tokio::test]
    async fn login_correct_credentials() {
        let s = make_state(RegistrationMode::Open).await;
        let _ = register(
            &s,
            RegisterReq {
                username: "alice".into(),
                password: "secret123!".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let resp = login(
            &s,
            LoginReq {
                username: "alice".into(),
                password: "secret123!".into(),
                device_name: "d2".into(),
            },
        )
        .await
        .unwrap();
        assert!(resp.token.starts_with("pks_"));
    }

    #[tokio::test]
    async fn login_wrong_password() {
        let s = make_state(RegistrationMode::Open).await;
        let _ = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "passw0rd!!".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let err = login(
            &s,
            LoginReq {
                username: "userx".into(),
                password: "wrong".into(),
                device_name: "d".into(),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn login_unknown_user() {
        let s = make_state(RegistrationMode::Open).await;
        let err = login(
            &s,
            LoginReq {
                username: "ghost".into(),
                password: "any".into(),
                device_name: "d".into(),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn change_password_revokes_other_tokens() {
        let s = make_state(RegistrationMode::Open).await;
        let r1 = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "passw0rd!!".into(),
                device_name: "d1".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let _r2 = login(
            &s,
            LoginReq {
                username: "userx".into(),
                password: "passw0rd!!".into(),
                device_name: "d2".into(),
            },
        )
        .await
        .unwrap();
        let toks = s.tokens.list_for_user(&r1.user_id).await.unwrap();
        let r1_id = toks
            .iter()
            .find(|t| t.device_name == "d1")
            .unwrap()
            .id
            .clone();
        change_password(
            &s,
            &r1.user_id,
            &r1_id,
            ChangePasswordReq {
                current_password: "passw0rd!!".into(),
                new_password: "newpass1234".into(),
            },
        )
        .await
        .unwrap();
        let live: Vec<_> = s
            .tokens
            .list_for_user(&r1.user_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|t| t.revoked_at.is_none())
            .collect();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, r1_id);
    }

    #[tokio::test]
    async fn verify_credentials_returns_user_on_valid_login() {
        let s = make_state(RegistrationMode::Open).await;
        let _ = register(
            &s,
            RegisterReq {
                username: "alice".into(),
                password: "secret123!".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let user = verify_credentials(&s, "alice", "secret123!").await.unwrap();
        assert_eq!(user.username, "alice");
    }

    #[tokio::test]
    async fn verify_credentials_rejects_wrong_password() {
        let s = make_state(RegistrationMode::Open).await;
        let _ = register(
            &s,
            RegisterReq {
                username: "alice".into(),
                password: "secret123!".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let err = verify_credentials(&s, "alice", "wrong").await.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn verify_credentials_rejects_unknown_user() {
        let s = make_state(RegistrationMode::Open).await;
        let err = verify_credentials(&s, "ghost", "any").await.unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::UNAUTHORIZED);
    }
}
