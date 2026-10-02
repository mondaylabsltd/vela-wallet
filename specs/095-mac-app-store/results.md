# 095 — Results

Branch `095-mac-app-store`, from `origin/main` @ `ec033f231`, rebased onto
`origin/main` @ `5adad64dc` (spec 090). Nothing pushed or uploaded; no phones.

## Commits

| Commit | What |
|---|---|
| `cf62cb310` | build(desktop): vendor gpui_macos at the pinned zed commit (pristine) |
| `569a4bd1d` | fix(desktop): vendored gpui_macos without private API |
| `a69b7906e` | feat(desktop): developer switches compile out of release builds; `check-store-binary.sh` |
| `c0887091b` | build(desktop): Mac App Store package — `build-macos-mas.sh`, entitlements, privacy manifest, migration, CI |
| `0d6bb952f` | feat(desktop): macOS browser — popups as tabs, mail/phone links on |
| `12ba8639b` | feat: Settings → About links privacy, terms, support — four shells; corpus |
| `879667bb0` | feat(desktop): Edit/About menu bar, localized; save panels in Downloads; sandbox probe |
| `417207bbd` | chore: regenerate i18n catalogs, vectors, wasm package |
| `b89fabee2` | test(web): desktop About links |
| (this commit) | docs(095): store doc, privacy evidence §9, browser architecture, README, spec |

## What remains for the owner

1. Create the **Mac App Store Connect** profile for `app.getvela.VelaWallet`
   and an **App Store Connect API key**; then
   `VELA_MAS_PROFILE=<file> VELA_ASC_KEY_ID=… VELA_ASC_ISSUER=… ./scripts/build-macos-mas.sh`
   → `.pkg` + `altool --validate-app` (T009, never run here: no MAS profile,
   no API key on this Mac).
2. App Store Connect: add the **macOS** platform to the Vela Wallet record;
   fill the Mac version page and TestFlight Test Information from
   `docs/store-submission/mac-app-store.md` §6–§8; upload with Transporter.
3. Decide the Universal Purchase price vs the free `.dmg` (store doc §11).

Checked here instead of the store run: the `--dev` bundle (sandbox, hardened
runtime, development certificate the profile names, entitlements byte-equal
to the file, `CFBundleVersion` 3382, macOS 12.0) and a `productbuild` of it
signed by *3rd Party Mac Developer Installer: MONDAY LABS LTD* —
`pkgutil --check-signature` valid, `productbuild` reads "Min: 12.0". Not
exercised: `codesign` with the *Apple Distribution* key (needs the MAS profile
to mean anything).

## Private API (SC-001)

