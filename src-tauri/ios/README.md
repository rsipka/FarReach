# Building Reacher CM for iOS

Reacher CM builds for iPhone and iPad with Tauri 2's iOS support. This needs a Mac
with Xcode; an Intel Mac works. A free Apple account is enough to run Reacher CM on
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
when you switch away from Reacher CM for more than a few seconds.

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
`src-tauri/gen/apple` (not committed), and puts Reacher CM's icons in it. If init
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

### Unsigned, for LiveContainer

LiveContainer runs an unsigned app and signs it with its own certificate, so
no Apple account is needed:

```sh
npx tauri ios build --no-sign --target aarch64
```

writes `src-tauri/gen/apple/build/arm64/ReacherCM.ipa`; import it in LiveContainer.
Inside LiveContainer the local network permission and the keychain are
LiveContainer's own.

Without a Mac: the **iOS build** workflow (`.github/workflows/ios-build.yml`)
builds the same unsigned IPA on a GitHub macOS runner. Run it from the Actions
tab; it also runs on every push to main and on pull requests in the full CI
tier (see `build-check.yml`); the IPA is the run's
`ReacherCM-ios-unsigned` artifact (a zip holding the `.ipa`).

### Updates through a LiveContainer source

Each tagged release, dev pre-releases included, also attaches
`ReacherCM_<version>_ios_arm64_unsigned.ipa` and `apps.json`, an AltStore-style
source listing that IPA (and up to nine older ones). Add

```
https://raw.githubusercontent.com/rsipka/ReacherCM/update-feed/apps.json
```

in LiveContainer's Sources tab (or open
`livecontainer://source?url=` followed by that address, URL-encoded). The
address is the `update-feed` branch, which `release.yml` rewrites with each
release, so a new build appears as an update. SideStore and AltStore Classic read the same file.
`scripts/ios-source.mjs` writes it in `release.yml`; test it with
`npm run ios-source:test`.

## Bundle identifier

iOS uses the app's own `identifier` from `src-tauri/tauri.conf.json`. Apple
makes every bundle identifier unique to one team, so anyone else building this
fork with their own Apple account changes it to one of their own.

## Free Apple account limits

- An installed build stops opening after 7 days. Build and install again from
  the Mac to renew it.
- At most 3 apps of your own on one device at a time.
- No TestFlight and no App Store. Those, and builds that last a year, need the
  paid Apple Developer Program.
