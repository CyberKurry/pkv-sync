# PKV Sync User Manual

English | [简体中文](./user-manual.zh-CN.md)

## Install Plugin Manually

1. Download `pkv-sync-plugin.zip` from a release.
2. Extract it into your vault:

   ```text
   <vault>/.obsidian/plugins/pkv-sync/
   ```

3. In Obsidian, enable community plugins.
4. Enable **PKV Sync**.

## Connect to a Server

Ask your admin for the PKV Sync server URL, usually:

```text
https://sync.example.com/k_xxx/
```

Paste it in **Settings -> PKV Sync -> Server URL** and click **Connect**.

If the deployment key is embedded in the URL, the plugin fills it in
automatically.

## Login or Register

Depending on server settings:

- If registration is disabled, ask the admin to create an account.
- If invite-only registration is enabled, enter your invite code.
- If open registration is enabled, create an account directly.

After login, choose the remote vault to sync.

## Sync Behavior

PKV Sync:

- Pushes local changes after about 2 seconds of inactivity
- Pulls remote changes about every 60 seconds
- Syncs when you switch files, lose focus, or manually run **PKV Sync: Manual sync now**
- Recovers unsynced local changes on the next Obsidian start

Keep Obsidian open long enough for large attachments to upload.

## Conflicts

If two devices edit the same file offline, PKV Sync keeps both versions.

Example:

```text
note.md
note.conflict-2026-04-25-143022-Android-device.md
```

Open both files in Obsidian, merge manually, then delete the conflict file.

## Device Tokens

Each login creates a device token.

- Use plugin settings to log out from the current device.
- Ask your admin to revoke lost devices.
- Changing your password keeps the current device signed in and revokes other devices.

## Privacy Reminder

PKV Sync is not end-to-end encrypted. The server administrator and anyone with
server filesystem access can read synced vault contents.
