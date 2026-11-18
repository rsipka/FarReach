# Reacher CM releases

`release.yml` builds a release for every tag that starts with `v`:

- `vX.Y.Z-dev.N` makes **Reacher CM Dev**, published as a GitHub pre-release.
- `vX.Y.Z` makes a stable release, published as the latest release. Only a
  stable release signs the apt and dnf repositories (secrets
  `APT_GPG_PRIVATE_KEY`, `APT_GPG_PASSPHRASE`) and then redeploys the website,
  which hosts the dnf repository.

## With the script

`npm run release` does the steps below (run it on an up-to-date, clean main;
`gh` signed in lets it open the pull request and check the Build check):

```sh
npm run release -- dev --dry-run   # show what would change, change nothing
npm run release -- dev             # next dev build: bump, draft notes, open a PR
npm run release -- stable          # stable release (dates the CHANGELOG section)
npm run release -- 0.10.0-dev.1    # any version; --minor/--major with dev or stable
# merge the pull request, then:
npm run release -- tag             # checks main, the full Build check, tags and pushes
```

`--direct` commits the bump to main and tags in one go, without a pull
request. `tag --run-ci` starts the full Build check on main when the last
green one is older than the code. `npm run release -- --help` lists all
options. The release notes are drafted from the CHANGELOG entries added since
the last tag; edit them when the script asks, or in the pull request.

A stable release takes build number 999 for Windows and Android
(`X.Y.Z.999`, `X*10000000 + Y*100000 + Z*1000 + 999`), so it installs over
that version's dev builds; release.yml checks this too.

## Making a dev build by hand


1. Set the version to `X.Y.Z-dev.N` in `package.json`, `package-lock.json`
   (twice), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and
   `src-tauri/tauri.conf.json`, and add a `<release version="X.Y.Z-dev.N"
   type="development">` entry at the top of `<releases>` in
   `src-tauri/linux/com.reachercm.client.metainfo.xml` (a Rust test checks it).
2. In `src-tauri/tauri.conf.json` set `bundle.windows.wix.version` to
   `X.Y.Z.N` and `bundle.android.versionCode` to
   `X*10000000 + Y*100000 + Z*1000 + N` (0.9.2-dev.4 is `902004`). Windows and
   Android refuse to install a build whose number did not go up. The workflow
   stops at once if any of these does not match the tag.
3. Merge that to main. Pull requests check Linux only, so make sure the
   full Build check (Windows, macOS, Android) is green on main first: the
   night's run if nothing was merged since, otherwise Actions > Build check >
   Run workflow on main. Then tag main and push the tag:
   `git tag vX.Y.Z-dev.N && git push origin vX.Y.Z-dev.N`.

Pushing the tag starts the Windows, macOS, Linux, Android and iOS builds.
The release is published only when every platform is in it, so a failed
build leaves it as a draft: re-run the failed jobs. Do not make the release
by hand on GitHub; the workflow makes it.

iOS gets the numeric version (`X.Y.Z`) as its marketing version and the dev
number as its build number, since iOS allows only numbers there.

## Update feed

GitHub's `releases/latest` skips pre-releases, so the updaters do not read
it. The last step of a release replaces the `update-feed` branch with one
commit holding that release's `latest.json` and `apps.json`:

- Desktop updater and the Android update check:
  `https://raw.githubusercontent.com/rsipka/ReacherCM/update-feed/latest.json`
- LiveContainer source:
  `https://raw.githubusercontent.com/rsipka/ReacherCM/update-feed/apps.json`

So every install, dev or stable, is offered the newest release of either
kind. Builds from before this feed (up to 0.9.2-dev.3) still read
`releases/latest`; install the next build by hand once. The AppImage's own
zsync updates still follow GitHub's latest release, so they skip dev
builds.

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
same keys to update existing installs. Reacher CM uses its own application ID
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
