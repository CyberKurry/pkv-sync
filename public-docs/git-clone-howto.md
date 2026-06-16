# Git Clone Your PKV Vault

English | [绠€浣撲腑鏂嘳(./git-clone-howto.zh-CN.md) | [绻侀珨涓枃](./git-clone-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./git-clone-howto.ja.md) | [頃滉淡鞏碷(./git-clone-howto.ko.md)

Document version: v1.4.5.

PKV Sync can expose each vault as a read-only Git repository over HTTPS.

## Prerequisites

- Server admin has enabled "Git smart HTTP" in Sync & Storage settings.
- `git` binary is available on the server.
- You have a valid device token.

## Clone

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

The underscore before the colon is the username. Any value works; only the token
matters as the password.

### Example

If your server is at `sync.example.com`, your vault ID is
`6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c`, and your device token is
`pks_0f1e2d3c4b5a6978...`, run:

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c
```

Vault IDs are 32-character lowercase hex (no dashes). The Admin WebUI and
`pkvsyncd user list` show valid IDs; placeholders like `abc123` are rejected
with `400 invalid_vault_id`.

## Materialize

After cloning, blob files appear as pointer JSON because the PKV Sync server
stores large files separately. Run:

```bash
pkvsyncd materialize <vault-id> -o ./output
```

This replaces pointer files with actual binary content, producing a fully usable
local copy of your vault.

## Notes

- The repository is **read-only** over HTTP. You cannot push changes back via Git.
- Use the PKV Sync plugin to make changes and push them through the normal sync API.
- If the server admin disables Git smart HTTP, clone or fetch operations return HTTP 503.
