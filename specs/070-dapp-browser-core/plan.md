# Implementation Plan: 070 — One dApp browser engine, in the core

**Branch**: `070-dapp-browser-core` | **Spec**: [spec.md](spec.md) | **Research**: [research.md](research.md) | **Contract**: [contracts/dapp-browser.md](contracts/dapp-browser.md)

## Summary

Move the decision half of the in-app browser — routing, per-tab/per-document
bookkeeping, per-origin chains, signing serialisation, receipt translation,
error words, and the injected script — into `vela-core`
(`app::dapp_browser` + `app::dapp_rpc` + `provider/inpage.js`). Rewire the
three native shells onto it, deleting their copies; pin the extension's JS
table to the core; fix the security and robustness defects the audits found
while each shell is open.

## Technical context

- Rust core (Crux 0.x machines, `CoreHost` pattern on every shell), UniFFI for
  Kotlin/Swift, wasm-bindgen for web, direct crate dependency on desktop.
- Android: Kotlin/Compose, `androidx.webkit` (`WebViewCompat.addWebMessageListener`,
  `addDocumentStartJavaScript`). iOS: SwiftUI + WKWebView (iOS 17.4). Desktop:
  gpui + wry. Extension: MV3, esbuild.
- Devices: Xiaomi alioth (Android 13, serial `9d5f42fb`), iPhone 11
  (iOS 26.5, `F30282CB-…`). Parallel space for signing with real (dust) money.

## Constitution check

The repo's `.specify/memory/constitution.md` is the unfilled template; the
working rules are `docs/agent-rules/AI-CODING-RULES.md` and README "one
source of truth". This plan satisfies them: rules move DOWN into the core, each
shell keeps only I/O and drawing, every moved rule keeps (or gains) a test.
Risk: **High** for the Android bridge change and signing pinning (security
boundary) — device evidence required; **Medium** for the rest.

## Phases

| Phase | Deliverable | Gate |
|---|---|---|
| 0 | `dapp_rpc` (pure) + `provider/inpage.js` moved + `provider_script(host)` | `cargo test -p vela-core` |
| 1 | `dapp_browser` machine + conformance suite (`tests/app_dapp_browser.rs`) | ≥ 60 tests green |
| 2 | Bindings: UniFFI `DappBrowserCore` + free fns; wasm `dappRpcClassify` for the extension pin; TS types regenerated | `gen:core-types` zero diff, `build-web --check` |
| 3 | Android on the new core (WebMessageListener, renderer-gone, tab pause, UX list), JVM tests | 623+ JVM green; device pass (quickstart A) |
| 4 | iOS on the new core (frame origin, teardown, process-terminated, UX list), hermetic tests | iOS tests green; device pass (quickstart B) |
| 5 | Desktop on the new core (queue replaces silent drop; granted address) | `cargo test` desktop |
| 6 | Extension: bundle core provider, pin table over wasm, fix comments; Settings connected-sites on desktop/iOS/Android; scan routing | web unit + extension e2e |
| 7 | Delete `dapp_permissions`' browser half; docs (`docs/dapp-browser/ARCHITECTURE.md` rewritten for what exists) | CI gates (memory: ci-gates-after-core-changes) |

## Project structure (touched)

```
rust/crates/vela-core/provider/inpage.js            (moved from extension, classic script)
rust/crates/vela-core/src/app/dapp_rpc.rs           (new, pure)
rust/crates/vela-core/src/app/dapp_browser.rs       (new machine)
rust/crates/vela-core/tests/app_dapp_browser.rs     (new)
rust/crates/vela-core-uniffi/src/{lib,onboarding_bridge}.rs
rust/crates/vela-core-wasm/src/{lib,wallet_state}.rs
app-android/…/feature/browser/core/*                 (router/table/bridge deleted; controller slimmed)
app-ios/…/Features/Explore/Core/*                     (same)
app-desktop/vela-wallet/src/{webview.rs,wallet/browser_host.rs,executor/dapp_rpc.rs,wallet/page.rs}
app-web/vela-wallet/extension/{build.mjs,manifest.json,background.js,lib/protocol.js}
```

## Complexity tracking

- The per-document id adds one field to every message. It replaces three
  different "which page is this for" heuristics (front tab fallback, URL at
  message time, navigation callbacks) with one fact — net simpler.
- Keeping `dapp_permissions` until phase 7 costs one release of two machines;
  it keeps `main` green between shells.
