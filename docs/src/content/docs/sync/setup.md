---
title: "Set Up Encrypted Cloud Sync"
description: "Sync your encrypted Reach vault across devices through your own Turso database, step by step: account, token and settings."
---

## Step 1: Create a Turso Account

Head to [turso.tech](https://turso.tech) and sign up if you don't have an account yet. The free tier is more than enough for personal use.

You can also check out the [Turso quickstart guide](https://docs.turso.tech/quickstart) if you want to get familiar with how Turso works before setting it up in Reach.

## Step 2: Install the Turso CLI (optional but helpful)

The Turso CLI makes it easy to grab your credentials. You don't strictly need it (you can do everything from the dashboard), but it's handy.

**macOS / Linux:**
```bash
curl -sSfL https://get.tur.so/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://get.tur.so/install.ps1 | iex
```

After installing, log in:
```bash
turso auth login
```

This opens your browser to authenticate. Once done, you're good to go.

For more details, see the [Turso CLI reference](https://docs.turso.tech/cli/introduction).

## Step 3: Get Your Organization Name

Your org name is basically your account identifier. You can find it in a few ways:

**From the dashboard:** Log into [turso.tech](https://turso.tech). The org name is in the URL: `turso.tech/your-org-name`.

**From the CLI:**
```bash
turso org list
```

This prints your organizations. Use the one you want Reach to create databases in.

## Step 4: Create a Platform API Token

Reach needs a Platform API Token to create and manage databases on your Turso account.

**From the dashboard:** Go to your org settings, find the API Tokens section, and create a new token.

**From the CLI:**
```bash
turso auth api-tokens mint reach-app
```

This creates a token called "reach-app". Copy it somewhere safe, you'll need it in the next step. The token won't be shown again.

For more on API tokens, see [Turso Platform API docs](https://docs.turso.tech/platform/api).

## Step 5: Enter Credentials in Reach

You can do this in two places:

- During the **first-run wizard** (Step 2 of setup)
- Or later in **Settings > Sync**

Enter your organization name and the API token you just created, then click "Save & Setup".

## Step 6: Done

Reach creates a personal database on your Turso account automatically. This stores your encrypted vault data in the cloud.

That's it. Once set up, your data syncs automatically. New sessions, updated secrets, everything stays in sync.

You can verify the database was created:
```bash
turso db list
```

You should see a database with a name like `reach-personal-xxxxx`.

## Setting Up a Second Device

Each device makes its own encryption identity, and a vault can only be opened by the identity that made it (or members it was shared with). So a second device does not sign in with Turso credentials: it is linked to the first one.

1. On the device that already syncs, open **Settings > Sync > Link another device** and click **Show link code**.
2. Install Reacher CM on the new device. On its first screen, paste the code under "Already syncing on another device?" and click **Join**. A device that is already set up can do the same in **Settings > Sync > Join from another device**; its own identity and data are replaced.
3. Your vaults sync down, whatever storage they use (Turso, a folder, SFTP, Git, SMB). The master password is the first device's.

The code holds your identity's secret key and the sync tokens, so treat it like the backup key: move it yourself over a channel you trust, and do not keep it. Entering the same Turso organization and API token on an unlinked device does not reach your data; Reacher CM says so instead of creating a second personal database.

## A Note About Security

The Turso credentials are stored locally in your app settings. The API token gives access to create databases on your account, so keep it safe. Don't share it with anyone you don't trust with your Turso account.

The database tokens Reach uses to reach your vaults are kept in `vault_identity.json` sealed under your vault key, never in plain text, and Reach's data folder is readable only by your user account on Linux and macOS. Sync URLs must be encrypted (`libsql://` or `https://`); plain `http://` is accepted only for a server on the same machine.

## Useful Links

- [Turso quickstart](https://docs.turso.tech/quickstart)
- [Turso CLI docs](https://docs.turso.tech/cli/introduction)
- [Platform API reference](https://docs.turso.tech/platform/api)
- [Turso pricing](https://turso.tech/pricing) (free tier covers personal use)
