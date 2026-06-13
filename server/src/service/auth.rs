use crate::api::error::ApiError;
use crate::auth::{password, token};
use crate::db::repos::{InviteRepo, NewToken, NewUser, RegistrationMode, User, UserRepo};
use crate::service::AppState;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Deserialize)]
pub struct RegisterReq {
    pub username: String,
    pub password: String,
    pub device_id: String,
    pub device_name: String,
    pub invite_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
    pub device_id: String,
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
static DUMMY_PASSWORD_HASH: LazyLock<String> =
    LazyLock::new(|| password::hash("dummy-password").expect("dummy password is valid"));

pub fn validate_username(u: &str) -> Result<(), ApiError> {
    let len = u.chars().count();
    if !(USERNAME_MIN..=USERNAME_MAX).contains(&len) {
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
    matches!(e, sqlx::Error::Database(db) if db.is_unique_violation())
}

fn validate_auth_password(plaintext: &str) -> Result<(), ApiError> {
    password::validate_strong(plaintext).map_err(|e| match e {
        password::PasswordError::TooLong { .. } | password::PasswordError::TooWeak => {
            ApiError::bad_request("weak_password", e.to_string())
        }
        _ => ApiError::internal(e.to_string()),
    })
}

fn hash_auth_password(plaintext: &str) -> Result<String, ApiError> {
    validate_auth_password(plaintext)?;
    password::hash(plaintext).map_err(|e| match e {
        password::PasswordError::TooShort { .. }
        | password::PasswordError::TooLong { .. }
        | password::PasswordError::TooWeak => ApiError::bad_request("weak_password", e.to_string()),
        _ => ApiError::internal(e.to_string()),
    })
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
            let _ = password::verify(password, &DUMMY_PASSWORD_HASH)
                .map_err(|e| ApiError::internal(e.to_string()))?;
            return Err(ApiError::unauthorized("invalid credentials"));
        }
    };
    let ok = crate::auth::password::verify(password, &user.password_hash)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !ok {
        return Err(ApiError::unauthorized("invalid credentials"));
    }
    if !user.is_active {
        // Return UNAUTHORIZED (not FORBIDDEN) so that disabled accounts cannot
        // be distinguished from wrong-password attempts via the HTTP status
        // code (account state enumeration), and so that the login handler's
        // reservation failure path consumes rate-limit budget for these
        // attempts too. The error message is also identical to a wrong
        // password to avoid any leak.
        return Err(ApiError::unauthorized("invalid credentials"));
    }
    Ok(user)
}

pub async fn register(state: &AppState, req: RegisterReq) -> Result<AuthResp, ApiError> {
    validate_username(&req.username)?;
    let (device_id, device_name) = validate_device_fields(&req.device_id, &req.device_name)?;
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
    let pwd_hash = hash_auth_password(&req.password)?;
    let user = if let Some(code) = &req.invite_code {
        create_user_claiming_invite(state, req.username.clone(), pwd_hash, code).await?
    } else {
        state
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
            })?
    };
    issue_token(
        state,
        &user.id,
        &user.username,
        user.is_admin,
        device_id,
        device_name,
    )
    .await
}

async fn create_user_claiming_invite(
    state: &AppState,
    username: String,
    password_hash: String,
    invite_code: &str,
) -> Result<User, ApiError> {
    let id = uuid::Uuid::new_v4().simple().to_string();
    let now = chrono::Utc::now().timestamp();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, is_admin, is_active, created_at)
         VALUES (?, ?, ?, 0, 1, ?)",
    )
    .bind(&id)
    .bind(&username)
    .bind(&password_hash)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique_error(&e) {
            ApiError::conflict("username_taken", "username already exists")
        } else {
            ApiError::from(e)
        }
    })?;

    let claimed = sqlx::query(
        "UPDATE invites SET used_at = ?, used_by = ?
         WHERE code = ? AND used_at IS NULL AND (expires_at IS NULL OR expires_at > ?)",
    )
    .bind(now)
    .bind(&id)
    .bind(invite_code)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if !claimed {
        return Err(ApiError::bad_request(
            "invalid_invite",
            "invite not available",
        ));
    }
    tx.commit().await?;
    Ok(User {
        id,
        username,
        password_hash,
        is_admin: false,
        is_active: true,
        created_at: now,
        last_login_at: None,
    })
}

pub async fn login(state: &AppState, req: LoginReq) -> Result<AuthResp, ApiError> {
    let (device_id, device_name) = validate_device_fields(&req.device_id, &req.device_name)?;
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
        device_id,
        device_name,
    )
    .await
}

