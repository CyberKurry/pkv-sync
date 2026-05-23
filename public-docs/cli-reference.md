# CLI Reference

English | [简体中文](./cli-reference.zh-CN.md)

## pkvsyncd materialize

Expand a PKV Sync vault's bare git repository into a plain file tree on disk.

### Synopsis

```text
pkvsyncd materialize <vault-id> -o <output-dir> [--at <commit-sha>]
```

### Options

- `-o, --output <DIR>`: output directory (must not exist or be empty).
- `--at <SHA>`: materialize at a specific commit (default: HEAD).

### Description

Reads the vault's bare git repository and writes each file to the output directory:

- Text files are written as-is.
- Binary files stored as `pkvsync_pointer` JSON are resolved by copying the actual blob from the server's blob storage.

The command is synchronous and does not require the server to be running. It reads directly from the on-disk git repository and blob storage under the configured `data_dir`.

### Examples

```bash
# Materialize the latest version
pkvsyncd materialize abc123 -o ./my-vault

# Materialize a specific commit
pkvsyncd materialize abc123 -o ./my-vault-old --at def456
```

### Exit Codes

- `0`: success.
- `1`: error, such as output dir not empty, vault not found, blob missing, or invalid commit SHA.

## pkvsyncd mcp

Start the MCP server for AI tools.

### Synopsis

```text
pkvsyncd mcp [--transport stdio|http] [--vault <vault-id>] [--token <pks-token>] [--bind <addr>]
```

### Options

- `--transport <stdio|http>`: transport mode (default: `stdio`).
- `--vault <vault-id>`: required for stdio; the single vault exposed to the client.
- `--token <pks-token>`: bearer device token for stdio; if omitted, `PKV_TOKEN` is used.
- `--bind <addr>`: HTTP bind address (default: `127.0.0.1:6711`).

### Description

stdio mode reads JSON-RPC from stdin and writes JSON-RPC to stdout. HTTP mode serves a stateless Streamable HTTP MCP endpoint at `/mcp`. Both modes expose `list_vaults`, `list_files`, `read_file`, `read_file_at_commit`, `search`, `write_file`, and `delete_file`.

### Examples

```bash
# stdio, token from environment
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault abc123

# local Streamable HTTP endpoint
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

HTTP mode requires the server deployment key header on every request.

## pkvsyncd upgrade

Download a PKV Sync release binary side-by-side with the current executable.

### Synopsis

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <version>]
```

### Options

- `--dry-run`: show the selected release, asset, and target path without
  downloading anything.
- `--yes`: skip the interactive confirmation prompt.
- `--version <version>`: download a specific release such as `0.9.1` instead of
  the latest release.

### Description

The command selects the release asset for the current platform, verifies the
download against `SHA256SUMS`, writes `pkvsyncd.new` next to the current binary
(`pkvsyncd.new.exe` on Windows), and prints the systemd/manual swap steps. It
does not hot replace the running server.

Docker and Kubernetes deployments should upgrade by pulling or changing the
image tag and restarting the service or rollout. When the command detects a
container environment, it prints image-based guidance and exits without writing
a binary.

### Examples

```bash
# Preview the upgrade plan
pkvsyncd upgrade --dry-run

# Download the latest verified binary
pkvsyncd upgrade --yes

# Download a specific release
pkvsyncd upgrade --yes --version 0.9.1
```
