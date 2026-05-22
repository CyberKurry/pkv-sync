use axum::http::{header, HeaderMap};
use serde::Deserialize;
use tower_cookies::{Cookie, Cookies};

pub const COOKIE_NAME: &str = "pkv_admin_lang";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AdminLang {
    En,
    ZhCn,
    ZhHant,
    Ja,
    Ko,
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
    pub devices: &'static str,
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
    pub timezone: &'static str,
    pub registration_mode: &'static str,
    pub login_failure_threshold: &'static str,
    pub login_window_seconds: &'static str,
    pub login_lock_seconds: &'static str,
    pub save: &'static str,
    pub time: &'static str,
    pub action: &'static str,
    pub ip: &'static str,
    pub user_agent: &'static str,
    pub extra_exclude_globs: &'static str,
    pub extra_exclude_globs_hint: &'static str,
    pub setup_title: &'static str,
    pub setup_description: &'static str,
    pub setup_username_label: &'static str,
    pub setup_password_label: &'static str,
    pub setup_confirm_label: &'static str,
    pub setup_submit: &'static str,
    pub setup_password_too_weak: &'static str,
    pub setup_password_mismatch: &'static str,
    pub setup_username_invalid: &'static str,
    pub setup_success: &'static str,
    pub setup_required: &'static str,
    pub setup_begin: &'static str,
    pub update_available: &'static str,
    pub update_release_notes: &'static str,
}

