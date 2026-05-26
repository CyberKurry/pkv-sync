use crate::admin::i18n::AdminText;
use crate::db::repos::RuntimeConfig;
use crate::service::update_check::UpdateStatus;
use askama::Template;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate<'a> {
    pub t: AdminText,
    pub error: Option<&'a str>,
    pub success: Option<&'a str>,
    pub setup_required: bool,
    pub username_value: String,
    pub version: &'static str,
}

#[derive(Template)]
#[template(path = "setup.html")]
pub struct SetupTemplate<'a> {
    pub t: AdminText,
    pub error: Option<&'a str>,
    pub username_value: String,
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
    pub update_status: Option<UpdateStatus>,
    pub recent_activities: Vec<ActivityView>,
    pub current_version: &'static str,
    /// Localised "X ago" or empty when no check has run yet.
    pub last_update_check_display: String,
    /// Live count of SSE subscribers across all vaults.
    pub sse_subscribers: usize,
    /// Localised "X ago" string for the most recent sync activity (push /
    /// pull / rollback / MCP write), or empty when no activity exists.
    pub last_sync_activity_display: String,
    /// "live" when at least one SSE subscriber is connected, "active" when
    /// no subscriber but a sync activity happened recently, "quiet" otherwise.
    /// Templates use this to pick the badge colour/class.
    pub sync_status_state: &'static str,
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
    pub avatar_label: String,
    pub is_admin: bool,
    pub is_active: bool,
    pub created_at: String,
    pub vault_count: i64,
    pub last_sync_at: Option<String>,
}

