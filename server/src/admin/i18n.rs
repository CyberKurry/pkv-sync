use axum::http::{header, HeaderMap};
use serde::Deserialize;
use tower_cookies::{Cookie, Cookies};

pub const COOKIE_NAME: &str = "pkv_admin_lang";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AdminLang {
    En,
    ZhCn,
}

#[derive(Debug, Clone, Copy)]
pub struct AdminText {
    pub html_lang: &'static str,
    pub language_label: &'static str,
    pub admin_title: &'static str,
    pub login_title: &'static str,
    pub username: &'static str,
    pub password: &'static str,
    pub login: &'static str,
    pub logout: &'static str,
    pub dashboard: &'static str,
    pub signed_in_as: &'static str,
    pub users: &'static str,
    pub vaults: &'static str,
    pub invites: &'static str,
    pub settings: &'static str,
    pub activity: &'static str,
    pub cpu: &'static str,
    pub memory: &'static str,
    pub disk: &'static str,
    pub uptime: &'static str,
    pub maintenance: &'static str,
    pub run_blob_gc: &'static str,
    pub create_user: &'static str,
    pub admin: &'static str,
    pub active: &'static str,
    pub created: &'static str,
    pub details: &'static str,
    pub user: &'static str,
    pub id: &'static str,
    pub reset_password: &'static str,
    pub new_password: &'static str,
    pub reset: &'static str,
    pub disable: &'static str,
    pub enable: &'static str,
    pub demote: &'static str,
    pub promote: &'static str,
    pub tokens: &'static str,
    pub new_device_token: &'static str,
    pub copy_token_now: &'static str,
    pub create_device_token: &'static str,
    pub device_name: &'static str,
    pub create_token: &'static str,
    pub device: &'static str,
    pub last_used: &'static str,
    pub revoked: &'static str,
    pub revoke: &'static str,
    pub create_vault: &'static str,
    pub owner: &'static str,
    pub name: &'static str,
    pub files: &'static str,
    pub size: &'static str,
    pub last_sync: &'static str,
    pub reconcile: &'static str,
    pub delete: &'static str,
    pub create_invite: &'static str,
    pub expires_at: &'static str,
    pub code: &'static str,
    pub expires: &'static str,
    pub used: &'static str,
    pub runtime_settings: &'static str,
    pub server_name: &'static str,
    pub registration_mode: &'static str,
    pub login_failure_threshold: &'static str,
    pub login_window_seconds: &'static str,
    pub login_lock_seconds: &'static str,
    pub save: &'static str,
    pub time: &'static str,
    pub action: &'static str,
    pub ip: &'static str,
    pub user_agent: &'static str,
}

impl AdminLang {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "en" => Some(Self::En),
            "zh-CN" | "zh-cn" | "zh" => Some(Self::ZhCn),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }

    pub fn text(self) -> AdminText {
        match self {
            Self::En => AdminText::en(),
            Self::ZhCn => AdminText::zh_cn(),
        }
    }
}

impl AdminText {
    pub fn en() -> Self {
        Self {
            html_lang: "en",
            language_label: "English | 简体中文",
            admin_title: "PKV Sync Admin",
            login_title: "PKV Sync Admin Login",
            username: "Username",
            password: "Password",
            login: "Login",
            logout: "Logout",
            dashboard: "Dashboard",
            signed_in_as: "Signed in as",
            users: "Users",
            vaults: "Vaults",
            invites: "Invites",
            settings: "Settings",
            activity: "Activity",
            cpu: "CPU",
            memory: "Memory",
            disk: "Disk",
            uptime: "Uptime",
            maintenance: "Maintenance",
            run_blob_gc: "Run Blob GC",
            create_user: "Create user",
            admin: "Admin",
            active: "Active",
            created: "Created",
            details: "Details",
            user: "User",
            id: "ID",
            reset_password: "Reset password",
            new_password: "New password",
            reset: "Reset",
            disable: "Disable",
            enable: "Enable",
            demote: "Demote",
            promote: "Promote",
            tokens: "Tokens",
            new_device_token: "New device token",
            copy_token_now: "Copy this token now. It will not be shown again.",
            create_device_token: "Create device token",
            device_name: "Device name",
            create_token: "Create token",
            device: "Device",
            last_used: "Last used",
            revoked: "Revoked",
            revoke: "Revoke",
            create_vault: "Create vault",
            owner: "Owner",
            name: "Name",
            files: "Files",
            size: "Size",
            last_sync: "Last sync",
            reconcile: "Reconcile",
            delete: "Delete",
            create_invite: "Create invite",
            expires_at: "Expires at (unix seconds, blank = never)",
            code: "Code",
            expires: "Expires",
            used: "Used",
            runtime_settings: "Runtime Settings",
            server_name: "Server name",
            registration_mode: "Registration mode",
            login_failure_threshold: "Login failure threshold",
            login_window_seconds: "Login window seconds",
            login_lock_seconds: "Login lock seconds",
            save: "Save",
            time: "Time",
            action: "Action",
            ip: "IP",
            user_agent: "User-Agent",
        }
    }

