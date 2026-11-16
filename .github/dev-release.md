# FarReach Dev releases

Use a tag such as `v0.9.2-dev.1`, matching the version in `package.json`,
`package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and
`src-tauri/tauri.conf.json`. Increment the Android `versionCode` and the
Windows WiX version when preparing another dev build. The workflow uses
the numeric upstream version as the iOS marketing version and assigns a
separate bundle build number.

Pushing the tag starts the Windows, macOS, Linux, Android, and iOS builds.
GitHub publishes the downloads as the fork's latest release after the
packages pass verification. Download the unsigned iOS IPA from the release
and import it into LiveContainer. The iOS job uses Tauri's `--no-sign` option
and requires no Apple credentials. Linux packages are available as direct
downloads.

## GitHub Actions setup

Add these under **Settings → Secrets and variables → Actions** in the fork.

| Secret | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of the fork's Tauri updater private key file, generated with `npx tauri signer generate -w /path/outside/the/repo/reach-dev.key`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password used when generating the updater key, if set. |
| `ANDROID_KEYSTORE_BASE64` | Base64-encoded Android release keystore with alias `reach`. |
| `ANDROID_KEYSTORE_PASSWORD` | Password shared by the Android keystore and its signing key. |

| Variable | Value |
| --- | --- |
| `TAURI_SIGNING_PUBLIC_KEY` | Contents of the updater key's `.pub` file. |

Keep backups of the Tauri and Android signing keys. Future builds need the
same keys to update existing installs. FarReach uses its own application ID
and can be installed alongside upstream Reach.

To create a new Android keystore, run the following with a JDK installed.
Use the same password for both prompts and store the keystore outside the repo.

```sh
keytool -genkeypair -keystore /path/outside/the/repo/reach-dev.jks \
  -storetype JKS -alias reach -keyalg RSA -keysize 2048 -validity 10000
```

The macOS desktop build uses ad-hoc signing by default. For Developer ID
signing and notarization, configure the separate `APPLE_SIGNING_IDENTITY`,
`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`,
`APPLE_PASSWORD`, and `APPLE_TEAM_ID` secrets.
