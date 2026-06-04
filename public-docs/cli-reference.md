# CLI Reference

English | [简体中文](./cli-reference.zh-CN.md) | [繁體中文](./cli-reference.zh-Hant.md) | [日本語](./cli-reference.ja.md) | [한국어](./cli-reference.ko.md)

`pkvsyncd` is the PKV Sync server daemon binary. It hosts the HTTP/WebSocket
sync API, the admin UI, the MCP server, and a small set of operational
subcommands.

## Global options

These flags apply to every subcommand:

- `-c, --config <PATH>`: path to the TOML config file. Default: `/etc/pkv-sync/config.toml`.
- `-h, --help`: show help.
- `-V, --version`: print the CLI version.

```bash
pkvsyncd -c /opt/pkv-sync/config.toml serve
```

## Subcommands

`pkvsyncd` exposes nine subcommands. The most common operational flows are
`serve`, `genkey`, `migrate up`, `user add`, `backup`, and `restore`.

## pkvsyncd serve

Start the HTTP server.

### Synopsis

```text
pkvsyncd serve
```

### Description

Runs the public sync HTTP listener, the admin UI, the SSE stream, the Git
smart HTTP routes, and the MCP HTTP endpoint when configured. The listener
binds to `[server].bind_addr` from `config.toml`. Run this as a foreground
process under systemd or in a container.

### Example

```bash
pkvsyncd -c /etc/pkv-sync/config.toml serve
```

## pkvsyncd migrate

Database migration commands. The only operation is `up`.

### Synopsis

```text
pkvsyncd migrate up
```

### Description

Applies all pending SQLite migrations from `server/migrations/` against the
database at `[storage].db_path`. Safe to re-run; already-applied migrations
are skipped. The HTTP server also runs pending migrations at startup, so a
manual `migrate up` is typically only needed for cold-restore flows or when
migrating an offline backup.

### Example

```bash
pkvsyncd migrate up
```

## pkvsyncd genkey

Generate a random deployment key suitable for `[server].deployment_key`.

### Synopsis

```text
pkvsyncd genkey
```

### Description

Prints a cryptographically random `k_*` token to stdout. Paste the value
into `config.toml` and share it with the plugin/admin clients via your own
secure channel.

### Example

```bash
pkvsyncd genkey
# k_3f4a5e6b7c8d9e0f1a2b3c4d5e6f7a8b
```

## pkvsyncd user

User management commands. Useful for operational recovery (forgotten
password, locked account) and for scripted bootstrapping of secondary
operator accounts.

### Synopsis

```text
pkvsyncd user add <USERNAME> [--admin]
pkvsyncd user passwd <USERNAME>
pkvsyncd user list
pkvsyncd user set-active <USERNAME> --active <true|false>
```

### Subcommands

- `add <USERNAME> [--admin]`: create a user, prompting for the password interactively.
- `passwd <USERNAME>`: reset a user's password, prompting for the new value.
- `list`: list all users with their admin/active status and creation time.
- `set-active <USERNAME> --active <true|false>`: disable or re-enable a user. Disabled users keep their tokens but cannot log in or sync.

### Examples

```bash
# Create an admin account for emergency access
pkvsyncd user add alice --admin

# Reset a forgotten password
pkvsyncd user passwd alice

# Disable a departing user without deleting their data
pkvsyncd user set-active alice --active false
```

## pkvsyncd materialize

Expand a PKV Sync vault's bare git repository into a plain file tree on disk.

### Synopsis

```text
pkvsyncd materialize <VAULT-ID> -o <OUTPUT-DIR> [--at <COMMIT-SHA>]
```

### Options

- `-o, --output <DIR>`: output directory (must not exist or be empty).
- `--at <SHA>`: materialize at a specific commit (default: HEAD).

### Description

Reads the vault's bare git repository under `data_dir/vaults/<vault-id>` and
writes each file to the output directory:

- Text files are written as-is.
- Binary files stored as `pkvsync_pointer` JSON are resolved by copying the
  actual blob from the server's blob storage (`data_dir/blobs/`).

The command is synchronous and does not require the server to be running.
It reads directly from the on-disk git repository and blob storage under
the configured `data_dir`.

### Examples

```bash
# Materialize the latest version
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault

# Materialize a specific commit
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault-old --at abc123def456
```

### Exit codes

- `0`: success.
- `1`: error, such as output dir not empty, vault not found, blob missing, or invalid commit SHA.

> Vault IDs are 32-character lowercase hex (no dashes). The examples above
> use real-shape IDs; admin UI and `pkvsyncd user list` show valid IDs.

## pkvsyncd backup

Snapshot server data into a portable backup directory.

### Synopsis

```text
pkvsyncd backup -o <OUTPUT-DIR> [--data-dir <DIR>] [--gzip] [--include-config]
```

### Options