    pub fn zh_cn() -> Self {
        Self {
            html_lang: "zh-CN",
            language_label: "English | 简体中文",
            admin_title: "PKV Sync 管理后台",
            login_title: "PKV Sync 管理后台登录",
            username: "用户名",
            password: "密码",
            login: "登录",
            logout: "退出登录",
            dashboard: "仪表盘",
            signed_in_as: "当前登录",
            users: "用户",
            vaults: "笔记库",
            invites: "邀请码",
            settings: "设置",
            activity: "活动",
            cpu: "CPU",
            memory: "内存",
            disk: "磁盘",
            uptime: "运行时间",
            maintenance: "维护",
            run_blob_gc: "运行 Blob 垃圾回收",
            create_user: "创建用户",
            admin: "管理员",
            active: "启用",
            created: "创建时间",
            details: "详情",
            user: "用户",
            id: "ID",
            reset_password: "重置密码",
            new_password: "新密码",
            reset: "重置",
            disable: "禁用",
            enable: "启用",
            demote: "降级",
            promote: "提升",
            tokens: "设备 Token",
            new_device_token: "新的设备 Token",
            copy_token_now: "请现在复制此 token。它不会再次显示。",
            create_device_token: "创建设备 Token",
            device_name: "设备名称",
            create_token: "创建 Token",
            device: "设备",
            last_used: "上次使用",
            revoked: "已撤销",
            revoke: "撤销",
            create_vault: "创建笔记库",
            owner: "所有者",
            name: "名称",
            files: "文件数",
            size: "大小",
            last_sync: "上次同步",
            reconcile: "修复元数据",
            delete: "删除",
            create_invite: "创建邀请码",
            expires_at: "过期时间（Unix 秒，留空表示永不过期）",
            code: "代码",
            expires: "过期",
            used: "已使用",
            runtime_settings: "运行时设置",
            server_name: "服务器名称",
            registration_mode: "注册模式",
            login_failure_threshold: "登录失败阈值",
            login_window_seconds: "登录窗口秒数",
            login_lock_seconds: "登录锁定秒数",
            save: "保存",
            time: "时间",
            action: "操作",
            ip: "IP",
            user_agent: "User-Agent",
        }
    }
}

pub fn detect(headers: &HeaderMap, cookies: &Cookies) -> AdminLang {
    if let Some(cookie) = cookies.get(COOKIE_NAME) {
        if let Some(lang) = AdminLang::parse(cookie.value()) {
            return lang;
        }
    }
    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .split(',')
                .map(str::trim)
                .find_map(|part| AdminLang::parse(part.split(';').next().unwrap_or(part)))
        })
        .unwrap_or(AdminLang::En)
}

pub fn language_cookie(lang: AdminLang, secure: bool) -> Cookie<'static> {
    let mut cookie = Cookie::new(COOKIE_NAME, lang.as_str());
    cookie.set_secure(secure);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    cookie.set_path("/admin");
    cookie
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_languages() {
        assert_eq!(AdminLang::parse("en"), Some(AdminLang::En));
        assert_eq!(AdminLang::parse("zh-CN"), Some(AdminLang::ZhCn));
        assert_eq!(AdminLang::parse("zh"), Some(AdminLang::ZhCn));
        assert_eq!(AdminLang::parse("fr"), None);
    }
}
