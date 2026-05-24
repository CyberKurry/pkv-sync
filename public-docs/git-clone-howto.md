# Git Clone Your PKV Vault

English | [简体中文](./git-clone-howto.zh-CN.md) | [繁體中文](./git-clone-howto.zh-Hant.md) | [日本語](./git-clone-howto.ja.md) | [한국어](./git-clone-howto.ko.md)

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

If your server is at `sync.example.com`, your vault ID is `abc123`, and your
device token is `pks_0f1e2d3c4b5a6978...`, run:

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/abc123
```

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
