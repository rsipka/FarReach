FarReach Dev build 4, based on upstream v0.9.2 and the fork's current main.

- New since dev.2: macOS login over VNC (Apple Remote Desktop), 1Password, Bitwarden and KeePass, password manager entries in saved credentials, configurable right click and copy on select in the terminal, the app's own right-click menu, the AI assistant asks before running commands, Linux packages install next to Reach (`farreach`).
- Updates now come from https://raw.githubusercontent.com/rsipka/FarReach/update-feed/latest.json, so dev builds reach the updater. Builds up to dev.3 need this one installed by hand once.
- iPhone and iPad: an unsigned IPA for LiveContainer. Add https://raw.githubusercontent.com/rsipka/FarReach/update-feed/apps.json as a source in LiveContainer to get later builds as updates.
