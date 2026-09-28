# Implementation Plan: 079 — The dApp browser and signing hold up on a bad network, on every client

**Branch**: `079-android-dapp-browser-stability` | **Date**: 2026-09-28 | **Spec**: [spec.md](spec.md) | **Research**: [research.md](research.md) | **Contract**: [contracts/core-rules.md](contracts/core-rules.md)

## Summary

The device pass found sixteen defects on Android; the audit found most of them on iOS and the
desktop too, each written separately in each shell (research R0). The fix follows the owner's
"统一修复，保持一致性": every *decision* the defects share moves into `vela-core` as a small pure rule
or a field of an existing machine's view; each client keeps only drawing and platform I/O, and
draws the same states with the same words. Then each client is fixed and verified on a device —
Android on the Xiaomi, iOS on the iPhone 11, desktop on this Mac — and the trusted signer page
(web) gets its one-slide, plain-words, offline form.

## Technical context

- Core: Rust, Crux machines (`tx_tracker`, `fee_policy`, `sign_request`, `dapp_browser`,
  `browser_history`), new pure module `app::browser_load`. Exported through UniFFI (Kotlin, Swift),
  wasm-bindgen (web/extension), direct crate use (desktop).
- Android: Kotlin/Compose, system WebView (`WebViewClient` error codes). iOS: SwiftUI + WKWebView
  (`NSURLErrorDomain` codes). Desktop: gpui + wry (WKWebView on macOS; wry exposes only
  Started/Finished — see R4). Extension: MV3 side panel + SvelteKit signing host. Trusted signer:
  `app-web/trusted-signer` (static page on sign.getvela.app, spec 075/076).
- i18n: shared corpus `rust/crates/vela-core/i18n/locales/<15 locales>/*.json`, the six-step gate
  (memory: i18n corpus gates) — every new key in all 15 locales, wasm fingerprint moves.
