# `.obsidian` 瑷畾銈掋儑銉愩偆銈归枔銇у悓鏈熴仚銈?

[English](./dot-obsidian-sync-howto.md) | [绠€浣撲腑鏂嘳(./dot-obsidian-sync-howto.zh-CN.md) | [绻侀珨涓枃](./dot-obsidian-sync-howto.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./dot-obsidian-sync-howto.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.3銆?

PKV Sync 銇€氬父銆乭idden path 銈掑悓鏈熴仐銇俱仜銈撱€倂ault 銇斻仺銇?allowlist 銇倛銈娿€丱bsidian 鍐呴儴銉囥偅銉偗銉堛儶鍏ㄤ綋銇с伅銇亸銆佸繀瑕併仾 `.obsidian` 瑷畾銉曘偂銈ゃ儷銇犮亼銈?opt in 銇с亶銇俱仚銆?

## 鏂般仐銇?vault 銇屾棦瀹氥仹鍚屾湡銇欍倠銈傘伄

鏂般仐銇?vault 銇伅銆佹銇?starter allowlist 銇岃ō瀹氥仌銈屻伨銇欍€?

- Themes锛歚.obsidian/themes/**`
- CSS snippets锛歚.obsidian/snippets/**`
- Hotkeys锛歚.obsidian/hotkeys.json`
- App preferences锛歚.obsidian/app.json`
- Appearance preferences锛歚.obsidian/appearance.json`
- Enabled community plugin list锛歚.obsidian/community-plugins.json`
- Enabled core plugin list锛歚.obsidian/core-plugins.json`

鍚伨銈屻倠銇伅鏈夊姽鍖栨笀銇?plugin list 銇伩銇с仚銆俻lugin code 銇?plugin settings 銇棦瀹氥仹銇悓鏈熴仌銈屻伨銇涖倱銆?

鏃㈠瓨 vault 銇€乻tarter list 銈掗仼鐢ㄣ仚銈嬨伨銇х┖銇?allowlist 銇伨銇俱仹銇欍€?

- **Admin WebUI锛歏aults -> Settings -> Apply starter allowlist** 銇笂瑷樸伄 7 glob 銇欍伖銇︺倰 starter list 銇ㄣ仐銇︽浉銇嶈炯銇裤伨銇欍€?
- **Obsidian plugin锛歋ettings -> PKV Sync -> Apply recommended starter list** 銇渶銈傚畨鍏ㄣ仾 2 glob锛坄.obsidian/themes/**` 銇?`.obsidian/snippets/**`锛夈伄銇裤倰鏇搞亶杈笺伩銇俱仚銆倀hemes 銇?CSS snippets 銇儑銉愩偆銈归枔銇у叡鏈夈仐銇︺倐閫氬父瀹夊叏銇с仚銇屻€佹畫銈?5 銇ゃ伄 glob 銇儲銉笺偠銉煎浐鏈夈伄 app state 銇Е銈屻倠銇熴倎銆乸lugin 銇槑绀虹殑銇垽鏂仾銇椼伀銇湁鍔瑰寲銇椼伨銇涖倱銆?

7 glob 銇畬鍏ㄣ仾 starter 銈掗仼鐢ㄣ仚銈嬨伀銇€丄dmin WebUI 銇儨銈裤兂銈掍娇銇嗐亱銆乸lugin 銇?allowlist editor 銇?glob 銈掓墜鍕曘仹璨笺倞浠樸亼銇︺亸銇犮仌銇勩€?

## 甯搞伀鍚屾湡銇曘倢銇亜銈傘伄

娆°伄 hard exclusions 銇€乤llowlist 銇拷鍔犮仐銇︺倐甯搞伀鍎厛銇曘倢銇俱仚銆?

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

杩藉姞銇?glob 銈掕ō瀹氥仹銇嶃伨銇欍亴銆併儶銈广偗銇埄鐢ㄨ€呭伌銇у彈銇戝叆銈屻倠蹇呰銇屻亗銈娿伨銇欍€?

- `.obsidian/plugins/*/data.json`锛歱lugin settings銆侫PI keys銆丱Auth tokens銆丩LM keys 銇屽惈銇俱倢銈嬨亾銇ㄣ亴銇傘倞銇俱仚銆俷ative E2EE 銇屽叆銈嬨伨銇с€佸悓鏈熷唴瀹广伅 server 銇?plaintext 銇т繚瀛樸仌銈屻伨銇欍€?
- `.obsidian/plugins/**`锛歱lugin code銆侴it history 銇屾€ラ€熴伀澧椼亪銆乨esktop-only plugin 銇?mobile 銇у銈屻倠鍙兘鎬с亴銇傘倞銇俱仚銆?
- `.claude/**` 銈?`.codex/**` 銇仼浠栥伄 hidden directories锛歛gent state 銇?sensitive local context 銈掑惈銈€銇撱仺銇屻亗銈娿伨銇欍€?

## 銉兗銉倰绶ㄩ泦銇欍倠鍫存墍

- Obsidian锛?*Settings -> PKV Sync** 銈掗枊銇嶃€佺従鍦ㄣ伄 vault 銈掗伕銇炽€?*.obsidian sync rules** 銈掔法闆嗐仐銇︿繚瀛樸仐銇俱仚銆?
- Admin WebUI锛?*Vaults** 銈掗枊銇嶃€乿ault 銇?**Settings** 銈掗伕銇炽€乤llowlist 銈掔法闆嗐仐銇︿繚瀛樸仐銇俱仚銆?
