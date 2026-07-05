# macOS build & release setup

TokenTank's code is already cross-platform (Tauri, the autostart plugin, and
the transcript parser all work on macOS; on macOS it runs as a menu-bar app
with no Dock icon). What macOS needs beyond the code is **signing and
notarization** — without it, Gatekeeper tells users "TokenTank is damaged and
can't be opened," which is far worse than Windows' SmartScreen warning.

Two ways to build. CI is recommended so releases are reproducible and you
don't hand-build on a laptop.

---

## Option A — GitHub Actions (recommended)

`.github/workflows/release.yml` already builds Windows + macOS (universal
Intel/Apple Silicon) on every `v*` tag and attaches installers to a draft
GitHub release. It signs the macOS build automatically **if** these repo
secrets are set. Add them under Settings → Secrets and variables → Actions.

| Secret | What it is |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64 of your "Developer ID Application" cert exported as `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Lucas Powell (TEAMID)` |
| `APPLE_ID` | `lucas@growth8020.com` |
| `APPLE_PASSWORD` | An app-specific password (not your Apple ID password) |
| `APPLE_TEAM_ID` | Your 10-character Apple Developer Team ID |

Once the secrets exist: `git tag v0.5.0 && git push origin v0.5.0`. The
workflow builds both platforms, notarizes the mac app, and opens a draft
release for you to review and publish.

---

## One-time Apple setup (with lucas@growth8020.com)

1. **Enrol in the Apple Developer Program** ($99/year) at
   <https://developer.apple.com/programs/> using `lucas@growth8020.com`.
   Enrolment can take a day or two to approve.

2. **Create a "Developer ID Application" certificate.** In Xcode
   (Settings → Accounts → Manage Certificates → +) on your MacBook or Mac
   Mini, or via the developer portal. This is the cert for apps distributed
   *outside* the Mac App Store.

3. **Export it as `.p12`.** In Keychain Access, right-click the certificate
   (with its private key) → Export → `.p12`, set a password. That file +
   password become `APPLE_CERTIFICATE` (base64-encoded:
   `base64 -i cert.p12 | pbcopy`) and `APPLE_CERTIFICATE_PASSWORD`.

4. **Get your Team ID** from the top-right of the developer portal, or
   `APPLE_TEAM_ID` is shown in the certificate name.

5. **Create an app-specific password** at <https://account.apple.com> →
   Sign-In and Security → App-Specific Passwords. That's `APPLE_PASSWORD`
   (used for notarization uploads).

6. **Find your signing identity string:** on a Mac with the cert installed,
   `security find-identity -v -p codesigning` — copy the
   `Developer ID Application: …` line into `APPLE_SIGNING_IDENTITY`.

---

## Option B — build locally on a Mac

On the MacBook or Mac Mini, with the toolchain installed
(`brew install node`, `rustup`, `xcode-select --install`):

```bash
git clone https://github.com/lucaspowell8020/tokentank
cd tokentank
npm ci
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# Signing (same values as the secrets above):
export APPLE_SIGNING_IDENTITY="Developer ID Application: … (TEAMID)"
export APPLE_ID="lucas@growth8020.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="TEAMID"

npx tauri build --target universal-apple-darwin
# → src-tauri/target/universal-apple-darwin/release/bundle/dmg/TokenTank_x.y.z_universal.dmg
```

Then upload the `.dmg` to the GitHub release and point the product page's
macOS download at it.

---

## Follow-ups once macOS ships

- The tray icon is a colored numeric gauge; macOS menu bars conventionally
  use monochrome template images. It will render, but consider a
  Mac-specific monochrome variant if it looks out of place.
- Point the `/tokentank` product page's macOS CTA at the `.dmg` (currently
  an email-notify capture) and update the "Windows first" copy.