- `-o, --output <DIR>`: backup output directory (must not exist or be empty).
- `--data-dir <DIR>`: data directory override for offline operations. Defaults to `[storage].data_dir` from the loaded config.
- `--gzip`: also create a `.tar.gz` archive next to the backup directory.
- `--include-config`: include the loaded `config.toml` in the backup. By default backups omit config because it can contain deployment keys and other local secrets.

### Description

Snapshots the SQLite database (via VACUUM INTO so the source is not blocked),
every vault's bare git repository, and the blob store, into a self-contained
directory with a `MANIFEST.json`. The HTTP server may continue running during
backup; vault pushes are momentarily quiesced per-vault while their repos are
copied.

By default, backups omit `config.toml`; add `--include-config` only when you
intend to store the config and protect its secrets.

### Example

```bash
pkvsyncd backup -o /var/backups/pkv-2026-05-25 --gzip
```

## pkvsyncd restore

Restore a backup directory into a data directory.

### Synopsis

```text
pkvsyncd restore -i <BACKUP-DIR> [--data-dir <DIR>] [--force]
```

### Options

- `-i, --input <DIR>`: backup directory containing `MANIFEST.json`.
- `--data-dir <DIR>`: target data directory override. Defaults to `[storage].data_dir`.
- `--force`: clear a non-empty target data directory before restoring.

### Description

Validates the backup `MANIFEST.json`, copies the SQLite DB, vault repos, and
blob store into the target data dir. Stop the HTTP server before restoring.
After restore, run `pkvsyncd migrate up` if you are restoring a backup taken
by an older server version.

### Example

```bash
pkvsyncd restore -i /var/backups/pkv-2026-05-25 --data-dir /var/lib/pkv-sync --force
```

## pkvsyncd verify

Verify vault git repositories and content-addressed blobs.

### Synopsis

```text
pkvsyncd verify [--data-dir <DIR>] [--no-fail]
```

### Options

- `--data-dir <DIR>`: data directory override.
- `--no-fail`: return exit code 0 even when verification finds errors. Useful for monitoring scripts that want to log without paging.

### Description

For each vault under `data_dir/vaults/`:

- Runs `git fsck --strict` on the bare repository.
- Walks the HEAD tree and verifies every `pkvsync_pointer` resolves to a blob whose on-disk SHA-256 matches its filename.

Reports per-vault error counts. Exits non-zero when any vault has errors,
unless `--no-fail` is set.

### Example

```bash
pkvsyncd verify --data-dir /var/lib/pkv-sync
```

## pkvsyncd mcp

Start the MCP (Model Context Protocol) server for AI tools.

### Synopsis

```text
pkvsyncd mcp [--transport stdio|http] [--vault <VAULT-ID>] [--token <PKS-TOKEN>] [--bind <ADDR>]
```

### Options

- `--transport <stdio|http>`: transport mode. Default: `stdio`.
- `--vault <VAULT-ID>`: required for stdio; the single vault exposed to the client.
- `--token <PKS-TOKEN>`: bearer device token for stdio. If omitted, the `PKV_TOKEN` environment variable is used.
- `--bind <ADDR>`: HTTP bind address. Default: `127.0.0.1:6711`.

### Description

`stdio` mode reads JSON-RPC from stdin and writes JSON-RPC to stdout. `http`
mode serves a stateless Streamable HTTP MCP endpoint at `/mcp`. Both modes
expose the same toolset: `list_vaults`, `list_files`, `read_file`,
`read_file_at_commit`, `search`, `write_file`, and `delete_file`. Write tools
are rate-limited at 60 writes per minute per `(token, vault)`.

`http` mode requires every request to carry the server deployment key header,
just like the regular sync API.

This subcommand remains the standalone MCP process. To serve the same Streamable HTTP transport from the main server port, set `[mcp].embed_in_serve = true` and use `pkvsyncd serve`.

### Examples

```bash
# stdio with the token from the environment
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c

# Local Streamable HTTP endpoint
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

## pkvsyncd upgrade

Download a PKV Sync release binary side-by-side with the current executable.

### Synopsis

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <VERSION>]
```

### Options

- `--dry-run`: show the selected release, asset, and target path without downloading anything.
- `--yes`: skip the interactive confirmation prompt.
- `--version <VERSION>`: download a specific release such as `1.0.10` instead of the latest release.

### Description

The command selects the release asset for the current platform, verifies the
download against `SHA256SUMS`, writes `pkvsyncd.new` next to the current binary
(`pkvsyncd.new.exe` on Windows), and prints the systemd/manual swap steps. It
does not hot-replace the running server.

Docker and Kubernetes deployments should upgrade by pulling or changing the
image tag and restarting the service or rollout. When the command detects a
container environment, it prints image-based guidance and exits without
writing a binary.

### Examples

```bash
# Preview the upgrade plan
pkvsyncd upgrade --dry-run

# Download the latest verified binary
pkvsyncd upgrade --yes

# Download a specific release
pkvsyncd upgrade --yes --version 1.0.10
```
