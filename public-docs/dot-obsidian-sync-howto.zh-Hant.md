# 跨裝置同步 `.obsidian` 設定

[English](./dot-obsidian-sync-howto.md) | [简体中文](./dot-obsidian-sync-howto.zh-CN.md) | 繁體中文 | [日本語](./dot-obsidian-sync-howto.ja.md) | [한국어](./dot-obsidian-sync-howto.ko.md)

文件版本：v1.4.1。

PKV Sync 預設避開隱藏路徑。它提供按 vault 設定的 allowlist，讓你可以選擇性同步 `.obsidian` 設定檔，而不是同步整個 Obsidian 內部目錄。

## 新 vault 預設同步什麼

新 vault 會得到這組起步 allowlist：

- 主題：`.obsidian/themes/**`
- CSS snippets：`.obsidian/snippets/**`
- 快捷鍵：`.obsidian/hotkeys.json`
- App 偏好：`.obsidian/app.json`
- 外觀偏好：`.obsidian/appearance.json`
- 已啟用社群外掛清單：`.obsidian/community-plugins.json`
- 已啟用核心外掛清單：`.obsidian/core-plugins.json`

這裡只包含已啟用外掛清單。外掛程式碼和外掛設定預設不會同步。

既有 vault 會保持空 allowlist，直到你套用起步清單。

- **Admin WebUI：Vaults -> Settings -> Apply starter allowlist** 會寫入上述完整 7 條 glob 起步清單。
- **Obsidian 外掛：設定 -> PKV Sync -> Apply recommended starter list** 只寫入兩條最安全的 glob（`.obsidian/themes/**` 與 `.obsidian/snippets/**`）—— 主題與 CSS snippets 通常可以安全跨裝置共享；其餘 5 條 glob 會觸及使用者專屬的 app 狀態，外掛不會在沒有明確決定下啟用它們。

要使用完整 7 條 glob 起步清單，請按 Admin WebUI 按鈕，或在外掛的 allowlist 編輯器中手動貼入這些 glob。

## 永不同步

以下硬性排除始終優先，即使你把它們加入 allowlist 也不會同步：

- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.git/**`
- `.trash/**`
- `.conflict-*`
- `*.lock`
- `*.tmp`

## 進階 opt-in

你可以新增額外 glob，但需要自行承擔風險：

- `.obsidian/plugins/*/data.json`：外掛設定。這裡可能包含 API key、OAuth token 或 LLM key。原生端到端加密落地前，同步內容會以明文存放在 server。
- `.obsidian/plugins/**`：外掛程式碼。這會讓 Git 歷史快速膨脹，且桌面專用外掛同步到行動端時可能無法運行。
- 其他隱藏目錄，例如 `.claude/**` 或 `.codex/**`：agent 狀態可能包含敏感本機上下文。

## 在哪裡編輯規則

- Obsidian：開啟 **設定 -> PKV Sync**，選擇目前 vault，編輯 **.obsidian sync rules**，然後儲存。
- Admin WebUI：開啟 **Vaults**，選擇 vault 的 **Settings**，編輯 allowlist，然後儲存。
