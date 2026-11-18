# CI runners

## Linux

- **ubuntu-22.04** for the Linux app build (release.yml) and the Linux Rust
  tests and clippy (build-check.yml). A Linux app only starts on systems whose
  glibc and webkit2gtk are at least as new as the build machine's, so the
  oldest supported Ubuntu gives packages that run on Ubuntu 22.04, Debian 12,
  Mint 21 and newer. Move both to 24.04 together when GitHub retires the 22.04
  image (expected around 2027).
- **ubuntu-24.04** for the Android jobs and publish-release: heavier jobs
  that should not change image under us when `ubuntu-latest` moves.
- **ubuntu-latest** for the light jobs (change detection, results, frontend,
  CodeQL, Pages, create-release).

## macOS: GitHub's Macs or our own

Every macOS job runs on `macos-15` (GitHub's), unless the repository variable
`MACOS_RUNNER_LABEL` is set. Then the macOS jobs started by a push to main, a
release tag, the nightly run or Run workflow go to the self-hosted runner with
that label. Pull requests (forks and Dependabot too) always use GitHub's Macs:
this repository is public, and a pull request must never run code on a
personal computer.

GitHub has no automatic fallback. While the variable is set and the Mac is
off or asleep, those jobs wait in the queue (up to a day). Clear the variable
to go back to GitHub's Macs.

`rerun-macos.yml` starts a failed run once more when the only failures are
macOS jobs that never really ran (no step failed, for example no runner was
found).

### Setting up a Mac as the runner

Needs macOS 26 (Intel or Apple silicon), about 40 GB free, and the Mac on
power.

1. Install **Xcode** from the App Store, open it once, then in Terminal:
   `sudo xcodebuild -license accept`
2. Install **Homebrew** (https://brew.sh) and then `brew install xcodegen`.
3. Install **Rust** with rustup (https://rustup.rs). The workflows pick the
   Rust version and targets; Node is downloaded by the workflow.
   The jobs put rustup's `~/.cargo/bin` first in PATH, so another Rust
   (MacPorts, Homebrew) on the Mac does not get in the way.
4. On GitHub: repository **Settings > Actions > Runners > New self-hosted
   runner**, choose macOS and your architecture (x64 for Intel). Run the
   *Download* commands it shows in Terminal, in a folder such as
   `~/actions-runner`.
5. Run its *Configure* command, adding a label and name:
   `./config.sh --url https://github.com/<owner>/<repo> --token <token> --labels reachercm-mac --name my-mac`
   Accept the defaults for the runner group and work folder.
6. Start it as a background service, so it runs whenever you are logged in:
   `./svc.sh install && ./svc.sh start`
7. Keep the Mac awake on power: System Settings > Battery > Options >
   *Prevent automatic sleeping on power adapter when the display is off*.
8. On GitHub: **Settings > Secrets and variables > Actions > Variables >
   New repository variable**, name `MACOS_RUNNER_LABEL`, value `reachercm-mac`.
9. Try it: **Actions > iOS build > Run workflow** on main. The job should
   show the runner name `my-mac`.

To stop using it, delete the variable (step 8). To remove the runner:
`./svc.sh uninstall`, then `./config.sh remove --token <token>` with a token
from the runner's page on GitHub.

Release builds on the Mac use the repository's secrets (the updater signing
key, and Apple signing if it is ever set up), so only run the runner on a
Mac you trust.
