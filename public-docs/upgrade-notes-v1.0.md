# Upgrade notes: 0.x to 1.0

English | [简体中文](./upgrade-notes-v1.0.zh-CN.md) | [繁體中文](./upgrade-notes-v1.0.zh-Hant.md) | [日本語](./upgrade-notes-v1.0.ja.md) | [한국어](./upgrade-notes-v1.0.ko.md)

Document version: v1.1.1.

PKV Sync 1.0 is the first stable release. It also resets the SQLite migration
baseline for future 1.x maintenance.

## Important database note

PKV Sync 1.0 ships a single `0001_initial.sql` baseline migration. SQLite
databases created by 0.x releases are **not supported for in-place upgrade** to
1.0.0.

If you run a 0.x server, choose one of these paths:

1. Keep the old deployment on the final 0.8.x patch release only long enough
   to back up, materialize, or export data for migration.
2. Back up or materialize each vault, start a fresh 1.0 data directory, create
   users and vaults again, then import or push the vault contents into the new
   server.
3. Keep a full `pkvsyncd backup` of the 0.x data root before trying any
   migration rehearsal.

Do not point the 1.0 binary or Docker image at an existing 0.x `metadata.db`.

## What 1.0 stabilizes

Starting at 1.0, the following surfaces follow semantic versioning:

- Public REST routes documented in `public-docs/openapi.yaml`.
- MCP stdio and Streamable HTTP tool behavior documented in the MCP how-to.
- SQLite migrations for 1.x fresh databases; future 1.x migrations are
  append-only after this v1 baseline.
- Per-vault git repository layout and content-addressed blob storage.
- CLI subcommands and existing flags.
- Obsidian plugin settings and sync behavior, subject to normal backward
  compatible 1.x feature additions.

Routes not documented in OpenAPI, such as Admin Web UI form handlers, are
internal implementation details.

## Recommended 0.x to 1.0 sequence

1. If possible, update the old deployment to the final 0.8.x patch release
   first, then use it only for backup, materialize, or export preparation.
2. Run `pkvsyncd backup --output <backup-dir>` and store the result safely.
3. For each vault, either use an up-to-date Obsidian client, `git clone`, or
   `pkvsyncd materialize <vault-id> --output <dir>` to produce a current file
   tree.
4. Stop the old server.
5. Start PKV Sync 1.0 with a new empty `data_dir` and `metadata.db`.
6. Complete `/setup`, recreate users and vaults, then push or import the
   materialized vault contents.
7. Ask users to update the Obsidian plugin to 1.0.0.

## Plugin compatibility

The bundled 1.0 Obsidian plugin is the supported plugin for the 1.0 server.
Older v0.8.x plugins use the same core sync API, but new fixes and self-update
hardening are only maintained in 1.0+.

## Breaking changes from 0.x

- 0.x SQLite databases are not upgraded in place because migrations were
  squashed into a single v1 baseline.
- First-run setup remains browser-based; fresh servers do not print random
  admin passwords to logs.

Vault file contents, git history, and blobs can still be carried forward by
backup/materialize/recreate/import workflows.

## Known caveats

- Native per-vault E2EE is not part of 1.0. Use
  [`git-crypt`](./git-crypt-howto.md) today if you need client-side encrypted
  file contents and can accept plaintext paths.
- `/metrics` is disabled by default and requires production authentication
  gates when enabled.
- Use `public_host` in production. Admin POSTs intentionally fail closed when
  the server cannot determine the configured HTTPS public origin.