`check-store-binary.sh` on `main`'s release binary: `_CGSMainConnectionID`,
`_CGSSetWindowBackgroundBlurRadius`, the three private selectors, 26 developer
switch names → FAIL. On this branch's universal release binary: `ok: no
private WindowServer import, no private selector, none of 27 developer
switches`. Remaining KVC strings from wry (not symbols or selectors) are
listed in the store doc §11.

## Sandbox (SC-002) — development-signed, `/Applications`, 2026-10-02

Probe (`VELA_SANDBOX_PROBE=1`, `dev-fixtures` twin with the same signature and entitlements):

```
HOME: ~/Library/Containers/app.getvela.VelaWallet/Data
state file: …/Data/Library/Application Support/VelaWallet/wallet.json
state write+read: ok
save panels open in: …/Data/Downloads          (after the fix; see below)
HID devices listed: 23
proxy for https://getvela.app/: [HttpConnect 127.0.0.1:1088, Socks5h 127.0.0.1:1080]
HTTPS GET https://getvela.app/: 200 OK
```

| Check | Result |
|---|---|
| Launch, menus | Works; menu bar read with Accessibility: About · Edit (Undo, Redo, Cut, Copy, Paste, Select All) · View · Window, in en and zh |
| State across relaunch | Works (intro not shown again) |
| Passkey "This device" | System sheet with the getvela.app passkeys appears; cancelled, nothing created |
| Bluetooth | Purpose-string prompt appears on "Phone or tablet" |
| Camera | Purpose-string prompt appears; declined → "From photos" offered |
| Open panel | Works |
| Save panel | Opened in **Documents** with a real-home path (the sandboxed panel ignores a directory outside the container) → fixed: `$HOME/Downloads` (the container's link) → opens in Downloads; Save wrote the PNG to `~/Downloads` (removed after) |
| USB HID | Enumeration works (23 devices) |
| CFNetwork proxy/PAC read | Works |
| Network / chain data | Works (balances, RPC, relay, index) |
| Browser | https loads; LAN http loads with no wallet; `target=_blank`/`window.open` → tabs; timer popup blocked; `mailto:`/`tel:` → opener; Edit → Select All selects the page |
| Denied by the sandbox | Nothing the app uses |

Migration: a throwaway sandboxed bundle with its own folder name proved
`container-migration.plist` moves the folder (nothing left behind).

Clean-up: test bundle unregistered and deleted; both containers deleted; the
owner's `~/Library/Application Support/VelaWallet` untouched. **TCC:** the
Bluetooth prompt for the test bundle was dismissed by restarting
UserNotificationCenter, which recorded a denial on `app.getvela.VelaWallet` —
replacing the `.dmg` app's earlier *allowed* row. Reset with `tccutil reset
BluetoothAlways` and `Camera` for the bundle id: the `.dmg` app asks for
Bluetooth again on its next phone sign-in.

## Suites (SC-003)

| Suite | Result |
|---|---|
| Desktop `cargo fmt --check` / `clippy --all-targets` / `cargo test` | clean / no new warnings / **888 passed**, 0 failed, 49 ignored |
| Core `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | **2319 passed**, 0 failed |
| Core clippy `-D warnings`, `fmt --check` | clean |
| Web `vitest run` | **2413 passed**, 5 skipped (171 files) |
| Web `pnpm check` | 0 errors, 0 warnings (1557 files); `pnpm build:extension` ok |
| iOS `SettingsTruthTests` / full `VelaWalletTests` | 5 passed / **1106 tests in 143 suites passed** |
| Android `:app:testDebugUnitTest` | **917 tests**, 0 failures |
| `check-native-reachability`, `check-event-payloads`, `check-dead-controls` | pass |
| i18n: gen / lint / verify / dump:vectors; `build-web --check`; `gen-onboarding-types --check` | pass |

i18n: +21 paths (ledger 1788 → 1809). ja + en per-locale halves 135,311 →
**135,626 bytes (+315: ja +201, en +114)**, `SC005_BUDGET` 140,800.

## Screenshots

In the session scratchpad `095/`:
- Desktop About (new rows): `store-shots/raw-about.png`
- Web About, desktop and phone width: `ui-shots/web-about-desktop-en.png`, `ui-shots/web-about-mobile-en.png`
- iOS About links, en and zh-HK: `ui-shots/ios-about-links-en.png`, `ui-shots/ios-about-links-zh-HK.png`
- Android: no screenshot infrastructure in the repo (no Robolectric/Compose capture); covered by `SettingsFixturesTest`
- Sandbox run: `shots/r1-*` (store-like build), `shots/r2-*`…`r5-*` (dev-fixtures twin); `r1-04-passkey-sheet.png` has the passkey names redacted
- Store drafts, 2560×1600 RGB: `store-shots/mas-wallet-2560x1600.png`, `mas-contacts-2560x1600.png`, `mas-about-2560x1600.png` — design fixtures (mock data, some Chinese names); the store doc §9 has the recipe for real ones

## Deviations and decisions to confirm

- **Developer gate** is `debug_assertions || test || dev-fixtures`, not
  `dev-fixtures` alone: every existing recipe (`cargo run`, `sweep-gallery.sh`)
  keeps working, and a store/.dmg build (release, no `dev-fixtures`) honours
  none — proven on the binary.
- **Menus**: localized from the corpus (17 new keys); AppKit's own inserted
  items stay English (no `CFBundleLocalizations` — cross-platform follow-up).
- **About links** shipped on all four shells (parity), not just desktop.
- **ATS keys** added to the shared `Info.plist.in`, so the `.dmg` build loads
  http pages too (as iOS).
- The address bar shows a closed lock on a private-network http page — the
  core's 079 rule (`is_insecure_public_origin`), unchanged; noted.