pub fn avatar_label(username: &str) -> String {
    username
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_else(|| "?".into())
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
    pub fingerprint: String,
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
    pub fingerprint: String,
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
            success: None,
            setup_required: false,
            username_value: String::new(),
            version,
        }
        .render()
        .unwrap();
        assert!(html.contains("PKV Sync Admin"));
        assert!(html.contains("bad"));
        assert!(html.contains(&format!("PKV Sync v{version}")));
    }

    #[test]
    fn login_and_setup_templates_do_not_leak_english_placeholders_in_chinese() {
        let login_html = LoginTemplate {
            t: AdminText::zh_cn(),
            error: None,
            success: None,
            setup_required: false,
            username_value: String::new(),
            version: env!("CARGO_PKG_VERSION"),
        }
        .render()
        .unwrap();
        let setup_html = SetupTemplate {
            t: AdminText::zh_cn(),
            error: None,
            username_value: String::new(),
            version: env!("CARGO_PKG_VERSION"),
        }
        .render()
        .unwrap();

        for leaked in [
            "Enter your username",
            "Sign in to your admin panel",
            "placeholder=\"Password\"",
            "placeholder=\"admin\"",
            "绠€浣撲腑鏂",
            "绻侀珨涓",
            "鏃ユ湰",
            "頃滉淡",
        ] {
            assert!(
                !login_html.contains(leaked) && !setup_html.contains(leaked),
                "localized auth templates leaked English placeholder: {leaked}"
            );
        }
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
            update_status: None,
            current_version: env!("CARGO_PKG_VERSION"),
            last_update_check_display: String::new(),
            sse_subscribers: 0,
            last_sync_activity_display: String::new(),
            sync_status_state: "quiet",
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
        assert!(
            !html.contains("All systems healthy"),
            "Sync Status card must reflect live state, not the legacy static placeholder"
        );
        assert!(
            html.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
            "dashboard must display the running server version"
        );
        assert!(html.contains("class=\"app-shell\""));
        assert!(html.contains("href=\"/admin/devices\""));
        assert!(!html.contains("unpkg.com"));
    }

    #[test]
    fn dashboard_template_renders_update_banner() {
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
            update_status: Some(UpdateStatus {
                latest_version: "0.8.1".into(),
                current_version: "0.8.0".into(),
                release_url: "https://github.com/cyberkurry/pkv-sync/releases/tag/v0.8.1".into(),
                notes_excerpt: "Release notes".into(),
            }),
            current_version: env!("CARGO_PKG_VERSION"),
            last_update_check_display: "1 minute ago".into(),
            sse_subscribers: 0,
            last_sync_activity_display: String::new(),
            sync_status_state: "quiet",
            recent_activities: Vec::new(),
        }
        .render()
        .unwrap();
        assert!(html.contains("v0.8.1"));
        assert!(html.contains("Release notes"));
        assert!(html.contains("update-banner"));
    }

    #[test]
    fn admin_css_uses_designer_shell_tokens() {
        let css = include_str!("../../static/admin.css");
        // Light is the default; explicit dark override and an OS-preference
        // fallback must both exist so dark mode works in either picker.
        assert!(css.contains("color-scheme: light"));
        assert!(css.contains("color-scheme: dark"));
        assert!(css.contains("[data-theme=\"dark\"]"));
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        // Core layout primitives must still be present.
        assert!(css.contains(".app-shell"));
        assert!(css.contains(".sidebar-nav"));
        // Design token namespace.
        assert!(css.contains("--pkv-paper"));
        assert!(css.contains("--pkv-ink"));
        assert!(css.contains("--pkv-mark"));
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
        // The shell must not pin a magic max width that breaks ultrawide /
        // narrow layouts.
        assert!(!css.contains("min(1440px"));
        assert!(!css.contains("min(900px"));
        assert!(!css.contains("1057px"));
        assert!(!css.contains("width: min(100%, 1057px"));
        assert!(!css.contains("width: min(100%, 900px"));
        // Sticky full-height sidebar that turns into a slide-out drawer on
        // small screens.
        assert!(css.contains(".sidebar"));
        assert!(css.contains("height: 100vh"));
        assert!(css.contains(".sidebar-toggle"));
        assert!(css.contains(".mobile-menu-button"));
        assert!(css.contains(".sidebar-scrim"));
        // The mobile breakpoint must repack the shell into a single column.
        assert!(css.contains("grid-template-columns: 1fr;"));
    }

    #[test]
    fn admin_shell_uses_language_select_in_sidebar() {
        let layout = include_str!("../../templates/layout.html");
        let css = include_str!("../../static/admin.css");
        let js = include_str!("../../static/admin.js");
        assert!(layout.contains("language-switch"));
        assert!(layout.contains("select"));
        assert!(layout.contains("data-next=\"/admin\""));
        assert!(layout.contains("data-language-select"));
        assert!(layout.contains("/admin/static/admin.js"));
        assert!(!layout.contains("onchange="));
        assert!(!layout.contains("<span>|</span>"));
        assert!(css.contains(".sidebar-control select"));
        assert!(css.contains("width: 100%"));
        assert!(js.contains("data-language-select"));
        assert!(js.contains("querySelectorAll(\"[data-language-select]\")"));
    }

    #[test]
    fn admin_shell_uses_single_theme_cycle_button_in_sidebar() {
        let layout = include_str!("../../templates/layout.html");
        let css = include_str!("../../static/admin.css");
        let js = include_str!("../../static/admin.js");
        let sprite = include_str!("../../static/lucide-icons.svg");
        assert!(layout.contains("data-theme-toggle"));
        assert!(layout.contains("data-theme-icon-use"));
        assert!(layout.contains("data-label-auto"));
        assert!(layout.contains("data-label-light"));
        assert!(layout.contains("data-label-dark"));
        assert!(layout.contains("type=\"button\""));
        assert!(!layout.contains("<script>"));
        assert!(css.contains(".theme-toggle-button"));
        assert!(css.contains(".theme-toggle-icon"));
        assert!(js.contains("pkv-admin-theme"));
        assert!(js.contains("dataset.theme"));
        assert!(js.contains("data-theme-toggle"));
        assert!(js.contains("querySelectorAll(\"[data-theme-toggle]\")"));
        assert!(js.contains("monitor"));
        assert!(js.contains("sun"));
        assert!(js.contains("moon"));
        assert!(sprite.contains(r#"id="monitor""#));
        assert!(sprite.contains(r#"id="sun""#));
        assert!(sprite.contains(r#"id="moon""#));
    }

    #[test]
    fn admin_css_keeps_working_paper_controls_polished() {
        let css = include_str!("../../static/admin.css").replace("\r\n", "\n");
        let sprite = include_str!("../../static/lucide-icons.svg");
        assert!(sprite.contains(r#"fill="none""#));
        assert!(sprite.contains(r#"stroke="currentColor""#));
        assert!(sprite.contains(r#"stroke-width="2""#));
        let symbol_re = regex::Regex::new(r#"<symbol[^>]*id="([^"]+)"[^>]*>"#).unwrap();
        for cap in symbol_re.captures_iter(sprite) {
            let symbol = cap.get(0).unwrap().as_str();
            assert!(
                symbol.contains(r#"fill="none""#),
                "icon {} is not self-contained",
                &cap[1]
            );
            assert!(
                symbol.contains(r#"stroke="currentColor""#),
                "icon {} is not self-contained",
                &cap[1]
            );
            assert!(
                symbol.contains(r#"stroke-width="2""#),
                "icon {} is not self-contained",
                &cap[1]
            );
        }
        assert!(css.contains(".admin-icon,\n.nav-icon"));
        assert!(css.contains("stroke: currentColor;"));
        assert!(css.contains("fill: none;"));
        assert!(css.contains("stroke-width: 2;"));
        assert!(css.contains(".nav-icon {\n    width: 18px;"));
        assert!(css.contains(".metric-head .admin-icon {\n    width: 30px;"));
        assert!(css.contains(".page-bar h1 {\n    font-family: var(--pkv-font-body);"));
        assert!(css.contains(".panel-header h2 {\n    font-family: var(--pkv-font-body);"));
        assert!(css.contains("letter-spacing: 0;"));
        assert!(css.contains(".activity-panel .panel-header"));
        assert!(css.contains(".theme-toggle-button span"));
        assert!(css.contains(".user-card .icon-button"));
        assert!(css.contains(".login-form input"));
        assert!(css
            .contains(".form-panel label:not(.check-row),\n.settings-form label:not(.check-row)"));
        assert!(!css.contains(".field-search::before"));
        assert!(css.contains(".field-search .admin-icon"));
        assert!(css.contains(".settings-section h2 {\n    font-family: var(--pkv-font-body);"));
        assert!(css.contains(".danger-row {\n    display: flex;"));
        assert!(css.contains(".settings-tabs a {\n    display: inline-flex;"));
        assert!(css.contains(".toolbar {\n    display: flex;\n    justify-content: flex-start;"));
        assert!(css.contains(".header-filter-form {\n    display: flex;\n    flex-wrap: wrap;"));
        assert!(css.contains(".header-filter-form > select"));
        assert!(css.contains(".card-actions {\n    display: flex;"));
        assert!(css.contains(".segmented input:checked + span"));
        assert!(css.contains(".segmented label:hover > span"));
        assert!(css.contains(".segmented input:focus-visible + span"));
        assert!(css.contains(".input-with-unit > span,\n.input-with-unit > small"));
        assert!(css.contains("input:disabled,\nselect:disabled,\ntextarea:disabled"));
        assert!(css.contains(".danger-row .danger"));
        assert!(css.contains(".sidebar-close {\n        display: inline-flex;"));
        assert!(css.contains("width: 44px;\n        height: 44px;"));
        assert!(css.contains("min-height: 44px;"));
        assert!(css.contains(".table-panel {\n    padding: 0;\n    overflow-x: auto;"));
        assert!(css.contains(".file-link {"));
        assert!(css.contains(".hint {"));
        assert!(css.contains(".diff-split {\n    display: grid;"));
        assert!(css.contains(".diff-split-row {\n    display: grid;"));
    }

    #[test]
    fn admin_css_keeps_page_header_actions_from_crushing_title() {
        let css = include_str!("../../static/admin.css").replace("\r\n", "\n");
        assert!(css.contains(".page-bar {\n    display: grid;"));
        assert!(css.contains("grid-template-columns: minmax(0, 1fr) auto;"));
        assert!(css.contains(".page-title-row {\n    display: flex;"));
        assert!(css.contains(".page-bar h1 {\n    font-family: var(--pkv-font-body);"));
        assert!(css.contains("overflow-wrap: anywhere;"));
        assert!(css.contains(".page-actions {\n    display: flex;"));
        assert!(css.contains("justify-content: flex-end;"));
        assert!(css.contains(".header-filter-form {\n    display: flex;\n    flex-wrap: wrap;"));
        assert!(css.contains(".page-bar {\n        grid-template-columns: 1fr;"));
        assert!(css.contains(".page-actions {\n        justify-content: flex-start;"));
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
            update_status: None,
            current_version: env!("CARGO_PKG_VERSION"),
            last_update_check_display: String::new(),
            sse_subscribers: 0,
            last_sync_activity_display: String::new(),
            sync_status_state: "quiet",
            recent_activities: Vec::new(),
        }
        .render()
        .unwrap();
        assert!(html.contains("id=\"admin-sidebar-toggle\""));
        assert!(html.contains("class=\"mobile-menu-button\""));
        assert!(html.contains("/admin/static/lucide-icons.svg#gauge"));
        assert!(html.contains("/admin/static/lucide-icons.svg#users-round"));
        assert!(html.contains("/admin/static/lucide-icons.svg#monitor"));
        assert!(html.contains("/admin/static/lucide-icons.svg#user-plus"));
        assert!(html.contains("/admin/static/lucide-icons.svg#menu"));
        assert!(!html.contains("<path d=\"M4 13.5a8 8"));
        assert!(!html.contains("/admin/static/lucide-icons.svg#server"));
    }

    fn user(id: &str, username: &str, is_admin: bool) -> UserAdminView {
        UserAdminView {
            id: id.into(),
            username: username.into(),
            avatar_label: avatar_label(username),
            is_admin,
            is_active: true,
            created_at: "1970-01-01 00:00:01".into(),
            vault_count: 0,
            last_sync_at: None,
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
        assert!(html.contains(
            "class=\"avatar avatar-small\" aria-hidden=\"true\">A</span><strong>admin</strong>"
        ));
        assert!(
            !html.contains("<td>—</td>"),
            "users table must show real vault and sync state instead of decorative dashes"
        );
    }

    #[test]
    fn users_template_uses_selected_admin_language() {
        let html = UsersTemplate {
            t: AdminText::zh_cn(),
            users: vec![user("u1", "cross", false)],
            query: String::new(),
            status: String::new(),
            message: None,
        }
        .render()
        .unwrap();
        for leaked in [
            "User Management",
            "Search users",
            "All Status",
            ">Status<",
            ">Active<",
            ">Inactive<",
            ">Actions<",
            ">Apply<",
            ">Clear<",
            "No users match",
        ] {
            assert!(
                !html.contains(leaked),
                "Simplified Chinese users page leaked English UI text: {leaked}"
            );
        }
    }

    #[test]
    fn user_detail_template_renders() {
        let html = UserDetailTemplate {
            t: AdminText::en(),
            user: user("u1", "admin", true),
            tokens: vec![TokenAdminView {
                id: "7067c8ad2ef34dc69eae6c08f03932fd".into(),
                fingerprint: "7067c8ad...32fd".into(),
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
        assert!(html.contains("/admin/static/lucide-icons.svg#shield"));
        assert!(html.contains("class=\"user-profile-avatar\" aria-hidden=\"true\">A</span>"));
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
                id: "7067c8ad2ef34dc69eae6c08f03932fd".into(),
                fingerprint: "7067c8ad...32fd".into(),
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
        // The detail layout must split into two columns on wide screens.
        assert!(css.contains(".user-detail-layout {\n    display: grid;"));
        // The user-action grid must be a multi-column grid for dense actions.
        assert!(css.contains(".user-action-grid {\n    display: grid;"));
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
        let stored_token_id = "7067c8ad2ef34dc69eae6c08f03932fd";
        let html = DevicesTemplate {
            t: AdminText::en(),
            users: vec![user_option("u1", "admin")],
            tokens: vec![DeviceTokenAdminView {
                id: stored_token_id.into(),
                fingerprint: "7067c8ad...32fd".into(),
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
        assert!(html.contains("name=\"device_name\" type=\"text\""));
        assert!(
            !html.contains(stored_token_id),
            "device list must not show the full stored token identifier"
        );
    }

    #[test]
    fn devices_template_uses_selected_admin_language() {
        let html = DevicesTemplate {
            t: AdminText::zh_cn(),
            users: vec![user_option("u1", "cross")],
            tokens: Vec::new(),
            created_token: None,
        }
        .render()
        .unwrap();
        for leaked in [
            "Device Tokens",
            ">Token<",
            ">Device ID<",
            ">Status<",
            ">Actions<",
        ] {
            assert!(
                !html.contains(leaked),
                "Simplified Chinese devices page leaked English UI text: {leaked}"
            );
        }
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
        assert!(html.contains("name=\"name\" type=\"text\""));
    }

    #[test]
    fn vaults_template_uses_selected_admin_language() {
        let html = VaultsTemplate {
            t: AdminText::zh_cn(),
            vaults: vec![VaultAdminView {
                id: "v1".into(),
                user_id: "u1".into(),
                owner_username: "admin".into(),
                name: "main".into(),
                created_at: "1970-01-01 00:00:01".into(),
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
        for leaked in [
            "Total Vaults",
            "Total Storage",
            "Synced Today",
            ">Idle<",
            "Browse files",
            ">Settings<",
        ] {
            assert!(
                !html.contains(leaked),
                "Simplified Chinese vaults page leaked English UI text: {leaked}"
            );
        }
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
        assert!(html.contains(AdminText::en().vault_settings));
        assert!(html.contains("main"));
        assert!(html.contains("notes/**"));
        assert!(html.contains(".obsidian/app.json"));
        assert!(html.contains("name=\"apply_starter\""));
        assert!(html.contains(
            "type=\"submit\" form=\"vault-settings-form\" name=\"action\" value=\"save\""
        ));
        assert!(html.contains("type=\"submit\" name=\"action\" value=\"save\""));
        assert!(html.contains("type=\"submit\" name=\"apply_starter\" value=\"1\""));
        assert!(html.contains("/admin/static/lucide-icons.svg#save"));
        assert!(!html.contains("success-gradient"));
    }

    #[test]
    fn vault_browser_templates_use_selected_admin_language() {
        let vault = VaultBrowserView {
            id: "v1".into(),
            name: "main".into(),
            owner_username: "admin".into(),
        };

        let settings = VaultSettingsTemplate {
            t: AdminText::zh_cn(),
            vault: vault.clone(),
            extra_sync_globs_display: "notes/**".into(),
        }
        .render()
        .unwrap();
        let files = VaultFilesTemplate {
            t: AdminText::zh_cn(),
            vault: vault.clone(),
            files: vec![VaultFileEntryView {
                path: "note.md".into(),
                name: "note.md".into(),
                size_display: "1 KB".into(),
                kind: "file".into(),
                view_url: "/view".into(),
            }],
        }
        .render()
        .unwrap();
        let file_view = VaultFileViewTemplate {
            t: AdminText::zh_cn(),
            vault: vault.clone(),
            path: "note.md".into(),
            at: Some("abcdef1".into()),
            size_display: "1 KB".into(),
            binary: true,
            content: String::new(),
            history_url: "/history".into(),
            diff_url: Some("/diff".into()),
            enable_diff_endpoint: true,
        }
        .render()
        .unwrap();
        let history = VaultHistoryTemplate {
            t: AdminText::zh_cn(),
            vault: vault.clone(),
            path: "note.md".into(),
            entries: Vec::new(),
        }
        .render()
        .unwrap();
        let diff = VaultDiffTemplate {
            t: AdminText::zh_cn(),
            vault,
            path: "note.md".into(),
            from: Some("parent".into()),
            to: "head".into(),
            from_label: "parent".into(),
            to_label: "head".into(),
            binary: true,
            truncated: true,
            rows: Vec::new(),
        }
        .render()
        .unwrap();
        let html = format!("{settings}{files}{file_view}{history}{diff}");
        for leaked in [
            "Vault Settings",
            "Extra sync globs",
            "Apply starter allowlist",
            "No files in this vault yet",
            ">Path<",
            ">Kind<",
            ">View<",
            ">Files<",
            ">History<",
            "Diff with previous",
            "Viewing commit",
            "Binary file preview is not available",
            "No history for this file yet",
            "Large diff truncated",
            "Diff preview is not available",
        ] {
            assert!(
                !html.contains(leaked),
                "Simplified Chinese vault browser UI leaked English text: {leaked}"
            );
        }
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
    fn invites_template_uses_selected_admin_language() {
        let html = InvitesTemplate {
            t: AdminText::zh_cn(),
            invites: vec![InviteAdminView {
                code: "inv_abc".into(),
                created_at: "1970-01-01 00:00:01 +00:00 UTC".into(),
                expires_at: None,
                used_at: None,
            }],
            pending_invites: 1,
            used_invites: 0,
            revoked_invites: 0,
        }
        .render()
        .unwrap();

        for leaked in [
            "Invite Codes",
            "Pending Invites",
            "Never expires",
            "Pending",
            "No pending invite codes",
        ] {
            assert!(
                !html.contains(leaked),
                "Simplified Chinese invites UI leaked English text: {leaked}"
            );
        }
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
        assert!(html.contains("name=\"server_name\" type=\"text\""));
        assert!(html.contains("value=\"100 MB\" disabled"));
        assert!(!html.contains("success-gradient"));
    }

    #[test]
    fn settings_template_uses_selected_admin_language() {
        let html = SettingsTemplate {
            t: AdminText::zh_cn(),
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
        for leaked in [
            "Save Changes",
            "General Settings",
            "Configure your server name",
            "Security",
            "Configure admin login protection",
            "Sync &amp; Storage",
            "Network",
        ] {
            assert!(
                !html.contains(leaked),
                "Simplified Chinese settings page leaked English UI text: {leaked}"
            );
        }
    }

    #[test]
    fn admin_shell_footer_uses_version_not_browser_clock() {
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
            update_status: None,
            current_version: env!("CARGO_PKG_VERSION"),
            last_update_check_display: String::new(),
            sse_subscribers: 0,
            last_sync_activity_display: String::new(),
            sync_status_state: "quiet",
            recent_activities: Vec::new(),
        }
        .render()
        .unwrap();
        assert!(!html.contains("data-pkv-colophon-now"));
        assert!(html.contains(r#"<span class="colophon-mark">PKV/SYNC</span>"#));
        assert!(html.contains(&format!(
            r#"<span class="colophon-version">v{}"#,
            env!("CARGO_PKG_VERSION")
        )));
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
        assert!(html.contains("<th>Vault</th>"));
        assert!(!html.contains("<th>Detail</th>"));
        assert!(html.contains("<summary>ID</summary>"));
        assert!(html.contains("PKVSync-Plugin"));
        assert!(html.contains("/admin/static/lucide-icons.svg#filter"));
        let css = include_str!("../../static/admin.css").replace("\r\n", "\n");
        assert!(css.contains(".header-filter-form {\n    display: flex;\n    flex-wrap: wrap;"));
    }

    #[test]
    fn vault_history_rollback_uses_history_icon_not_redo_icon() {
        let html = VaultHistoryTemplate {
            t: AdminText::en(),
            vault: VaultBrowserView {
                id: "v1".into(),
                name: "main".into(),
                owner_username: "admin".into(),
            },
            path: "note.md".into(),
            entries: vec![VaultHistoryEntryView {
                commit: "abcdef123456".into(),
                short_commit: "abcdef1".into(),
                parent: Some("parent1".into()),
                message: "Update note".into(),
                timestamp: "1970-01-01 00:00:01".into(),
                author_device: "desktop".into(),
                change_type: "modified".into(),
                view_url: "/view".into(),
                diff_url: "/diff".into(),
                rollback_url: "/rollback".into(),
            }],
        }
        .render()
        .unwrap();
        assert!(html.contains("Rollback"));
        assert!(html.contains("<strong class=\"row-title\">abcdef1</strong>"));
        assert!(!html.contains("<h2>abcdef1</h2>"));
        assert!(html.contains("/admin/static/lucide-icons.svg#history"));
        assert!(!html.contains("/admin/static/lucide-icons.svg#rotate-cw\"></use></svg>Rollback"));
    }

    #[test]
    fn login_template_submit_has_icon() {
        let html = LoginTemplate {
            t: AdminText::en(),
            error: None,
            success: None,
            setup_required: false,
            username_value: String::new(),
            version: env!("CARGO_PKG_VERSION"),
        }
        .render()
        .unwrap();
        assert!(html.contains("/admin/static/lucide-icons.svg#log-in"));
    }

    #[test]
    fn login_and_setup_templates_expose_language_and_theme_controls() {
        let login = LoginTemplate {
            t: AdminText::en(),
            error: None,
            success: None,
            setup_required: false,
            username_value: String::new(),
            version: env!("CARGO_PKG_VERSION"),
        }
        .render()
        .unwrap();
        let setup = SetupTemplate {
            t: AdminText::en(),
            error: None,
            username_value: String::new(),
            version: env!("CARGO_PKG_VERSION"),
        }
        .render()
        .unwrap();
        for html in [login, setup] {
            assert!(html.contains("/admin/static/admin.js"));
            assert!(html.contains("data-language-select"));
            assert!(html.contains("data-theme-toggle"));
            assert!(html.contains("data-theme-icon-use"));
            assert!(html.contains("/admin/static/lucide-icons.svg#monitor"));
            assert!(html.contains("data-label-auto"));
            assert!(!html.contains("<span>|</span>"));
        }
    }
}
