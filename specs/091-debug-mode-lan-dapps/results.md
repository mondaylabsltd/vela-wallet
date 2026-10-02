# 091 — Results

Branch `091-debug-mode-lan-dapps`, from `origin/main` @ `ec033f231`. Nothing pushed.

## Commits

| Commit | What |
|---|---|
| `56ffb6a82` | core: `offers_wallet`, the private-host tables and `private_host_js`, `provider_script(host, debug_mode)`, `DebugModeChanged`, `prefs::DebugMode` + `version_tapped`, bindings, i18n, regenerated artefacts, web cross-test |
| `daa6f57f5` | desktop: About entry + switch, preferences, `BrowserHost::follow_debug_mode`, the one webview rebuilt |
| `041ce0b15` | iOS: About entry + switch, preferences, per-tab script swap |
| `98c1eee79` | Android: About entry + switch, preferences, per-tab script swap |
| (this commit) | docs: spec, plan, tasks, results; 088 results; ARCHITECTURE; store review notes |

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
  `version_tapped` counts 7 taps, each ≤ 1,000 ms after the one before.

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

## Owner decisions needed

1. **Android cleartext (blocks the feature on Android).**
   - The release build has no network security config and targets SDK 36, so the WebView refuses every
     `http://` page (`ERR_CLEARTEXT_NOT_PERMITTED`; 088 audit A11/A16, 083 hand-off A-9). The debug build
     allows only `127.0.0.1` and `localhost`.
   - So on Android a LAN http dApp **does not load at all**, debug mode or not. The switch, the script and the
     gate are in place, and are tested against the real core with fake WebViews.
   - A network security config cannot express IP ranges. Allowing LAN http means
     `<base-config cleartextTrafficPermitted="true"/>` in `src/main/res/xml/network_security_config.xml`
     (plus the manifest attribute), and the same line in the debug file of that name, which overrides it.
   - That would also let public http pages load (without the wallet, like iOS today).
   - It would also remove the platform's https guard from the app's **own** traffic for the inputs the person
     types: the per-network RPC override, the four service endpoints, and the registry URL. None of these has
     an https check in the core today: `network_admin.rs` stores them as typed, and the https check only
     shows a badge. Recommendation: add an https-or-loopback rule for those inputs to the core first, or
     limit the config change to debug builds.
2. **App Review 2.3.1(a).** Apple forbids undocumented hidden features. The hidden switch is now listed in
   `docs/store-submission/privacy-and-review.md` §3b. The §4 notes block has 27 characters free, so a line
   about it there means shortening another line. That is your call.

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
- Not run: an end-to-end LAN page in the desktop browser. That route needs a signed-in live wallet; the
  mock `VELA_PAGE=explore` does not load the webview.

### i18n

- `about.debugMode`, `about.debugModeBody`, `about.debugModeRevealed` in 15 locales. zh-HK is written
  Cantonese (「畀本地網絡上嘅 http 網頁用個錢包。只係開發用。」); zh-TW uses 除錯.
- ja + en: +451 bytes (en +181, ja +270). Runtime JSON residency is 139,093 of 139,800; the compiled-catalog
  cold start is 135,065.

## Test totals

| Suite | Result |
|---|---|
| core `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2318 passed, 0 failed |
| core `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings` | clean |
| core `cargo fmt --all --check`, `build-web --check`, `gen-onboarding-types --check` | clean, current |
| web `npx vitest run` (after `pnpm build:extension`) | 170 files, 2391 passed, 5 skipped |
| web `pnpm check` | 0 errors, 0 warnings |
| desktop `cargo fmt --check` / `cargo clippy --all-targets` / `cargo test` | clean / no new warnings / 883 passed, 49 ignored |
| Android `:app:testDebugUnitTest` | 917 tests in 105 suites, 0 failed |
| iOS `VelaWalletTests` (own simulator clone, deleted) | 1113 tests in 142 suites passed; focused 117 in 10 suites; drift + debug 23 in 2 |
| `check-native-reachability` / `check-event-payloads` / `check-dead-controls` | ok / 0 mismatches (532 sites) / 0 |

## Screenshots

`/private/tmp/claude-501/-Volumes-data-production-vela-wallet/d4a496f4-ab96-4481-916a-65326d48067e/scratchpad/shots/`:

- desktop: `desktop/about-debug-on-en.png`, `about-debug-off-en.png`, `about-debug-on-zh-dark.png`,
  `about-debug-off-zh-dark.png`, `about-hidden-en.png`, `reveal-1.png` (after 7 taps: row + notice)
- iOS: `ios/091-about-hidden-en.png`, `091-about-revealed-notice-en.png`, `091-about-switched-on-en.png`,
  `091-about-debug-{off,on}-{en,zh}.png`
- Android (own emulator, Pixel 7 API 34): `android/about-revealed-off-notice-zh-light.png`,
  `about-debug-on-zh-light.png`, `about-debug-{on,off}-en-dark.png`
