# 091 — Results

Branch `091-debug-mode-lan-dapps`, from `origin/main` @ `ec033f231`. Nothing pushed.

## Commits

| Commit | What |
|---|---|
| `56ffb6a82` | core: `offers_wallet`, the private-host tables and `private_host_js`, `provider_script(host, debug_mode)`, `DebugModeChanged`, `prefs::DebugMode` + `version_tapped`, bindings, i18n, regenerated artefacts, web cross-test |
| `daa6f57f5` | desktop: About entry + switch, preferences, `BrowserHost::follow_debug_mode`, the one webview rebuilt |
| `041ce0b15` | iOS: About entry + switch, preferences, per-tab script swap |
| `98c1eee79` | Android: About entry + switch, preferences, per-tab script swap |
| `08d66cd56` | docs: spec, plan, tasks, results; 088 results; ARCHITECTURE; store review notes |
| `d7294b3b3` | second ruling, core: the build fact gates the reading and the taps; the mode leaves `PrefsRecord`; regenerated artefacts |
| `88a6e0518` | second ruling, shells: the build fact on desktop, iOS and Android; Android debug-only cleartext; iOS `Info.plist` back to `main` |
| (this commit) | second ruling, docs |

## Second ruling (2026-10-02): developer builds only

"正式版应该都是禁止的吧，只有调试开发的时候能就行": store builds forbid it entirely.

- **Core.**
  - `prefs::debug_mode(entries, developer_build)` is the only reading of `vela.debugMode`, and it returns hidden
    (off) outside a developer build, whatever is stored.
  - `version_tapped(…, developer_build)` counts nothing there.
  - The mode left the shared `prefs::read` / `PrefsRecord` / wasm `prefsRead`, so no shell can read it ungated.
  - UniFFI: `prefs_debug_mode(entries, developer_build)`, `prefs_version_tapped(…, developer_build)`.
- **Build facts** (the builds that carry the parallel space; the shells pass this and nothing else):
  - Android `BuildConfig.DEBUG` (the `debug` variant);
  - iOS `#if DEBUG` (`DebugMode.developerBuild`);
  - desktop `cfg!(feature = "dev-fixtures")` (`preferences::DEVELOPER_BUILD`, the compile-time half of
    `parallel_space::active`).
- **A store build** never reveals on taps, draws no row, ignores a stored `on`, tells the browser
  `DebugModeChanged { on: false }` and injects the spec-088 script. Each piece is tested in the core and in
  each shell.
- **Android cleartext.**
  - Release is unchanged: no network security config and targetSdk 36, so all cleartext is refused.
  - Only `src/debug/res/xml/network_security_config.xml` gains `<base-config cleartextTrafficPermitted="true"/>`.
    Its `127.0.0.1` / `localhost` entries stay.
  - `CleartextPolicyTest` reads the source sets: release grants none (no config in `main` or `release`, no
    `usesCleartextTraffic`, targetSdk ≥ 28), and debug allows it.
- **iOS.**
  - `Info.plist` is byte-identical to `main`; the comment line added earlier is reverted.
  - It already allows web-content loads and local networking for every configuration (spec 088), so no
    per-configuration key is needed.
  - Shown on a simulator: see the evidence below.
- **Desktop Windows.** The rebuild path is unchanged (`set_debug_mode` retires the view outside the paint pass;
  `place` rebuilds it, as the dead-engine path does). It is compiled and tested on macOS only. The app crate
  cannot be cross-checked for Windows (`check-windows.sh` header).

## The rule (core)

- `dapp_permissions::offers_wallet(origin, debug_mode)`:
  - debug mode **off** is `is_secure_context` exactly (spec 088);
  - debug mode **on** adds `http` on a host that `is_loopback_or_private_host` accepts.
  The host rule is now four tables (names, `.local`, IPv6 first-group ranges, IPv4 ranges), with the same
  behaviour as before. `insecure_origin_classification_table` passes unchanged.
- `private_host_js()` writes the same tables into the JS function the debug script runs on
  `location.hostname`.
- `provider_script(host, false)` is byte-identical to spec 088. `provider_script(host, true)` differs only in
  its first line.
