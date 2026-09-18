# 063 quickstart — publishing a macOS package people can open

Everything else in this spec is done. This is the part only the account holder can do:
three credentials, then one environment. About twenty minutes, most of it Apple's pages.

## 0. What you are making

| Secret (environment `release`) | What it is |
|---|---|
| `MACOS_CERTIFICATE_P12_BASE64` | the **Developer ID Application** certificate + private key, exported as `.p12`, base64 |
| `MACOS_CERTIFICATE_PASSWORD` | the password you give that `.p12` when exporting |
| `MACOS_PROVISION_PROFILE_BASE64` | a **Developer ID** provisioning profile for `app.getvela.VelaWallet` with Associated Domains, base64 |
| `MACOS_NOTARY_KEY_P8_BASE64` + `MACOS_NOTARY_KEY_ID` + `MACOS_NOTARY_ISSUER_ID` | an App Store Connect API key, for the notary service |
| *(or instead)* `MACOS_NOTARY_APPLE_ID` + `MACOS_NOTARY_PASSWORD` | an Apple ID and an **app-specific** password |

## 1. The provisioning profile (N1)

This Mac already has the *development* profile for the App ID, so the App ID exists and has
Associated Domains enabled. What is missing is the distribution one.

developer.apple.com → Account → Certificates, IDs & Profiles → **Profiles** → **+**
→ under *Distribution* choose **Developer ID** → App ID `app.getvela.VelaWallet` → select the
Developer ID Application certificate → name it `VelaWallet Developer ID` → download.

If the page offers **two** certificates with the same name: there are two in your keychain
(issued 2023-08-19 and 2025-02-13, both valid to 2027-02-01). Pick one and remember which —
the `.p12` in step 2 must be the same one. The workflow checks this and says so in words if
they differ.

## 2. The certificate (N2)

Keychain Access → *login* → *My Certificates* → `Developer ID Application: Qin Xie (F9W689P9NE)`
— the one whose *Not Valid Before* matches your choice in step 1; expand it to confirm a
private key sits under it → right-click → **Export…** → `.p12` → give it a password.

Not `Apple Development` and not `Apple Distribution`: neither can sign a package for
download, and the workflow refuses them by name.

## 3. Notary credentials (N3)

appstoreconnect.apple.com → Users and Access → **Integrations** → App Store Connect API →
Team Keys → **+** → name `vela-notary`, access *Developer* → download the `.p8` (offered
once) and note the **Key ID** and the **Issuer ID** at the top of the page.

If the account cannot create team keys: account.apple.com → Sign-In and Security →
App-Specific Passwords → generate one, and use the Apple-ID pair of secrets instead. (An
app-specific password is revoked whenever the Apple ID password changes; the API key is not.)

## 4. Prove it on this Mac before touching CI (recommended — also does N8)

```bash
cd app-desktop/vela-wallet

# once: puts the notary credentials in your login keychain under a name
xcrun notarytool store-credentials vela-notary \
  --key ~/Downloads/AuthKey_XXXXXXXXXX.p8 --key-id XXXXXXXXXX --issuer xxxxxxxx-xxxx-…

security find-identity -v -p codesigning        # copy the SHA-1 of the certificate you chose

VELA_SIGN_IDENTITY=<that SHA-1> \
VELA_PROVISION_PROFILE=~/Downloads/VelaWallet_Developer_ID.provisionprofile \
VELA_NOTARY_PROFILE=vela-notary \
./scripts/build-macos-app.sh --arch arm64 --notarize
```

It ends with *"Signed, notarized and stapled: this is a package a person can open."* or with
Apple's own log naming what it refused. Then the ten minutes that no script can do:

1. AirDrop or upload the `.dmg` somewhere and **download it through a browser** (a file that
   never left the Mac is not quarantined, and proves nothing about Gatekeeper).
2. Open it, drag to Applications, launch. No warning beyond "downloaded from the internet".
3. Create or open a wallet with **This device** — proves the profile and `associated-domains`.
4. Open the **scanner** — proves `NSCameraUsageDescription` and the camera entitlement. This
   is the one path that has never run in a packaged app.

## 5. The environment (N4) — policy first, secrets second

```bash
R=mondaylabsltd/vela-wallet

# `release`: reachable only from desktop-v* tags. Do this BEFORE adding any secret.
gh api -X PUT repos/$R/environments/release --input - <<'JSON'
{"deployment_branch_policy":{"protected_branches":false,"custom_branch_policies":true}}
JSON
gh api -X POST repos/$R/environments/release/deployment-branch-policies \
  -f name='desktop-v*' -f type=tag

# `build-check`: deliberately empty. Every non-tag run uses it and finds nothing.
gh api -X PUT repos/$R/environments/build-check

base64 -i DeveloperID.p12                       | gh secret set MACOS_CERTIFICATE_P12_BASE64   --env release -R $R
gh secret set MACOS_CERTIFICATE_PASSWORD                                                       --env release -R $R
base64 -i VelaWallet_Developer_ID.provisionprofile | gh secret set MACOS_PROVISION_PROFILE_BASE64 --env release -R $R
base64 -i AuthKey_XXXXXXXXXX.p8                 | gh secret set MACOS_NOTARY_KEY_P8_BASE64     --env release -R $R
gh secret set MACOS_NOTARY_KEY_ID                                                              --env release -R $R
gh secret set MACOS_NOTARY_ISSUER_ID                                                           --env release -R $R
```

Then delete the `.p12` and the `.p8` from Downloads, and keep one offline copy of each (a
password manager is fine). Losing them is recoverable — Apple reissues both — but not quickly.

## 6. What the next `desktop-v*` tag does

- **Credentials complete** → the macOS job signs, notarizes (six round trips: app and image
  for each of three architectures), verifies with `stapler validate` + `spctl`, and attaches
  three `.dmg` files and `SHA256SUMS-macos`.
- **Anything missing or wrong** → the job says what, in words, and attaches nothing. Windows
  and Linux publish regardless; the release notes already tell a Mac user what an absent
  `.dmg` means.

The credentials cannot be exercised from a branch — that is the point of the tag policy — so
the first real tag after step 5 is the CI test. Step 4 is how you know it will pass.

The certificate expires **2027-02-01**. Packages notarized before then keep opening; new ones
need a renewed certificate, a regenerated profile, and steps 1, 2 and 5 again.
