# Obsidian Sync 銇嬨倝绉昏銇欍倠

[English](./migrate-from-obsidian-sync.md) | [绠€浣撲腑鏂嘳(./migrate-from-obsidian-sync.zh-CN.md) | [绻侀珨涓枃](./migrate-from-obsidian-sync.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./migrate-from-obsidian-sync.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.5銆?

銇撱伄鏂囨浉銇姊扮炕瑷炽伀銈堛倠鍒濈増銇с仚銆傚叕闁嬪墠銇儘銈ゃ儐銈ｃ儢瑭辫€呫伀銈堛倠銉儞銉ャ兗銈掓帹濂ㄣ仐銇俱仚銆?

銇撱伄銈偆銉夈仹銇€併仚銇с伀 Obsidian Sync 銈掍娇銇ｃ仸銇勩倠 Obsidian vault 銇従鍦ㄣ伄銉曘偂銈ゃ儷銈掋€佹柊銇椼亜 PKV Sync vault 銇彇銈婅炯銈€鏂规硶銈掕鏄庛仐銇俱仚銆?

绉昏銇с伅銆併亾銇缃伀鐝惧湪瀛樺湪銇欍倠銉曘偂銈ゃ儷銇犮亼銈掑彇銈婅炯銇裤伨銇欍€侽bsidian Sync 銇饱姝淬€併儶銉兗銉堛伄銉愩兗銈搞儳銉冲饱姝淬€佸墛闄ゆ笀銇裤儠銈°偆銉伄灞ユ銆佺鍚堛儭銈裤儑銉笺偪銇彇銈婅炯銇裤伨銇涖倱銆侾KV Sync 銇饱姝淬伅銆佹柊銇椼亜 PKV vault 銈掍綔鎴愩仚銈嬬Щ琛?commit 銇嬨倝濮嬨伨銈娿伨銇欍€?

绉昏銇?Obsidian Sync 銈掔劇鍔瑰寲銆併偄銉炽偆銉炽偣銉堛兗銉€佸鏇淬仐銇俱仜銈撱€侾KV Sync 銇祼鏋溿倰纰鸿獚銇椼仧銇傘仺銇?Obsidian Sync 銈掑仠姝仐銇熴亜鍫村悎銇€丱bsidian 銇ф墜鍕曘仹銈儠銇仐銇︺亸銇犮仌銇勩€?

## 濮嬨倎銈嬪墠銇?

- 绉昏銇娇銇嗚缃仹 Obsidian Sync 銇悓鏈熴亴瀹屼簡銇欍倠銇俱仹寰呫仭銇俱仚銆?
- 绉昏鍓嶃伀 vault 銉曘偐銉儉銉笺倰鎵嬪嫊銇с儛銉冦偗銈儍銉椼仐銇俱仚銆?
- 鍙兘銇с亗銈屻伆銆佸彇銈婅炯銇夸腑銇?Obsidian 銈掗枆銇樸倠銇嬨€佸皯銇亸銇ㄣ倐銉曘偂銈ゃ儷绶ㄩ泦銈掗伩銇戙伨銇欍€?
- 绉昏鍏堛伄 PKV Sync 銈点兗銉愩兗銈偒銈︺兂銉堛倰鍏堛伀浣滄垚銇俱仧銇⒑瑾嶃仐銇俱仚銆?

## 鍙栥倞杈笺伨銈屻倠銈傘伄

PKV Sync 銇柊銇椼亜 vault 銈掍綔鎴愩仐銆佺従鍦ㄣ伄鍙栥倞杈笺伩鍐呭銈掓渶鍒濄伄 PKV 灞ユ entry 銇ㄣ仐銇?commit 銇椼伨銇欍€?

閫氬父銇?Markdown 銉曘偂銈ゃ儷銆佹坊浠樸儠銈°偆銉€佷竴鑸殑銇?vault 銉曘偂銈ゃ儷銇€丳KV Sync 銇挤鍒堕櫎澶栥伀涓€鑷淬仐銇亜闄愩倞鍙栥倞杈笺伨銈屻伨銇欍€?

## 銈广偔銉冦儣銇曘倢銈嬨倐銇?

銈ゃ兂銉濄兗銈裤兗銇?Obsidian Sync 銇唴閮ㄣ儠銈°偆銉€丳KV Sync plugin 鑷韩銇姸鎱嬨€丱S 銇偢銉ｃ兂銈儠銈°偆銉€併儹銉笺偒銉疅琛屾檪銉曘偂銈ゃ儷銈掋偣銈儍銉椼仐銇俱仚銆備緥锛?

- `.obsidian/sync/`
- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.obsidian/plugins/pkv-sync/`锛坧lugin 鑷韩銇ō瀹氥仺 token store 銇儹銉笺偒銉檺瀹氥仹淇濇寔銇曘倢銇俱仚锛?
- `.trash/**`
- `.git/**`
- `.DS_Store`锛坢acOS锛?
- `Thumbs.db`锛圵indows锛?
- `*.tmp` 銈?`*.lock` 銇仼銇竴鏅傘儠銈°偆銉?
- 瑁呯疆鍥烘湁銇?workspace銆乧ache銆乼rash銆佷竴鏅傘儠銈°偆銉?

閬告姙銇椼仧 `.obsidian` 瑷畾銉曘偂銈ゃ儷銇€併亗銇ㄣ仹 vault 銇斻仺銇?`.obsidian` allowlist 銇嬨倝鍚屾湡銇с亶銇俱仚銆傝┏銇椼亜瑕忓墖銇?`.obsidian` 瑷畾鍚屾湡銈偆銉夈倰鍙傜収銇椼仸銇忋仩銇曘亜銆?

## 绉昏寰?

鍒ャ伄瑁呯疆銇ф柊銇椼亜 PKV vault 銈掗枊銇嶃€併儙銉笺儓銇ㄦ坊浠樸儠銈°偆銉亴姝ｃ仐銇忚銇堛倠銇撱仺銈掔⒑瑾嶃仐銇俱仚銆傜⒑瑾嶃亴绲傘倧銈嬨伨銇с€佹墜鍕曘儛銉冦偗銈儍銉椼伅淇濇寔銇椼仸銇忋仩銇曘亜銆?

Obsidian Sync 銇?PKV Sync 銈掑悓銇樸儠銈┿儷銉€銉笺仹鍕曘亱銇楃稓銇戙倠鍫村悎銇€佹厧閲嶃伀澶夋洿銇椼仸銇忋仩銇曘亜銆? 銇ゃ伄鍚屾湡銈枫偣銉嗐儬銇屽悓銇樸儠銈°偆銉仹绔跺悎銇欍倠鍙兘鎬с亴銇傘倞銆丳KV Sync 銇Щ琛?commit 浠ュ緦銇彈銇戝彇銇ｃ仧澶夋洿銇犮亼銈掕閷层仐銇俱仚銆?
