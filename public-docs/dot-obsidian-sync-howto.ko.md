# 鞐煬 旮瓣赴鞐愳劀 `.obsidian` 靹れ爼 霃欔赴頇?

[English](./dot-obsidian-sync-howto.md) | [绠€浣撲腑鏂嘳(./dot-obsidian-sync-howto.zh-CN.md) | [绻侀珨涓枃](./dot-obsidian-sync-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./dot-obsidian-sync-howto.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.5.

PKV Sync電?旮半掣鞝侅溂搿?hidden path毳?頂柬暕雼堧嫟. vault氤?allowlist毳?鞝滉车頃橂瘈搿?鞝勳泊 Obsidian 雮措秬 霐旊爥韯半Μ臧€ 鞎勲媹霛?靹犿儩頃?`.obsidian` 靹れ爼 韺岇澕毵?opt in頃?靾?鞛堨姷雼堧嫟.

## 靸?vault臧€ 旮半掣鞙茧 霃欔赴頇旐晿電?頃

靸?vault鞐愲姅 雼れ潓 starter allowlist臧€ 鞝侅毄霅╇媹雼?

- Themes: `.obsidian/themes/**`
- CSS snippets: `.obsidian/snippets/**`
- Hotkeys: `.obsidian/hotkeys.json`
- App preferences: `.obsidian/app.json`
- Appearance preferences: `.obsidian/appearance.json`
- Enabled community plugin list: `.obsidian/community-plugins.json`
- Enabled core plugin list: `.obsidian/core-plugins.json`

韽暔霅橂姅 瓴冹潃 enabled plugin list肟愳瀰雼堧嫟. plugin code鞕€ plugin settings電?旮半掣鞙茧 霃欔赴頇旊悩歆€ 鞎婌姷雼堧嫟.

旮办〈 vault電?starter list毳?鞝侅毄頃橁赴 鞝勱箤歆€ 牍?allowlist毳?鞙犾頃╇媹雼?

- **Admin WebUI: Vaults -> Settings -> Apply starter allowlist**電?鞙勳潣 7-glob starter list 鞝勳泊毳?旮半頃╇媹雼?
- **Obsidian plugin: Settings -> PKV Sync -> Apply recommended starter list**電?臧€鞛?鞎堨爠頃?霊?glob(`.obsidian/themes/**`鞕€ `.obsidian/snippets/**`)毵?旮半頃╇媹雼? themes鞕€ CSS snippets電?氤错喌 鞐煬 旮瓣赴鞐愳劀 瓿奠湢頃措弰 鞎堨爠頃?氚橂┐, 雮橂ǜ歆€ 雼れ劘 glob鞚€ 靷毄鞛愲硠 app state鞐?雼筷赴 霑岆鞐?plugin鞚€ 氇呾嫓鞝侅澑 瓴办爼 鞐嗢澊電?頇滌劚頇旐晿歆€ 鞎婌姷雼堧嫟.

7-glob starter 鞝勳泊毳?鞝侅毄頃橂牑氅?Admin WebUI 氩勴娂鞚?靷毄頃橁卑雮?plugin鞚?allowlist editor鞐?歆侅爲 glob鞚?攵欖棳雱ｌ溂靹胳殧.

## 鞝堧寑 霃欔赴頇旐晿歆€ 鞎婋姅 頃

雼れ潓 hard exclusions電?allowlist鞐?於旉皜頃措弰 頃儊 鞖办劆頃╇媹雼?

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

於旉皜 glob鞚?靹れ爼頃?靾?鞛堨毵?鞙勴棙鞚€ 靷毄鞛愱皜 臧愳垬頃挫暭 頃╇媹雼?

- `.obsidian/plugins/*/data.json`: plugin settings鞛呺媹雼? API key, OAuth token, LLM key臧€ 霌れ柎 鞛堨潉 靾?鞛堨姷雼堧嫟. native E2EE臧€ 鞝滉车霅橁赴 鞝勱箤歆€ 霃欔赴頇旊悳 雮挫毄鞚€ server鞐?plaintext搿?鞝€鞛ル惄雼堧嫟.
- `.obsidian/plugins/**`: plugin code鞛呺媹雼? Git history臧€ 牍犽ゴ瓴?旎れ 靾?鞛堦碃, desktop-only plugin鞚?mobile鞐愳劀 旯 靾?鞛堨姷雼堧嫟.
- `.claude/**` 霕愲姅 `.codex/**` 臧欖潃 雼るジ hidden directories: agent state鞐?氙缄皭頃?搿滌滑 context臧€ 韽暔霅?靾?鞛堨姷雼堧嫟.

## 攴滌箼鞚?韼胳頃橂姅 鞙勳箻

- Obsidian: **Settings -> PKV Sync**鞐愳劀 順勳灛 vault毳?靹犿儩頃橁碃 **.obsidian sync rules**毳?韼胳頃?霋?鞝€鞛ロ暕雼堧嫟.
- Admin WebUI: **Vaults**毳?鞐搓碃 vault鞚?**Settings**毳?靹犿儩頃?allowlist毳?韼胳頃?霋?鞝€鞛ロ暕雼堧嫟.
