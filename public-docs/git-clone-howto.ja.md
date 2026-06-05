# PKV vault を Git clone する

[English](./git-clone-howto.md) | [简体中文](./git-clone-howto.zh-CN.md) | [繁體中文](./git-clone-howto.zh-Hant.md) | 日本語 | [한국어](./git-clone-howto.ko.md)

ドキュメントバージョン: v1.0.13。

PKV Sync は、各 vault を HTTPS 経由の read-only Git repository として公開できます。

## Prerequisites

- Server admin が Sync & Storage settings で「Git smart HTTP」を有効化している。
- Server 上で `git` binary が利用できる。
- 有効な device token を持っている。

## Clone

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

コロン前の underscore は username です。値は何でも構いません。password 部分の token だけが使われます。

### Example

server が `sync.example.com`、vault ID が `6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c`、device token が `pks_0f1e2d3c4b5a6978...` の場合：

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c
```

Vault ID は 32 文字の小文字 hex（dash なし）です。Admin WebUI と `pkvsyncd user list` で有効な ID を確認できます。`abc123` のような placeholder は `400 invalid_vault_id` で拒否されます。

## Materialize

clone 後、PKV Sync server が大きなファイルを別途保存しているため、blob files は pointer JSON として表示されます。次を実行します。

```bash
pkvsyncd materialize <vault-id> -o ./output
```

pointer files を実際の binary content に置き換え、完全に利用可能なローカル vault copy を生成します。

## Notes

- HTTP 経由の repository は **read-only** です。Git で変更を push できません。
- 変更は PKV Sync plugin で行い、通常の sync API で push してください。
- Server admin が Git smart HTTP を無効化すると、clone や fetch は HTTP 503 を返します。
