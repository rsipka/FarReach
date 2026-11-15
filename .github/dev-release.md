# FarReach Dev releases

Use a tag such as `v0.9.2-dev.1`, matching the version in `package.json`,
`package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and
`src-tauri/tauri.conf.json`. Increment the Android `versionCode` and the
Windows WiX version when preparing another dev build. The workflow uses
the numeric upstream version as the iOS marketing version and assigns a
separate App Store Connect build number.

Pushing the tag starts the Windows, macOS, Linux, Android, and iOS builds.
GitHub publishes the downloads as the fork's latest release after the
packages pass verification and Apple accepts the TestFlight upload.
Apple then processes the build before it appears in TestFlight.
Linux packages are available as direct downloads.

## GitHub Actions setup

Add these under **Settings → Secrets and variables → Actions** in the fork.

| Secret | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of the fork's Tauri updater private key file, generated with `npx tauri signer generate -w /path/outside/the/repo/reach-dev.key`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password used when generating the updater key, if set. |
| `ANDROID_KEYSTORE_BASE64` | Base64-encoded Android release keystore with alias `reach`. |
| `ANDROID_KEYSTORE_PASSWORD` | Password shared by the Android keystore and its signing key. |
| `IOS_CERTIFICATE` | Base64-encoded Apple Distribution certificate and private key exported as a `.p12` file. |
| `IOS_CERTIFICATE_PASSWORD` | Password used to export the `.p12`. |
| `IOS_MOBILE_PROVISION` | Base64-encoded App Store Connect distribution provisioning profile for `com.farreach.client`. |
| `APP_STORE_CONNECT_KEY` | Contents of the App Store Connect team API key's `.p8` file. |

| Variable | Value |
| --- | --- |
| `TAURI_SIGNING_PUBLIC_KEY` | Contents of the updater key's `.pub` file. |
| `APPLE_DEVELOPMENT_TEAM` | Apple Developer Team ID. |
| `APP_STORE_CONNECT_KEY_ID` | App Store Connect team API key ID. |
| `APP_STORE_CONNECT_ISSUER_ID` | App Store Connect team API issuer ID. |

Keep backups of the Tauri and Android signing keys. Future builds need the
same keys to update existing installs. FarReach uses its own application ID
and can be installed alongside upstream Reach.

To create a new Android keystore, run the following with a JDK installed.
Use the same password for both prompts and store the keystore outside the repo.

```sh
keytool -genkeypair -keystore /path/outside/the/repo/reach-dev.jks \
  -storetype JKS -alias reach -keyalg RSA -keysize 2048 -validity 10000
```

Use a paid Apple Developer team and create an App Store Connect app record
for `com.farreach.client`. Set up its TestFlight testers in App Store
Connect. Apple may ask for export compliance information after upload.

The macOS desktop build uses ad-hoc signing by default. For Developer ID
signing and notarization, configure the separate `APPLE_SIGNING_IDENTITY`,
`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`,
`APPLE_PASSWORD`, and `APPLE_TEAM_ID` secrets.
