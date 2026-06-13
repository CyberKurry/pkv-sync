# 여러 기기에서 `.obsidian` 설정 동기화

[English](./dot-obsidian-sync-howto.md) | [简体中文](./dot-obsidian-sync-howto.zh-CN.md) | [繁體中文](./dot-obsidian-sync-howto.zh-Hant.md) | [日本語](./dot-obsidian-sync-howto.ja.md) | 한국어

문서 버전: v1.4.0.

PKV Sync는 기본적으로 hidden path를 피합니다. vault별 allowlist를 제공하므로 전체 Obsidian 내부 디렉터리가 아니라 선택한 `.obsidian` 설정 파일만 opt in할 수 있습니다.

## 새 vault가 기본으로 동기화하는 항목

새 vault에는 다음 starter allowlist가 적용됩니다.

- Themes: `.obsidian/themes/**`
- CSS snippets: `.obsidian/snippets/**`
- Hotkeys: `.obsidian/hotkeys.json`
- App preferences: `.obsidian/app.json`
- Appearance preferences: `.obsidian/appearance.json`
- Enabled community plugin list: `.obsidian/community-plugins.json`
- Enabled core plugin list: `.obsidian/core-plugins.json`

포함되는 것은 enabled plugin list뿐입니다. plugin code와 plugin settings는 기본으로 동기화되지 않습니다.

기존 vault는 starter list를 적용하기 전까지 빈 allowlist를 유지합니다.

- **Admin WebUI: Vaults -> Settings -> Apply starter allowlist**는 위의 7-glob starter list 전체를 기록합니다.
- **Obsidian plugin: Settings -> PKV Sync -> Apply recommended starter list**는 가장 안전한 두 glob(`.obsidian/themes/**`와 `.obsidian/snippets/**`)만 기록합니다. themes와 CSS snippets는 보통 여러 기기에서 공유해도 안전한 반면, 나머지 다섯 glob은 사용자별 app state에 닿기 때문에 plugin은 명시적인 결정 없이는 활성화하지 않습니다.

7-glob starter 전체를 적용하려면 Admin WebUI 버튼을 사용하거나 plugin의 allowlist editor에 직접 glob을 붙여넣으세요.

## 절대 동기화하지 않는 항목

다음 hard exclusions는 allowlist에 추가해도 항상 우선합니다.

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

추가 glob을 설정할 수 있지만 위험은 사용자가 감수해야 합니다.

- `.obsidian/plugins/*/data.json`: plugin settings입니다. API key, OAuth token, LLM key가 들어 있을 수 있습니다. native E2EE가 제공되기 전까지 동기화된 내용은 server에 plaintext로 저장됩니다.
- `.obsidian/plugins/**`: plugin code입니다. Git history가 빠르게 커질 수 있고, desktop-only plugin이 mobile에서 깨질 수 있습니다.
- `.claude/**` 또는 `.codex/**` 같은 다른 hidden directories: agent state에 민감한 로컬 context가 포함될 수 있습니다.

## 규칙을 편집하는 위치

- Obsidian: **Settings -> PKV Sync**에서 현재 vault를 선택하고 **.obsidian sync rules**를 편집한 뒤 저장합니다.
- Admin WebUI: **Vaults**를 열고 vault의 **Settings**를 선택해 allowlist를 편집한 뒤 저장합니다.
