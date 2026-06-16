# 璺ㄨ澶囧悓姝?`.obsidian` 閰嶇疆

[English](./dot-obsidian-sync-howto.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./dot-obsidian-sync-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./dot-obsidian-sync-howto.ja.md) | [頃滉淡鞏碷(./dot-obsidian-sync-howto.ko.md)

鏂囨。鐗堟湰锛歷1.4.3銆?

PKV Sync 榛樿閬垮紑闅愯棌璺緞銆傚畠鎻愪緵鎸夌瑪璁板簱閰嶇疆鐨?allowlist锛岃浣犲彲浠ラ€夋嫨鎬у悓姝?`.obsidian` 閰嶇疆鏂囦欢锛岃€屼笉鏄悓姝ユ暣涓?Obsidian 鍐呴儴鐩綍銆?

## 鏂扮瑪璁板簱榛樿鍚屾浠€涔?

鏂扮瑪璁板簱浼氬緱鍒拌繖缁勮捣姝?allowlist锛?

- 涓婚锛歚.obsidian/themes/**`
- CSS snippets锛歚.obsidian/snippets/**`
- 蹇嵎閿細`.obsidian/hotkeys.json`
- 搴旂敤鍋忓ソ锛歚.obsidian/app.json`
- 澶栬鍋忓ソ锛歚.obsidian/appearance.json`
- 宸插惎鐢ㄧぞ鍖烘彃浠跺垪琛細`.obsidian/community-plugins.json`
- 宸插惎鐢ㄦ牳蹇冩彃浠跺垪琛細`.obsidian/core-plugins.json`

杩欓噷浠呭寘鍚凡鍚敤鎻掍欢鍒楄〃銆傛彃浠朵唬鐮佸拰鎻掍欢璁剧疆榛樿涓嶄細鍚屾銆?

宸叉湁绗旇搴撲細淇濇寔绌?allowlist锛岀洿鍒颁綘搴旂敤璧锋娓呭崟銆?

- **Admin WebUI锛歏aults -> Settings -> Apply starter allowlist** 浼氬啓鍏ヤ笂杩板畬鏁寸殑 7 鏉?glob 璧锋娓呭崟銆?
- **Obsidian 鎻掍欢锛歋ettings -> PKV Sync -> Apply recommended starter list** 鍙啓鍏ユ渶瀹夊叏鐨勪袱鏉?glob锛坄.obsidian/themes/**` 鍜?`.obsidian/snippets/**`锛夆€斺€斾富棰樺拰 CSS snippet 璺ㄨ澶囧叡浜€氬父鏄畨鍏ㄧ殑锛岃€屽彟澶栦簲鏉?glob 娑夊強鐢ㄦ埛鐗瑰畾鐨勫簲鐢ㄧ姸鎬侊紝鎻掍欢涓嶄細鍦ㄦ病鏈夋槑纭喅瀹氱殑鎯呭喌涓嬪惎鐢ㄥ畠浠€?

濡傛灉鎯宠瀹屾暣鐨?7 鏉?glob 璧锋娓呭崟锛岃浣跨敤 Admin WebUI 鎸夐挳锛屾垨鑰呮妸杩欎簺 glob 鎵嬪姩绮樿创鍒版彃浠剁殑 allowlist 缂栬緫鍣ㄤ腑銆?

## 姘镐笉鍚屾

浠ヤ笅纭帓闄ゅ缁堜紭鍏堬紝鍗充娇浣犳妸瀹冧滑鍔犲叆 allowlist 涔熶笉浼氬悓姝ワ細

- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.git/**`
- `.trash/**`
- `.conflict-*`
- `*.lock`
- `*.tmp`

## 杩涢樁 opt-in

浣犲彲浠ユ坊鍔犻澶?glob锛屼絾闇€瑕佽嚜琛屾壙鎷呴闄╋細

- `.obsidian/plugins/*/data.json`锛氭彃浠惰缃€傝繖閲屽彲鑳藉寘鍚?API key銆丱Auth token 鎴?LLM key銆傚湪绔埌绔姞瀵嗚惤鍦板墠锛屽悓姝ュ唴瀹逛細浠ユ槑鏂囧瓨鏀惧湪鏈嶅姟绔€?
- `.obsidian/plugins/**`锛氭彃浠朵唬鐮併€傝繖浼氳 Git 鍘嗗彶蹇€熻啫鑳€锛屽苟涓旀闈笓鐢ㄦ彃浠跺悓姝ュ埌绉诲姩绔椂鍙兘鏃犳硶杩愯銆?
- 鍏朵粬闅愯棌鐩綍锛屼緥濡?`.claude/**` 鎴?`.codex/**`锛歛gent 鐘舵€佸彲鑳藉寘鍚晱鎰熺殑鏈湴涓婁笅鏂囥€?

## 鍦ㄥ摢閲岀紪杈戣鍒?

- Obsidian锛氭墦寮€ **璁剧疆 -> PKV Sync**锛岄€夋嫨褰撳墠绗旇搴擄紝缂栬緫 **.obsidian 鍚屾瑙勫垯**锛岀劧鍚庝繚瀛樸€?
- Admin WebUI锛氭墦寮€ **Vaults**锛岀偣鍑绘煇涓瑪璁板簱鐨?**Settings**锛岀紪杈?allowlist锛岀劧鍚庝繚瀛樸€?
