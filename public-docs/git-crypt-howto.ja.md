# PKV Sync で git-crypt を使う

[English](./git-crypt-howto.md) | [简体中文](./git-crypt-howto.zh-CN.md) | [繁體中文](./git-crypt-howto.zh-Hant.md) | 日本語 | [한국어](./git-crypt-howto.ko.md)

ドキュメントバージョン: v1.3.2。

> **Note:** これは native end-to-end encryption（E2EE）が提供されるまでの暫定ガイドです。PKV Sync server は filenames と commit metadata を引き続き見ることができます。

## Overview

[git-crypt](https://github.com/AGWA/git-crypt) は Git repository 内で transparent file encryption を提供します。PKV Sync は vault を Git repository として公開できるため、sensitive files が server に届く前に git-crypt で暗号化できます。

## Setup

### 1. git-crypt をインストールする

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows, via scoop
scoop install git-crypt
```

### 2. clone した vault で git-crypt を初期化する

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. 暗号化するファイルを設定する

`.gitattributes` を作成または編集します。

```gitattributes
# 既定で全ファイルを暗号化
* filter=git-crypt diff=git-crypt

# ただし .gitattributes 自体は暗号化しない
.gitattributes !filter !diff
```

推奨は selective encryption です。

```gitattributes
# 特定 pattern だけ暗号化
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. 共同編集者と key を共有する

symmetric key を export します。

```bash
git-crypt export-key ../vault-key
```

各 collaborator が import します。

```bash
git-crypt unlock ../vault-key
```

## Limitations

- **Filenames は暗号化されません。** PKV Sync server は file paths と directory structure を見ることができます。
- **git-crypt は Git client 側で動作します。** Server は ciphertext blobs を保存します。key なしで clone すると、encrypted files は不透明な binary data として見えます。
- **Key management は手動です。** key を失うと encrypted files は復旧できません。
- **Git clone workflow 専用です。** PKV Sync Obsidian plugin は git-crypt を理解しません。encrypted files は vault を clone し、Git で直接扱う必要があります。
- **`pkvsyncd materialize` は git-crypt を認識しません。** PKV Sync が `pkvsync_pointer` JSON として保存したファイル（通常は text-extension list より大きい binaries）は、materialize 時に server の blob store から解決され、生バイトとしてクライアントに到着します。git-crypt の filter はクライアント側でこれらを見ないため、`*.pdf` などの blob 保存対象拡張子を git-crypt で暗号化しても期待される ciphertext stream にはなりません。git-crypt の pattern は、PKV Sync が text として扱うファイル種別（server で設定された `text_extensions` list、既定：`md`、`canvas`、`base`、`json`、`txt`、`css`）に限定してください。

## Recommended Workflow

1. 日常的な note-taking には Obsidian plugin と未暗号化ファイルを使います。
2. E2EE が必要な sensitive files には Git clone と git-crypt を使います。
3. git-crypt key を安全に backup します。