- `dapp_browser`:
  - `Event::DebugModeChanged { on }`, and the gate asks `offers_wallet`.
  - Turning debug mode off retires every open document that is no longer offered: 4900 for each open request,
    the sheet cancelled, and the signing line moved on.
  - A tab not offered the wallet shows no connection.
- `prefs`: `vela.debugMode` is `off` / `on`. Absent or unknown reads as hidden, and hidden means off.
  `version_tapped` counts 7 taps, each ≤ 1,000 ms after the one before. Both work in developer builds only
  (see above).

## When a change applies (what each shell does)

| Shell | Mechanism | A page already open |
|---|---|---|
| iOS | `WKUserContentController`: the tab's one user script is swapped (`ProviderBridge.replaceScript`) | Turned on: it gets the wallet at its next document. Turned off: the core withdraws the wallet at once (4900), and the next document has no provider. |
| Android | `addDocumentStartJavaScript`'s `ScriptHandler` is kept, removed and replaced (`ProviderScript.swap`) | Same as iOS. |
| Desktop | wry takes initialization scripts only at build, so `webview::set_debug_mode` retires a view built for the other mode, outside the paint pass. The core is told `page_gone`, and the next `place` rebuilds the view at the page it showed. | The page loads again with the new script when the browser is next drawn. |

## Web and the extension

- **Web wallet:** it has no in-app browser, so there is nothing to switch.
- **Chrome extension:** spec 088 did not close LAN http here. The extension bundles `provider/inpage.js`
  directly as a MAIN-world content script on `*://*/*`, with no secure-context gate. `provider_script` is not
  used. So the extension already offers the wallet to LAN http pages. A signature from **public** http is
  refused by `popup_origin_refusal` (spec 089). No switch was added.
- **Linux desktop:** it has no in-app browser (`Section::Explore.available()`), so About's version reveals
  nothing there.

## Owner decisions (resolved)

1. **Android cleartext.** Resolved by the second ruling: release stays without cleartext, and the debug build
   allows it (above).
   - Still worth knowing: the app's own typed-in endpoints (per-network RPC override, the four service
     endpoints, the registry URL) have no https check in the core. On release the platform block is their
     guard.
   - On a debug build that block is now gone for them too. That is acceptable for a developer build, but an
     https-or-loopback rule in the core would make it independent of the build.
2. **App Review 2.3.1(a).** A store build has no hidden feature. §3b of
   `docs/store-submission/privacy-and-review.md` now has one line saying debug mode exists only in developer
   builds. §4 is unchanged.

## Evidence

### Core

- `offers_wallet_table`: 43 origins, both modes. It includes `10.0.0.1.evil.com`, `192.168.1.5.nip.io`,
  `192.168.1.5:3000`, `[fd00::1]`, `[fc12:3456::1]`, `[fe80::2]`, `foo.local`, `FOO.LOCAL:8080`,
  `foo.local.evil.com`, `local`, `172.15/172.32`, `[2001:db8::1]`, `[fe81::1]`, `[::ffff:192.168.1.5]`,
  `999.1.1.1`, public http, https, loopback, `ws:`, `file:` and garbage.
