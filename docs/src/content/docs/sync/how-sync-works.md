---
title: "How Sync Works"
description: "How Reach keeps every vault on your device, syncs changes with its backend, handles conflicts, and what a backend can see."
---

Every vault lives in a file on your device, whether it syncs or not. Reach reads and writes that file, so your sessions open at once and you can add or edit them offline. A synced vault exchanges its changes with its backend in the background: when it unlocks, a few seconds after each change, and every minute.

Your personal data can sync to Turso or to storage of your own: a folder or network share, an SFTP server or a Git repository (see [Sync to Your Own Storage](/sync/own-storage/)). Shared vaults sync through Turso.

## What the backend sees

Every secret is encrypted on your device before it leaves. In the current format a backend stores one sealed record per secret: the name, category, timestamps and contents are all inside the seal, and the id it is filed under is a keyed hash, so a setting's key or a session's id does not show either. A record is bound to its vault and its id, so a backend cannot move one record into another's place, and Reach refuses an older copy of a record it has already seen.

A Turso database set up by an older Reach keeps its old table, which older versions also read. In that format Turso can see each secret's name and category (the contents are still encrypted). **Settings > Sync > Sync status** shows such a vault as "Old format" with an **Upgrade format** button. Upgrade once every device runs a Reach version that has this button: older versions no longer see the vault's changes afterwards. The old table is kept in the database under a new name as a backup.

## Conflicts

Reach merges record by record, so two devices only conflict when both changed the same item before syncing.

- Changed on one device only: that change wins.
- Changed on both: the newer change wins on every device. If the losing change was to a session or a saved credential, it is kept as a copy named "… (conflict copy *date*)", and the Sync status counts it until you dismiss it.
- Deleted on one device and edited on another: the edit wins, so nothing you changed disappears.

## Sync status

**Settings > Sync > Sync status** lists every synced vault with its last sync, changes still waiting to be sent, the last error if a sync failed, and a **Sync now** button.
