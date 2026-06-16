# 閫氳繃 Git 鍏嬮殕浣犵殑 PKV 浠撳簱

[English](./git-clone-howto.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./git-clone-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./git-clone-howto.ja.md) | [頃滉淡鞏碷(./git-clone-howto.ko.md)

鏂囨。鐗堟湰锛歷1.4.3銆?

PKV Sync 鍙互灏嗘瘡涓粨搴擄紙vault锛夐€氳繃 HTTPS 浠ュ彧璇?Git 浠撳簱鐨勫舰寮忔毚闇插嚭鏉ャ€?

## 鍓嶆彁鏉′欢

- 鏈嶅姟鍣ㄧ鐞嗗憳宸插湪鈥滃悓姝ヤ笌瀛樺偍鈥濊缃腑鍚敤鈥淕it smart HTTP鈥濄€?
- 鏈嶅姟鍣ㄤ笂鏈夊彲鐢ㄧ殑 `git` 浜岃繘鍒舵枃浠躲€?
- 浣犳嫢鏈夋湁鏁堢殑璁惧浠ょ墝锛坉evice token锛夈€?

## 鍏嬮殕

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

鍐掑彿鍓嶇殑涓嬪垝绾挎槸鐢ㄦ埛鍚嶃€傚彲浠ュ～鍐欎换鎰忓€硷紱鍙湁瀵嗙爜閮ㄥ垎鐨勪护鐗屾湁鏁堛€?

### 绀轰緥

濡傛灉浣犵殑鏈嶅姟鍣ㄥ湴鍧€涓?`sync.example.com`锛屼粨搴?ID 涓?`6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c`锛岃澶囦护鐗屼负 `pks_0f1e2d3c4b5a6978...`锛岃繍琛岋細

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c
```

Vault ID 鏄?32 涓瓧绗︾殑灏忓啓鍗佸叚杩涘埗锛堜笉鍚繛瀛楃锛夈€侫dmin WebUI 鍜?`pkvsyncd user list` 浼氭樉绀烘湁鏁?ID锛涘儚 `abc123` 杩欐牱鐨勫崰浣嶇浼氳浠?`400 invalid_vault_id` 鎷掔粷銆?

## 杩樺師锛圡aterialize锛?

鍏嬮殕鍚庯紝浜岃繘鍒舵枃浠朵細浠ユ寚閽?JSON 鐨勫舰寮忓嚭鐜帮紝鍥犱负 PKV Sync 鏈嶅姟鍣ㄤ細鍗曠嫭瀛樺偍澶ф枃浠躲€傝繍琛岋細

```bash
pkvsyncd materialize <vault-id> -o ./output
```

杩欎細灏嗘寚閽堟枃浠舵浛鎹负瀹為檯鐨勪簩杩涘埗鍐呭锛岀敓鎴愪竴涓畬鏁村彲鐢ㄧ殑鏈湴浠撳簱鍓湰銆?

## 娉ㄦ剰浜嬮」

- 璇ヤ粨搴撻€氳繃 HTTP **鍙**銆備綘涓嶈兘閫氳繃 Git 鎺ㄩ€佹洿鏀广€?
- 璇蜂娇鐢?PKV Sync 鎻掍欢杩涜鏇存敼锛屽苟閫氳繃甯歌鍚屾 API 鎺ㄩ€併€?
- 濡傛灉鏈嶅姟鍣ㄧ鐞嗗憳绂佺敤浜?Git smart HTTP锛屽厠闅嗘垨鎷夊彇鎿嶄綔灏嗚繑鍥?HTTP 503銆?
