# PKV Sync User Manual

English | [简体中文](./user-manual.zh-CN.md) | [繁體中文](./user-manual.zh-Hant.md) | [日本語](./user-manual.ja.md) | [한국어](./user-manual.ko.md)

This manual is for Obsidian users who connect to an existing PKV Sync server.
Ask your server administrator for the server share URL and an account or invite
code before you start.

## Manual Plugin Install

1. Download `pkv-sync-plugin.zip` from the matching GitHub release.
2. Extract the archive into your vault:

   ```text
   <vault>/.obsidian/plugins/pkv-sync/
   ```

3. In Obsidian, enable community plugins.
4. Enable **PKV Sync**.

The extracted directory should contain `main.js`, `manifest.json`, and
`styles.css`.

## Plugin Updates

The PKV Sync settings page includes an **Updates** section. By default the
plugin checks the connected PKV Sync server for the bundled plugin version; this
is the preferred source for self-hosted deployments because upgrading the server
also publishes the matching plugin assets. When the server is configured with
`public_host`, the plugin asset URLs stay anchored to that external host. You
can switch the update source to GitHub releases when needed.

When an update is available, **Update now** downloads `main.js`,
`manifest.json`, and `styles.css` when present, verifies SHA-256 hashes, writes
the plugin files, and prompts you to reload Obsidian. The command palette also
includes **PKV Sync: Check for PKV Sync plugin updates**.

## Connect to a Server

The server share URL usually looks like this:

```text
https://sync.example.com/k_xxx/
```

Open **Settings -> PKV Sync**, paste the share URL, then click **Connect**. If
the deployment key is embedded in the URL, the plugin fills it automatically.

If you entered the wrong server or need to move to another self-hosted server,
use **Change server** on the login screen to return to the server settings
without reinstalling the plugin.

## Login or Register

Registration behavior depends on the server runtime setting:

- **Disabled**: an administrator must create your account.
- **Invite only**: enter the invite code provided by an administrator.
- **Open**: create an account directly.

After login, select an existing remote vault or create a new one. When you
connect a local vault that is already identical to the selected remote vault,
PKV Sync adopts the matching files into its local sync index instead of
creating a full-vault set of conflict files.

## Sync Behavior

PKV Sync runs inside Obsidian and syncs the current vault:

- Local file changes are pushed after a short debounce interval.
- Remote changes are polled periodically.
- Manual sync is available from the settings page and command palette.
- Relevant file create/modify/delete events schedule a sync.
- Window blur can trigger a sync.
- On startup, unsynced local changes are detected from the vault contents and
  the local sync index.

Keep Obsidian open while large attachments upload. The plugin reads the server
configuration after connecting and uses the server-provided text extension list
and maximum file size rules.

## Selective `.obsidian` Sync

PKV Sync can sync selected Obsidian configuration files through a per-vault
allowlist. New remote vaults start with rules for themes, CSS snippets,
hotkeys, app preferences, appearance preferences, and enabled plugin lists.

Existing remote vaults keep an empty allowlist until you opt in. In
**Settings -> PKV Sync**, select the current vault, edit **.obsidian sync
rules**, then save. The recommended starter list button fills the same starter
rules used for new vaults.

Plugin code and plugin settings are not synced by default. See
[`dot-obsidian-sync-howto.md`](./dot-obsidian-sync-howto.md) before adding
advanced rules such as `.obsidian/plugins/**` or plugin `data.json` files.

## Last Sync Time

The settings page shows the last successful sync as relative time. Use the small
expander next to it to show the exact timestamp in this format:

```text
YYYY/MM/DD HH:MM:SS
```

The plugin uses the selected IANA timezone, defaulting to `Asia/Shanghai`.

## Theme

The plugin settings page has a **Theme** dropdown with three options:

- **Auto** (default): follow the current Obsidian app theme.
- **Light** / **Dark**: force a specific theme for the PKV Sync settings UI regardless of the app theme.

The setting only affects the plugin's own modals (settings, sync status, conflict resolver, file/vault history). It does not change the Obsidian app theme.

## History, Diff, and Restore

