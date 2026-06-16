# 鍦?PKV Sync 涓娇鐢?git-crypt

[English](./git-crypt-howto.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./git-crypt-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./git-crypt-howto.ja.md) | [頃滉淡鞏碷(./git-crypt-howto.ko.md)

鏂囨。鐗堟湰锛歷1.4.3銆?

> **娉ㄦ剰锛?* 杩欐槸鍘熺敓绔埌绔姞瀵嗭紙E2EE锛夊彂甯冨墠鐨勮繃娓℃柟妗堛€侾KV Sync 鏈嶅姟鍣ㄤ粛鐒跺彲浠ョ湅鍒版枃浠跺悕鍜屾彁浜ゅ厓鏁版嵁銆?

## 姒傝堪

[git-crypt](https://github.com/AGWA/git-crypt) 鍙互鍦?Git 浠撳簱鍐呭疄鐜伴€忔槑鐨勬枃浠跺姞瀵嗐€傜敱浜?PKV Sync 灏嗕粨搴撲互 Git 浠撳簱褰㈠紡鏆撮湶锛屼綘鍙互浣跨敤 git-crypt 鍦ㄦ晱鎰熸枃浠跺埌杈炬湇鍔″櫒涔嬪墠杩涜鍔犲瘑銆?

## 璁剧疆

### 1. 瀹夎 git-crypt

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows锛岄€氳繃 scoop
scoop install git-crypt
```

### 2. 鍦ㄥ厠闅嗙殑浠撳簱涓垵濮嬪寲 git-crypt

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. 閰嶇疆瑕佸姞瀵嗙殑鏂囦欢

鍒涘缓鎴栫紪杈?`.gitattributes`锛?

```gitattributes
# 榛樿鍔犲瘑鎵€鏈夋枃浠?
* filter=git-crypt diff=git-crypt

# 浣嗕笉瑕佸姞瀵?.gitattributes 鏂囦欢鏈韩
.gitattributes !filter !diff
```

閫夋嫨鎬у姞瀵嗭紙鎺ㄨ崘锛夛細

```gitattributes
# 鍙姞瀵嗙壒瀹氭ā寮?
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. 涓庡崗浣滆€呭叡浜瘑閽?

瀵煎嚭瀵圭О瀵嗛挜锛?

```bash
git-crypt export-key ../vault-key
```

姣忎綅鍗忎綔鑰呭鍏ワ細

```bash
git-crypt unlock ../vault-key
```

## 闄愬埗

- **鏂囦欢鍚嶆湭鍔犲瘑銆?* PKV Sync 鏈嶅姟鍣ㄥ彲浠ョ湅鍒版枃浠惰矾寰勫拰鐩綍缁撴瀯銆?
- **git-crypt 鍦?Git 瀹㈡埛绔繍琛屻€?* 鏈嶅姟鍣ㄥ瓨鍌ㄧ殑鏄瘑鏂囥€傚鏋滀綘鍦ㄦ病鏈夊瘑閽ョ殑鎯呭喌涓嬪厠闅嗭紝鍔犲瘑鏂囦欢浼氭樉绀轰负涓嶉€忔槑鐨勪簩杩涘埗鏁版嵁銆?
- **瀵嗛挜绠＄悊鏄墜鍔ㄧ殑銆?* 濡傛灉瀵嗛挜涓㈠け锛屽姞瀵嗘枃浠舵棤娉曟仮澶嶃€?
- **浠呴€傜敤浜?Git 鍏嬮殕宸ヤ綔娴併€?* PKV Sync Obsidian 鎻掍欢涓嶇悊瑙?git-crypt銆備綘蹇呴』鍏嬮殕浠撳簱骞堕€氳繃 Git 鐩存帴鎿嶄綔鍔犲瘑鏂囦欢銆?
- **`pkvsyncd materialize` 涓嶆劅鐭?git-crypt銆?* PKV Sync 浠?`pkvsync_pointer` JSON 褰㈠紡瀛樺偍鐨勬枃浠讹紙閫氬父鏄枃鏈墿灞曞悕娓呭崟涔嬪鐨勪簩杩涘埗鏂囦欢锛夊湪 materialize 鏃朵細浠庢湇鍔″櫒鐨?blob 瀛樺偍涓В鏋愶紝骞朵互鍘熷瀛楄妭钀藉湴鈥斺€攇it-crypt 鐨?filter 鍦ㄥ鎴风鏍规湰鐪嬩笉鍒板畠浠紝鍥犳閫氳繃 git-crypt 鍔犲瘑 `*.pdf` 鎴栧叾浠栦互 blob 褰㈠紡瀛樺偍鐨勬墿灞曞悕锛屼笉浼氬緱鍒伴鏈熺殑瀵嗘枃娴併€傝鎶?git-crypt 妯″紡闄愬埗鍦?PKV Sync 瑙嗕负鏂囨湰鐨勬墿灞曞悕锛堟湇鍔″櫒閰嶇疆鐨?`text_extensions` 鍒楄〃锛岄粯璁や负 `md`銆乣canvas`銆乣base`銆乣json`銆乣txt`銆乣css`锛夈€?

## 鎺ㄨ崘宸ヤ綔娴?

1. 浣跨敤 Obsidian 鎻掍欢杩涜鏃ュ父绗旇璁板綍锛堟湭鍔犲瘑鏂囦欢锛夈€?
2. 瀵逛簬闇€瑕佺鍒扮鍔犲瘑鐨勬晱鎰熸枃浠讹紝浣跨敤 Git 鍏嬮殕鍜?git-crypt銆?
3. 瀹夊叏澶囦唤 git-crypt 瀵嗛挜銆?
