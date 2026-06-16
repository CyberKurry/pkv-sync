# 鍦?PKV Sync 涓娇鐢?git-crypt

[English](./git-crypt-howto.md) | [绠€浣撲腑鏂嘳(./git-crypt-howto.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./git-crypt-howto.ja.md) | [頃滉淡鞏碷(./git-crypt-howto.ko.md)

鏂囦欢鐗堟湰锛歷1.4.3銆?

> **娉ㄦ剰锛?* 閫欐槸鍘熺敓绔埌绔姞瀵嗭紙E2EE锛夌櫦甯冨墠鐨勯亷娓℃柟妗堛€侾KV Sync server 浠嶇劧鍙互鐪嬪埌妾斿悕鍜?commit metadata銆?

## 姒傝堪

[git-crypt](https://github.com/AGWA/git-crypt) 鍙湪 Git repository 鍏у鐝鹃€忔槑妾旀鍔犲瘑銆傜敱鏂?PKV Sync 灏?vault 鏆撮湶鐐?Git repository锛屼綘鍙互鍦ㄦ晱鎰熸獢妗堝埌閬?server 鍓嶄娇鐢?git-crypt 鍔犲瘑銆?

## 瑷畾

### 1. 瀹夎 git-crypt

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows, via scoop
scoop install git-crypt
```

### 2. 鍦?clone 鐨?vault 涓垵濮嬪寲 git-crypt

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. 瑷畾瑕佸姞瀵嗙殑妾旀

寤虹珛鎴栫法杓?`.gitattributes`锛?

```gitattributes
# 闋愯ō鍔犲瘑鎵€鏈夋獢妗?
* filter=git-crypt diff=git-crypt

# 浣嗕笉瑕佸姞瀵?.gitattributes 鏈韩
.gitattributes !filter !diff
```

寤鸿浣跨敤閬告搰鎬у姞瀵嗭細

```gitattributes
# 鍙姞瀵嗙壒瀹?patterns
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. 鑸囧崝浣滆€呭垎浜?key

鍖嚭 symmetric key锛?

```bash
git-crypt export-key ../vault-key
```

姣忎綅鍗斾綔鑰呭尟鍏ワ細

```bash
git-crypt unlock ../vault-key
```

## 闄愬埗

- **妾斿悕涓嶆渻鍔犲瘑銆?* PKV Sync server 鍙互鐪嬪埌妾旀璺緫鍜岀洰閷勭祼妲嬨€?
- **git-crypt 鍦?Git client 绔亱浣溿€?* Server 鍎插瓨鐨勬槸 ciphertext blobs銆傛矑鏈?key 鏅?clone锛宔ncrypted files 鏈冮’绀虹偤涓嶉€忔槑 binary data銆?
- **Key management 鏄墜鍕曠殑銆?* Key 閬哄け鏅?encrypted files 鐒℃硶寰╁師銆?
- **鍙仼鐢ㄦ柤 Git clone workflow銆?* PKV Sync Obsidian 澶栨帥涓嶄簡瑙?git-crypt銆備綘蹇呴爤 clone vault 涓﹂€忛亷 Git 鐩存帴铏曠悊 encrypted files銆?
- **`pkvsyncd materialize` 涓嶈獚璀?git-crypt銆?* 琚?PKV Sync 浠?`pkvsync_pointer` JSON 鍎插瓨鐨勬獢妗堬紙閫氬父鏄笉鍦ㄦ枃瀛楀壇妾斿悕娓呭柈鍏х殑浜岄€蹭綅妾旓級锛屾渻鍦?materialize 鏅傚皪鐓?server 鐨?blob store 閭勫師鐐哄師濮?bytes 鈥斺€?鐢ㄦ埗绔殑 git-crypt filter 瀹屽叏鐪嬩笉鍒伴€欎簺妾旀锛屽洜姝ょ敤 git-crypt 鍔犲瘑 `*.pdf` 鎴栧叾浠栬瀛樼偤 blob 鐨勫壇妾斿悕锛屼笉鏈冪敘鐢熼爯鏈熺殑瀵嗘枃娴併€傝珛鎶?git-crypt patterns 闄愬埗鍦?PKV Sync 瑕栫偤鏂囧瓧鐨勬獢妗堥鍨嬶紙浼烘湇鍣ㄨō瀹氱殑 `text_extensions` 娓呭柈锛岄爯瑷偤 `md`銆乣canvas`銆乣base`銆乣json`銆乣txt`銆乣css`锛夈€?

## 寤鸿宸ヤ綔娴?

1. 鏃ュ父绛嗚浣跨敤 Obsidian 澶栨帥铏曠悊鏈姞瀵嗘獢妗堛€?
2. 闇€瑕?E2EE 鐨勬晱鎰熸獢妗堜娇鐢?Git clone 鍜?git-crypt銆?
3. 瀹夊叏鍌欎唤 git-crypt key銆?
