# Using git-crypt with PKV Sync

English | [简体中文](./git-crypt-howto.zh-CN.md) | [繁體中文](./git-crypt-howto.zh-Hant.md) | [日本語](./git-crypt-howto.ja.md) | [한국어](./git-crypt-howto.ko.md)

Document version: v1.3.0.

> **Note:** This is a stop-gap guide for end-to-end encryption (E2EE) before
> native E2EE ships. The PKV Sync server can still see filenames and commit
> metadata.

## Overview

[git-crypt](https://github.com/AGWA/git-crypt) enables transparent file
encryption inside a Git repository. Since PKV Sync exposes vaults as Git
repositories, you can use git-crypt to encrypt sensitive files before they
reach the server.

## Setup

### 1. Install git-crypt

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows, via scoop
scoop install git-crypt
```

### 2. Initialize git-crypt in a cloned vault

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. Configure which files to encrypt

Create or edit `.gitattributes`:

```gitattributes
# Encrypt all files by default
* filter=git-crypt diff=git-crypt

# But don't encrypt the .gitattributes file itself
.gitattributes !filter !diff
```

For selective encryption, which is recommended:

```gitattributes
# Only encrypt specific patterns
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. Share the key with collaborators

Export the symmetric key:

```bash
git-crypt export-key ../vault-key
```

Each collaborator imports it:

```bash
git-crypt unlock ../vault-key
```

## Limitations

- **Filenames are not encrypted.** The PKV Sync server can see file paths and
  directory structure.
- **git-crypt operates on the Git client side.** The server stores ciphertext
  blobs. When you clone without the key, encrypted files appear as opaque
  binary data.
- **Key management is manual.** If a key is lost, encrypted files cannot be
  recovered.
- **Only works with the Git clone workflow.** The PKV Sync Obsidian plugin does
  not understand git-crypt. You must clone the vault and work through Git
  directly for encrypted files.
- **`pkvsyncd materialize` is not git-crypt-aware.** Files that PKV Sync stored
  as `pkvsync_pointer` JSON (typically binaries above the text-extension list)
  are resolved against the server's blob store during materialize and arrive
  as raw bytes — git-crypt's filter never sees them on the client side, so
  encrypting `*.pdf` or other blob-stored extensions via git-crypt does not
  produce the expected ciphertext stream. Restrict git-crypt patterns to file
  types PKV Sync treats as text (the server-configured `text_extensions` list,
  default: `md`, `canvas`, `base`, `json`, `txt`, `css`).

## Recommended Workflow

1. Use the Obsidian plugin for day-to-day note-taking with unencrypted files.
2. Use Git clone and git-crypt for sensitive files that need E2EE.
3. Keep the git-crypt key backed up securely.
