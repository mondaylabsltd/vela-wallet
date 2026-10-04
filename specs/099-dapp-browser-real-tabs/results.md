# 099 — Results (2026-10-04)

Branch `099-dapp-browser-real-tabs`, PR #413.

## What changed, by client

| | Real tabs | Layers on screen | Read deadline | Confirm gate + reason | Landing from the relay's send | Signer kinds |
|---|---|---|---|---|---|---|
| Core | `browser_tabs::plan_engines`, recency in `explore_sites` | `dapp_record` rows, layers, reasons, log line, inspector, report | `READ_DEADLINE_MS` in every read | `sign_confirm::confirm_state` | `tx_tracker` 3 s first ask, `relay_sent_at_ms`, `landing_pace`, forgotten ops | `Failed.signer` → `SignErrorKind::Signer*` |
| Desktop | one webview per tab (no reload, no hold), the core's plan | status line + Tab status panel + Copy; bug report names the latest trouble | raced in the host | `confirm_state_of` | both landings | from the passkey classifier |
| iOS | already per-tab; the core's plan + memory warnings | status line + sheet + Copy | raced in `DbrExecutor` | `signConfirmState` (both old gates gone) | send + sheet + aftercare | yes |
| Android | already per-tab; the core's plan + `onTrimMemory` | status line + sheet + Copy | raced in `BrowserExecutor` | `signConfirmState` | send + sheet | yes |
| Web | (the person's own browser) | — | — | `signConfirmState` (wasm) | send + dApp landing | `PasskeyError.kind` |

## Verification

- **Core**: `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` green; new
  `app_dapp_browser_099` (13), `app_browser_tabs_099` (7), `app_sign_landing_099` (11), signer tests
  in `app_sign_request` (3), recency in `app_explore_sites` (2); 14 tracker tests moved to the 3 s
  first ask / status past the window. Clippy `-D warnings` clean. Residency 144,350 / 144,400.
- **Desktop**: `cargo test` 923 passed (macOS); CI `desktop` and `desktop-windows` green (the
  Windows-only webview code compiles there).
- **Android**: `:app:testDebugUnitTest` 997/997 (13 new in `BrowserTabsAndLayersTest`, real core).
- **iOS**: simulator 1251 passed, 1 failed — `SettingsParityDeviceTests/testEraseReturnsToTheFirstRun`,
  which fails on HEAD without these changes; `Spec099Tests` against the real core and real WKWebViews.
- **Web**: vitest 2639 passed; the 3 failures (`extension/package.test.ts`) read a locally built
  extension package older than the rebuilt wasm (CI builds it fresh). `svelte-check`: only the
  pre-existing `background.test.ts` error.
- **Wire**: `check-event-payloads` 0 mismatches over 548 dispatch sites.

## On devices

- **Xiaomi (Android, debug build)** — the test dApp at `127.0.0.1:8137` (adb reverse):
  - log lines from the core, e.g. `dapp tab=t-… origin=http://127.0.0.1:8137 page=ready
    provider=offered`, `… method=eth_blockNumber class=read outcome=answered ms=1079` (the pool's own
    line: 1050 ms), `… method=eth_signTransaction class=local outcome=failed code=4200 layer=wallet
    reason=unsupported_method`;
  - the status line under the bar: "⚠ Vela 不支持此请求 · eth_signTransaction · 标签页状态 ✕";
  - the 标签页状态 sheet: origin, 页面已加载, 此页面可使用钱包, the rows newest first with ✓ and
    durations, 复制详情.
- **Mac (signed bundle)** and **iPhone 11 (debug build)**: installed and running; the on-screen pass
  (two dApp tabs switched back and forth, a request from a background tab, the shut slide's line,
  the landing before and after the relay sends) is the owner's — not done here.
- **Not measured**: SC-003 (memory with ten tabs vs six), SC-004 against a blackholed RPC on a device
  (the deadline is unit-tested on every client), SC-006 (killing one WebContent process), SC-007 on
  a relay-funding chain.

## Corrections made on the way

- The 2026-10-04 explanation of the locked slide ("the fee went stale") was wrong: no client gates on
  `FeeView.stale`. The slide now says which part of the gate is shut (research R7).
- The plan put the relay's send time on `SendReceiptView` and `Following`; it lives only on the
  tracker entry, which every landing already reads (contracts/core.md, "As built").

## Open

- Owner's device pass on the Mac and the iPhone (above).
- Export `off_chain` / another-tier to UniFFI and wasm so the fee row's display test leaves the
  phones and the web too (today only the gate is the core's), and move the status line's priority
  into the core.
- Residency has ~50 bytes of room: the next string needs the owner's budget call.
