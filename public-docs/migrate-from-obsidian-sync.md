# Migrate from Obsidian Sync

English | [简体中文](./migrate-from-obsidian-sync.zh-CN.md) | [繁體中文](./migrate-from-obsidian-sync.zh-Hant.md) | [日本語](./migrate-from-obsidian-sync.ja.md) | [한국어](./migrate-from-obsidian-sync.ko.md)

Document version: v1.3.2.

This guide explains how to import the current files from an Obsidian vault that
already uses Obsidian Sync into a new PKV Sync vault.

The migration imports the files currently present on this device. It does not
import Obsidian Sync history, remote version history, deleted-file history, or
conflict metadata. PKV Sync history starts at the migration commit that creates
the new PKV vault.

The migration also does not disable, uninstall, or change Obsidian Sync. If you
want to stop using Obsidian Sync after checking the PKV Sync result, turn it off
manually in Obsidian.

## Before you start

- Let Obsidian Sync finish syncing on the device you will migrate from.
- Make a manual backup of the vault folder before migration.
- Keep Obsidian closed during the import if possible, or avoid editing files
  while the import runs.
- Create or choose the destination PKV Sync server account first.

## What gets imported

PKV Sync creates a new vault and commits the current import as the first PKV
history entry.

Regular Markdown files, attachments, and normal vault files are imported unless
they match PKV Sync's hard exclusions.

## What is skipped

The importer skips Obsidian Sync internals, the PKV Sync plugin's own state,
OS-junk files, and local runtime files, including:

- `.obsidian/sync/`
- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.obsidian/plugins/pkv-sync/` (the plugin's own settings and token store stay local-only)
- `.trash/**`
- `.git/**`
- `.DS_Store` (macOS)
- `Thumbs.db` (Windows)
- temporary files such as `*.tmp` and `*.lock`
- device-specific workspace, cache, trash, and temporary files

Selected `.obsidian` configuration files may be synced later through the
per-vault `.obsidian` allowlist. See the `.obsidian` configuration sync guide
for those rules.

## After migration

Open the new PKV vault on another device and confirm that notes and attachments
look correct. Keep your manual backup until you have checked the migrated vault.

If you continue running Obsidian Sync and PKV Sync against the same folder, make
changes carefully. Two sync systems can race on the same files, and PKV Sync
will only track changes it receives after the migration commit.