impl AdminLang {
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.to_ascii_lowercase();
        match normalized.as_str() {
            "en" => Some(Self::En),
            value
                if value.starts_with("zh-hant")
                    || value.starts_with("zh-tw")
                    || value.starts_with("zh-hk")
                    || value.starts_with("zh-mo") =>
            {
                Some(Self::ZhHant)
            }
            "zh-cn" | "zh" => Some(Self::ZhCn),
            value if value.starts_with("ja") => Some(Self::Ja),
            value if value.starts_with("ko") => Some(Self::Ko),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
            Self::ZhHant => "zh-Hant",
            Self::Ja => "ja",
            Self::Ko => "ko",
        }
    }

    pub fn text(self) -> AdminText {
        match self {
            Self::En => AdminText::en(),
            Self::ZhCn => AdminText::zh_cn(),
            Self::ZhHant => AdminText::zh_hant(),
            Self::Ja => AdminText::ja(),
            Self::Ko => AdminText::ko(),
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
            login: "Sign In",
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
            devices: "Devices",
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
            expires_at: "Expires at (blank = never)",
            code: "Code",
            expires: "Expires",
            used: "Used",
            runtime_settings: "Runtime Settings",
            server_name: "Server name",
            timezone: "Timezone",
            registration_mode: "Registration mode",
            login_failure_threshold: "Login failure threshold",
            login_window_seconds: "Login window seconds",
            login_lock_seconds: "Login lock seconds",
            save: "Save",
            time: "Time",
            action: "Action",
            ip: "IP",
            user_agent: "User-Agent",
            extra_exclude_globs: "Extra Exclude Globs",
            extra_exclude_globs_hint: "File patterns to exclude from sync (one per line). These are in addition to the built-in hard excludes.",
            setup_title: "PKV Sync Initial Setup",
            setup_description: "Create the first administrator account for this PKV Sync server.",
            setup_username_label: "Administrator username",
            setup_password_label: "Password",
            setup_confirm_label: "Confirm password",
            setup_submit: "Create administrator",
            setup_password_too_weak: "Use at least 12 characters with uppercase, lowercase, and a number.",
            setup_password_mismatch: "Passwords do not match.",
            setup_username_invalid: "Use 3-32 ASCII letters, numbers, underscores, or hyphens.",
            setup_success: "Setup complete. Please sign in.",
            setup_required: "This server needs first-run setup before you can sign in.",
            setup_begin: "Open setup",
            update_available: "New PKV Sync version available",
            update_release_notes: "Release notes",
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
            devices: "设备",
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
            expires_at: "过期时间（留空表示永不过期）",
            code: "代码",
            expires: "过期",
            used: "已使用",
            runtime_settings: "运行时设置",
            server_name: "服务器名称",
            timezone: "时区",
            registration_mode: "注册模式",
            login_failure_threshold: "登录失败阈值",
            login_window_seconds: "登录窗口秒数",
            login_lock_seconds: "登录锁定秒数",
            save: "保存",
            time: "时间",
            action: "操作",
            ip: "IP",
            user_agent: "User-Agent",
            setup_title: "PKV Sync 初始设置",
            setup_description: "为此 PKV Sync 服务器创建第一个管理员账号。",
            setup_username_label: "管理员用户名",
            setup_password_label: "密码",
            setup_confirm_label: "确认密码",
            setup_submit: "创建管理员",
            setup_password_too_weak: "请使用至少 12 个字符，并包含大写字母、小写字母和数字。",
            setup_password_mismatch: "两次输入的密码不一致。",
            setup_username_invalid: "请使用 3-32 个 ASCII 字母、数字、下划线或连字符。",
            setup_success: "设置完成。请登录。",
            setup_required: "此服务器需要先完成初始设置，然后才能登录。",
            setup_begin: "打开设置",
            update_available: "有新的 PKV Sync 版本可用",
            update_release_notes: "发行说明",
            extra_exclude_globs: "额外排除规则",
            extra_exclude_globs_hint:
                "从同步中排除的文件模式（每行一个）。这些规则在内置硬排除之外额外生效。",
        }
    }

    pub fn zh_hant() -> Self {
        Self {
            html_lang: "zh-Hant",
            language_label: "English | 简体中文 | 繁體中文 | 日本語 | 한국어",
            admin_title: "PKV Sync 管理後台",
            login_title: "PKV Sync 管理後台登入",
            username: "使用者名稱",
            password: "密碼",
            login: "登入",
            logout: "登出",
            dashboard: "儀表板",
            signed_in_as: "目前登入",
            users: "使用者",
            vaults: "筆記庫",
            invites: "邀請碼",
            settings: "設定",
            activity: "活動",
            create_user: "建立使用者",
            active: "啟用",
            created: "建立時間",
            details: "詳情",
            user: "使用者",
            reset_password: "重設密碼",
            new_password: "新密碼",
            reset: "重設",
            disable: "停用",
            enable: "啟用",
            tokens: "裝置 Token",
            new_device_token: "新的裝置 Token",
            create_device_token: "建立裝置 Token",
            device_name: "裝置名稱",
            create_token: "建立 Token",
            device: "裝置",
            devices: "裝置",
            last_used: "上次使用",
            revoked: "已撤銷",
            revoke: "撤銷",
            create_vault: "建立筆記庫",
            owner: "擁有者",
            name: "名稱",
            files: "檔案數",
            size: "大小",
            last_sync: "上次同步",
            reconcile: "修復中繼資料",
            delete: "刪除",
            create_invite: "建立邀請碼",
            expires_at: "過期時間（留空表示永不過期）",
            code: "代碼",
            expires: "過期",
            used: "已使用",
            runtime_settings: "執行階段設定",
            server_name: "伺服器名稱",
            timezone: "時區",
            registration_mode: "註冊模式",
            login_failure_threshold: "登入失敗閾值",
            login_window_seconds: "登入視窗秒數",
            login_lock_seconds: "登入鎖定秒數",
            save: "儲存",
            time: "時間",
            action: "操作",
            setup_title: "PKV Sync 初始設定",
            setup_description: "為此 PKV Sync 伺服器建立第一個管理員帳號。",
            setup_username_label: "管理員使用者名稱",
            setup_password_label: "密碼",
            setup_confirm_label: "確認密碼",
            setup_submit: "建立管理員",
            setup_password_too_weak: "請使用至少 12 個字元，並包含大寫字母、小寫字母和數字。",
            setup_password_mismatch: "兩次輸入的密碼不一致。",
            setup_username_invalid: "請使用 3-32 個 ASCII 字母、數字、底線或連字號。",
            setup_success: "設定完成。請登入。",
            setup_required: "此伺服器需要先完成初始設定，然後才能登入。",
            setup_begin: "開啟設定",
            update_available: "有新的 PKV Sync 版本可用",
            update_release_notes: "發行說明",
            extra_exclude_globs: "額外排除規則",
            extra_exclude_globs_hint:
                "從同步中排除的檔案模式（每行一個）。這些規則會在內建硬排除之外額外生效。",
            ..Self::zh_cn()
        }
    }

    pub fn ja() -> Self {
        Self {
            html_lang: "ja",
            language_label: "English | 简体中文 | 繁體中文 | 日本語 | 한국어",
            admin_title: "PKV Sync 管理",
            login_title: "PKV Sync 管理ログイン",
            username: "ユーザー名",
            password: "パスワード",
            login: "ログイン",
            logout: "ログアウト",
            dashboard: "ダッシュボード",
            users: "ユーザー",
            vaults: "Vault",
            invites: "招待コード",
            settings: "設定",
            activity: "アクティビティ",
            device_name: "デバイス名",
            devices: "デバイス",
            save: "保存",
            time: "時刻",
            action: "操作",
            setup_title: "PKV Sync 初期設定",
            setup_description: "この PKV Sync サーバーの最初の管理者アカウントを作成します。",
            setup_username_label: "管理者ユーザー名",
            setup_password_label: "パスワード",
            setup_confirm_label: "パスワード確認",
            setup_submit: "管理者を作成",
            setup_password_too_weak: "12 文字以上で、大文字・小文字・数字を含めてください。",
            setup_password_mismatch: "パスワードが一致しません。",
            setup_username_invalid:
                "3-32 文字の ASCII 英数字、アンダースコア、ハイフンを使用してください。",
            setup_success: "設定が完了しました。ログインしてください。",
            setup_required: "ログインする前に、このサーバーの初期設定が必要です。",
            setup_begin: "設定を開く",
            update_available: "新しい PKV Sync バージョンがあります",
            update_release_notes: "リリースノート",
            ..Self::en()
        }
    }

    pub fn ko() -> Self {
        Self {
            html_lang: "ko",
            language_label: "English | 简体中文 | 繁體中文 | 日本語 | 한국어",
            admin_title: "PKV Sync 관리자",
            login_title: "PKV Sync 관리자 로그인",
            username: "사용자 이름",
            password: "비밀번호",
            login: "로그인",
            logout: "로그아웃",
            dashboard: "대시보드",
            users: "사용자",
            vaults: "Vault",
            invites: "초대 코드",
            settings: "설정",
            activity: "활동",
            device_name: "장치 이름",
            devices: "장치",
            save: "저장",
            time: "시간",
            action: "작업",
            setup_title: "PKV Sync 초기 설정",
            setup_description: "이 PKV Sync 서버의 첫 관리자 계정을 만듭니다.",
            setup_username_label: "관리자 사용자 이름",
            setup_password_label: "비밀번호",
            setup_confirm_label: "비밀번호 확인",
            setup_submit: "관리자 만들기",
            setup_password_too_weak: "12자 이상이며 대문자, 소문자, 숫자를 포함해야 합니다.",
            setup_password_mismatch: "비밀번호가 일치하지 않습니다.",
            setup_username_invalid: "3-32자의 ASCII 영문자, 숫자, 밑줄 또는 하이픈을 사용하세요.",
            setup_success: "설정이 완료되었습니다. 로그인하세요.",
            setup_required: "로그인하기 전에 이 서버의 초기 설정을 완료해야 합니다.",
            setup_begin: "설정 열기",
            update_available: "새 PKV Sync 버전을 사용할 수 있습니다",
            update_release_notes: "릴리스 노트",
            ..Self::en()
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
        assert_eq!(AdminLang::parse("zh-Hant"), Some(AdminLang::ZhHant));
        assert_eq!(AdminLang::parse("zh-TW"), Some(AdminLang::ZhHant));
        assert_eq!(AdminLang::parse("ja-JP"), Some(AdminLang::Ja));
        assert_eq!(AdminLang::parse("ko-KR"), Some(AdminLang::Ko));
        assert_eq!(AdminLang::parse("fr"), None);
    }
}
