use crate::admin::i18n::AdminText;
use crate::db::repos::{Invite, RuntimeConfig, TokenRow, User};
use askama::Template;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate<'a> {
    pub t: AdminText,
    pub error: Option<&'a str>,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub t: AdminText,
    pub username: String,
    pub users: i64,
    pub vaults: i64,
    pub cpu_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_used_gb: u64,
    pub disk_total_gb: u64,
    pub uptime_seconds: u64,
}

#[derive(Template)]
#[template(path = "users.html")]
pub struct UsersTemplate {
    pub t: AdminText,
    pub users: Vec<User>,
    pub message: Option<String>,
}

#[derive(Template)]
#[template(path = "user_detail.html")]
pub struct UserDetailTemplate {
    pub t: AdminText,
    pub user: User,
    pub tokens: Vec<TokenRow>,
    pub message: Option<String>,
    pub created_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VaultAdminView {
    pub id: String,
    pub user_id: String,
    pub owner_username: String,
    pub name: String,
    pub created_at: i64,
    pub last_sync_at: Option<i64>,
    pub size_bytes: i64,
    pub file_count: i64,
}

#[derive(Template)]
#[template(path = "vaults.html")]
pub struct VaultsTemplate {
    pub t: AdminText,
    pub vaults: Vec<VaultAdminView>,
    pub users: Vec<User>,
    pub message: Option<String>,
}

#[derive(Template)]
#[template(path = "invites.html")]
pub struct InvitesTemplate {
    pub t: AdminText,
    pub invites: Vec<Invite>,
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub t: AdminText,
    pub cfg: RuntimeConfig,
}

#[derive(Debug, Clone)]
pub struct ActivityView {
    pub timestamp: i64,
    pub username: String,
    pub action: String,
    pub vault_id: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Template)]
#[template(path = "activity.html")]
pub struct ActivityTemplate {
    pub t: AdminText,
    pub activities: Vec<ActivityView>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::RegistrationMode;

    #[test]
    fn login_template_renders() {
        let html = LoginTemplate {
            t: AdminText::en(),
            error: Some("bad"),
        }
        .render()
        .unwrap();
        assert!(html.contains("PKV Sync Admin"));
        assert!(html.contains("bad"));
    }

    #[test]
    fn dashboard_template_renders() {
        let html = DashboardTemplate {
            t: AdminText::en(),
            username: "admin".into(),
            users: 1,
            vaults: 2,
            cpu_percent: 3.0,
            memory_used_mb: 10,
            memory_total_mb: 20,
            disk_used_gb: 1,
            disk_total_gb: 2,
            uptime_seconds: 5,
        }
        .render()
        .unwrap();
        assert!(html.contains("admin"));
        assert!(html.contains("Users"));
        assert!(html.contains("Run Blob GC"));
        assert!(!html.contains("unpkg.com"));
    }

    fn user(id: &str, username: &str, is_admin: bool) -> User {
        User {
            id: id.into(),
            username: username.into(),
            password_hash: "h".into(),
            is_admin,
            is_active: true,
            created_at: 1,
            last_login_at: None,
        }
    }

    #[test]
    fn users_template_renders() {
        let html = UsersTemplate {
            t: AdminText::en(),
            users: vec![user("u1", "admin", true)],
            message: Some("created".into()),
        }
        .render()
        .unwrap();
        assert!(html.contains("admin"));
        assert!(html.contains("Create user"));
    }

    #[test]
    fn user_detail_template_renders() {
        let html = UserDetailTemplate {
            t: AdminText::en(),
            user: user("u1", "admin", true),
            tokens: vec![TokenRow {
                id: "t1".into(),
                user_id: "u1".into(),
                device_name: "desktop".into(),
                created_at: 1,
                last_used_at: Some(2),
                revoked_at: None,
            }],
            message: None,
            created_token: None,
        }
        .render()
        .unwrap();
        assert!(html.contains("desktop"));
        assert!(html.contains("Reset password"));
    }

    #[test]
    fn user_detail_template_renders_created_token_once() {
        let html = UserDetailTemplate {
            t: AdminText::en(),
            user: user("u1", "admin", true),
            tokens: Vec::new(),
            message: None,
            created_token: Some("pks_abc".into()),
        }
        .render()
        .unwrap();
        assert!(html.contains("pks_abc"));
    }

    #[test]
    fn vaults_template_renders_rows() {
        let html = VaultsTemplate {
            t: AdminText::en(),
            vaults: vec![VaultAdminView {
                id: "v1".into(),
                user_id: "u1".into(),
                owner_username: "admin".into(),
                name: "main".into(),
                created_at: 1,
                last_sync_at: None,
                size_bytes: 0,
                file_count: 0,
            }],
            users: vec![user("u1", "admin", true)],
            message: None,
        }
        .render()
        .unwrap();
        assert!(html.contains("main"));
        assert!(html.contains("admin"));
        assert!(html.contains("Reconcile"));
    }

    #[test]
    fn invites_template_renders() {
        let html = InvitesTemplate {
            t: AdminText::en(),
            invites: vec![Invite {
                code: "inv_abc".into(),
                created_by: "u1".into(),
                created_at: 1,
                expires_at: Some(2),
                used_at: None,
                used_by: None,
            }],
        }
        .render()
        .unwrap();
        assert!(html.contains("inv_abc"));
        assert!(html.contains("Create invite"));
    }

    #[test]
    fn settings_template_renders_current_config() {
        let html = SettingsTemplate {
            t: AdminText::en(),
            cfg: RuntimeConfig {
                registration_mode: RegistrationMode::InviteOnly,
                server_name: "Vault Hub".into(),
                login_failure_threshold: 5,
                login_window_seconds: 60,
                login_lock_seconds: 120,
                max_file_size: 100 * 1024 * 1024,
                text_extensions: RuntimeConfig::default().text_extensions.clone(),
            },
        }
        .render()
        .unwrap();
        assert!(html.contains("Vault Hub"));
        assert!(html.contains("invite_only"));
        assert!(html.contains("Save"));
    }

    #[test]
    fn activity_template_renders_rows() {
        let html = ActivityTemplate {
            t: AdminText::en(),
            activities: vec![ActivityView {
                timestamp: 1,
                username: "admin".into(),
                action: "vault_pull".into(),
                vault_id: Some("v1".into()),
                client_ip: Some("127.0.0.1".into()),
                user_agent: Some("PKVSync-Plugin/0.1.0".into()),
            }],
        }
        .render()
        .unwrap();
        assert!(html.contains("vault_pull"));
        assert!(html.contains("PKVSync-Plugin"));
    }
}