- Machine tests: `debug_mode_offers_the_wallet_to_a_lan_page` (7 LAN origins connect and sign),
  `debug_mode_never_offers_the_wallet_to_public_http` (8 public origins),
  `without_debug_mode_a_lan_page_is_not_answered`,
  `turning_debug_mode_off_withdraws_the_wallet_from_an_open_lan_page` (4900 × 2, sheet cancelled, the https
  tab's signature opens next and is answered), and `turning_debug_mode_on_applies_from_the_next_document`.
- Prefs: `debug_mode_reads_hidden_until_revealed`, `seven_quick_taps_reveal_the_switch`,
  `a_slow_tap_starts_the_count_again` (exactly 1,000 ms counts, 1,001 ms does not, a clock going backwards
  does not), `once_revealed_nothing_counts`.
- Script: `script_is_one_classic_script_per_host` and `the_scripts_host_test_is_the_cores_tables`.

### The generated JS against the Rust rule

- **Web (Node VM):** `core-table.test.ts` runs the whole `dappProviderScript('desktop', debug)` in a VM for
  51 origins, both modes, and asserts "said hello" equals `dappOffersWallet(new URL(o).origin, debug)`.
  `http://192.168.1` and `http://0x7f.1` are normalised by the URL parser first, as a WebView does.
- **iOS (JavaScriptCore):** `ProviderScriptTests.theScriptsGateIsTheCoresRule` runs the same check for 20
  origins.

### Desktop GUI run (macOS, debug build, isolated `VELA_STATE_DIR`)

- Seven HID clicks on the version stored `vela.debugMode = off`; the row and the notice appeared.
- One click on the row stored `on`.
- Both in the `VELA_PAGE=settings` About.
- After the second ruling, both builds of the same About:
  - default (non-`dev-fixtures`) build with `on` stored: no row;
  - `dev-fixtures` build: the row, on.
- Not run: an end-to-end LAN page in the desktop browser. That route needs a signed-in live wallet; the
  mock `VELA_PAGE=explore` does not load the webview.

### iOS Debug build: a LAN http page in WKWebView (simulator)

A temporary in-app-hosted test (not committed) loaded `http://192.168.0.7:8765/` (this Mac's LAN address) in a
`BrowserEngine`. The page reports to its own server:

```
GET /?run=ios-debug-on   → /probe?eth=true&secure=false&vela=true
GET /?run=ios-debug-off  → /probe?eth=false&secure=false&vela=false
```

So the Debug app's ATS lets the LAN page load. `isSecureContext` is false there, and the core-written debug
script installs the provider only when debug mode is on. Log:
`scratchpad/ios-lan-probe-evidence.log`.

### i18n

- `about.debugMode`, `about.debugModeBody`, `about.debugModeRevealed` in 15 locales. zh-HK is written
  Cantonese (「畀本地網絡上嘅 http 網頁用個錢包。只係開發用。」); zh-TW uses 除錯.
- ja + en: +451 bytes (en +181, ja +270). Runtime JSON residency is 139,093 of 139,800; the compiled-catalog
  cold start is 135,065.

## Test totals (after the second ruling)

| Suite | Result |
|---|---|
| core `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2320 passed, 0 failed |
| core `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings` | clean |
| core `cargo fmt --all --check`, `build-web --check`, `gen-onboarding-types --check` | clean, current |
| web `npx vitest run` | 170 files, 2391 passed, 5 skipped |
| web `pnpm check` | 0 errors, 0 warnings |
| desktop `cargo fmt --check` / `cargo clippy --all-targets` | clean / no new warnings |
| desktop `cargo test` (as CI) / `cargo test --features dev-fixtures` | 883 passed, 49 ignored / 887 passed, 50 ignored |
| Android `:app:testDebugUnitTest` | 920 tests in 106 suites, 0 failed |
| iOS `VelaWalletTests` (own simulator clone, deleted) | 1115 tests in 142 suites passed |
| `check-native-reachability` / `check-event-payloads` / `check-dead-controls` | ok / 0 mismatches / 0 |

## Screenshots

`/private/tmp/claude-501/-Volumes-data-production-vela-wallet/d4a496f4-ab96-4481-916a-65326d48067e/scratchpad/shots/`:

- desktop: `desktop/about-debug-on-en.png`, `about-debug-off-en.png`, `about-debug-on-zh-dark.png`,
  `about-debug-off-zh-dark.png`, `about-hidden-en.png`, `reveal-1.png` (after 7 taps: row + notice).
  After the second ruling, in `desktop-rework/`:
  - `nondev-build-stored-on-en.png`: default build, `on` stored, no row;
  - `dev-build-on-en.png`: `dev-fixtures` build;
  - `dev-build-off-zh-dark.png`.
- iOS: `ios/091-about-hidden-en.png`, `091-about-revealed-notice-en.png`, `091-about-switched-on-en.png`,
  `091-about-debug-{off,on}-{en,zh}.png`
- Android (own emulator, Pixel 7 API 34): `android/about-revealed-off-notice-zh-light.png`,
  `about-debug-on-zh-light.png`, `about-debug-{on,off}-en-dark.png`
