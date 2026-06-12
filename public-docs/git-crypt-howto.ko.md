# PKV Sync에서 git-crypt 사용하기

[English](./git-crypt-howto.md) | [简体中文](./git-crypt-howto.zh-CN.md) | [繁體中文](./git-crypt-howto.zh-Hant.md) | [日本語](./git-crypt-howto.ja.md) | 한국어

문서 버전: v1.3.1.

> **Note:** native end-to-end encryption(E2EE)이 제공되기 전까지의 임시 가이드입니다. PKV Sync server는 여전히 filenames와 commit metadata를 볼 수 있습니다.

## Overview

[git-crypt](https://github.com/AGWA/git-crypt)는 Git repository 안에서 transparent file encryption을 제공합니다. PKV Sync는 vault를 Git repository로 노출할 수 있으므로, 민감한 파일이 server에 도달하기 전에 git-crypt로 암호화할 수 있습니다.

## Setup

### 1. git-crypt 설치

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows, via scoop
scoop install git-crypt
```

### 2. clone한 vault에서 git-crypt 초기화

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. 암호화할 파일 설정

`.gitattributes`를 만들거나 편집합니다.

```gitattributes
# 기본으로 모든 파일 암호화
* filter=git-crypt diff=git-crypt

# 단 .gitattributes 자체는 암호화하지 않음
.gitattributes !filter !diff
```

선택적 암호화를 권장합니다.

```gitattributes
# 특정 pattern만 암호화
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. collaborator와 key 공유

symmetric key를 export합니다.

```bash
git-crypt export-key ../vault-key
```

각 collaborator가 import합니다.

```bash
git-crypt unlock ../vault-key
```

## Limitations

- **Filenames는 암호화되지 않습니다.** PKV Sync server는 file paths와 directory structure를 볼 수 있습니다.
- **git-crypt는 Git client 쪽에서 동작합니다.** Server는 ciphertext blobs를 저장합니다. key 없이 clone하면 encrypted files는 불투명한 binary data로 보입니다.
- **Key management는 수동입니다.** key를 잃으면 encrypted files를 복구할 수 없습니다.
- **Git clone workflow에서만 동작합니다.** PKV Sync Obsidian plugin은 git-crypt를 이해하지 않습니다. encrypted files는 vault를 clone하고 Git으로 직접 다뤄야 합니다.
- **`pkvsyncd materialize`는 git-crypt를 인식하지 않습니다.** PKV Sync가 `pkvsync_pointer` JSON으로 저장한 파일(주로 text-extension list보다 큰 binaries)은 materialize 시 서버의 blob store에서 resolve되어 raw bytes로 도착합니다. git-crypt의 filter는 클라이언트 쪽에서 이를 보지 못하므로, `*.pdf`나 그 외 blob에 저장되는 확장자를 git-crypt로 암호화해도 기대한 ciphertext stream이 만들어지지 않습니다. git-crypt patterns는 PKV Sync가 text로 취급하는 파일 형식(server에서 설정하는 `text_extensions` list, 기본값: `md`, `canvas`, `base`, `json`, `txt`, `css`)으로 제한하세요.

## Recommended Workflow

1. 일상 note-taking에는 Obsidian plugin과 암호화하지 않은 파일을 사용합니다.
2. E2EE가 필요한 민감한 파일에는 Git clone과 git-crypt를 사용합니다.
3. git-crypt key를 안전하게 backup합니다.
