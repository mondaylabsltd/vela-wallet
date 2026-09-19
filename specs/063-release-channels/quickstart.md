# 063 quickstart — publishing a macOS package people can open

**The founder's choice (2026-09-18): sign on the founder's Mac, upload by hand.** Nothing
that can sign as us is stored on GitHub. CI builds Windows and Linux from the tag and
creates the release; its macOS job has no credentials, so it verifies the build and attaches
nothing; one command on this Mac supplies the three `.dmg` files.

The first release that can carry a macOS package is the **next** one. `desktop-v0.9.2`'s
tag predates `--notarize`, the camera usage string and the camera entitlement, and a release
is built from its tag, exactly.

## A. Once — three things only the account holder can make

**A1. A Developer ID certificate in the company's name (recommended).**
The two in the keychain were issued before the account became an organization and still
read `Developer ID Application: Qin Xie`; that is the name a signed app carries.

> Xcode → Settings → Accounts → team **MONDAY LABS LTD** → Manage Certificates… → **+** →
> **Developer ID Application**

Xcode makes the key, requests the certificate and installs it. **Do not revoke the old
ones**: revoking a Developer ID certificate makes Gatekeeper refuse everything ever signed
with it, other apps included. An unused certificate costs nothing (the limit is five).

**A2. A Developer ID provisioning profile** for this app, with Associated Domains.

> developer.apple.com → Certificates, IDs & Profiles → Profiles → **+** → *Distribution:*
> **Developer ID** → App ID `app.getvela.VelaWallet` → select the certificate from A1 →
> download. Leaving it in `~/Downloads` is enough.

Without it a signed app has no platform passkeys, and newer macOS kills it at launch.

**A3. Notary credentials, kept in your login keychain.**

```bash
xcrun notarytool store-credentials vela-notary --apple-id YOU@example.com --team-id F9W689P9NE
```

It **asks** for the password — nothing secret goes on the command line. Give it an
*app-specific* password (account.apple.com → Sign-In and Security → App-Specific
Passwords), not the Apple ID password.

## B. Each release — one command

After the `desktop-v*` tag is pushed and CI has created the release:

```bash
git fetch --tags && git checkout desktop-v0.9.3
cd app-desktop/vela-wallet
./scripts/release-macos-local.sh desktop-v0.9.3            # builds, signs, notarizes, verifies — uploads nothing
./scripts/release-macos-local.sh desktop-v0.9.3 --upload   # the same, then attaches to the release
```

Before compiling, the script checks and says in words if it cannot continue: the checkout is
the tag and clean; the profile is a *Developer ID* one for this app (it finds it by itself);
the keychain holds the certificate **that profile names** — which is how it chooses between
same-named certificates, by SHA-1 and never by name; the notary credentials work; and, with
`--upload`, the release exists and has no macOS packages yet (`--replace` to overwrite, which
changes checksums under people who already downloaded).

It attaches `VelaWallet-<version>-macos-{universal,arm64,x86_64}.dmg` and `SHA256SUMS-macos`.
About forty minutes cold: two architectures to compile, then six notary round trips.

**Then the ten minutes no script can do** — the first time, and whenever packaging changes:
download one `.dmg` **through a browser** (a file that never left this Mac is not
quarantined and proves nothing about Gatekeeper); open it; make a wallet with **This
device** (proves the profile); open the **scanner** (proves the camera key and entitlement —
the one path that has never run in a packaged app).

## C. The alternative, not chosen: let CI sign

`desktop-macos-packages.yml` can sign and notarize by itself when a `release` environment,
restricted to `desktop-v*` tags, holds `MACOS_CERTIFICATE_P12_BASE64`,
`MACOS_CERTIFICATE_PASSWORD`, `MACOS_PROVISION_PROFILE_BASE64` and notary credentials
(`MACOS_NOTARY_KEY_P8_BASE64` + `MACOS_NOTARY_KEY_ID` + `MACOS_NOTARY_ISSUER_ID`, or
`MACOS_NOTARY_APPLE_ID` + `MACOS_NOTARY_PASSWORD`). It buys a hands-free release and costs a
copy of the signing key on GitHub. The gate is the same either way: no credentials, no
macOS package. If it is ever wanted, set the tag policy on the environment **before** adding
a secret:

```bash
R=mondaylabsltd/vela-wallet
gh api -X PUT repos/$R/environments/release --input - <<'JSON'
{"deployment_branch_policy":{"protected_branches":false,"custom_branch_policies":true}}
JSON
gh api -X POST repos/$R/environments/release/deployment-branch-policies -f name='desktop-v*' -f type=tag
```

The certificate expires **2027-02-01** (a new one from A1: five years from its issue).
Packages notarized before an expiry keep opening; new ones need A1 and A2 again.
