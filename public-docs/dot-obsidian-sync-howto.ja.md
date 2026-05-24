# `.obsidian` 設定をデバイス間で同期する

[English](./dot-obsidian-sync-howto.md) | [简体中文](./dot-obsidian-sync-howto.zh-CN.md) | [繁體中文](./dot-obsidian-sync-howto.zh-Hant.md) | 日本語 | [한국어](./dot-obsidian-sync-howto.ko.md)

PKV Sync は通常、hidden path を同期しません。vault ごとの allowlist により、Obsidian 内部ディレクトリ全体ではなく、必要な `.obsidian` 設定ファイルだけを opt in できます。

## 新しい vault が既定で同期するもの

新しい vault には、次の starter allowlist が設定されます。

- Themes：`.obsidian/themes/**`
- CSS snippets：`.obsidian/snippets/**`
- Hotkeys：`.obsidian/hotkeys.json`
- App preferences：`.obsidian/app.json`
- Appearance preferences：`.obsidian/appearance.json`
- Enabled community plugin list：`.obsidian/community-plugins.json`
- Enabled core plugin list：`.obsidian/core-plugins.json`

含まれるのは有効化済み plugin list のみです。plugin code と plugin settings は既定では同期されません。

既存 vault は、plugin settings または Admin WebUI で starter list を適用するまで空の allowlist のままです。

## 常に同期されないもの

次の hard exclusions は、allowlist に追加しても常に優先されます。

- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.git/**`
- `.trash/**`
- `.conflict-*`
- `*.lock`
- `*.tmp`

## Advanced opt-in

追加の glob を設定できますが、リスクは利用者側で受け入れる必要があります。

- `.obsidian/plugins/*/data.json`：plugin settings。API keys、OAuth tokens、LLM keys が含まれることがあります。native E2EE が入るまで、同期内容は server に plaintext で保存されます。
- `.obsidian/plugins/**`：plugin code。Git history が急速に増え、desktop-only plugin が mobile で壊れる可能性があります。
- `.claude/**` や `.codex/**` など他の hidden directories：agent state が sensitive local context を含むことがあります。

## ルールを編集する場所

- Obsidian：**Settings -> PKV Sync** を開き、現在の vault を選び、**.obsidian sync rules** を編集して保存します。
- Admin WebUI：**Vaults** を開き、vault の **Settings** を選び、allowlist を編集して保存します。
