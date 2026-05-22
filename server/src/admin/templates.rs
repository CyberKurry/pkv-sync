use crate::admin::i18n::AdminText;
use crate::db::repos::RuntimeConfig;
use askama::Template;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate<'a> {
    pub t: AdminText,
    pub error: Option<&'a str>,
    pub version: &'static str,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub t: AdminText,
    pub username: String,
    pub users: i64,
    pub vaults: i64,
    pub cpu_percent: f32,
    pub cpu_display: String,
    pub cpu_cores_display: String,
    pub memory_display: String,
    pub memory_total_display: String,
    pub disk_used_display: String,
    pub disk_total_display: String,
    pub uptime_display: String,
    pub recent_activities: Vec<ActivityView>,
}

#[derive(Template)]
#[template(path = "users.html")]
pub struct UsersTemplate {
    pub t: AdminText,
    pub users: Vec<UserAdminView>,
    pub query: String,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserAdminView {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct UserOptionView {
    pub id: String,
    pub username: String,
}

#[derive(Template)]
#[template(path = "user_detail.html")]
pub struct UserDetailTemplate {
    pub t: AdminText,
    pub user: UserAdminView,
    pub tokens: Vec<TokenAdminView>,
    pub message: Option<String>,
    pub created_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TokenAdminView {
    pub id: String,
    pub device_name: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Template)]
#[template(path = "devices.html")]
pub struct DevicesTemplate {
    pub t: AdminText,
    pub users: Vec<UserOptionView>,
    pub tokens: Vec<DeviceTokenAdminView>,
    pub created_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeviceTokenAdminView {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub device_id: String,
    pub device_name: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VaultAdminView {
    pub id: String,
    pub user_id: String,
    pub owner_username: String,
    pub name: String,
    pub created_at: String,
    pub last_sync_at: Option<String>,
    pub size_display: String,
    pub size_bytes: i64,
    pub file_count: i64,
}

#[derive(Template)]
#[template(path = "vaults.html")]
pub struct VaultsTemplate {
    pub t: AdminText,
    pub vaults: Vec<VaultAdminView>,
    pub users: Vec<UserOptionView>,
    pub message: Option<String>,
    pub total_vaults: usize,
    pub total_size_display: String,
    pub synced_today: usize,
    pub enable_history_ui: bool,
}

#[derive(Debug, Clone)]
pub struct VaultBrowserView {
    pub id: String,
    pub name: String,
    pub owner_username: String,
}

#[derive(Debug, Clone)]
pub struct VaultFileEntryView {
    pub path: String,
    pub name: String,
    pub size_display: String,
    pub kind: String,
    pub view_url: String,
}

#[derive(Template)]
#[template(path = "vault_files.html")]
pub struct VaultFilesTemplate {
    pub t: AdminText,
    pub vault: VaultBrowserView,
    pub files: Vec<VaultFileEntryView>,
}

#[derive(Template)]
#[template(path = "vault_settings.html")]
pub struct VaultSettingsTemplate {
    pub t: AdminText,
    pub vault: VaultBrowserView,
    pub extra_sync_globs_display: String,
}

#[derive(Template)]
#[template(path = "vault_file_view.html")]
pub struct VaultFileViewTemplate {
    pub t: AdminText,
    pub vault: VaultBrowserView,
    pub path: String,
    pub at: Option<String>,
    pub size_display: String,
    pub binary: bool,
    pub content: String,
    pub history_url: String,
    pub diff_url: Option<String>,
    pub enable_diff_endpoint: bool,
}

#[derive(Debug, Clone)]
pub struct VaultHistoryEntryView {
    pub commit: String,
    pub short_commit: String,
    pub parent: Option<String>,
    pub message: String,
    pub timestamp: String,
    pub author_device: String,
    pub change_type: String,
    pub view_url: String,
    pub diff_url: String,
    pub rollback_url: String,
}

#[derive(Template)]
#[template(path = "vault_history.html")]
pub struct VaultHistoryTemplate {
    pub t: AdminText,
    pub vault: VaultBrowserView,
    pub path: String,
    pub entries: Vec<VaultHistoryEntryView>,
}

#[derive(Debug, Clone)]
pub struct DiffRowView {
    pub class: String,
    pub full_width: bool,
    pub text: String,
    pub left_line: Option<usize>,
    pub right_line: Option<usize>,
    pub left_class: String,
    pub right_class: String,
    pub left_text: String,
    pub right_text: String,
}

#[derive(Template)]
#[template(path = "vault_diff.html")]
pub struct VaultDiffTemplate {
    pub t: AdminText,
    pub vault: VaultBrowserView,
    pub path: String,
    pub from: Option<String>,
    pub to: String,
    pub from_label: String,
    pub to_label: String,
    pub binary: bool,
    pub truncated: bool,
    pub rows: Vec<DiffRowView>,
}

#[derive(Template)]
#[template(path = "invites.html")]
pub struct InvitesTemplate {
    pub t: AdminText,
    pub invites: Vec<InviteAdminView>,
    pub pending_invites: usize,
    pub used_invites: usize,
    pub revoked_invites: usize,
}

#[derive(Debug, Clone)]
pub struct InviteAdminView {
    pub code: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub used_at: Option<String>,
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub t: AdminText,
    pub cfg: RuntimeConfig,
    pub max_file_size_display: String,
    pub text_extensions_display: String,
    pub extra_exclude_globs_display: String,
    pub git_available: bool,
}

#[derive(Debug, Clone)]
pub struct ActivityView {
    pub timestamp: String,
    pub username: String,
    pub action: String,
    pub vault_id: Option<String>,
    pub vault_name: Option<String>,
    pub device_name: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActivityFilterUser {
    pub id: String,
    pub username: String,
}

#[derive(Template)]
#[template(path = "activity.html")]
pub struct ActivityTemplate {
    pub t: AdminText,
    pub activities: Vec<ActivityView>,
    pub users: Vec<ActivityFilterUser>,
    pub selected_user_id: String,
    pub selected_action: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::RegistrationMode;

    #[test]
    fn login_template_renders() {
        let version = env!("CARGO_PKG_VERSION");
        let html = LoginTemplate {
            t: AdminText::en(),
            error: Some("bad"),
            version,
        }
        .render()
        .unwrap();
        assert!(html.contains("PKV Sync Admin"));
        assert!(html.contains("bad"));
        assert!(html.contains(&format!("PKV Sync v{version}")));
    }

    #[test]
    fn dashboard_template_renders() {
        let html = DashboardTemplate {
            t: AdminText::en(),
            username: "admin".into(),
            users: 1,
            vaults: 2,
            cpu_percent: 3.0,
            cpu_display: "3".into(),
            cpu_cores_display: "1 core".into(),
            memory_display: "10 MB".into(),
            memory_total_display: "20 MB".into(),
            disk_used_display: "1 GB".into(),
            disk_total_display: "2 GB".into(),
            uptime_display: "5s".into(),
            recent_activities: vec![ActivityView {
                timestamp: "1970-01-01 00:00:01 +00:00 UTC".into(),
                username: "admin".into(),
                action: "push".into(),
                vault_id: Some("v1".into()),
                vault_name: Some("main".into()),
                device_name: Some("Laptop".into()),
                client_ip: None,
                user_agent: None,
            }],
        }
        .render()
        .unwrap();
        assert!(html.contains("admin"));
        assert!(html.contains("Users"));
        assert!(html.contains("Sync Status"));
        assert!(html.contains("class=\"app-shell\""));
        assert!(html.contains("href=\"/admin/devices\""));
        assert!(!html.contains("unpkg.com"));
    }

    #[test]
    fn admin_css_uses_designer_shell_tokens() {
        let css = include_str!("../../static/admin.css");
        assert!(css.contains("color-scheme: light"));
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        assert!(css.contains("#0f111c"));
        assert!(css.contains("#141623"));
        assert!(css.contains("#161928"));
        assert!(css.contains(".app-shell"));
        assert!(css.contains(".sidebar-nav"));
    }

    #[test]
    fn admin_templates_reference_existing_icon_symbols() {
        let sprite = include_str!("../../static/lucide-icons.svg");
        let templates = [
            ("activity", include_str!("../../templates/activity.html")),
            ("dashboard", include_str!("../../templates/dashboard.html")),
            ("devices", include_str!("../../templates/devices.html")),
            ("invites", include_str!("../../templates/invites.html")),
            ("layout", include_str!("../../templates/layout.html")),
            ("login", include_str!("../../templates/login.html")),
            ("settings", include_str!("../../templates/settings.html")),
            ("users", include_str!("../../templates/users.html")),
            (
                "user_detail",
                include_str!("../../templates/user_detail.html"),
            ),
            ("vaults", include_str!("../../templates/vaults.html")),
            (
                "vault_diff",
                include_str!("../../templates/vault_diff.html"),
            ),
            (
                "vault_files",
                include_str!("../../templates/vault_files.html"),
            ),
            (
                "vault_file_view",
                include_str!("../../templates/vault_file_view.html"),
            ),
            (
                "vault_history",
                include_str!("../../templates/vault_history.html"),
            ),
            (
                "vault_settings",
                include_str!("../../templates/vault_settings.html"),
            ),
        ];
        let re = regex::Regex::new(r#"lucide-icons\.svg#([A-Za-z0-9_-]+)"#).unwrap();
        for (name, template) in templates {
            for cap in re.captures_iter(template) {
                let id = &cap[1];
                assert!(
                    sprite.contains(&format!(r#"id="{id}""#)),
                    "template {name} references missing icon {id}"
                );
            }
        }
    }

    #[test]
    fn admin_shell_is_fluid_and_has_mobile_drawer_tokens() {
        let css = include_str!("../../static/admin.css");
        assert!(!css.contains("min(1440px"));
        assert!(!css.contains("min(900px"));
        assert!(!css.contains("1057px"));
        assert!(css.contains("width: 100vw"));
        assert!(css.contains("height: 100vh"));
        assert!(css.contains(".sidebar-toggle"));
        assert!(css.contains(".mobile-menu-button"));
        assert!(!css.contains("width: min(100%, 1057px"));
        assert!(!css.contains("width: min(100%, 900px"));
    }

    #[test]
    fn dashboard_template_uses_lucide_sprite_and_mobile_toggle() {
        let html = DashboardTemplate {
            t: AdminText::en(),
            username: "admin".into(),
            users: 1,
            vaults: 2,
            cpu_percent: 3.0,
            cpu_display: "3".into(),
            cpu_cores_display: "1 core".into(),
            memory_display: "10 MB".into(),
            memory_total_display: "20 MB".into(),
            disk_used_display: "1 GB".into(),
            disk_total_display: "2 GB".into(),
            uptime_display: "5s".into(),
            recent_activities: Vec::new(),
        }
        .render()
        .unwrap();
        assert!(html.contains("id=\"admin-sidebar-toggle\""));
        assert!(html.contains("class=\"mobile-menu-button\""));
        assert!(html.contains("/admin/static/lucide-icons.svg#gauge"));
        assert!(html.contains("/admin/static/lucide-icons.svg#users-round"));
        assert!(html.contains("/admin/static/lucide-icons.svg#menu"));
        assert!(!html.contains("<path d=\"M4 13.5a8 8"));
    }

    fn user(id: &str, username: &str, is_admin: bool) -> UserAdminView {
        UserAdminView {
            id: id.into(),
            username: username.into(),
            is_admin,
            is_active: true,
            created_at: "1970-01-01 00:00:01".into(),
        }
    }

    fn user_option(id: &str, username: &str) -> UserOptionView {
        UserOptionView {
            id: id.into(),
            username: username.into(),
        }
    }

    #[test]
    fn users_template_renders() {
        let html = UsersTemplate {
            t: AdminText::en(),
            users: vec![user("u1", "admin", true)],
            query: "adm".into(),
            status: "active".into(),
            message: Some("created".into()),
        }
        .render()
        .unwrap();
        assert!(html.contains("admin"));
        assert!(html.contains("Create user"));
        assert!(html.contains("/admin/static/lucide-icons.svg#filter"));
        assert!(html.contains("/admin/static/lucide-icons.svg#x"));
        assert!(html.contains("/admin/static/lucide-icons.svg#square-pen"));
    }

    #[test]
    fn user_detail_template_renders() {
        let html = UserDetailTemplate {
            t: AdminText::en(),
            user: user("u1", "admin", true),
            tokens: vec![TokenAdminView {
                id: "t1".into(),
                device_name: "desktop".into(),
                created_at: "1970-01-01 00:00:01".into(),
                last_used_at: Some("1970-01-01 00:00:02".into()),
                revoked_at: None,
            }],
            message: None,
            created_token: None,
        }
        .render()
        .unwrap();
        assert!(html.contains("desktop"));
        assert!(html.contains("Reset password"));
        assert!(html.contains("<h1>User Details</h1>"));
        assert!(html.contains("/admin/static/lucide-icons.svg#ban"));
        assert!(html.contains("/admin/static/lucide-icons.svg#key-round"));
        assert!(!html.contains("User: admin"));
        assert!(!html.contains("+00:00 UTC"));
        assert!(!html.contains("+08:00 CST"));
        assert!(!html.contains("Asia/Shanghai"));
    }

    #[test]
    fn user_detail_template_uses_admin_shell_detail_layout() {
        let html = UserDetailTemplate {
            t: AdminText::en(),
            user: user("u1", "admin", true),
            tokens: vec![TokenAdminView {
                id: "t1".into(),
                device_name: "desktop".into(),
                created_at: "1970-01-01 00:00:01".into(),
                last_used_at: None,
                revoked_at: None,
            }],
            message: None,
            created_token: None,
        }
        .render()
        .unwrap();
        assert!(html.contains("class=\"user-detail-layout\""));
        assert!(html.contains("class=\"panel user-profile-panel\""));
        assert!(html.contains("class=\"user-action-grid\""));
        assert!(html.contains("class=\"panel table-panel tokens-table\""));
        assert!(!html.contains("class=\"metric-grid three\""));

        let css = include_str!("../../static/admin.css").replace("\r\n", "\n");
        assert!(css.contains(".user-detail-layout"));
        assert!(css.contains(".user-profile-panel"));
        assert!(css.contains(".user-action-grid"));
        assert!(css.contains(".tokens-table"));
        assert!(css.contains(".page-bar {\n    display: grid;"));
        assert!(css.contains(".user-profile-head {\n    display: grid;"));
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
    fn devices_template_renders_rows() {
        let html = DevicesTemplate {
            t: AdminText::en(),
            users: vec![user_option("u1", "admin")],
            tokens: vec![DeviceTokenAdminView {
                id: "t1".into(),
                user_id: "u1".into(),
                username: "admin".into(),
                device_id: "device-a".into(),
                device_name: "MacBook Pro".into(),
                created_at: "1970-01-01 00:00:01 +00:00 UTC".into(),
                last_used_at: Some("1970-01-01 00:00:02 +00:00 UTC".into()),
                revoked_at: None,
            }],
            created_token: Some("pks_device_token".into()),
        }
        .render()
        .unwrap();
        assert!(html.contains("MacBook Pro"));
        assert!(html.contains("pks_device_token"));
        assert!(html.contains("Create token"));
        assert!(html.contains("/admin/static/lucide-icons.svg#ban"));
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
                created_at: "1970-01-01 00:00:01 +00:00 UTC".into(),
                last_sync_at: None,
                size_display: "0 B".into(),
                size_bytes: 0,
                file_count: 0,
            }],
            users: vec![user_option("u1", "admin")],
            message: None,
            total_vaults: 1,
            total_size_display: "0 B".into(),
            synced_today: 0,
            enable_history_ui: true,
        }
        .render()
        .unwrap();
        assert!(html.contains("main"));
        assert!(html.contains("admin"));
        assert!(html.contains("Reconcile"));
        assert!(html.contains("/admin/static/lucide-icons.svg#folder-open"));
        assert!(html.contains("/admin/vaults/v1/settings"));
        assert!(include_str!("../../static/lucide-icons.svg").contains("id=\"folder-open\""));
    }

    #[test]
    fn vault_settings_template_renders_current_globs() {
        let html = VaultSettingsTemplate {
            t: AdminText::en(),
            vault: VaultBrowserView {
                id: "v1".into(),
                name: "main".into(),
                owner_username: "admin".into(),
            },
            extra_sync_globs_display: "notes/**\n.obsidian/app.json".into(),
        }
        .render()
        .unwrap();
        assert!(html.contains("Vault Settings"));
        assert!(html.contains("main"));
        assert!(html.contains("notes/**"));
        assert!(html.contains(".obsidian/app.json"));
        assert!(html.contains("name=\"apply_starter\""));
        assert!(html.contains("/admin/static/lucide-icons.svg#save"));
    }

    #[test]
    fn invites_template_renders() {
        let html = InvitesTemplate {
            t: AdminText::en(),
            invites: vec![InviteAdminView {
                code: "inv_abc".into(),
                created_at: "1970-01-01 00:00:01 +00:00 UTC".into(),
                expires_at: Some("1970-01-01 00:00:02 +00:00 UTC".into()),
                used_at: None,
            }],
            pending_invites: 1,
            used_invites: 0,
            revoked_invites: 0,
        }
        .render()
        .unwrap();
        assert!(html.contains("inv_abc"));
        assert!(html.contains("Create invite"));
        assert!(html.contains("type=\"datetime-local\""));
        assert!(html.contains("/admin/static/lucide-icons.svg#plus"));
    }

    #[test]
    fn settings_template_renders_current_config() {
        let html = SettingsTemplate {
            t: AdminText::en(),
            cfg: RuntimeConfig {
                registration_mode: RegistrationMode::InviteOnly,
                server_name: "Vault Hub".into(),
                timezone: "UTC".into(),
                login_failure_threshold: 5,
                login_window_seconds: 60,
                login_lock_seconds: 120,
                max_file_size: 100 * 1024 * 1024,
                text_extensions: RuntimeConfig::default().text_extensions.clone(),
                enable_history_ui: true,
                enable_diff_endpoint: true,
                extra_exclude_globs: vec![],
                inline_content_max_bytes: 8192,
                sse_heartbeat_seconds: 30,
                push_debounce_ms: 250,
                enable_git_smart_http: false,
                enable_metrics: false,
                enable_auto_merge: true,
            },
            max_file_size_display: "100 MB".into(),
            text_extensions_display: "md, txt".into(),
            extra_exclude_globs_display: String::new(),
            git_available: true,
        }
        .render()
        .unwrap();
        assert!(html.contains("Vault Hub"));
        assert!(html.contains("invite_only"));
        assert!(html.contains("select name=\"timezone\""));
        assert!(html.contains("option value=\"UTC\" selected"));
        assert!(html.contains("Save"));
        assert!(html.contains("name=\"enable_auto_merge\""));
        assert!(html.contains("/admin/static/lucide-icons.svg#trash-2"));
    }

    #[test]
    fn settings_template_orders_sections_like_sidebar() {
        let html = SettingsTemplate {
            t: AdminText::en(),
            cfg: RuntimeConfig {
                registration_mode: RegistrationMode::InviteOnly,
                server_name: "Vault Hub".into(),
                timezone: "UTC".into(),
                login_failure_threshold: 5,
                login_window_seconds: 60,
                login_lock_seconds: 120,
                max_file_size: 100 * 1024 * 1024,
                text_extensions: RuntimeConfig::default().text_extensions.clone(),
                enable_history_ui: true,
                enable_diff_endpoint: true,
                extra_exclude_globs: vec![],
                inline_content_max_bytes: 8192,
                sse_heartbeat_seconds: 30,
                push_debounce_ms: 250,
                enable_git_smart_http: false,
                enable_metrics: false,
                enable_auto_merge: true,
            },
            max_file_size_display: "100 MB".into(),
            text_extensions_display: "md, txt".into(),
            extra_exclude_globs_display: String::new(),
            git_available: true,
        }
        .render()
        .unwrap();
        let general = html.find("id=\"general\"").unwrap();
        let security = html.find("id=\"security\"").unwrap();
        let sync = html.find("id=\"sync-storage\"").unwrap();
        let network = html.find("id=\"network\"").unwrap();
        assert!(general < security);
        assert!(security < sync);
        assert!(sync < network);
    }

    #[test]
    fn activity_template_renders_rows() {
        let html = ActivityTemplate {
            t: AdminText::en(),
            activities: vec![ActivityView {
                timestamp: "1970-01-01 00:00:01 +00:00 UTC".into(),
                username: "admin".into(),
                action: "vault_pull".into(),
                vault_id: Some("v1".into()),
                vault_name: Some("Main Vault".into()),
                device_name: Some("Laptop".into()),
                client_ip: Some("127.0.0.1".into()),
                user_agent: Some("PKVSync-Plugin/0.1.0".into()),
            }],
            users: vec![ActivityFilterUser {
                id: "u1".into(),
                username: "admin".into(),
            }],
            selected_user_id: String::new(),
            selected_action: String::new(),
        }
        .render()
        .unwrap();
        assert!(html.contains("vault_pull"));
        assert!(html.contains("Main Vault"));
        assert!(html.contains("Laptop"));
        assert!(html.contains("<summary>ID</summary>"));
        assert!(html.contains("PKVSync-Plugin"));
        assert!(html.contains("/admin/static/lucide-icons.svg#filter"));
    }

    #[test]
    fn login_template_submit_has_icon() {
        let html = LoginTemplate {
            t: AdminText::en(),
            error: None,
            version: env!("CARGO_PKG_VERSION"),
        }
        .render()
        .unwrap();
        assert!(html.contains("/admin/static/lucide-icons.svg#log-in"));
    }
}
