# PKV Sync 銇?git-crypt 銈掍娇銇?

[English](./git-crypt-howto.md) | [绠€浣撲腑鏂嘳(./git-crypt-howto.zh-CN.md) | [绻侀珨涓枃](./git-crypt-howto.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./git-crypt-howto.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.3銆?

> **Note:** 銇撱倢銇?native end-to-end encryption锛圗2EE锛夈亴鎻愪緵銇曘倢銈嬨伨銇с伄鏆畾銈偆銉夈仹銇欍€侾KV Sync server 銇?filenames 銇?commit metadata 銈掑紩銇嶇稓銇嶈銈嬨亾銇ㄣ亴銇с亶銇俱仚銆?

## Overview

[git-crypt](https://github.com/AGWA/git-crypt) 銇?Git repository 鍐呫仹 transparent file encryption 銈掓彁渚涖仐銇俱仚銆侾KV Sync 銇?vault 銈?Git repository 銇ㄣ仐銇﹀叕闁嬨仹銇嶃倠銇熴倎銆乻ensitive files 銇?server 銇眾銇忓墠銇?git-crypt 銇ф殫鍙峰寲銇с亶銇俱仚銆?

## Setup

### 1. git-crypt 銈掋偆銉炽偣銉堛兗銉仚銈?

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows, via scoop
scoop install git-crypt
```

### 2. clone 銇椼仧 vault 銇?git-crypt 銈掑垵鏈熷寲銇欍倠

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. 鏆楀彿鍖栥仚銈嬨儠銈°偆銉倰瑷畾銇欍倠

`.gitattributes` 銈掍綔鎴愩伨銇熴伅绶ㄩ泦銇椼伨銇欍€?

```gitattributes
# 鏃㈠畾銇у叏銉曘偂銈ゃ儷銈掓殫鍙峰寲
* filter=git-crypt diff=git-crypt

# 銇熴仩銇?.gitattributes 鑷綋銇殫鍙峰寲銇椼仾銇?
.gitattributes !filter !diff
```

鎺ㄥエ銇?selective encryption 銇с仚銆?

```gitattributes
# 鐗瑰畾 pattern 銇犮亼鏆楀彿鍖?
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. 鍏卞悓绶ㄩ泦鑰呫仺 key 銈掑叡鏈夈仚銈?

symmetric key 銈?export 銇椼伨銇欍€?

```bash
git-crypt export-key ../vault-key
```

鍚?collaborator 銇?import 銇椼伨銇欍€?

```bash
git-crypt unlock ../vault-key
```

## Limitations

- **Filenames 銇殫鍙峰寲銇曘倢銇俱仜銈撱€?* PKV Sync server 銇?file paths 銇?directory structure 銈掕銈嬨亾銇ㄣ亴銇с亶銇俱仚銆?
- **git-crypt 銇?Git client 鍋淬仹鍕曚綔銇椼伨銇欍€?* Server 銇?ciphertext blobs 銈掍繚瀛樸仐銇俱仚銆俴ey 銇仐銇?clone 銇欍倠銇ㄣ€乪ncrypted files 銇笉閫忔槑銇?binary data 銇ㄣ仐銇﹁銇堛伨銇欍€?
- **Key management 銇墜鍕曘仹銇欍€?* key 銈掑け銇嗐仺 encrypted files 銇京鏃с仹銇嶃伨銇涖倱銆?
- **Git clone workflow 灏傜敤銇с仚銆?* PKV Sync Obsidian plugin 銇?git-crypt 銈掔悊瑙ｃ仐銇俱仜銈撱€俥ncrypted files 銇?vault 銈?clone 銇椼€丟it 銇х洿鎺ユ壉銇嗗繀瑕併亴銇傘倞銇俱仚銆?
- **`pkvsyncd materialize` 銇?git-crypt 銈掕獚璀樸仐銇俱仜銈撱€?* PKV Sync 銇?`pkvsync_pointer` JSON 銇ㄣ仐銇︿繚瀛樸仐銇熴儠銈°偆銉紙閫氬父銇?text-extension list 銈堛倞澶с亶銇?binaries锛夈伅銆乵aterialize 鏅傘伀 server 銇?blob store 銇嬨倝瑙ｆ焙銇曘倢銆佺敓銉愩偆銉堛仺銇椼仸銈儵銈ゃ偄銉炽儓銇埌鐫€銇椼伨銇欍€俫it-crypt 銇?filter 銇偗銉┿偆銈兂銉堝伌銇с亾銈屻倝銈掕銇亜銇熴倎銆乣*.pdf` 銇仼銇?blob 淇濆瓨瀵捐薄鎷″嫉瀛愩倰 git-crypt 銇ф殫鍙峰寲銇椼仸銈傛湡寰呫仌銈屻倠 ciphertext stream 銇伅銇倞銇俱仜銈撱€俫it-crypt 銇?pattern 銇€丳KV Sync 銇?text 銇ㄣ仐銇︽壉銇嗐儠銈°偆銉ó鍒ワ紙server 銇цō瀹氥仌銈屻仧 `text_extensions` list銆佹棦瀹氾細`md`銆乣canvas`銆乣base`銆乣json`銆乣txt`銆乣css`锛夈伀闄愬畾銇椼仸銇忋仩銇曘亜銆?

## Recommended Workflow

1. 鏃ュ父鐨勩仾 note-taking 銇伅 Obsidian plugin 銇ㄦ湭鏆楀彿鍖栥儠銈°偆銉倰浣裤亜銇俱仚銆?
2. E2EE 銇屽繀瑕併仾 sensitive files 銇伅 Git clone 銇?git-crypt 銈掍娇銇勩伨銇欍€?
3. git-crypt key 銈掑畨鍏ㄣ伀 backup 銇椼伨銇欍€?