pub async fn change_password(
    state: &AppState,
    user_id: &str,
    current_token_id: &str,
    req: ChangePasswordReq,
) -> Result<(), ApiError> {
    let user = match state.users.find_by_id(user_id).await? {
        Some(user) => user,
        None => {
            let _ = password::verify(&req.current_password, &DUMMY_PASSWORD_HASH)
                .map_err(|e| ApiError::internal(e.to_string()))?;
            return Err(ApiError::unauthorized("current password incorrect"));
        }
    };
    let ok = password::verify(&req.current_password, &user.password_hash)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !ok {
        return Err(ApiError::unauthorized("current password incorrect"));
    }
    let new_hash = hash_auth_password(&req.new_password)?;
    let now = chrono::Utc::now().timestamp();
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(&new_hash)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE tokens SET revoked_at = ?
         WHERE user_id = ? AND id != ? AND revoked_at IS NULL",
    )
    .bind(now)
    .bind(user_id)
    .bind(current_token_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn issue_token(
    state: &AppState,
    user_id: &str,
    username: &str,
    is_admin: bool,
    device_id: &str,
    device_name: &str,
) -> Result<AuthResp, ApiError> {
    let (device_id, device_name) = validate_device_fields(device_id, device_name)?;
    let raw = token::generate();
    let h = token::hash(&raw);
    state
        .tokens
        .create_replacing_device(NewToken {
            user_id,
            token_hash: &h,
            device_id,
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

fn validate_device_fields<'a>(
    device_id: &'a str,
    device_name: &'a str,
) -> Result<(&'a str, &'a str), ApiError> {
    let device_id = device_id.trim();
    let device_name = device_name.trim();
    if device_id.is_empty() || device_id.len() > 128 {
        return Err(ApiError::bad_request(
            "invalid_device",
            "device_id length must be 1-128",
        ));
    }
    if device_id.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "invalid_device",
            "device_id cannot contain control characters",
        ));
    }
    if device_name.is_empty() || device_name.len() > 64 {
        return Err(ApiError::bad_request(
            "invalid_device",
            "device_name length must be 1-64",
        ));
    }
    if device_name.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "invalid_device",
            "device_name cannot contain control characters",
        ));
    }
    Ok((device_id, device_name))
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
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
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
                device_id: "device-disabled".into(),
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
                password: "Passw0rdStrong".into(),
                device_id: "device-open".into(),
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
                password: "Passw0rdStrong".into(),
                device_id: "device-dupe-1".into(),
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
                password: "Passw0rdStrong".into(),
                device_id: "device-dupe-2".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn unique_error_detection_uses_database_error_kind() {
        let s = make_state(RegistrationMode::Open).await;
        let user = NewUser {
            username: "dupe".into(),
            password_hash: "hash".into(),
            is_admin: false,
        };
        s.users.create(user.clone()).await.unwrap();

        let err = s.users.create(user).await.unwrap_err();

        assert!(is_unique_error(&err));
    }

    #[tokio::test]
    async fn register_weak_password_rejected() {
        let s = make_state(RegistrationMode::Open).await;
        let err = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "short".into(),
                device_id: "device-weak".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "weak_password");
    }

    #[tokio::test]
    async fn register_rejects_password_without_setup_strength() {
        let s = make_state(RegistrationMode::Open).await;
        let err = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "passw0rd!!".into(),
                device_id: "device-weak-complexity".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "weak_password");
        assert!(s.users.find_by_username("userx").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn register_invalid_username_rejected() {
        let s = make_state(RegistrationMode::Open).await;
        let err = register(
            &s,
            RegisterReq {
                username: "ab".into(),
                password: "passw0rd!!".into(),
                device_id: "device-bad-user".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "invalid_username");
    }

    #[test]
    fn username_length_validation_counts_characters() {
        let source = include_str!("auth.rs");
        let fn_start = source
            .find("pub fn validate_username")
            .expect("validate_username implementation exists");
        let next_fn = source[fn_start..]
            .find("\nfn ")
            .map(|idx| fn_start + idx)
            .expect("next helper follows validate_username");
        let implementation = &source[fn_start..next_fn];

        assert!(implementation.contains(".chars().count()"));
    }

    #[tokio::test]
    async fn register_invalid_device_id_does_not_create_user() {
        let s = make_state(RegistrationMode::Open).await;
        let err = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "passw0rd!!".into(),
                device_id: "bad\ndevice".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "invalid_device");
        assert!(s.users.find_by_username("userx").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn register_invite_race_leaves_one_user_and_token() {
        let s = make_state(RegistrationMode::InviteOnly).await;
        let creator = s
            .users
            .create(NewUser {
                username: "admin".into(),
                password_hash: "hash".into(),
                is_admin: true,
            })
            .await
            .unwrap();
        let invite = s.invites.create(&creator.id, None).await.unwrap();

        let first = register(
            &s,
            RegisterReq {
                username: "racea".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-race-a".into(),
                device_name: "a".into(),
                invite_code: Some(invite.code.clone()),
            },
        );
        let second = register(
            &s,
            RegisterReq {
                username: "raceb".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-race-b".into(),
                device_name: "b".into(),
                invite_code: Some(invite.code.clone()),
            },
        );

        let (first, second) = tokio::join!(first, second);
        let successes = [first.as_ref().ok(), second.as_ref().ok()]
            .into_iter()
            .flatten()
            .count();
        let failures = [first.as_ref().err(), second.as_ref().err()]
            .into_iter()
            .flatten()
            .filter(|err| err.code == "invalid_invite")
            .count();

        assert_eq!(successes, 1);
        assert_eq!(failures, 1);
        let (registered_users,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE username IN ('racea', 'raceb')")
                .fetch_one(&s.pool)
                .await
                .unwrap();
        let (registered_tokens,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tokens
             WHERE device_id IN ('device-race-a', 'device-race-b')",
        )
        .fetch_one(&s.pool)
        .await
        .unwrap();
        assert_eq!(registered_users, 1);
        assert_eq!(registered_tokens, 1);
    }

    #[tokio::test]
    async fn register_invite_race_does_not_leave_user_when_delete_fails() {
        let s = make_state(RegistrationMode::InviteOnly).await;
        let creator = s
            .users
            .create(NewUser {
                username: "admin".into(),
                password_hash: "hash".into(),
                is_admin: true,
            })
            .await
            .unwrap();
        let winner = s
            .users
            .create(NewUser {
                username: "winner".into(),
                password_hash: "hash".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        s.invites.create(&creator.id, None).await.unwrap();
        sqlx::query(
            "CREATE TRIGGER mark_invite_used_after_loser_insert
             AFTER INSERT ON users
             WHEN NEW.username = 'race_loser'
             BEGIN
               UPDATE invites
               SET used_at = 123, used_by = (SELECT id FROM users WHERE username = 'winner')
               WHERE used_at IS NULL;
             END",
        )
        .execute(&s.pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER block_loser_delete
             BEFORE DELETE ON users
             WHEN OLD.username = 'race_loser'
             BEGIN
               SELECT RAISE(FAIL, 'delete blocked');
             END",
        )
        .execute(&s.pool)
        .await
        .unwrap();
        let invite = s
            .invites
            .list_active(chrono::Utc::now().timestamp())
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        let err = register(
            &s,
            RegisterReq {
                username: "race_loser".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-race-loser".into(),
                device_name: "loser".into(),
                invite_code: Some(invite.code),
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "invalid_invite");
        assert_eq!(winner.username, "winner");
        let (registered_users,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE username = 'race_loser'")
                .fetch_one(&s.pool)
                .await
                .unwrap();
        assert_eq!(registered_users, 0);
    }

    #[tokio::test]
    async fn login_correct_credentials() {
        let s = make_state(RegistrationMode::Open).await;
        let _ = register(
            &s,
            RegisterReq {
                username: "alice".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-a".into(),
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
                password: "Passw0rdStrong".into(),
                device_id: "device-b".into(),
                device_name: "d2".into(),
            },
        )
        .await
        .unwrap();
        assert!(resp.token.starts_with("pks_"));
    }

    #[tokio::test]
    async fn login_revokes_previous_token_for_same_device_id() {
        let s = make_state(RegistrationMode::Open).await;
        let first = register(
            &s,
            RegisterReq {
                username: "alice".into(),
                password: "Passw0rdStrong".into(),
                device_id: "stable-device".into(),
                device_name: "Laptop".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let _second = login(
            &s,
            LoginReq {
                username: "alice".into(),
                password: "Passw0rdStrong".into(),
                device_id: "stable-device".into(),
                device_name: "Laptop renamed".into(),
            },
        )
        .await
        .unwrap();

        let rows = s.tokens.list_for_user(&first.user_id).await.unwrap();
        let live: Vec<_> = rows.iter().filter(|t| t.revoked_at.is_none()).collect();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].device_id, "stable-device");
        assert_eq!(live[0].device_name, "Laptop renamed");
        let revoked: Vec<_> = rows.iter().filter(|t| t.revoked_at.is_some()).collect();
        assert_eq!(revoked.len(), 1);
    }

    #[tokio::test]
    async fn login_wrong_password() {
        let s = make_state(RegistrationMode::Open).await;
        let _ = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-login-wrong-register".into(),
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
                device_id: "device-login-wrong".into(),
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
                device_id: "device-ghost".into(),
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
                password: "Passw0rdStrong".into(),
                device_id: "device-change-1".into(),
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
                password: "Passw0rdStrong".into(),
                device_id: "device-change-2".into(),
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
                current_password: "Passw0rdStrong".into(),
                new_password: "Newpass1234Strong".into(),
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
    async fn change_password_rejects_password_without_setup_strength() {
        let s = make_state(RegistrationMode::Open).await;
        let resp = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-change-weak".into(),
                device_name: "d1".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let token_id = s
            .tokens
            .list_for_user(&resp.user_id)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.device_id == "device-change-weak")
            .unwrap()
            .id;

        let err = change_password(
            &s,
            &resp.user_id,
            &token_id,
            ChangePasswordReq {
                current_password: "Passw0rdStrong".into(),
                new_password: "newpass1234".into(),
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.code, "weak_password");
        assert!(verify_credentials(&s, "userx", "Passw0rdStrong")
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn change_password_uses_same_error_for_missing_user_and_wrong_password() {
        let s = make_state(RegistrationMode::Open).await;
        let resp = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-change-enum".into(),
                device_name: "d1".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let token_id = s
            .tokens
            .list_for_user(&resp.user_id)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.device_id == "device-change-enum")
            .unwrap()
            .id;

        let wrong_password = change_password(
            &s,
            &resp.user_id,
            &token_id,
            ChangePasswordReq {
                current_password: "wrong-password".into(),
                new_password: "Newpass1234Strong".into(),
            },
        )
        .await
        .unwrap_err();
        let missing_user = change_password(
            &s,
            "missing-user",
            &token_id,
            ChangePasswordReq {
                current_password: "wrong-password".into(),
                new_password: "Newpass1234Strong".into(),
            },
        )
        .await
        .unwrap_err();

        assert_eq!(wrong_password.status, axum::http::StatusCode::UNAUTHORIZED);
        assert_eq!(missing_user.status, wrong_password.status);
        assert_eq!(missing_user.code, wrong_password.code);
        assert_eq!(missing_user.message, wrong_password.message);
    }

    #[tokio::test]
    async fn change_password_keeps_old_password_when_token_revoke_fails() {
        let s = make_state(RegistrationMode::Open).await;
        let resp = register(
            &s,
            RegisterReq {
                username: "userx".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-change-rollback-1".into(),
                device_name: "d1".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        login(
            &s,
            LoginReq {
                username: "userx".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-change-rollback-2".into(),
                device_name: "d2".into(),
            },
        )
        .await
        .unwrap();
        let token_id = s
            .tokens
            .list_for_user(&resp.user_id)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.device_id == "device-change-rollback-1")
            .unwrap()
            .id;
        sqlx::query(
            "CREATE TRIGGER fail_token_revoke
             BEFORE UPDATE OF revoked_at ON tokens
             WHEN NEW.revoked_at IS NOT NULL
             BEGIN
               SELECT RAISE(FAIL, 'token revoke blocked');
             END",
        )
        .execute(&s.pool)
        .await
        .unwrap();

        let err = change_password(
            &s,
            &resp.user_id,
            &token_id,
            ChangePasswordReq {
                current_password: "Passw0rdStrong".into(),
                new_password: "Newpass1234Strong".into(),
            },
        )
        .await
        .unwrap_err();

        assert_eq!(err.status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        assert!(verify_credentials(&s, "userx", "Passw0rdStrong")
            .await
            .is_ok());
        assert!(verify_credentials(&s, "userx", "Newpass1234Strong")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn verify_credentials_returns_user_on_valid_login() {
        let s = make_state(RegistrationMode::Open).await;
        let _ = register(
            &s,
            RegisterReq {
                username: "alice".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-verify".into(),
                device_name: "d".into(),
                invite_code: None,
            },
        )
        .await
        .unwrap();
        let user = verify_credentials(&s, "alice", "Passw0rdStrong")
            .await
            .unwrap();
        assert_eq!(user.username, "alice");
    }

    #[tokio::test]
    async fn verify_credentials_rejects_wrong_password() {
        let s = make_state(RegistrationMode::Open).await;
        let _ = register(
            &s,
            RegisterReq {
                username: "alice".into(),
                password: "Passw0rdStrong".into(),
                device_id: "device-verify-wrong".into(),
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

    #[test]
    fn unknown_user_dummy_hash_is_precomputed() {
        let source = include_str!("auth.rs");
        let fn_start = source
            .find("pub async fn verify_credentials")
            .expect("verify_credentials exists");
        let next_fn = source[fn_start + 1..]
            .find("\npub async fn")
            .map(|idx| fn_start + 1 + idx)
            .expect("next async function follows verify_credentials");
        let implementation = &source[fn_start..next_fn];

        assert!(source.contains("static DUMMY_PASSWORD_HASH"));
        assert!(!implementation.contains("password::hash(\"dummy-password\")"));
        assert!(implementation.contains("password::verify(password, &DUMMY_PASSWORD_HASH)"));
    }
}
