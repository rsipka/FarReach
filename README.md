<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="FarReach" width="80" />
</p>

<h1 align="center">FarReach</h1>

<p align="center">
  A fork of <a href="https://github.com/alexandrosnt/Reach">Reach</a>, the modern, cross-platform SSH client and remote management tool.
</p>

<p align="center">
  <a href="https://github.com/rsipka/FarReach/releases">Download</a> · <a href="https://github.com/rsipka/FarReach/issues">Report a Bug</a> · <a href="https://reachssh.com/">Reach documentation</a> · <a href="https://github.com/alexandrosnt/Reach">Upstream Reach</a>
</p>

> **FarReach is built on Reach** by [Alexander Nt.](https://github.com/alexandrosnt) and the
> [Reach contributors](https://github.com/alexandrosnt/Reach/graphs/contributors). Almost everything
> here is their work. FarReach adds features that are tested here first and offered back to Reach
> as pull requests; most of them are meant to land upstream. If you don't need what FarReach adds,
> use [Reach](https://github.com/alexandrosnt/Reach) itself.
>
> FarReach has its own app id, data folder and keychain entries, so it installs and runs next to
> Reach. It starts empty: restore a Reach backup or point it at your sync location to bring data over.

---

## Why FarReach?

PuTTY hasn't changed in decades, MobaXterm is Windows-only, and Termius charges a subscription for the basics. Reach, which FarReach is built on, is an SSH client built from scratch: native Rust networking, a fast UI, encrypted storage, and the same app on Windows, macOS, Linux and Android. No Electron, no subscription, no account.

FarReach adds what people moving from Remote Desktop Manager and similar tools miss most: importing from RDM, saved credentials shared by many sessions, session notes, Telnet, broadcast input, and sync through a folder, SFTP, Git or an SMB share.

## Added in FarReach

- **Import from Remote Desktop Manager**: RDP, SSH, VNC and Telnet entries, folders, logins and notes from an RDM export.
- **Saved credentials**: one login kept once in a vault and used by many sessions, by every session in a folder, or by a jump host.
- **Session notes and custom fields**, like RDM's Documentation tab, encrypted and synced with the session.
- **Telnet sessions** for switches, routers and console servers, with automatic login.
- **Broadcast input**: type into several terminals at once.
- **Favorites** on the start screen, and an option to open sessions with a double click.
- **Local-first sync**: every vault works offline and syncs in the background, through Turso, a folder, SFTP, Git or an SMB share without mounting it. Conflicting edits are kept as copies, not lost.
- **Czech translation**, and early **iOS** builds.

See the [changelog](CHANGELOG.md) for details. Most of these are offered back to Reach.

## Features

### Connect

- **SSH** with tabs, split panes and tab groups, rendered with WebGL.
- **Your `~/.ssh/config`, all of it.** ProxyJump, ProxyCommand, ControlMaster, Kerberos (GSSAPI), X11 forwarding, PKCS#11 and FIDO2 keys, and the rest of the 104 keywords.
- **Remote Desktop (RDP) and VNC** in a tab. RDP shares the clipboard, files and a local folder as a drive.
- **WSL distros and local shells** next to your servers, plus a **serial console** for routers and switches.
- **Host keys checked properly.** Trust on first use, a loud stop when a key changes, and never a silent accept.

### Work

- **Command palette** (Ctrl+Shift+P): any session, tab, page, snippet or action in a few keystrokes.
- **SFTP explorer**: drag and drop, edit in place, compress and extract on the server, drag files straight to your desktop.
- **Port forwarding**: local and SOCKS proxies from the Tunnels panel, plus everything your ssh_config forwards.
- **Snippets** with Tab completion, and **Recipes**: parameterised scripts with a declared risk level, checked before they run.
- **Live server stats** (CPU, memory, disk, network, who's logged in) without installing an agent.
- **Session logging** to plain text files.

### Stay safe

- **Encrypted vault** for sessions, passwords and keys (XChaCha20-Poly1305, Argon2id), with optional encrypted cloud sync.
- **Master password**, plus unlock with Windows Hello, Touch ID, Android biometrics or a FIDO2 security key. Auto-lock when idle or when your computer locks.
- **Password managers**: point a session at a Passbolt entry instead of keeping a copy.
- **Backups** of your sessions and settings, optionally password-protected.
- **Zero-trust defaults**: anything that opens a door, like session sharing or the MCP server, stays off until you switch it on.

### DevOps (opt-in, in Settings → DevOps)

- **Databases**: PostgreSQL, MySQL, MariaDB, SQL Server, SQLite and Redis. A data grid, a schema-aware SQL editor, a table designer that shows the `ALTER` before it runs, and confirmations scaled to the risk. Connect directly or through an SSH session.
- **Containers and Kubernetes**: Docker, Podman, Compose, Kubernetes and Helm on the servers you already reach.
- **Ansible** and **OpenTofu**: run playbooks and plans with streaming output. On Windows, Ansible runs through WSL.

Each tool is off on a fresh install. Switching one off hides it without deleting anything it saved.

### Collaborate

- **Session sharing**: two terminals side by side, peer to peer and end-to-end encrypted. The owner decides whether the other side can type.
- **MCP server**: hand a session to Claude Code, Cursor or any MCP client. Loopback only, with guards against writing blind or reading secrets.
- **AI assistant** with your own API key.

### Make it yours

- **Themes** that colour the whole app, not just the terminal, from a registry pinned by SHA-256.
- **Interface size** (Ctrl+= / Ctrl+-) for the whole window, separate from the terminal's text size.
- **Lua plugins** in a sandbox.
- **Ten languages**: English, Czech, German, Spanish, French, Italian, Greek, Bulgarian, Russian and Chinese.
- **A real phone app**: Esc, Ctrl, Alt and arrow keys above the keyboard, pinch to zoom, and a layout that respects notches and large text.

## Download

Get FarReach from its [releases page](https://github.com/rsipka/FarReach/releases):

- Windows: `.exe`, `.msi`
- macOS: `.dmg`, Apple silicon and Intel
- Linux: `.AppImage`, `.deb`, `.rpm`
- Android: `.apk`

FarReach updates itself from those releases. Reach itself is also on Scoop and in apt/dnf repositories; FarReach is not.

## Built with

[Tauri 2](https://v2.tauri.app), Rust and Svelte 5. The SSH stack is pure Rust ([russh](https://github.com/warp-tech/russh)), with no OpenSSH dependency, and the UI uses the system webview instead of a bundled Chromium, so FarReach stays small and light.

| | |
|---|---|
| **Backend** | Rust, Tokio, russh |
| **Frontend** | Svelte 5, SvelteKit, TypeScript, Tailwind CSS 4 |
| **Terminal** | xterm.js with WebGL |
| **Crypto** | XChaCha20-Poly1305, Argon2id, X25519 |

### Build from source

You'll need [Rust](https://rustup.rs), [Node.js 22+](https://nodejs.org) and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
git clone https://github.com/rsipka/FarReach.git
cd FarReach
npm install
npm run tauri dev     # run it
npm run tauri build   # package it
```

## Community

- **[Reach Discord](https://discord.gg/CSbEybvDVV)**: the upstream Reach community.
- **[FarReach issues](https://github.com/rsipka/FarReach/issues)**: bugs in FarReach and in the features it adds.
- **[Reach issues](https://github.com/alexandrosnt/Reach/issues)**: bugs that Reach has too belong upstream. For a larger change, open an issue there first.
- **[Changelog](CHANGELOG.md)**: what changed in each release.

Pull requests, translations and recipes for the [registry](https://github.com/alexandrosnt/reach-recipes-registry) are all welcome.

## Support Reach

FarReach exists because of Reach. Reach is free and MIT licensed, and it stays that way: nothing is or ever will be paywalled. It's built in spare time with no budget, which is why the installers aren't code-signed yet. If Reach saves you time, **[GitHub Sponsors](https://github.com/sponsors/alexandrosnt)** helps pay for a signing certificate, test hardware, and more evenings spent on Reach. A good bug report or telling a colleague helps just as much.

## Contributors

Thank you to everyone who has sent code to Reach, which FarReach is built on. The picture is drawn from the repository's own contributor list, so a merged pull request puts you in it (it can take a few days to show).

<a href="https://github.com/alexandrosnt/Reach/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=alexandrosnt/Reach&max=100" alt="Contributors to Reach" />
</a>

## License

[MIT](LICENSE)
