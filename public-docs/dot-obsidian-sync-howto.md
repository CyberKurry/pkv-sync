# Sync `.obsidian` configuration across devices

English | [简体中文](./dot-obsidian-sync-howto.zh-CN.md)

PKV Sync normally avoids hidden paths. It adds a per-vault allowlist so you
can opt in to selected `.obsidian` configuration files without syncing the
entire Obsidian internals directory.

## What new vaults sync by default

New vaults get this starter allowlist:

- Themes: `.obsidian/themes/**`
- CSS snippets: `.obsidian/snippets/**`
- Hotkeys: `.obsidian/hotkeys.json`
- App preferences: `.obsidian/app.json`
- Appearance preferences: `.obsidian/appearance.json`
- Enabled community plugin list: `.obsidian/community-plugins.json`
- Enabled core plugin list: `.obsidian/core-plugins.json`

Only the enabled plugin lists are included. Plugin code and plugin settings are
not synced by default.

Existing vaults keep an empty allowlist until you apply the starter list from
the plugin settings or Admin WebUI.

## Never synced

These hard exclusions win even if you add them to the allowlist:

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

You can add extra globs, but you accept the risk:

- `.obsidian/plugins/*/data.json`: plugin settings. These may contain API keys,
  OAuth tokens, or LLM keys. Until end-to-end encryption lands, the server
  stores synced content in plaintext.
- `.obsidian/plugins/**`: plugin code. This can grow Git history quickly and
  may break across desktop and mobile if a plugin is desktop-only.
- Other hidden directories, such as `.claude/**` or `.codex/**`: agent state may
  include sensitive local context.

## Where to edit rules

- Obsidian: **Settings -> PKV Sync**, select the current vault, edit
  **.obsidian sync rules**, then save.
- Admin WebUI: open **Vaults**, choose **Settings** for a vault, edit the
  allowlist, then save.
