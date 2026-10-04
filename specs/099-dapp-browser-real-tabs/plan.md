# Implementation Plan: 099 — The dApp browser: real tabs, and layers you can see

**Branch**: `099-dapp-browser-real-tabs` | **Date**: 2026-10-04 | **Spec**: [spec.md](spec.md)
**Design**: [research.md](research.md) · [data-model.md](data-model.md) · [contracts/core.md](contracts/core.md) · [quickstart.md](quickstart.md)

## Summary

The desktop gets one system webview per tab, so a switch shows a page instead of reloading it.
Every rule around that is decided once in `vela-core`, as the owner asked
(「跨端同样要使用 crux core 来保持规则逻辑一致性」), and desktop, iOS and Android all run it:
- which tabs keep a live engine;
- what each layer is doing (page, provider, request record, layer + reason vocabulary, read deadline, log line);
- why the signing slide is shut;
- how a passkey failed;
- how the landing counts down;
- when a forgotten op ends.

The shells keep engines and draw.

## Technical Context

- **Core**: Rust `vela-core` (Crux). Machines touched: `explore_sites`, `dapp_browser`,
  `sign_request`, `tx_tracker`, `send`; new pure module `browser_tabs`. Bindings: UniFFI (iOS,
  Android), wasm (web), direct link (desktop).
- **Shells**: desktop gpui + wry 0.56 (macOS WKWebView, Windows WebView2); iOS SwiftUI + WKWebView;
  Android Compose + WebView; web SvelteKit (signing sheet and landing only — its dApps run in the
  person's own browser through the extension).
- **Testing**: `cargo test -p vela-core` (new `tests/app_dapp_browser_099.rs`,
  `tests/app_browser_tabs_099.rs`, tracker/sign tests beside the existing ones); desktop
  `cargo test` + the gallery; iOS `xcodebuild test`; Android `./gradlew testDebugUnitTest` with the
  real core over JNA; web vitest over the real wasm. Device passes on the Mac (signed bundle),
  iPhone 11, Xiaomi.
- **Performance**: a switch shows the page in < 100 ms with no network; ≤ `LIVE_TABS_CAP` live
  engines; idle CPU with ten tabs ≈ one tab's; views stay small (full request rows only for the
  inspected tab).
- **Constraints**: the i18n residency budget (SC005 144,400; resident 136,490 now) — new keys
  are measured; Android typed enums decode a whole view or none, so every new variant lands in all
  mirrors in one commit; generated TS mirrors via `gen-core-types`; committed wasm via
  `build-web.mjs`.

## Constitution Check

`.specify/memory/constitution.md` is the unfilled template. The gates applied are the
project's standing rules:
- **The core decides and the shells draw.** Every rule above goes in the core, and each shell's
  own copy of a rule is removed: `confirm_enabled` ×4, the countdown ladder ×4, `ring_progress` ×2.
- **Minimal diffs.**
- **Money safety (082).** The forgotten-op ending needs the find-event scan caught up from an
  anchored start. A relay `not_found` alone never ends an op.
- **No secrets in logs or records:** no params, addresses or signatures, and host-only URLs.
- **i18n.** All 15 locales and the residency test.

Status: **pass**.

## Project Structure

```text
specs/099-dapp-browser-real-tabs/   spec · research · data-model · contracts/core.md · quickstart · tasks
rust/crates/vela-core/src/app/
  browser_tabs.rs        NEW  plan_engines, LIVE_TABS_CAP
  explore_sites.rs            recency
  dapp_browser.rs             busy, page/provider state, request rows, clock, read deadline, Log op, inspector
  sign_request.rs             confirm_state, signer kind, Following.relay_sent_at_ms
  tx_tracker.rs               FIRST_STATUS_POLL_MS, relay_sent_at_ms, forgotten ops, landing_pace
  send.rs                     SendReceiptView.relay_sent_at_ms
rust/crates/vela-core/i18n/locales/*/componentsUi.json, send.json
rust/crates/vela-core-uniffi/src/lib.rs   browser_engine_plan, sign_confirm_state, landing_pace
rust/crates/vela-core-wasm/src/lib.rs     signConfirmState, landingPace
app-desktop/vela-wallet/src/
  webview.rs                  one Browser per tab (map), per-view sinks carry the tab id
  wallet/browser_host.rs      tab id routing, no hold, engine plan, per-tab load watch/notices
  wallet/page.rs              switch = show/hide, status entry, inspector panel
  executor/dapp_browser.rs    read deadline, Log, now_ms
  signing/live.rs, flows/live.rs, signing/status.rs   confirm_state, landing_pace, signer words
app-ios/VelaWallet/VelaWallet/Features/{Explore/Core,Signing,Send}/   engine plan + memory warning, status/inspector, deadline, confirm/landing/signer
app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/{browser,signing,send}/   same
app-web/vela-wallet/src/lib/{signing,flows}/   confirm_state, landing_pace
docs/dapp-browser/ARCHITECTURE.md   the six layers and where each is seen
```

## Phases

1. **Core** (all rules, tests first, each test failing on the old core): browser_tabs + recency;
   dapp_browser record/state/deadline/log/inspector; confirm_state + signer kind; tracker pacing +
   relay_sent_at + forgotten ops + landing_pace; corpus keys; bindings; regenerate.
2. **Desktop**: per-tab webviews (the reload bug), tab-id routing, hold removed, engine plan,
   per-tab load watch/notices/crash/Back floor, status entry + inspector + copy, executor deadline
   and log, confirm/landing/signer words.
3. **Phones**: engine plan (with memory warnings), status entry + inspector, executor deadline and
   log, confirm/landing/signer from the core — removing their own copies.
4. **Web**: confirm_state and landing_pace from the wasm core.
5. **Verify**: suites, the quickstart on Mac / iPhone / Xiaomi, results.md, ARCHITECTURE.md.

## Complexity Tracking

| Choice | Why | Simpler alternative rejected |
|---|---|---|
| Up to six live webviews on desktop | Real tabs are the feature. | One view and a snapshot: impossible for sockets and JS state. |
| `now_ms` on nine events | Durations in the record, from the one clock the shell has. | A clock op round-trip per event: more traffic for the same number. |
| Full rows only in the inspected tab's view | Views render often. | Rows always in the view: 200 rows × tabs on every frame. |
