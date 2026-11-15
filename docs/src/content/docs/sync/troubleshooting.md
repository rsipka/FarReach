---
title: "Cloud Sync Troubleshooting"
description: "Fixes for common Reach cloud sync problems: sync not working after setup, a vault missing on a new device, a shared vault out of step, and keychain errors."
---

## Sync Not Working After Setup

- Double-check your Turso org name and API token in **Settings > Sync**.
- The API token must have permissions to create databases.
- Try clicking "Save & Setup" again to re-initialize the connection.

## Can't Access Vault on New Device

You need to import your identity first. Either import a full backup file or paste your backup key (**Settings > Sync > Export Backup Key** on your original device).

Without your identity, you can't decrypt anything. The data might sync down just fine, but it's encrypted and your new device doesn't have the keys yet.

## Shared Vault Not Syncing for a Member

- The member needs to accept the invite with the correct sync URL and token.
- Both users need an active internet connection.
- Check that the Turso database still exists. It gets deleted if the vault owner deletes the vault.

## Keychain Access Failed

This means Reach can't find your encryption key in the OS keychain. Usually happens after an OS reinstall or if the keychain got corrupted.

**Fix:** Import your backup key (you did save it, right?) using the "Restore Identity" option in settings.

**If you don't have the backup key:** You'll need to "Start Fresh", which creates a new identity. Your old encrypted data will be lost. This is why we keep telling you to save that backup key.

## Changes Not Showing on Another Device

Open **Settings > Sync > Sync status** on both devices. A vault that shows "Changes waiting to be sent" has not reached the backend yet; press **Sync now**, and check the error under it if it fails. A vault marked "Old format" on one device and not the other means one device runs an older Reach: update it before using **Upgrade format**.

## A Session Appears Twice, One Named "(conflict copy …)"

Two devices changed the same session before either synced. The newer change won, and the other was kept as the copy so nothing was lost. Keep whichever you want and delete the other. See [How Sync Works](/sync/how-sync-works/).
