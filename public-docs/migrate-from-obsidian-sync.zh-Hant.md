# 寰?Obsidian Sync 閬风Щ

[English](./migrate-from-obsidian-sync.md) | [绠€浣撲腑鏂嘳(./migrate-from-obsidian-sync.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./migrate-from-obsidian-sync.ja.md) | [頃滉淡鞏碷(./migrate-from-obsidian-sync.ko.md)

鏂囦欢鐗堟湰锛歷1.4.3銆?

鏈枃瑾槑濡備綍鎶婂凡缍撲娇鐢?Obsidian Sync 鐨?Obsidian 绛嗚搴洰鍓嶆獢妗堝尟鍏ュ埌鏂扮殑 PKV Sync 绛嗚搴€?

閬风Щ鍙尟鍏ョ洰鍓嶈缃笂鐝炬湁鐨勬獢妗堛€傚畠涓嶆渻鍖叆 Obsidian Sync 姝峰彶銆侀仩绔増鏈鍙层€佸凡鍒櫎妾旀姝峰彶鎴栬绐佷腑绻艰硣鏂欍€侾KV Sync 鐨勬鍙叉渻寰炲缓绔嬫柊 PKV 绛嗚搴殑閬风Щ鎻愪氦闁嬪銆?

閬风Щ涔熶笉鏈冨仠鐢ㄣ€佽В闄ゅ畨瑁濇垨淇敼 Obsidian Sync銆傜⒑瑾?PKV Sync 绲愭灉涔嬪緦锛屽鏋滀綘鎯冲仠姝娇鐢?Obsidian Sync锛岃珛鍦?Obsidian 涓墜鍕曢棞闁夈€?

## 闁嬪涔嬪墠

- 鍏堢瓑寰?Obsidian Sync 鍦ㄧ敤鏂奸伔绉荤殑瑁濈疆涓婂畬鎴愬悓姝ャ€?
- 閬风Щ鍓嶆墜鍕曞倷浠芥暣鍊嬬瓎瑷樺韩璩囨枡澶俱€?
- 濡傛湁鍙兘锛屽尟鍏ユ湡闁撲繚鎸?Obsidian 闂滈枆锛屾垨鑷冲皯涓嶈绶ㄨ集妾旀銆?
- 鍏堝缓绔嬫垨纰鸿獚鐩 PKV Sync 鏈嶅嫏绔赋铏熴€?

## 鏈冨尟鍏ヤ粈楹?

PKV Sync 鏈冨缓绔嬩竴鍊嬫柊绛嗚搴紝涓︽妸鐩墠鍖叆鍏у浣滅偤绗竴姊?PKV 姝峰彶鎻愪氦銆?

鏅€?Markdown 妾旀銆侀檮浠跺拰甯歌绛嗚搴獢妗堟渻琚尟鍏ワ紝闄ら潪瀹冨€戝懡涓?PKV Sync 鐨勫挤鍒舵帓闄よ鍓囥€?

## 鏈冭烦閬庝粈楹?

鍖叆鍣ㄦ渻璺抽亷 Obsidian Sync 鍏ч儴妾旀銆丳KV Sync 澶栨帥鑷韩鐙€鎱嬨€丱S 鍨冨溇妾旀浠ュ強鏈鍩疯闅庢妾旀锛屽寘鎷細

- `.obsidian/sync/`
- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.obsidian/plugins/pkv-sync/`锛堝鎺涜嚜韬殑瑷畾鑸?token store 鍍呬繚鐣欏湪鏈锛?
- `.trash/**`
- `.git/**`
- `.DS_Store`锛坢acOS锛?
- `Thumbs.db`锛圵indows锛?
- `*.tmp`銆乣*.lock` 绛夋毇瀛樻獢妗?
- 瑁濈疆灏堝爆鐨勫伐浣滃崁銆佸揩鍙栥€佸洖鏀剁珯鍜屾毇瀛樻獢妗?

閮ㄥ垎 `.obsidian` 瑷畾妾斾箣寰屽彲浠ラ€忛亷鎸夌瓎瑷樺韩 `.obsidian` allowlist 鍚屾銆傜浉闂滆鍓囪珛闁辫畝 `.obsidian` 瑷畾鍚屾鎸囧崡銆?

## 閬风Щ涔嬪緦

鍦ㄥ彟涓€鍙拌缃笂闁嬪暉鏂扮殑 PKV 绛嗚搴紝纰鸿獚绛嗚鍜岄檮浠剁湅璧蜂締姝ｇ⒑銆傛鏌ュ畬鎴愬墠锛岃珛淇濈暀鎵嬪嫊鍌欎唤銆?

濡傛灉浣犵辜绾岃畵 Obsidian Sync 鍜?PKV Sync 浣跨敤鍚屼竴鍊嬭硣鏂欏ぞ锛岃珛璎规厧淇敼妾旀銆傚叐鍊嬪悓姝ョ郴绲卞彲鑳芥渻鍚屾檪鎿嶄綔鍚屼竴鎵规獢妗堬紝鑰?PKV Sync 鍙渻瑷橀寗閬风Щ鎻愪氦涔嬪緦鏀跺埌鐨勮畩鏇淬€?
