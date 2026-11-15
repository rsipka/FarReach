# Building Reach for iOS

Reach builds for iPhone and iPad with Tauri 2's iOS support. This needs a Mac
with Xcode; an Intel Mac works. A free Apple account is enough to run Reach on
your own devices, with the limits listed at the end.

## What works on iOS

The same as on Android: SSH, SFTP, tunnels, Telnet, RDP (no sound yet), VNC,
databases, Redis, Kubernetes, Docker and Podman over SSH, the vault and its
sync, and AI chat.

Not on iOS, as not on Android: the local terminal, serial ports, ProxyCommand
and LocalCommand, OpenTofu and Ansible run locally, WSL, the X server, USB
security keys, PKCS#11 and security-key middleware, and Kerberos (GSSAPI).
Face ID unlock is not there yet: it needs an iOS half of the
`reach-unlock` plugin.

iOS suspends an app shortly after it leaves the screen, so open sessions drop
when you switch away from Reach for more than a few seconds.

## One-time setup on the Mac

1. **Xcode** from the App Store. It has to be new enough for the iOS version
   on your iPhone: an Xcode older than the phone's iOS cannot install on it.
   Open Xcode once to accept its license and let it install its components,
   then point the command-line tools at it:

   ```sh
   sudo xcode-select -s /Applications/Xcode.app
   ```

2. **Homebrew, CocoaPods, Node and Rust**:

   ```sh
   brew install cocoapods node
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
   ```

   `aarch64-apple-ios` is the iPhone. The simulator is `x86_64-apple-ios` on an
   Intel Mac and `aarch64-apple-ios-sim` on Apple silicon.

3. **Your Apple account in Xcode**: Xcode › Settings › Accounts › + › Apple
   ID. A free account becomes a "Personal Team". Select it, then Manage
   Certificates › + › Apple Development, so Xcode has a signing certificate.

4. **The iPhone**: connect it by cable and tap Trust. On iOS 16 or later, turn
   on Settings › Privacy & Security › Developer Mode (the phone restarts).

## Build

From the repository root:

```sh
npm ci
./scripts/ios-init.sh
```

`ios-init.sh` runs `tauri ios init`, which writes the Xcode project to
`src-tauri/gen/apple` (not committed), and puts Reach's icons in it. If init
does not find your team on its own, set it first:

```sh
export APPLE_DEVELOPMENT_TEAM=XXXXXXXXXX
```

The team ID is the "Organizational Unit" of your Apple Development certificate
in Keychain Access.

### In the simulator (no signing needed)

```sh
npx tauri ios dev
```

and pick a simulator. The first build compiles every Rust dependency for iOS
and takes a while, longer on an Intel Mac; later builds are incremental.

### On your iPhone

With the phone connected:

```sh
npx tauri ios dev
```

and pick the phone. This runs against the development server on the Mac, so
the phone and the Mac must be on the same network.

For a build that runs on its own:

```sh
npx tauri ios build --export-method debugging
```

then install the `.ipa` from `src-tauri/gen/apple/build` through Xcode ›
Window › Devices and Simulators › your phone › +. You can also open the project
with `npx tauri ios dev --open` and press Run in Xcode.

The first time, iOS refuses to open an app from a free account: Settings ›
General › VPN & Device Management › your Apple ID › Trust.

## Bundle identifier

`src-tauri/tauri.ios.conf.json` sets `io.github.rsipka.reach`. Apple makes
every bundle identifier unique to one team, so anyone else building this fork
changes it to one of their own.

## Free Apple account limits

- An installed build stops opening after 7 days. Build and install again from
  the Mac to renew it.
- At most 3 apps of your own on one device at a time.
- No TestFlight and no App Store. Those, and builds that last a year, need the
  paid Apple Developer Program.