When the server reports history support and **Enable history and diff UI** is on
in plugin settings, you can inspect file history from:

- **PKV Sync: Show file history**
- **PKV Sync: Show vault history**
- the file right-click menu: **PKV Sync: File history**
- the file right-click menu: **PKV Sync: Diff with previous**

The file-history modal lists commits for the current file with time, device,
commit id, and change type. Text files can show unified diffs. Binary files can
be listed and restored, but PKV Sync does not render binary diffs.

The vault-history modal lists the most recent commits across the whole vault
with time, device, change count, and commit id. Open a commit to see its
per-file change summary.

Restoring a version reads the selected historical content from the server,
writes it back to the local Obsidian vault, and lets the normal sync engine push
that write as a new commit. If the current local file differs from the last
synced hash, the confirmation dialog warns that unsynced local changes will be
overwritten.

PKV Sync does not keep a full offline history cache in the plugin. History and
diff views require the server to be reachable.

## Conflict Files

If two devices edit the same file offline, PKV Sync keeps both versions. The
remote or local alternate version is saved as a generated conflict file:

```text
note.md
note.conflict-2026-04-25-143022-Desktop.md
```

Generated conflict files are excluded from future sync. Review both files in
Obsidian, merge the content you want to keep, then delete the conflict file.

You can manage generated conflict files from:

- **Settings -> PKV Sync -> Conflict files**
- **PKV Sync: List conflict files**
- **PKV Sync: Delete conflict files**

The delete action only targets PKV Sync generated conflict filenames. Normal
files such as `my.conflict-resolution-notes.md` remain eligible for sync.

## Delete a Remote Vault

The plugin settings page exposes a **Delete vault** action for the currently
selected remote vault. The action:

1. Opens a confirmation modal that requires you to type the exact vault name.
2. Calls the server's `DELETE /api/vaults/:id` endpoint, which removes the vault row, its git repository, and blob references for that vault.
3. Detaches the local Obsidian vault from PKV Sync (local files are left
   untouched on disk).

Vault deletion is irreversible on the server. Take a backup via
`pkvsyncd backup` first if you need a safety net. Other devices currently
connected to the deleted vault will see a 404 on their next sync and need to
re-pair to a different vault.

## Device Tokens

Logging in issues a bearer device token. Authenticated use renews the token, so
active devices stay signed in while devices idle for 90 days expire. The plugin
keeps a stable device ID, so logging in again from the same device replaces that
device's previous active token instead of accumulating duplicates.

The Obsidian plugin stores the active token and deployment key in
`<vault>/.obsidian/plugins/pkv-sync/data.json`. Treat that file as sensitive:
protect plaintext backups and cloud-sync targets, and do not share it. If the
file may have leaked, log out or ask an administrator to revoke the device token,
then connect again.

- Use plugin settings to log out from the current device.
- Ask an administrator to revoke tokens for lost devices.
- Changing your password keeps the current device signed in and revokes your
  other device tokens.

## MCP Access

If your administrator enables MCP, either embedded at `/mcp` on the main server
or through the `pkvsyncd mcp` command, AI tools can access your vault using a
bearer device token. MCP offers vault listing, file listing, file reads at HEAD
or a commit, simple text search, and explicit write/delete tools guarded by
optimistic concurrency. See [`mcp-howto.md`](./mcp-howto.md) for stdio and
Streamable HTTP setup examples.

## Commands

PKV Sync adds these command palette actions:

- Show sync status
- Refresh account info
- Manual sync now
- View sync status details
- Check for PKV Sync plugin updates
- Import current vault to PKV Sync (one-shot migration from Obsidian Sync)
- List conflict files
- Delete conflict files
- Resolve conflict files
- Show file history
- Show vault history

## Privacy Reminder

PKV Sync is not end-to-end encrypted out of the box. The server administrator
and anyone with server filesystem access can read synced vault contents and
attachments. Use it only with a server and administrator you trust.

If you need confidentiality from the server itself today, the project
recommends `git-crypt` as an interim per-vault encryption layer; see
[`git-crypt-howto.md`](./git-crypt-howto.md). Native per-vault E2EE is on the
1.x roadmap and not part of the 1.0 release.
