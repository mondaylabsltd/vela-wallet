# 063 research — what was checked, and how

Every claim the spec leans on, with the command or file that established it (2026-09-18).

## R1. Why the Android package would not install

`apksigner verify vela-wallet-android-0.9.2-unsigned.apk` → `DOES NOT VERIFY — Missing
META-INF/MANIFEST.MF`; no v1, v2 or v3 signature block. An unsigned APK cannot be parsed by
the package installer, which reports it as *package info is null*. Signed with a key the
device trusts (`zipalign` + `apksigner sign`), the same file installed on the founder's
phone (`adb install -r` → `Success`, `versionName=0.9.2`). Deliberate, not a regression:
`android-package.yml` said "UNSIGNED" in its first line.

## R2. Why the macOS package would not open

`build-macos-app.sh` took its ad-hoc branch in CI (`codesign --sign -`), and printed the
consequence itself: "ad-hoc signed, not notarized". A browser download is quarantined;
Gatekeeper refuses a quarantined, un-notarized bundle as *damaged*. The ad-hoc branch also
cannot carry `associated-domains`, so a bundle opened by force has no platform passkeys.

## R3. A self-built phone app (the founder's correction)

The first analysis said a self-built app "cannot create a wallet". Wrong. Three ceremonies
consult no OS association, by design and in their own header comments:

- `HybridCeremony.kt` / `HybridCeremony.swift` — "The Noise handshake and CTAP framing are
  the core's (`vela_core::cable`)".
- `UsbSecurityKeyCeremony.kt` — "the GMS-free security-key path … so a phone without Google
  services can still make and use a wallet with a hardware key".
- `SmartCardCtapCeremony.swift` — "consults no association and no Apple service; it is the
  escape hatch (FR-009c)".

What a self-built app loses is the platform authenticator, and on Android the GMS FIDO2 path
(`SecurityKeyCeremony.kt`), both of which check the signing identity against `getvela.app`.

## R4. The signing material on the founder's Mac

- `security find-identity -v -p codesigning`: **two** `Developer ID Application: Qin Xie
  (F9W689P9NE)` identities (issued 2023-08-19 and 2025-02-13, both to 2027-02-01), plus two
  `Apple Development` ones. Signing by *name* is therefore ambiguous on this Mac — the
  workflow and the docs use the SHA-1.
- Provisioning profiles: a **development** profile for `app.getvela.VelaWallet` with
  Associated Domains (to 2027-07-28) — so the App ID and the capability exist — and a
  Developer ID profile for a *different* app. None for this app. → N1.
- The certificates are issued to a person's name. That is what an *individual* enrollment
  looks like, and it matters beyond this spec → N6.

## R5. The rehearsal (`--distribution`, no notary)

Run locally against a real `aarch64` release build, the newer Developer ID identity, and
the development profile standing in for the missing one:

```
flags=0x10000(runtime)                      ← hardened runtime
Authority=Developer ID Application: Qin Xie (F9W689P9NE)
Timestamp=Sep 18, 2026 at 17:22:26          ← secure timestamp (the notary requires it)
com.apple.security.device.camera => true    ← FR-004
NSCameraUsageDescription = "Vela uses the camera to scan a QR code — …"
dmg: Authority=Developer ID Application …   ← the image is signed too
spctl: rejected — source=Unnotarized Developer ID
```

The last line is the right answer: everything Gatekeeper wants except Apple's ticket.
**Not rehearsed**: `notarytool` itself (needs N3), and a launch of the signed bundle (needs
N1 — a development profile under a Developer ID signature is not launchable).

The workflow's profile validation was run against both real profiles: the development one is
refused as "a development profile (it lists devices)", the other app's as "is for
'F9W689P9NE.com.shelchin.speakit'", and the certificate-in-profile loop matched both real
Developer ID certificates against a real Developer ID profile.

## R6. Can the desktop app go on the stores? (read-only survey; nothing built)

**Microsoft Store — no blocker found.** Passkeys already go through `webauthn.dll`
(`src/ctap/mod.rs:19`), CCID keys through PC/SC unprivileged, state lives under
`dirs::config_dir()`, no self-updater, no driver, no elevation. A *paid* listing needs MSIX
(to our understanding the Store does not collect payment for `.exe`/`.msi` submissions —
confirm in Partner Center); MSIX is also signed by the Store, which removes the certificate
question. Untested: WACK, and `webview_absent.rs` under MSIX.

**Mac App Store — two known obstacles.**
1. `gpui_macos/src/window.rs:121-123` (pinned rev `c97b7c0`) declares and calls
   `CGSMainConnectionID` / `CGSSetWindowBackgroundBlurRadius` — private API, by its own
   comment. App Store upload scans binaries for non-public symbols. Zed does not ship there,
   so upstream has no reason to change it; we would carry a patch.
2. The App Sandbox has never been on. `hidapi`, `pcsc`, CoreBluetooth, `nokhwa`, `wry` and
   the `scutil` subprocess (`src/executor/proxy.rs:428`) each need an entitlement and a
   real-device test inside it.
   Neither applies to Developer ID distribution, which is what this spec ships.

## R7. The stores, as the public sees them (2026-09-18)

`play.google.com/store/apps/details?id=app.getvela.wallet` → **404** with `gl=` US, HK, SG,
JP, DE, GB, TW. `itunes.apple.com/lookup?bundleId=app.getvela.VelaWallet` → `resultCount 0`.
The download page therefore keeps both as "coming soon" until N5 is answered; a link to a
404 is worse than a chip that says not yet.

## R8. `gh release create`: notes

`gh release create --help`: "Additional release notes can be prepended to automatically
generated notes by using the `--notes` flag." `--notes-file` feeds the same field, so the
install notes open the release and the generated changelog follows.