- Devices: Xiaomi alioth `9d5f42fb` (owner's wallet + fingerprint; parallel space), iPhone 11 "ABC"
  `00008030-001A75961445802E` (parallel space via XCUITest; UI Automation must be switched on by a
  person), macOS host for desktop. Fault injection: `scripts/device/chaos-proxy.py`.

## Constitution check

`.specify/memory/constitution.md` is the unfilled template; the working rules are the repo's
"one source of truth" and the agent rules. This plan moves rules DOWN into the core and deletes
the per-shell copies it replaces, adds a test for every moved rule, and keeps 070's security
invariants untouched (FR-021: attribution of origin/frame/chain/account and every answer code stay
the core's). Money rules: the tracker's "a timeout is never a failure" is kept (FR-005 reworded);
no signing path changes which key signs or what is signed.

Risk: **High** — the trusted signer handoff (US7, a signing boundary; 076 integrity must hold) and
the signing sheet's dismissal/answer semantics (US1: exactly one answer per request). **Medium** —
everything else (UI + pure rules).

## Phases

| Phase | Deliverable | Gate |
|---|---|---|
| 0 | Core rules: `app::browser_load` (failure class, retry schedule, site letter, visit rule), `tx_tracker` age-based cadence + `outcome` wording field, `fee_policy` re-quote schedule; bindings (UniFFI + wasm); i18n keys in 15 locales | `cargo test -p vela-core --features i18n-all,crux`; `gen-i18n`/`lint`/`verify`; `build-web --check` |
| 1 | Android: US1–US6 + US7 native half; JVM tests | JVM suite green; Xiaomi pass (quickstart A) incl. the fault matrix |
| 2 | iOS: same list; hermetic tests | iOS hermetic green (simulator); iPhone probe + fault matrix (quickstart B) |
| 3 | Desktop: same list where the surface exists (F4/F5 need the load watchdog, R4) | desktop `cargo test`; macOS run with screenshots at desktop + phone width (quickstart C) |
| 4 | Extension: signing surface parts (F7–F14, T1) | web unit + extension e2e |
| 5 | Trusted signer page: one slide, origin line, plain failure words, offline shell under 076 integrity | trusted-signer tests; Xiaomi + iPhone signatures with the page host dropped after a first visit |
| 6 | Results, evidence, memory; PR | CI gates |

Phases 1–5 depend only on phase 0, and each can land on its own.

## Design decisions (details in research.md R1–R8)

1. **One classifier, per-platform code tables in the core** (`browser_load::classify`): Android
   `WebViewClient.ERROR_*`, Apple `NSURLErrorDomain`, plus a certificate flag. Output: class
   (`offline | timeout | not_found | certificate | refused | other`), whether to auto-retry, and the
   corpus key for the reason. Cancelled navigations (Apple -999) are "not a failure".
2. **Retry schedule in the core**: `browser_load::retry_delay_ms(class, attempt)` → 2 s, 5 s, 10 s, then
   none; certificate never. Shells run the timer only while the tab is in front.
3. **Visit rule in the core, enforced by construction**: `browser_load::visit_to_record(load)` takes
   what the shell observed at finish time (url, title, icon, failed, http status) and answers the
   visit or nothing. The shell reads url + title + icon in ONE script from the same document.
4. **Loading from request, not commit**: each engine sets `loading` in its own `load()` / `reload()` /
   link interception; the failure panel stays up with `retrying` until commit or the next failure.
   Desktop has no failure callback from wry → a load watchdog (R4).
5. **Signing sheet**: dismissal becomes explicit-only on Android and iOS (desktop and extension
   already are); the post-approval body is each client's existing send-receipt status view
   (`StatusHero`), driven by `sign` + `tracker` views; after the core clears the sheet on the
   receipt, the client keeps the landing result for the tick (desktop's `dapp_landing` pattern).
6. **Tracker**: past the wait window the receipt cadence grows with age (12 s → 60 s after 10 min →
   5 min after 1 h), abandon at 24 h unchanged; a view field names the wording
   (`landing | still_confirming | unknown`) so every client says the same thing.
7. **Fee**: every client's signing fee row reuses its send flow's refresh control; a
   `QuoteUnavailable` failure says the service cannot be reached (existing string
   `componentsUi.funding.denialNetworkError`) and re-quotes on `fee_policy::requote_delay_ms(n)`.
8. **Security indicator**: icon only — closed lock (neutral) for `secure`, open lock (warning) for
   http — from the core's existing `DbrTabView.secure`; the visible "安全站点" text is removed from
   every browser surface. Screen readers get the protocol word.
9. **Trusted signer**: the native confirm becomes a button when the route is the trusted signer;
   the page learns (in the request it already receives) that the origin was observed by Vela's own
   browser and shows it as such; its failure words split cancelled / timed out / no key; an offline
   shell that serves exactly the published, integrity-checked assets (R7).

## Project structure (touched)

```
rust/crates/vela-core/src/app/browser_load.rs          (new, pure)
rust/crates/vela-core/src/app/{tx_tracker,fee_policy}.rs
rust/crates/vela-core/tests/app_browser_load.rs         (new)
rust/crates/vela-core-uniffi/src/lib.rs                 (exports)
rust/crates/vela-core-wasm/src/lib.rs                   (exports the extension needs)
rust/crates/vela-core/i18n/locales/*/{explore,componentsUi}.json
app-android/…/feature/browser/core/BrowserController.kt, feature/explore/**, feature/signing/**
app-ios/…/Features/Explore/**, Components/Explore/**, Features/Signing/**, Components/Signing/**
app-desktop/vela-wallet/src/{webview.rs,wallet/page.rs,wallet/browser_host.rs,explore/*,signing/*}
app-web/vela-wallet/extension/**, src/lib/signing/**      (signing surface)
app-web/trusted-signer/**                                (page)
scripts/device/chaos-proxy.py                            (added)
```

## Complexity tracking

| Choice | Why | Simpler alternative rejected because |
|---|---|---|
| Pure core module for page-load rules | three shells had three copies, all wrong differently | per-shell fixes would drift again — the audit is the proof |
| Load watchdog on desktop | wry has no load-failure callback | leaving desktop with no failure panel keeps F4/F5 there |
| Offline shell for the signer page | a signature needs no network; the page blocked it | "retry later" leaves the owner's case (bad network) broken |
