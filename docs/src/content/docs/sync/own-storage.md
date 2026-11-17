---
title: "Sync to Your Own Storage"
description: "Sync your personal data through a folder, an SMB share, an SFTP server or a Git repository instead of Turso."
---

Your personal data (sessions, saved credentials, folders, settings) can sync through storage you already have, with no Turso account:

- **Folder**: a folder on this computer, a mounted network share (SMB/NFS), a UNC path such as `\\nas\share\reach`, or a folder that a sync client such as Syncthing or Nextcloud keeps in step.
- **SMB share**: a Windows file share or a NAS, reached by Reach itself, with no need to mount it or map a drive letter.
- **SFTP server**: any server you can log in to with a password or a private key.
- **Git repository**: any repository you can push to, through the `git` command.

Shared vaults always stay on Turso, because each member needs a database they can reach on their own.

## Set it up

1. Open **Settings > Sync > Sync to your own storage**.
2. Pick where, and fill in the location.
3. Click **Test connection**. Reach tells you whether the location is empty (your data is written there on the first sync) or already holds your vault (this device joins it).
4. Click **Use for sync**.

On your other devices, use **Settings > Sync > Link another device** (see [Setting Up a Second Device](/sync/setup/#setting-up-a-second-device)): the linked device opens the same vault from the same location.

## What is stored

Reach keeps one file, `reach-vault.reach`, in the location. It holds your vault's header (needed to unlock it on a new device; the master key in it is wrapped by your identity) and one sealed record per item. Names, categories and contents are all encrypted on your device before they leave, as described in [How Sync Works](/sync/how-sync-works/). Changing the file outside Reach makes it fail to open rather than be read wrongly.

Conflicts are handled as for Turso: item by item, the newer change wins, and a losing session or credential is kept as a "conflict copy".

## Each kind in detail

### Folder

The folder must exist. If a network share is not connected, Reach reports that and waits; it never creates an empty folder in its place. Two devices writing at once are kept apart by a short-lived lock file, `reach-vault.reach.lock`, next to the vault file.

### SMB share

Give the server, the share and optionally a folder in it, with your user name (`DOMAIN\user` for a domain account) and password. Reach speaks SMB 2 and 3 itself (and encrypts the connection when the share asks for it), so this works the same on Windows, macOS, Linux and Android. The folder must exist. On Windows you can also use the **Folder** option with a UNC path, which logs in as you.

### SFTP

On the first **Test connection**, Reach shows the server's host key. Check it (for example with `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` on the server) and click **Trust this key**. Reach sends nothing to a server whose key is not the one you trusted, and tells you if the key changes.

The folder is under your login folder unless it starts with `/`. Reach does not use your SSH agent here, so a sync running in the background never waits for a confirmation.

### Git

Reach runs the `git` command, so your own SSH keys, `~/.ssh/config` and credential helpers work as they do in a terminal. For an https remote you can also give an access token. Use an empty repository, or a branch of its own (Reach uses `reach-vault` unless you name another): each sync replaces that branch with a single commit, so the repository does not grow with every change.

## How often it syncs

A vault on your own storage syncs when it unlocks, a few seconds after each change, and otherwise every five minutes, so a server is not contacted every minute for nothing. **Sync now** in the Sync status syncs at once.
