# 鍗囩骇璇存槑锛?.x 鍒?1.0

[English](./upgrade-notes-v1.0.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./upgrade-notes-v1.0.zh-Hant.md) | [鏃ユ湰瑾瀅(./upgrade-notes-v1.0.ja.md) | [頃滉淡鞏碷(./upgrade-notes-v1.0.ko.md)

鏂囨。鐗堟湰锛歷1.4.3銆?

PKV Sync 1.0 鏄涓€涓ǔ瀹氱増銆傚畠涔熶负鍚庣画 1.x 缁存姢閲嶇疆浜?SQLite migration 鍩虹嚎銆?

## 閲嶈鏁版嵁搴撹鏄?

PKV Sync 1.0 鍙彂甯冧竴涓?`0001_initial.sql` 鍩虹嚎 migration銆傜敱 0.x 鐗堟湰鍒涘缓鐨?
SQLite 鏁版嵁搴?*涓嶆敮鎸佸師鍦板崌绾?*鍒?1.0.0銆?

濡傛灉浣犳鍦ㄨ繍琛?0.x 鏈嶅姟绔紝璇烽€夋嫨涓嬮潰璺緞涔嬩竴锛?

1. 鏃ч儴缃插彧鍦ㄨ縼绉诲噯澶囨湡闂村仠鐣欏湪鏈€缁?0.8.x patch 鐗堟湰锛岀敤浜庡浠姐€乵aterialize 鎴栧鍑烘暟鎹€?
2. 鍏堝浠芥垨 materialize 姣忎釜绗旇搴擄紝浣跨敤鍏ㄦ柊鐨?1.0 鏁版嵁鐩綍鍚姩鏈嶅姟锛岄噸鏂板垱寤虹敤鎴峰拰绗旇搴擄紝鐒跺悗鎶婄瑪璁板簱鍐呭瀵煎叆鎴?push 鍒版柊鏈嶅姟绔€?
3. 鍦ㄤ换浣曡縼绉绘紨缁冨墠锛屽厛鐢?`pkvsyncd backup` 淇濆瓨瀹屾暣鐨?0.x 鏁版嵁鏍圭洰褰曘€?

涓嶈鎶?1.0 浜岃繘鍒舵垨 Docker 闀滃儚鐩存帴鎸囧悜宸叉湁鐨?0.x `metadata.db`銆?

## 1.0 绋冲畾鎵胯

浠?1.0 寮€濮嬶紝浠ヤ笅琛ㄩ潰閬靛惊璇箟鍖栫増鏈細

- `public-docs/openapi.yaml` 涓褰曠殑鍏紑 REST 璺敱銆?
- MCP how-to 涓褰曠殑 MCP stdio 鍜?Streamable HTTP 宸ュ叿琛屼负銆?
- 闈㈠悜 1.x 鍏ㄦ柊鏁版嵁搴撶殑 SQLite migrations锛涘湪杩欐 v1 鍩虹嚎涔嬪悗锛屾湭鏉?1.x migration 淇濇寔杩藉姞寮忋€?
- 姣忕瑪璁板簱 git 浠撳簱甯冨眬鍜屽唴瀹瑰鍧€ blob 瀛樺偍銆?
- CLI 瀛愬懡浠ゅ拰宸叉湁鍙傛暟銆?
- Obsidian 鎻掍欢璁剧疆鍜屽悓姝ヨ涓猴紝鍏佽 1.x 姝ｅ父娣诲姞鍚戝悗鍏煎鍔熻兘銆?

OpenAPI 涓病鏈夎褰曠殑璺敱锛屼緥濡?Admin Web UI 琛ㄥ崟澶勭悊鍣紝灞炰簬鍐呴儴瀹炵幇缁嗚妭銆?

## 鎺ㄨ崘鐨?0.x 鍒?1.0 娴佺▼

1. 濡傛潯浠跺厑璁革紝鍏堟妸鏃ч儴缃插崌绾у埌鏈€缁?0.8.x patch 鐗堟湰锛岀劧鍚庝粎鐢ㄥ畠瀹屾垚澶囦唤銆乵aterialize 鎴栧鍑哄噯澶囥€?
2. 杩愯 `pkvsyncd backup --output <backup-dir>` 骞跺Ε鍠勪繚瀛樺浠姐€?
3. 瀵规瘡涓瑪璁板簱锛屼娇鐢ㄦ渶鏂?Obsidian 瀹㈡埛绔€乣git clone`锛屾垨
   `pkvsyncd materialize <vault-id> --output <dir>` 寰楀埌褰撳墠鏂囦欢鏍戙€?
4. 鍋滄鏃ф湇鍔＄銆?
5. 浣跨敤鍏ㄦ柊鐨勭┖ `data_dir` 鍜?`metadata.db` 鍚姩 PKV Sync 1.0銆?
6. 瀹屾垚 `/setup`锛岄噸鏂板垱寤虹敤鎴峰拰绗旇搴擄紝鐒跺悗 push 鎴栧鍏?materialized 绗旇搴撳唴瀹广€?
7. 閫氱煡鐢ㄦ埛鎶?Obsidian 鎻掍欢鏇存柊鍒?1.0.0銆?

## 鎻掍欢鍏煎鎬?

1.0 鏈嶅姟绔殑鍙楁敮鎸佹彃浠舵槸闅忔湇鍔＄鎹嗙粦鐨?1.0 Obsidian 鎻掍欢銆傛棫鐨?v0.8.x 鎻掍欢浣跨敤鍚屼竴濂楁牳蹇冨悓姝?API锛屼絾鏂扮殑淇鍜岃嚜鏇存柊鍔犲浐鍙湪 1.0+ 涓淮鎶ゃ€?

## 鐩稿 0.x 鐨勭牬鍧忔€у彉鍖?

- 鐢变簬 migrations 宸插帇缂╀负鍗曚釜 v1 鍩虹嚎锛?.x SQLite 鏁版嵁搴撲笉鑳藉師鍦板崌绾с€?
- 棣栨杩愯 setup 浠嶇劧閫氳繃娴忚鍣ㄥ畬鎴愶紱鍏ㄦ柊鏈嶅姟绔笉浼氬啀鎶婇殢鏈虹鐞嗗憳瀵嗙爜鎵撳嵃鍒版棩蹇椼€?

绗旇鍐呭銆乬it 鍘嗗彶鍜?blobs 浠嶅彲閫氳繃 backup/materialize/recreate/import 宸ヤ綔娴佸甫鍒版柊閮ㄧ讲銆?

## 宸茬煡娉ㄦ剰浜嬮」

- 鍘熺敓 per-vault E2EE 涓嶅睘浜?1.0 鑼冨洿銆備粖澶╅渶瑕佸鎴风渚ф枃浠跺唴瀹瑰姞瀵嗙殑鐢ㄦ埛鍙互浣跨敤
  [`git-crypt`](./git-crypt-howto.zh-CN.md)锛屽苟鎺ュ彈璺緞浠嶄负鏄庢枃鐨勫彇鑸嶃€?
- `/metrics` 榛樿鍏抽棴锛涘惎鐢ㄥ悗浠嶉渶鐢熶骇璁よ瘉闂ㄧ銆?
- 鐢熶骇閮ㄧ讲璇烽厤缃?`public_host`銆傚綋鏈嶅姟绔棤娉曠‘瀹氶厤缃ソ鐨?HTTPS 鍏綉 origin 鏃讹紝admin POST 浼氭晠鎰?fail closed銆?
