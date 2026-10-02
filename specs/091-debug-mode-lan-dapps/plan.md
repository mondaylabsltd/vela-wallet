# Implementation Plan: 091 — Debug mode: the wallet for http dApps on the local network

**Branch**: `091-debug-mode-lan-dapps` | **Date**: 2026-10-02 | **Spec**: [spec.md](spec.md)
**Input**: owner ruling 2026-10-02 (spec 088 follow-up): a hidden debug-mode switch that lets the in-app
browsers offer the wallet to `http` pages on the device's own network.

## Summary

One core rule decides who is offered the wallet: `offers_wallet(origin, debug_mode)`. The browser machine's page
gate and the injected script both follow it. For the script, the core writes the host test into
`provider_script(host, debug_mode)` from the same tables the Rust rule reads. The setting is a preference
(`vela.debugMode`), and the 7-tap rule that reveals it is also the core's. The shells draw the hidden entry and
the switch, store the choice, and give the new script to their WebViews.

## Technical context

- **Core**: Rust `vela-core`, exposed through UniFFI (Swift, Kotlin) and wasm-bindgen (web tests).
- **Shells**: iOS (SwiftUI, WKWebView), Android (Compose, `androidx.webkit`), desktop (gpui + wry: WKWebView on
  macOS, WebView2 on Windows; Linux has no in-app browser).
- **Constraints**: no physical devices; simulators only. The i18n ja + en residency budget is 139,800 (owner,
  2026-10-02).

## Principles check

| Principle | How this plan keeps it |
|---|---|
| Rules decided once in vela-core | `offers_wallet` (who), `private_host_js` (the same host rule, written as JS by the core), `prefs::DebugMode` (stored spelling), `prefs::version_tapped` (7 taps, 1 s gap). |
| Shells only draw | Shells feed taps, store what the core says, show the row, and swap the script. None of them tests a host. |
| Parity | iOS, Android and desktop get the entry, the switch and the wiring. The web wallet has no in-app browser. The extension never closed LAN http (D7). Linux desktop has no browser, so no entry (D6). |
| Stable in an unstable environment | Turning debug mode off withdraws the wallet at once: open requests are answered 4900, and nothing is left hanging on a sheet. |
| Minimal, no duplication | The private-host rule moves from code to tables in place. The off script is byte-identical to spec 088. There is one toggle-row component per shell. |

## Design decisions

- **D1 One rule.** `offers_wallet(origin, debug) = is_secure_context(origin) || (debug && http &&
  is_loopback_or_private_host(host))`. It reuses the exact-match helper that already guards the signing block
  (invariant ③). Public http is never offered.
- **D2 The script's host test comes from the core.** WebViews decide at document start, before any message, so
  the script must test `location.hostname` itself. `is_loopback_or_private_host` now reads four tables (names,
  suffixes, IPv6 first-group ranges, IPv4 first/second-octet ranges). `private_host_js()` writes the same tables
  into a JS function, so a table change reaches both sides. `core-table.test.ts` runs the whole generated script
  (debug on and off) in a VM for ~50 origins and compares "said hello" with `dappOffersWallet`. Each origin goes
  through `new URL` first: the platform normalises shorthand (`http://192.168.1` → `192.168.0.1`) before either
  side sees it.
- **D3 Debug mode off is spec 088, byte for byte.** `provider_script(host, false)` emits the same prologue, and
  the test asserts it. Only the gate line differs between the two scripts.
- **D4 The machine.** `Event::DebugModeChanged { on }`; the model keeps `debug_mode` (default off). Turning off
  retires every open document whose origin is no longer offered. This reuses `retire_document`: 4900 for each
  open request, consent entries dropped, a signing sheet cancelled, and the line moved on. The tab view shows
  no connection for an origin that is not offered.
- **D5 When a change applies.** A WebView reads its injected script when a document starts.
  - iOS (`WKUserScript`) and Android (`addDocumentStartJavaScript`, keeping its `ScriptHandler`) swap the
    script in every open tab. It applies to that tab's next document (reload or navigation).
  - The desktop has one wry webview, and wry takes initialization scripts only when it builds the view. A
    change retires the view outside the paint pass, as the Windows dead-engine path already does. The page is
    gone for the core (`page_gone_events`), and the next `place` builds a new view at the page it showed.
- **D6 The entry.** The version line in About is the hidden entry: no affordance, and 7 taps each ≤ 1,000 ms
  apart. A tap that comes too late, or a clock that went backwards, restarts the count. The reveal stores
  `off`, shows "Debug mode is now available" briefly, and the row then lives in About, after the technical
  rows. The stored value is `vela.debugMode` = `off` | `on`. Absent or unknown means hidden, and hidden means
  off. Erase removes it, because it is in the `vela.` namespace. Linux desktop has no in-app browser, so its
  version line reveals nothing.
- **D7 Web and extension.** The web wallet has no in-app browser: dApps reach it through the extension or the
  request window. The extension's `inpage.js` and `content.js` match `*://*/*` and carry no secure-context
  gate. Spec 088 changed only `provider_script`, which the extension does not use (it bundles
  `provider/inpage.js` directly). So the extension already offers the wallet to LAN http pages. As before, a
  signature from public http is refused by `popup_origin_refusal` (spec 089). No switch is needed there.
- **D8 Android cleartext (finding).** The release build has no network security config and targets SDK 36, so
  the WebView refuses all cleartext http (`ERR_CLEARTEXT_NOT_PERMITTED`; 088 audit A11/A16, 083 hand-off
  A-9). The debug build allows only `127.0.0.1` and `localhost`. On Android a LAN http page therefore does not
  load at all, debug mode or not. A network security config cannot express IP ranges, so enabling it means
  `base-config cleartextTrafficPermitted="true"`. That is an owner decision, left open (results.md).

## Files

- Core: `rust/crates/vela-core/src/app/{dapp_permissions,dapp_rpc,dapp_browser}.rs`,
  `src/prefs.rs`, `tests/app_dapp_{permissions,browser}.rs`; UniFFI `lib.rs`, `settings_bridge.rs`;
  wasm `lib.rs`, `settings.rs`; corpus `i18n/locales/*/about.json`.
- Desktop: `src/executor/preferences.rs`, `src/webview.rs`, `src/webview_absent.rs`, `src/wallet/browser_host.rs`,
  `src/wallet/page.rs`, `src/settings/{mod,components}.rs`.
- iOS: `Core/Preferences.swift`, `Features/Explore/Core/{ProviderBridge,BrowserEngine,BrowserController}.swift`,
  `Features/Settings/*`, `App/RootView.swift`, tests.
- Android: `core/data/Preferences.kt`, `feature/browser/core/{ProviderBridge,BrowserController}.kt`,
  `feature/settings/*`, tests.
- Web test: `app-web/vela-wallet/src/lib/dapp/core-table.test.ts`.
