# 璺ㄨ缃悓姝?`.obsidian` 瑷畾

[English](./dot-obsidian-sync-howto.md) | [绠€浣撲腑鏂嘳(./dot-obsidian-sync-howto.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./dot-obsidian-sync-howto.ja.md) | [頃滉淡鞏碷(./dot-obsidian-sync-howto.ko.md)

鏂囦欢鐗堟湰锛歷1.4.5銆?

PKV Sync 闋愯ō閬块枊闅辫棌璺緫銆傚畠鎻愪緵鎸?vault 瑷畾鐨?allowlist锛岃畵浣犲彲浠ラ伕鎿囨€у悓姝?`.obsidian` 瑷畾妾旓紝鑰屼笉鏄悓姝ユ暣鍊?Obsidian 鍏ч儴鐩寗銆?

## 鏂?vault 闋愯ō鍚屾浠€楹?

鏂?vault 鏈冨緱鍒伴€欑祫璧锋 allowlist锛?

- 涓婚锛歚.obsidian/themes/**`
- CSS snippets锛歚.obsidian/snippets/**`
- 蹇嵎閸碉細`.obsidian/hotkeys.json`
- App 鍋忓ソ锛歚.obsidian/app.json`
- 澶栬鍋忓ソ锛歚.obsidian/appearance.json`
- 宸插暉鐢ㄧぞ缇ゅ鎺涙竻鍠細`.obsidian/community-plugins.json`
- 宸插暉鐢ㄦ牳蹇冨鎺涙竻鍠細`.obsidian/core-plugins.json`

閫欒！鍙寘鍚凡鍟熺敤澶栨帥娓呭柈銆傚鎺涚▼寮忕⒓鍜屽鎺涜ō瀹氶爯瑷笉鏈冨悓姝ャ€?

鏃㈡湁 vault 鏈冧繚鎸佺┖ allowlist锛岀洿鍒颁綘濂楃敤璧锋娓呭柈銆?

- **Admin WebUI锛歏aults -> Settings -> Apply starter allowlist** 鏈冨鍏ヤ笂杩板畬鏁?7 姊?glob 璧锋娓呭柈銆?
- **Obsidian 澶栨帥锛氳ō瀹?-> PKV Sync -> Apply recommended starter list** 鍙鍏ュ叐姊濇渶瀹夊叏鐨?glob锛坄.obsidian/themes/**` 鑸?`.obsidian/snippets/**`锛夆€斺€?涓婚鑸?CSS snippets 閫氬父鍙互瀹夊叏璺ㄨ缃叡浜紱鍏堕 5 姊?glob 鏈冭Ц鍙婁娇鐢ㄨ€呭皥灞殑 app 鐙€鎱嬶紝澶栨帥涓嶆渻鍦ㄦ矑鏈夋槑纰烘焙瀹氫笅鍟熺敤瀹冨€戙€?

瑕佷娇鐢ㄥ畬鏁?7 姊?glob 璧锋娓呭柈锛岃珛鎸?Admin WebUI 鎸夐垥锛屾垨鍦ㄥ鎺涚殑 allowlist 绶ㄨ集鍣ㄤ腑鎵嬪嫊璨煎叆閫欎簺 glob銆?

## 姘镐笉鍚屾

浠ヤ笅纭€ф帓闄ゅ绲傚劒鍏堬紝鍗充娇浣犳妸瀹冨€戝姞鍏?allowlist 涔熶笉鏈冨悓姝ワ細

- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.git/**`
- `.trash/**`
- `.conflict-*`
- `*.lock`
- `*.tmp`

## 閫查殠 opt-in

浣犲彲浠ユ柊澧為澶?glob锛屼絾闇€瑕佽嚜琛屾壙鎿旈ⅷ闅細

- `.obsidian/plugins/*/data.json`锛氬鎺涜ō瀹氥€傞€欒！鍙兘鍖呭惈 API key銆丱Auth token 鎴?LLM key銆傚師鐢熺鍒扮鍔犲瘑钀藉湴鍓嶏紝鍚屾鍏у鏈冧互鏄庢枃瀛樻斁鍦?server銆?
- `.obsidian/plugins/**`锛氬鎺涚▼寮忕⒓銆傞€欐渻璁?Git 姝峰彶蹇€熻啫鑴癸紝涓旀闈㈠皥鐢ㄥ鎺涘悓姝ュ埌琛屽嫊绔檪鍙兘鐒℃硶閬嬭銆?
- 鍏朵粬闅辫棌鐩寗锛屼緥濡?`.claude/**` 鎴?`.codex/**`锛歛gent 鐙€鎱嬪彲鑳藉寘鍚晱鎰熸湰姗熶笂涓嬫枃銆?

## 鍦ㄥ摢瑁＄法杓鍓?

- Obsidian锛氶枊鍟?**瑷畾 -> PKV Sync**锛岄伕鎿囩洰鍓?vault锛岀法杓?**.obsidian sync rules**锛岀劧寰屽劜瀛樸€?
- Admin WebUI锛氶枊鍟?**Vaults**锛岄伕鎿?vault 鐨?**Settings**锛岀法杓?allowlist锛岀劧寰屽劜瀛樸€?
