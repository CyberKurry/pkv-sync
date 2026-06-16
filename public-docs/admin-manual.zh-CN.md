# PKV Sync 绠＄悊鍛樻墜鍐?

[English](./admin-manual.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./admin-manual.zh-Hant.md) | [鏃ユ湰瑾瀅(./admin-manual.ja.md) | [頃滉淡鞏碷(./admin-manual.ko.md)

鏂囨。鐗堟湰锛歷1.4.3銆?

鏈枃瑕嗙洊鑷墭绠?PKV Sync 鏈嶅姟绔殑鏃ュ父绠＄悊銆傜綉缁滃拰涓绘満鍔犲浐璇峰悓鏃堕槄璇婚儴缃插姞鍥烘寚鍗椼€?

## 棣栨杩愯

1. 鐢熸垚閮ㄧ讲瀵嗛挜锛?

   ```bash
   pkvsyncd genkey
   ```

2. 鍩轰簬 `config.example.toml` 鍒涘缓 `/etc/pkv-sync/config.toml`銆?
3. 涓哄叏鏂扮殑 1.x 鏁版嵁鐩綍鍒濆鍖?v1 鏁版嵁搴撳熀绾匡細

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. 鍚姩鏈嶅姟绔細

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. 鍏ㄦ柊鏁版嵁搴撻娆″惎鍔ㄥ悗锛屽湪娴忚鍣ㄦ墦寮€ `/setup`锛屽垱寤虹涓€涓鐞嗗憳璐﹀彿銆侾KV Sync 涓嶅啀鎶婇殢鏈虹鐞嗗憳瀵嗙爜杈撳嚭鍒?stderr 鎴栧鍣ㄦ棩蹇椼€?
6. setup 瀹屾垚鍚庯紝鏃ュ父绠＄悊鍛樼櫥褰曚娇鐢?`/admin/login`銆?

PKV Sync 1.0 浣跨敤鍗曚釜 v1 SQLite 鍩虹嚎銆傜敱 0.x 鍒涘缓鐨勬暟鎹簱涓嶆敮鎸佸師鍦板崌绾у埌 1.0.0锛涜鎸?[`upgrade-notes-v1.0.zh-CN.md`](./upgrade-notes-v1.0.zh-CN.md) 鎿嶄綔銆傚湪杩欐 v1 鍩虹嚎涔嬪悗锛屽凡鍙戝竷鐨?1.x migration 淇濇寔杩藉姞寮忋€?

## Admin Web 闈㈡澘

鎵撳紑锛?

```text
https://sync.example.com/admin/login
```

绠＄悊鍚庡彴鍖呭惈锛?

- 浠〃鐩橈細绯荤粺銆佸瓨鍌ㄣ€佺瑪璁板簱銆佺敤鎴枫€佹渶杩戞椿鍔ㄦ寚鏍囷紝浠ュ強鏈夋柊鐗?PKV Sync 鏃剁殑鏇存柊鎻愮ず
- 鐢ㄦ埛鍒楄〃锛屾敮鎸佹悳绱㈠拰鐘舵€佺瓫閫?
- 鐢ㄦ埛璇︽儏椤碉細閲嶇疆瀵嗙爜銆佸惎鐢?绂佺敤銆佺鐞嗗憳鏉冮檺鎺у埗鍜?token 鏌ョ湅
- 鍏ㄥ眬璁惧 token 椤甸潰锛屽彲鍒楀嚭銆佸垱寤哄拰鎾ら攢 token
- 绗旇搴撳崱鐗囷細鎵€鏈夎€呫€佹枃浠舵暟銆佸ぇ灏忋€佷笂娆″悓姝ャ€佸厓鏁版嵁淇銆佸垹闄ゆ搷浣滃拰鎸夌瑪璁板簱鍚屾璁剧疆
- 鍙绗旇搴撴枃浠舵祻瑙堝櫒锛屾敮鎸佹枃浠堕瑙堛€佸崟鏂囦欢鍘嗗彶鏃堕棿绾垮拰 unified diff 娓叉煋
- 閭€璇风爜鍒涘缓锛屽彲閫夎繃鏈熸椂闂达紝娲昏穬閭€璇风爜鍒楄〃锛屼互鍙婂垹闄ゆ湭浣跨敤閭€璇风爜
- 杩愯鏃惰缃紝鍒嗕负 General銆丼ecurity銆丼ync & Storage銆丯etwork锛屽苟鍖呭惈鏇存柊妫€鏌ュ紑鍏冲拰闂撮殧
- 娲诲姩鏃ュ織锛屾敮鎸佹寜鐢ㄦ埛鍜屽姩浣滅湡瀹炵瓫閫?push/pull 浠ュ強绗旇搴撶敓鍛藉懆鏈熻褰?
- Blob 鍨冨溇鍥炴敹瑙﹀彂
- 鑻辨枃銆佺畝浣撲腑鏂囥€佺箒浣撲腑鏂囥€佹棩鏂囧拰闊╂枃璇█鍒囨崲

鍦?1.2.1 涓紝鐢ㄦ埛璇︽儏缁熻鏉ヨ嚜鐪熷疄鐨?vault 鏁伴噺鍜屾渶鍚庡悓姝ユ椂闂存埑锛屾椂闀挎爣绛惧凡瑕嗙洊鎵€鏈夐殢鐗堟湰鍙戝竷鐨?Admin 璇█锛屾湰鐗堜篃浼氬湪鍙敤鏃朵娇鐢ㄥ閲忔垨鎵瑰鐞嗚矾寰勬墽琛?reconciliation 涓庡厓鏁版嵁淇銆?

鏃堕棿鎴炽€佹寔缁椂闂淬€佸瓧鑺傚ぇ灏忋€佽繍琛屾椂闀垮拰娲诲姩鏁版嵁閮戒細浠ヤ汉绫诲彲璇诲舰寮忔樉绀恒€傞粯璁ゆ椂鍖烘槸 `Asia/Shanghai`锛屽彲鍦ㄨ缃腑淇敼銆?

## 鏇存柊閫氱煡

PKV Sync 榛樿姣?24 灏忔椂妫€鏌ヤ竴娆?GitHub release銆傚彂鐜版柊鐨勬湇鍔＄鐗堟湰鏃讹紝浠〃鐩樹細鏄剧ず鎻愮ず锛屽寘鍚綋鍓嶇増鏈€佹渶鏂扮増鏈€佸彂琛岃鏄庨摼鎺ュ拰绠€鐭憳瑕併€?

`config.toml` 涓殑 `[update_check].enabled` 鍜?`[update_check].interval_seconds` 鍙湪鍏ㄦ柊鏁版嵁搴撻娆″惎鍔ㄦ椂鍐欏叆杩愯鏃惰缃€備箣鍚庝互 Admin WebUI 鐨?Settings 椤甸潰涓哄噯锛氬湪 **Network** 鍒嗗尯鍒囨崲鏇存柊妫€鏌ユ垨璋冩暣闂撮殧锛屽悗鍙颁换鍔′細鍦ㄤ笅涓€杞鍙栨柊鐨勮繍琛屾椂鍊笺€傚鏋滃綋鍓嶅凡鍏抽棴鏇存柊妫€鏌ワ紝閲嶆柊寮€鍚悗绾?60 绉掑唴鐢熸晥銆俙[update_check].repo` 浠嶄繚鐣欎负闈欐€?`config.toml` 瀛楁锛屼緵绂荤嚎闀滃儚閮ㄧ讲浣跨敤銆?

```toml
[update_check]
enabled = false
interval_seconds = 86400
repo = "cyberkurry/pkv-sync"
```

鏇存柊妫€鏌ュ彧鎻愪緵淇℃伅銆侾KV Sync 涓嶄細鑷姩鏇挎崲姝ｅ湪杩愯鐨勬湇鍔＄浜岃繘鍒舵垨瀹瑰櫒闀滃儚銆?

## 鐢ㄦ埛绠＄悊

- 鍙湪 **Users** 椤甸潰鎴?CLI 鍒涘缓鐢ㄦ埛銆?
- 鐢ㄦ埛鍚嶅繀椤绘槸 3-32 涓?ASCII 瀛楁瘝銆佹暟瀛椼€乣_`銆乣-` 鎴?`.`銆?
- 绠＄悊鍛樺垱寤恒€佺鐞嗗憳閲嶇疆銆佸叕寮€娉ㄥ唽鍜岀敤鎴疯嚜琛屼慨鏀圭殑瀵嗙爜閮藉繀椤昏嚦灏?12 涓瓧绗︼紝骞跺寘鍚ぇ鍐欏瓧姣嶃€佸皬鍐欏瓧姣嶅拰鏁板瓧銆?
- 鐢ㄦ埛椤甸潰鐨勬悳绱㈠拰鐘舵€佺瓫閫夊彲浠ョ缉灏忚〃鏍艰寖鍥淬€?
- 鎵撳紑鐢ㄦ埛璇︽儏椤靛彲閲嶇疆瀵嗙爜銆佸惎鐢ㄦ垨绂佺敤璐﹀彿銆佹彁鍗囨垨闄嶄綆绠＄悊鍛樻潈闄愶紝骞舵煡鐪嬭鐢ㄦ埛鐨勮澶?token銆?
- 濡傛灉鍚庣画鍙兘闇€瑕佸璁″巻鍙诧紝浼樺厛绂佺敤鐢ㄦ埛鑰屼笉鏄垹闄ょ敤鎴枫€?
- Admin WebUI 浼氬湪绂佺敤鐢ㄦ埛鎴栭檷绾х鐞嗗憳鍓嶅脊鍑虹‘璁ゃ€傜鐢ㄨ嚜宸辩殑绠＄悊鍛樹細璇濄€侀檷绾ф渶鍚庝竴涓鐞嗗憳浼氳鎷︽埅锛屽苟鍦ㄧ敤鎴疯鎯呴〉鏄剧ず鏈湴鍖栧弽棣堛€?
- 涓嶈鎶婃墍鏈夊墿浣欑鐞嗗憳璐﹀彿閮界鐢ㄣ€?

浠?Admin WebUI 閲嶇疆瀵嗙爜浼氭挙閿€璇ョ敤鎴峰凡鏈夎澶?token銆傜敤鎴烽渶瑕侀噸鏂扮櫥褰曘€?

CLI 鍏滃簳鍛戒护锛?

```bash
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user add alice --admin
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

## 璁惧 Token

璁惧 bearer token 浼氬湪璁よ瘉璇锋眰鏃剁画鏈燂紝杩炵画 90 澶╂湭浣跨敤鎵嶄細杩囨湡锛屼笖鍗曚釜 token 鏈€闀挎湁鏁?365 澶┿€傜敤鎴峰彲浠ユ挙閿€鑷繁鐨?token锛岀鐞嗗憳鍙互鎾ら攢浠绘剰鐢ㄦ埛鐨?token銆?

杩愮淮娉ㄦ剰浜嬮」锛?

- Token 鏄庢枃鍙湪鍒涘缓鏃跺睍绀轰竴娆°€?
- 鏁版嵁搴撳彧淇濆瓨 SHA-256 token hash銆?
- 绠＄悊鍛?token 鍒楄〃鎺ュ彛鍜岃〃鏍煎彧灞曠ず鍏紑 token 鍏冩暟鎹紝涓嶈繑鍥炴槑鏂?token锛屼篃涓嶈繑鍥炲唴閮ㄨ繃鏈熸垨鎾ら攢瀛楁銆?
- 姣忔璁よ瘉璇锋眰閮戒細鎶?token 杩囨湡鏃堕棿寤堕暱鍒拌璇锋眰鏃堕棿涔嬪悗 90 澶╋紝浣嗕笉浼氳秴杩?token 鍒涘缓鍚?365 澶┿€?
- 鍚屼竴绋冲畾鎻掍欢璁惧 ID 鍐嶆鐧诲綍鏃讹紝浼氭浛鎹㈣璁惧鏃х殑娲昏穬 token銆?
- 琚椿鍔ㄨ褰曞紩鐢ㄧ殑宸叉挙閿€ token 鍙互娓呯悊锛屽悓鏃朵繚鐣欐椿鍔ㄥ巻鍙层€?

## 绗旇搴?

浠?Admin WebUI 鍒犻櫎绗旇搴撻渶瑕侀澶栫‘璁ゅ脊绐椼€傚嵆浣挎湭寮曠敤鐨?blob 鍙兘瑕佺瓑鍨冨溇鍥炴敹鍚庢墠娓呯悊锛屼篃搴旀妸鍒犻櫎瑙嗕负鐮村潖鎬ф搷浣溿€?

鍒犻櫎绗旇搴撲細绉婚櫎锛?

- 绗旇搴撴暟鎹簱琛?
- 浠庤琛岀骇鑱旂殑鐩稿叧鍏冩暟鎹?
- `data_dir/vaults/<vault-id>` 涓嬬殑鍚庣瑁?Git 浠撳簱
- 鍐呭瓨涓殑鎸夌瑪璁板簱 push 閿?

Blob 鏂囦欢鏄唴瀹瑰鍧€鐨勶紝鍙兘浼氫繚鐣欏埌鍨冨溇鍥炴敹纭鍏惰秴杩囧闄愭湡涓斾笉鍐嶈寮曠敤銆?

濡傛灉涓柇鎿嶄綔鍚庢枃浠舵暟銆佸ぇ灏忔垨 blob 寮曠敤鐪嬭捣鏉ヤ笉姝ｇ‘锛屽彲浠ヤ娇鐢ㄧ瑪璁板簱鍏冩暟鎹慨澶嶃€備慨澶嶆祦绋嬩細浠?tree entry 鐩存帴璇诲彇 blob pointer hash锛屽苟鎵归噺淇 blob 寮曠敤锛屼笉鍐嶉€愪釜閲嶆柊鎵撳紑 pointer 鏂囦欢銆?

### 鎸夌瑪璁板簱鍚屾璁剧疆

鍦?**Vaults** 椤甸潰鐐瑰嚮鏌愪釜绗旇搴撳崱鐗囦笂鐨?**Settings**锛屽彲浠ョ紪杈戣绗旇搴撶殑 `extra_sync_globs` allowlist銆傚畠鎺у埗鍝簺闅愯棌璺緞锛屽寘鎷€夊畾鐨?`.obsidian` 閰嶇疆鏂囦欢锛屽彲浠ュ弬涓庡悓姝ャ€?

鏂扮瑪璁板簱浼氳嚜鍔ㄨ幏寰楁帹鑽愯捣姝?allowlist銆傚凡鏈夌瑪璁板簱淇濇寔绌洪厤缃紝鐩村埌绠＄悊鍛樻垨绗旇搴撴墍鏈夎€呭簲鐢ㄨ捣姝ユ竻鍗曘€?*Apply starter allowlist** 浼氬啓鍏ユ帹鑽愭竻鍗曪紝鍖呮嫭涓婚銆丆SS snippets銆佸揩鎹烽敭銆佸簲鐢ㄥ亸濂姐€佸瑙傚亸濂藉拰宸插惎鐢ㄦ彃浠跺垪琛ㄣ€?

### 鍙鏂囦欢鍘嗗彶

鍦?**Vaults** 椤甸潰鐐瑰嚮鏌愪釜绗旇搴撳崱鐗囦笂鐨?**Browse files**銆傛枃浠舵祻瑙堝櫒浼氬垪鍑哄綋鍓?HEAD 涓殑鏂囦欢銆佸ぇ灏忎互鍙婃枃鏈?浜岃繘鍒剁被鍨嬨€傛墦寮€鏂囦欢鍚庯紝鏂囨湰鏂囦欢浼氭樉绀哄彧璇婚瑙堬紝骞舵彁渚?**History** 鍜?**Diff with previous** 閾炬帴銆?

鍘嗗彶椤典細鍒楀嚭璇ユ枃浠剁浉鍏崇殑鎻愪氦锛屽苟鎻愪緵鈥滄煡鐪嬭鎻愪氦鏃剁殑鏂囦欢鈥濆拰瀵瑰簲 diff 鐨勯摼鎺ャ€俤iff 椤典細鎸夎娓叉煋 unified diff锛屽苟鐢ㄩ鑹插尯鍒嗘柊澧炪€佸垹闄ゅ拰 hunk銆備簩杩涘埗鏂囦欢鍙樉绀哄厓鏁版嵁锛屼笉娓叉煋浜岃繘鍒?diff 鍐呭銆傚綋鍓嶅悓姝ヨ繃婊ゅ櫒鎷掔粷鐨勮矾寰勪篃浼氫粠鏂囦欢棰勮銆乧ommit 鍒楄〃銆佸巻鍙插拰 diff 椤甸潰闅愯棌銆?

娴忚鏂囦欢銆佸巻鍙插拰 diff 浼氳褰?`view_commit`銆乣view_history` 鍜?`view_diff` 娲诲姩銆侫dmin history 涓彁渚涚瑪璁板簱 rollback 鎺у埗锛涜鍦ㄧ‘璁ょ洰鏍囨彁浜ゅ悗鍐嶄娇鐢紝鍥犱负 rollback 浼氫粠閫夊畾鍘嗗彶鐐瑰垱寤烘柊鐨勭瑪璁板簱鐘舵€併€?

## 閭€璇风爜鍜屾敞鍐?

鍙粠 **Settings** 閰嶇疆娉ㄥ唽妯″紡锛?

- `disabled`锛氬彧鍏佽绠＄悊鍛樺垱寤鸿处鍙?
- `invite_only`锛氱敤鎴蜂娇鐢ㄩ個璇风爜娉ㄥ唽
- `open`锛氫换浣曟嫢鏈夐儴缃?URL 鐨勪汉閮藉彲浠ユ敞鍐?

鍒涘缓閭€璇风爜鏃跺彲浠ュ～鍐欐湭鏉ヨ繃鏈熸椂闂淬€侫dmin WebUI 浣跨敤浜虹被鍙鏃ユ湡鏃堕棿杈撳叆锛屽唴閮ㄤ粛瀛樺偍 Unix 绉掋€傚凡浣跨敤閭€璇风爜涓嶈兘閫氳繃 admin API 鍒犻櫎锛屽簲淇濈暀鐢ㄤ簬瀹¤鍘嗗彶銆?

鍙湁鍦ㄧ煭鏃堕棿绐楀彛鎴栧叿澶囬澶栫洃鎺у拰闄愭祦鐨勫叕寮€閮ㄧ讲涓紝鎵嶅缓璁娇鐢?`open`銆?

## 杩愯鏃惰缃?

璁剧疆椤电紪杈戜繚瀛樺湪 SQLite 涓殑閰嶇疆鍊?鏀瑰姩瀵规柊璇锋眰绔嬪嵆鐢熸晥(淇濆瓨鏃跺埛鏂板唴瀛樼紦瀛?銆?

**閫氱敤** 鈥?鏈嶅姟鍚嶇О銆侀粯璁ゆ椂鍖恒€乣enable_metrics` 鎸囨爣寮€鍏炽€傚紑鍚悗 `/metrics` 鍙敤锛屼絾浠嶉渶瑕侀儴缃插瘑閽ヤ腑闂翠欢銆佹彃浠?User-Agent guard 鍜岀鐞嗗憳 bearer token銆?

**瀹夊叏** 鈥?娉ㄥ唽妯″紡(`disabled` / `invite_only` / `open`)銆佺櫥褰曞け璐ラ槇鍊笺€佸け璐ョ獥鍙ｅ拰閿佸畾鏃堕暱銆傜櫥褰曢€熺巼闄愬埗鍣ㄥ悓鏃剁粺璁″凡澶辫触娆℃暟鍜屽湪閫斿瘑鐮侀獙璇?骞跺彂鏆村姏灏濊瘯鏃犳硶缁曡繃闃堝€笺€傝璇佸悓姝?API 璺敱鍙︽湁鍥哄畾绐楀彛闄愭祦锛氭寜璺敱銆佹柟娉曘€佸鎴风 IP 鍜?bearer 璁惧 token 鍒嗘《锛屾瘡 60 绉掓渶澶?600 娆¤姹傘€傚け璐ョ殑 bearer token 璁よ瘉灏濊瘯涔熶細鎸夊鎴风 IP 闄愭祦锛屾瘡 60 绉掓渶澶?120 娆★紝鍥犳杞崲浼€?token 涓嶈兘缁曡繃澶辫触棰勭畻銆?

**鍚屾涓庡瓨鍌?*
- 鏈€澶ф枃浠跺ぇ灏?榛樿 `100 MiB`)銆侭lob 涓婁紶璇锋眰浣撳缁堜細琚‖瀛樺偍涓婇檺闄愬埗锛堢敓浜х幆澧?`512 MiB`锛夛紝鍗充娇杩愯鏃惰缃璋冨緱鏇撮珮
- 鏀寔鐨勬枃鏈墿灞曞悕 鈥?鍒楄〃澶栫殑鏂囦欢鎸変簩杩涘埗 blob 澶勭悊銆侫dmin WebUI 涓鍒楄〃鍙灞曠ず锛涘闇€淇敼锛岃閫氳繃 `text_extensions` 杩愯鏃堕厤缃锛堟垨鐩存帴缂栬緫 SQLite `runtime_config` 琛級璋冩暣
- 棰濆 exclude glob 鈥?绠＄悊鍛樺彲璋?琛ュ厖鍐呯疆鐨?`.obsidian/`銆乣.trash/`銆乣.conflict-*`銆乣.git/` 鎺掗櫎娓呭崟
- 鍘嗗彶鐣岄潰鍜?diff 绔偣寮€鍏?
- **鏂囨湰鑷姩鍚堝苟**锛坄enable_auto_merge`锛岄粯璁ゅ紑鍚級锛氬惎鐢ㄥ悗锛屾湇鍔＄鍦ㄥ啓鍏ュ啿绐佹枃浠朵箣鍓嶄細鍏堝皾璇曚笁鏂规寜琛屽悎骞躲€備笉鐩镐氦鐨勭紪杈戜細骞插噣鍚堝苟锛涢噸鍙犵紪杈戜粛浼氱敓鎴愬甫鍚堝苟鏍囪鐨勫啿绐佹枃浠?
- **Push 鍘绘姈**(`push_debounce_ms`,榛樿 `250`):鏈湴缂栬緫绋冲畾鍒版帹閫佷箣闂寸殑寤惰繜銆傚彉灏忓彲缂╃煭绔埌绔欢杩?鍙樺ぇ鍙瘡娆?push 鍚堝苟鏇村鎸夐敭
- **SSE 鍐呰仈鍐呭涓婇檺**(`inline_content_max_bytes`,榛樿 `8192`,涓婇檺 `65536`):姝ゅ昂瀵镐互鍐呯殑鏂囨湰鍙樻洿闅?SSE 浜嬩欢鐩存帴涓嬪彂,鎺ユ敹绔彃浠舵棤闇€鍐?pull;瓒呰繃鍒欓檷绾ц蛋 pull
- **SSE 蹇冭烦**(`sse_heartbeat_seconds`,榛樿 `30`):浜嬩欢娴佷繚娲?閬垮厤绌洪棽 SSE 杩炴帴琚弽鍚戜唬鐞嗗垏鏂€傚苟鍙?SSE 璁㈤槄榛樿鎸夌敤鎴烽檺鍒朵负 16锛屽苟淇濈暀 1024 鐨勫叏灞€涓婇檺銆傚凡鎵撳紑鐨勪簨浠舵祦浼氬懆鏈熸€у鏌?bearer token锛泃oken 琚挙閿€鎴栬处鍙疯绂佺敤鍚庝細鍏抽棴銆?
- **Git smart HTTP**(`enable_git_smart_http`,榛樿鍏?:寮€鍚悗鎺堟潈璁惧鍙?`git clone https://_:<token>@host/git/<vault-id>`銆傛湇鍔″櫒杩橀渶瑕?`PATH` 涓湁 `git` 浜岃繘鍒?鍏紑鐨?`/api/config` 鑳藉姏涓や釜鏉′欢閮芥弧瓒虫墠鏄剧ず涓哄彲鐢?

**缃戠粶涓庢洿鏂版鏌?* 鈥?`public_host`銆佺洃鍚湴鍧€銆佸彲淇′唬鐞嗕互鍙?`[update_check].repo` 鍦ㄥ惎鍔ㄦ椂浠?`config.toml` 璇诲彇銆傛洿鏂版鏌ョ殑鍚敤鐘舵€佸拰闂撮殧鏄繚瀛樺湪 SQLite 涓殑杩愯鏃惰缃紱鍏佽鑼冨洿涓?60 绉掑埌 30 澶┿€?

## 娲诲姩鏃ュ織

娲诲姩鏃ュ織璁板綍 push銆乸ull銆乧reate_vault銆乨elete_vault銆乿iew_commit銆乿iew_history銆乿iew_diff 绛夊悓姝ャ€佺瑪璁板簱鐢熷懡鍛ㄦ湡涓庡彧璇绘祻瑙堟搷浣滐紝鍖呮嫭锛?

- 鐢ㄦ埛
- 绗旇搴?
- 鍔ㄤ綔
- 璁惧鍚?
- 鏂囦欢鏁?
- 瀛楄妭澶у皬
- 瀹㈡埛绔?IP
- User-Agent
- 璇︽儏
- 鏃堕棿鎴?

浣跨敤娲诲姩绛涢€夊彲浠ユ鏌ョ壒瀹氱敤鎴锋垨鎿嶄綔绫诲瀷銆?

`create_vault` 鍜?`delete_vault` 鏉ヨ嚜绠＄悊闈㈡澘銆佹彃浠跺拰 API 鐨勭瑪璁板簱鍒涘缓锛忓垹闄ゆ搷浣溿€?

## 鍒嗕韩鏈嶅姟绔?URL

鍒嗕韩鏈嶅姟绔垨 Admin WebUI 鎻愪緵鐨?URL锛?

```text
https://sync.example.com/k_xxx/
```

璇锋妸瀹冭涓烘晱鎰熶俊鎭€傚畠涓嶆槸鐢ㄦ埛瀵嗙爜锛屼絾鍖呭惈閮ㄧ讲瀵嗛挜锛屾槸鎻掍欢 API 娴侀噺鐨勭涓€閬撻璁よ瘉鍏ュ彛銆?

## 鍗囩骇 PKV Sync

浜岃繘鍒堕儴缃插彲鍏堣繍琛?`pkvsyncd upgrade --dry-run` 棰勮鏈€鏂?release銆佺洰鏍囪祫浜у拰鏃佽矾鍐欏叆璺緞銆傝繍琛?`pkvsyncd upgrade --yes` 浼氭妸鏍￠獙鍚庣殑 release 浜岃繘鍒朵笅杞藉埌褰撳墠鍙墽琛屾枃浠舵梺杈圭殑 `pkvsyncd.new`锛圵indows 涓?`pkvsyncd.new.exe`锛夈€傚懡浠や細鏍规嵁 `SHA256SUMS` 鏍￠獙 SHA-256锛屽苟鎵撳嵃 systemd锛忔墜鍔ㄦ浛鎹㈡楠わ紱瀹冧笉浼氱儹鏇挎崲姝ｅ湪杩愯鐨勮繘绋嬨€?

浣跨敤 `pkvsyncd upgrade --version 1.4.3` 鍙互鎸囧畾 release銆傝嫢鍛戒护鎵句笉鍒板尮閰嶈祫浜ф垨鏍￠獙鍜岋紝璇锋墜鍔ㄤ粠 GitHub release 涓嬭浇锛屽苟鑷鏍￠獙 `SHA256SUMS`銆?

瀵逛簬 0.x 閮ㄧ讲锛屼笉瑕佹妸 1.0 浜岃繘鍒舵垨闀滃儚鐩存帴鎸囧悜宸叉湁 `metadata.db`銆傝鍏堝浠姐€乵aterialize 鎴栧鍑虹瑪璁板簱鍐呭锛屼娇鐢ㄥ叏鏂扮殑 1.0 鏁版嵁鐩綍鍚姩鏈嶅姟锛屽啀鎶婄瑪璁板簱鍐呭瀵煎叆鎴?push 鍒版柊鏈嶅姟绔€傝瑙?[`upgrade-notes-v1.0.zh-CN.md`](./upgrade-notes-v1.0.zh-CN.md)銆?

Docker 鍜?Kubernetes 閮ㄧ讲搴旈€氳繃鎷夊彇鎴栦慨鏀瑰鍣ㄩ暅鍍?tag 鍗囩骇锛岀劧鍚庨噸鍚湇鍔℃垨 rollout銆倁pgrade CLI 妫€娴嬪埌瀹瑰櫒鐜鏃讹紝浼氳緭鍑洪暅鍍忓崌绾ф寚寮曪紝涓嶅啓鍏ユ梺璺簩杩涘埗銆?

### 鑷姩鍗囩骇锛坥pt-in锛?

涓婇潰涓ょ鏂瑰紡閮芥槸鎵嬪姩鐨勩€傝璁╁崌绾у厤鎵嬪姩鈥斺€斿湪绠＄悊闈㈡澘鏀跺埌閫氱煡鍚庝竴閿簲鐢ㄢ€斺€斿彲鍚敤**鍙€夌殑鍗囩骇鍣?*銆傛湇鍔＄鏈韩淇濇寔闈炵壒鏉冿細鐐瑰嚮 **Upgrade now** 鍙細鍚戞暟鎹洰褰曞啓鍏ヤ竴涓?`upgrade-request.json` 鏍囪锛涚敱鐙珛鐨勭壒鏉冨崌绾у櫒搴旂敤瀹冦€侀噸鍚湇鍔★紝骞跺湪鏂扮増鏈仴搴锋鏌ュけ璐ユ椂鑷姩鍥炴粴銆傚湪浣犲惎鐢ㄤ箣鍓嶏紝鍏ㄦ柊瀹夎鐨勫崌绾ц涓轰笌涓婃枃瀹屽叏涓€鑷淬€?

**systemd锛?* 浠?`deploy/updater/` 瀹夎鍗囩骇鑴氭湰涓庡崟鍏冿細

```sh
sudo install -m 0755 deploy/updater/pkv-sync-update.sh /usr/local/bin/
sudo cp deploy/updater/pkv-sync-updater.service deploy/updater/pkv-sync-updater.path /etc/systemd/system/
sudo systemctl enable --now pkv-sync-updater.path
```

root 鐨?`pkv-sync-updater.path` 鍗曞厓鐩戣璇ユ爣璁板苟杩愯涓€娆℃€х殑 `pkv-sync-updater.service`锛氬畠浼氭殏瀛樺苟 SHA-256 鏍￠獙 release 浜岃繘鍒躲€佹浛鎹㈠畠銆侀噸鍚?`pkv-sync`锛屽仴搴锋鏌ュけ璐ユ椂鍥炴粴鍒版棫浜岃繘鍒躲€傜敤 `sudo systemctl disable --now pkv-sync-updater.path` 鍏抽棴銆?

**Docker锛?* 鍚敤闅忛檮鐨?updater profile銆傚畠鍙€氳繃鍙楅檺鐨?`docker-socket-proxy` 璁块棶 Docker锛沗pkv-sync` 瀹瑰櫒鏈韩姘歌繙鎷夸笉鍒?socket锛?

```sh
docker compose -f docker-compose.yml -f deploy/updater/compose.updater.yml --profile updater up -d
```

鍗囩骇鍣ㄤ細鎷夊彇鎵€璇锋眰鐨?`X.Y.Z` 闀滃儚銆侀噸寤?`pkv-sync`銆佸鍏跺仛鍋ュ悍妫€鏌ワ紝澶辫触鏃堕噸鏂板浐瀹氬洖鏃?tag銆備綔涓烘浛浠ｏ紝浣犱篃鍙互璁?[Watchtower](https://containrrr.dev/watchtower/) 鎴?compose-updater 绛夌涓夋柟宸ュ叿鐩綇 `pkv-sync` 瀹瑰櫒鈥斺€斾絾瀹冧滑鏄寜璁″垝杞 `:latest`锛岃€岄潪閬靛惊涓€閿浐瀹氱殑鐩爣鐗堟湰銆?

鍗囩骇鏈熼棿浼氭湁鐭殏鐨勯噸鍚腑鏂紱瀹㈡埛绔細鑷姩閲嶈繛銆?

## 缁存姢娓呭崟

- 浣跨敤 `pkvsyncd backup --output <dir> [--data-dir <dir>] [--gzip]` 鐢熸垚杩愮淮蹇収銆傝緭鍑虹洰褰曞繀椤讳笉瀛樺湪鎴栦负绌猴紱鍛戒护浼氱敤 `VACUUM INTO` 蹇収 SQLite锛屽鍒?`vaults/` 鍜?`blobs/`锛屽苟鍐欏叆甯?pkvsyncd 鐗堟湰銆佺粍浠跺搱甯屻€佸ぇ灏忓拰鏁伴噺鐨?`MANIFEST.json`銆傞粯璁ゅ浠戒細鐪佺暐 `config.toml`锛涘彧鏈夊湪浣犳槑纭淇濆瓨骞朵繚鎶ら儴缃插瘑閽ュ拰鍏朵粬鏈満绉樺瘑鏃讹紝鎵嶆坊鍔?`--include-config`銆?
- 浣跨敤 `pkvsyncd restore --input <backup-dir> --data-dir <dir>` 鎭㈠鍒颁笉瀛樺湪鎴栦负绌虹殑鏁版嵁鐩綍銆傚彧鏈夌‘璁ょ洰鏍囧彲浠ュ厛娓呯┖鏃舵墠鍔?`--force`锛涙仮澶嶄細鍏堟牎楠?manifest 鍝堝笇锛屽鍒跺畬鎴愬悗鑷姩杩愯 verify銆?
- 缁存姢鍚庢垨涓绘満瀛樺偍寮傚父鍚庤繍琛?`pkvsyncd verify [--data-dir <dir>]`銆傚畠浼氭鏌ヨ寮曠敤鐨?blob 鏂囦欢锛屾姤鍛婂绔?blob锛岀敤 `git2` 鏍￠獙绗旇搴?git 浠撳簱锛屽苟鍦ㄧ己澶便€佹崯鍧忔垨 git 閿欒鏃惰繑鍥炲け璐ャ€俙--no-fail` 浼氫繚鐣欐姤鍛婁絾寮哄埗杩斿洖鎴愬姛閫€鍑虹爜銆?
- 浣跨敤 `pkvsyncd materialize <vault-id> -o <dir>` 鎶婄瑪璁板簱 HEAD 瀵煎嚭涓烘櫘閫氭枃浠舵爲锛堟枃鏈枃浠跺師鏍疯緭鍑猴紝浜岃繘鍒?blob 浠?blob 瀛樺偍瑙ｆ瀽锛夈€傞€傚悎绂荤嚎瀵煎嚭銆佷复鏃跺璁℃垨鍐疯縼绉汇€傞厤鍚?`--at <commit-sha>` 鍙?materialize 鏌愪釜鍘嗗彶 commit銆?
- 璁剧疆 `[mcp].embed_in_serve = true` 鍙湪涓?`pkvsyncd serve` 绔彛鐨?`/mcp` 鏆撮湶璇诲啓 MCP Streamable HTTP 绔偣锛涗篃鍙互杩愯 `pkvsyncd mcp --transport http --bind 127.0.0.1:6711` 浣滀负鐙珛 MCP 杩涚▼銆備娇鐢?`pkvsyncd mcp --vault <id>` 鍙惎鍔ㄤ粎 stdio 鐨勫崟绗旇搴撲細璇濄€?
- 澶ч噺鍒犻櫎闄勪欢鍚庤繍琛?blob 鍨冨溇鍥炴敹銆?
- 缁存姢鍓嶆鏌ヤ华琛ㄧ洏鏇存柊鎻愮ず鎴?GitHub release銆?
- 鍏虫敞鏃ュ織鍜屾椿鍔ㄤ腑閲嶅鍑虹幇鐨?`401`銆乣403`銆乣404`銆乣409` 鍜?`429` 鍝嶅簲銆?
- 淇濇寔鏈嶅姟绔簩杩涘埗銆佹彃浠跺寘銆丏ocker 闀滃儚銆佸弽鍚戜唬鐞嗗拰涓绘満绯荤粺鍙婃椂鏇存柊銆?
- 鎵?tag 鍙戠増鍓嶇‘璁?CI 閫氳繃銆?
- 妫€鏌ユ瘡涓?release 閮藉寘鍚?Linux amd64銆丩inux arm64銆乄indows x64銆佹彃浠?zip銆佹牎楠屽拰鍜?GHCR Docker 闀滃儚 tag銆?
