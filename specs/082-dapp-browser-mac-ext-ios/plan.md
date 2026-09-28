# Implementation Plan: 082 — The dApp browser holds up on the Mac, in Chrome and on the iPhone, and 079's leftovers close

**Branch**: `082-dapp-browser-mac-ext-ios` (worktree `vela-wallet-082`, off `main` de93634f; G3/G29 fixed in 35ac2b07, `VELA_DEV_PROXY` + chaos `mute` in bea04b57) | **Date**: 2026-09-28 | **Spec**: [spec.md](spec.md) | **Research**: [research.md](research.md) | **Data model**: [data-model.md](data-model.md) | **Contract**: [contracts/core-rules.md](contracts/core-rules.md) | **Verification**: [quickstart.md](quickstart.md)

## Summary

The owner's device pass on the Mac, in Chrome and on the iPhone found three kinds of defect.

- **Money-safety defects on every client.**
  - A payment that landed is reported "failed — try again" when the relay's reply is lost (G21).
  - The relay status is read through a method the relay does not serve, so a rejected
    operation never ends (G13).
  - A reverted dApp transaction is drawn as confirmed (W3).
- **An extension request lifecycle that signs for pages that are gone.** See G17, G19, G23, G18.
- **Many browser-chrome and wording faults.** The desktop and the iPhone name a site the page is
  not (G28), sit silent for a minute (G32), and draw a plain transfer as a blind contract call
  (G14).

079's leftovers are in scope too: dApp transactions in Activity, honest simulation warnings, one
address spelling, the empty-History wording, the signer page's doubled host, and the side-panel
check.

The fix follows 079's rule (FR-020). Every *decision* the defects share moves into `vela-core`
as a pure function, a constant, or a field of an existing machine's view. Each client keeps only
drawing and platform I/O, and deletes its own copy. The extension's worker is the one exception:
it cannot load the core, so it gets a pure JS module whose codes and limits are pinned to the
core by tests (RB2).

The work is split into workstreams **A–H**. Verification is [quickstart.md](quickstart.md). Its
fault rows touch only the app under test (ruling 6; this run's request 不能影响设备上的其他流量),
and one short owner-passkey batch comes at the end.

## Technical context

- **Core.** Rust. The Crux machines touched are `sign_request`, `tx_tracker`, `send`,
  `rpc_pool`, `clear_signing`, `activity_feed`, `dapp_permissions` / `dapp_browser` and
  `token_trust` (input only).
  - Pure modules: `user_op` (+ `user_op_hash`, `submit_step`); `browser_load` (+ desktop
    `LoadWatch` moved in, `address_bar`, `site_label`, `Proxy` class); new `sim_outcome`,
    `net_health`, `remote_mark`; `balance_dashboard::read_plan`.
  - Provider: `provider/inpage.js`, embedded by `dapp_rpc::PROVIDER_JS`.
  - Exported through UniFFI (Swift, Kotlin), wasm-bindgen (web, extension pages) and ts-rs;
    the desktop uses the crate directly.
- **Desktop.** gpui plus wry 0.56.1: one WKWebView, plus reads of `isLoading` /
  `estimatedProgress` / `URL` over objc (RD3).
  - ureq 3.4.2 for wallet HTTP. A new small CFNetwork FFI (`core-foundation` 0.10 /
    `-sys` 0.8, already in Cargo.lock) handles system-proxy and PAC resolution (RD2).
  - Dev-fixtures builds need macOS 14+ (`wry/mac-proxy`).
- **Extension.** MV3, `minimum_chrome_version` 116.
  - The service worker is plain JS with no wasm: `background.js`, new `lib/request-life.js`
    and `lib/swlog.js`, and `content.js`.
  - The side panel is the SvelteKit wallet page (`wallet.html?panel`).
- **iOS.** SwiftUI + WKWebView; `os.Logger` (RE11). **Android.** Kotlin/Compose, with parity
  changes and JVM tests only.
- **Signer page.** `app-web/trusted-signer`: a static page under 076 integrity
  (`BUILD_ALLOWED` / `LAUNCH` in `trusted_signer/integrity.rs`).
- **i18n.** ja + en residency goes from 138,750 to ≈ 138,729 of 138,800 with no cap raise
  (RI2). Path pins go 1782 → 1784 (RI3). All corpus edits land in one commit.
- **Devices.**
  - Mac host: desktop plus Chrome for Testing 151 in a scratch profile, and the owner's Chrome.
  - iPhone 11 "ABC": hardware `00008030-001A75961445802E`, CoreDevice
    `F30282CB-FA41-589E-A3BD-31855EB64414`.
  - Android: Xiaomi `9d5f42fb`, optional smoke.
  - Fault injection: `scripts/device/chaos-proxy.py`, reached only through per-app switches
    (RH1).
- **Constraints.**
  - Dust amounts only.
  - Nothing in the relay repo changes.
  - No signing path changes which key signs or what is signed.

## Constitution check

`.specify/memory/constitution.md` is the unfilled template. The gates are the repo's working
rules, as in 079.

| Rule | How 082 meets it |
|---|---|
| One source of truth (FR-020) | Every shared decision moves into core (tables in [contracts/core-rules.md](contracts/core-rules.md) §17). The per-shell copies are deleted in the same change: four submit loops, four status parsers, three ending derivations, three sim parsers, five F14 checks, two plain-send interceptions, four feed-status derivations. Each moved rule gets a core test. |
| Money rules | Time alone never produces a failure (079 kept). A may-have-been-sent op is never "try again" (RA1, RA10). The local nonce advances only on `Accepted` (RA5). The tracker is the only closer of on-chain records (RA8). |
| 070 security invariants | Attribution of origin, frame, chain and account, plus every answer code, stays the core's. 4900 (not decided) is never confused with 4001 (rejected) (RB6). A page that left is never signed for (RB2, RB5). |
| Supply-chain signing boundary | The signer page still renders on its own (RC8, RG11). A revert reason from a dApp's contract is sanitised and capped in core before any sheet draws it (RG8). |
| House UI rules | Busy ≠ disabled (RE5, RF4, RB12). No bottom sheet on the desktop. Look before done: every row gets a screenshot at the client's real width. |
| Device-verify every feature | [quickstart.md](quickstart.md), with the owner's passkey only where fixed keys cannot stand in. |
| i18n reuse first | 3 new keys, 1 reworded, 1 deleted. Net −21 B, under the cap (RI2). |
| Logs name the failure and no secret (FR-018/019) | RB14, RD12, RE11, plus one secret scan (RH7). |

Result: **pass**, with the justified extras under Complexity tracking.

**Risk: High** for three areas:
- **money safety**: A, the submit verdict, the nonce and the tracker's new ends;
- **the extension request lifecycle**: B, the worker ledger, claims and worker restarts;
- **signing presentation**: A's endings and phase, C's plain send, G's simulation severity.
  These are what a person approves on.

**Medium** for the desktop proxy policy (RD2: a hand-written FFI, and the removal of the silent
direct fall-back) and the load watchdogs (RD3, RE2). **Low** for layout and wording.

## Workstreams

| Id | Scope (findings) | Core | Clients | Research |
|---|---|---|---|---|
| **A** Money safety | G21 / W1 lost submit reply; G13 / W4 relay status method; W3 reverted dApp tx drawn as confirmed; G22 "Waiting for biometric" during a network wait | `user_op_hash`, `submit_step`; `rpc_pool` `NotConnected` + `maybe_delivered`; `tx_tracker` status method + parser, `MaybeSent` / `NotSent`; `sign_request` `maybe_sent`, `SignPhase`, `ending_of` / `ending_state`, no Confirmed patch, `DAPP_TX_ANSWER_WINDOW_MS`; `send` receipt states | all four | RA1–RA12 |
| **B** Extension lifecycle + web UI | G17, G18, G19, G23, EX6, EX7; G16 unstyled consent; G1 on the wide web layout; W23 worker logs | `TransportDropped` stops a pre-passkey pipeline (all four clients); `SignSubmitOutcome::AskerGone`; wasm `signRequestTtlMs` | extension worker, content, panel; web | RB1–RB15 |
| **C** Plain value transfer | G14 | `ClearSurface::PlainSend` + `ClearPlainSend` | all four + signer page | RC1–RC8 |
| **D** Desktop | W14 tabs held during a request (ruling 4); W6 / W8 system proxy + PAC, no direct fall-back (ruling 3); W7 retry restarting a live load; W18; G2, G6, G7, G30, G1, G11; W16, W17; W23. G3 / G29 are already fixed on the branch; G4 is deferred | `browser_load::LoadWatch` (moved), `LoadFailureClass::Proxy` | desktop | RD1–RD15 |
| **E** iOS + parity | G28 bar names the wrong site; G32 / W5 no watchdog, no Stop, no network-back; G31 −1000; G8, G9, G10, G12, G24, G25, G26; W20 logo misses; W23 iOS logs | `address_bar`, `GIVE_UP_MS` / `should_give_up` / `stalled`, `retry_when_network_returns`, `site_label`, `net_health`, `remote_mark`, `read_plan`, `TrackOperation::HoldingsMoved` | iOS; Android, desktop and web parity | RE1–RE14 |
| **F** Chain reads that hang | G20, G33, W11 slow notice + dead Retry, W12 op-hash receipts on the extension, W10 iOS fee row | `rpc_pool` `unreached_chains`; wasm `rpcReadTimeoutMs` / `rpcCooldownMs` | extension, desktop, iOS | RF1–RF6 |
| **G** 079 leftovers | L-D3 dApp tx in Activity (ruling 7); L-D5 sim severity; L-D6 address spelling; L-D7 empty History; L-HOST; L-PANEL; L-D2 / L-D4 hand-offs; L-SC rows | `activity_feed` kind / status / site + empty keys; `sim_outcome`; `rpc_pool` optional methods; `dapp_spelling`; `inpage.js` | all four + signer page | RG1–RG15 |
| **H** Verification | ruling 6 switches, chaos modes, e2e, hermetic probe, owner batch, logs as evidence | — | test infra | RH1–RH7 |
| **I18N** | the budget and the ledger | corpus | all | RI1–RI3 |

## Dependency order

1. **Core first, in one branch of commits** (phase 0).
   - Land the `tx_tracker` enum edits together: A's `NotSent` / `MaybeSent` and E's
     `HoldingsMoved`. That way ts-rs, UniFFI and wasm are regenerated once, not three times.
   - Land the i18n corpus change as one commit (RI3). Adding `maybeSent` alone breaks the cap.
2. **Bindings** (phase 1): ts-rs types, `rust/pkg-web` (the fingerprint moves), the VelaCoreKit
   xcframework, and the Android `.so` plus Kotlin bindings. The hand-written wires must learn
   every new enum variant in the same change, or Swift and Kotlin fail to decode the whole
   view. These are `SignWire`, `TrackerWire`, `RpcWire`, `ClearWire`, `FeedWire` and
   `ActivityWire`.
3. **Clients in parallel** (phases 2–5). Inside the extension, B's lifecycle lands before A's
   may-have-been-sent answer and F's op-hash receipts (RF3) are verified: "exactly one answer"
   holds only if the answer reaches the live page.
4. **Signer page** (phase 6, independent). The code and the dist build are done in 082. The owner
   deploys; `LAUNCH` moves only after `curl -I` shows 200 with the immutable header.
5. **Device pass** (quickstart), then `results.md` with the client matrix, then the PR.

## Phases

| Phase | Deliverable | Gate |
|---|---|---|
| 0 Core | A, B, C, E, F, G core surfaces ([contract](contracts/core-rules.md) §1–§11); `LoadWatch` moved from the desktop with its tests; `inpage.js`; corpus (+`maybeSent`, +`explore.loadProxy`, +`explore.requestOpen`, reworded `simUnavailableWarning`, −`send.txErrorTimeout`) | `cd rust && cargo test -p vela-core --features i18n-all,crux` (residency printed, ≤ 138,800; `--nocapture` on `i18n_residency`) · `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings` · `cargo fmt --all --check` · `npm --prefix scripts run gen:i18n && npm --prefix scripts run lint:i18n && npm --prefix scripts run verify:i18n && npm --prefix scripts run dump:vectors`, then `git diff --exit-code` on `paths.rs`, `i18n_catalogs`, `assets/i18n` · `node scripts/check-event-payloads.mjs` · `node scripts/check-ios-android-copy-parity.mjs` · the two chain vectors: `user_op_hash` of Gnosis tx `0xc6f3544f…4dc4` = its `UserOperationEvent` `topics[1]` (RA6) |
| 1 Bindings | ts-rs, wasm, xcframework, `.so` | `npm --prefix scripts run gen:core-types && npm --prefix scripts run build:wasm && npm --prefix scripts run verify:wasm` · `cd app-web/vela-wallet && pnpm sync:wasm && pnpm check` · `bash rust/scripts/check-ios-core-fresh.sh` prints ok · `bash rust/scripts/smoke-swift.sh`, `bash rust/scripts/smoke-kotlin.sh` |
| 2 Desktop | D, plus the desktop halves of A, C, E, F, G | `cd app-desktop/vela-wallet && cargo test && cargo test --features dev-fixtures && cargo clippy --all-targets --features dev-fixtures -- -D warnings` · live: `cargo test live_an_unknown_hash_is_pending_not_unreachable -- --ignored` · quickstart §2 |
| 3 Extension + web | B, F (RF2, RF3), the web halves of A, C, G; the G16 token gate | `cd app-web/vela-wallet && pnpm test:unit -- --run && pnpm check` · `npx playwright test -c playwright.isolated.config.ts e2e/extension-*.e2e.ts` (its webServer builds the app and the extension and serves them on port 4174, away from the 4173 other sessions share) · quickstart §3 |
| 4 iOS | E, the iOS halves of A, C, F, G, RE11 logs | Hermetic suite on a **cloned** simulator: `xcrun simctl clone "<base simulator>" vela-082` → `xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj -scheme VelaWallet -destination 'platform=iOS Simulator,name=vela-082' -only-testing:VelaWalletTests`. Assert the test count (Swift Testing can report a false zero) · `node app-ios/scripts/check-ios-dropped-judgement.mjs && node app-ios/scripts/check-ios-event-parity.mjs` · quickstart §4 |
| 5 Android parity | The Kotlin halves of A, C, E, G (wires, aftercare, plain send, feed, sim, bar, watchdog, `HoldingsMoved`, `read_plan`) | `bash rust/scripts/build-dev-fixtures.sh --host && cd app-android/vela-wallet && ./gradlew testDebugUnitTest` · `node scripts/check-android-dropped-judgement.mjs && node scripts/check-android-event-parity.mjs` · quickstart §5 (optional) |
| 6 Signer page | RG11 (`originShown`), RC8 (a no-calldata call is a send) | `node samples/origin-line-test.mjs`, `node samples/plain-send-test.mjs` and the existing sample suites (hostile, takeover, fee-leg, unlimited-line, single-file, slider, ceremony, channels) · `cargo test -p vela-core --test trusted_signer` · **owner**: deploy `dist/` from the 082 tree → `curl -I https://sign.getvela.app/b/<hash>/sign` → then move `LAUNCH` and rebuild wasm and native |
| 7 Results | quickstart §6 (owner batch) and §7; `results.md` client matrix; evidence; memory; PR | CI green |

Phases 2–5 depend only on phases 0–1. Each can land on its own.

## Design decisions (details in research.md)

1. **A lost reply is "may have been sent", tracked to its end.**
   - One core `submit_step` classifies every submit: Accepted / MaybeSent / NotSent.
   - It uses a sticky `maybe_delivered` from the pool. Only a provable non-delivery is "not
     sent — try again".
   - The client computes the userOpHash locally (EntryPoint v0.7). The relay's hash wins when
     there is one. [RA1, RA6]
2. **The dApp gets exactly one answer after a lost reply**: an `Ok` with the op hash, through
   079's still-confirming path. It gets the tx hash if the receipt comes inside a 120 s window
   measured from approval. It never gets 4900 or -32603.
   - The nonce is not bumped, so a retry reuses nonce N and at most one lands. [RA2, RA3, RA5,
     RA12]
3. **The tracker ends every op honestly.** `NotSent` means the relay answered `not_found` twice,
   ≥ 60 s after submit. `MaybeSent` holds until the relay acknowledges the op. Status comes from
   `pimlico_getUserOperationStatus`, with one parser. Time alone is still never a failure.
   [RA4, RA7]
4. **The sheet's ending comes from the tracker**: `ending_state`, where Dropped means Reverted
   with `failedHint`. `sign_request` stops writing Confirmed. The stage words come from
   `SignPhase`, driven by `CeremonyStarted` / `CeremonyDone`. [RA8, RA9, RA10]
5. **The extension's worker owns each request's life.**
   - A `storage.session` ledger keyed by Chrome's `documentId`, and a page port that shows the
     page is still there.
   - Resume, don't settle, on a worker restart.
   - Three claims: before a grant, before the passkey, before the relay POST. A request that
     is not live means nothing is signed (`AskerGone`).
   - 4900 with plain English, never Chrome's text.
   - One queue per window. The panel stays the panel. [RB1–RB11]
6. **Empty calldata is a plain send**, whatever the recipient, drawn from the core's
   `PlainSend`. The value is hex-only, so no figure differs from what is submitted. The phones'
   interception is deleted. [RC1–RC8]
7. **Desktop.**
   - Tabs are held while a request is open (one funnel; the column comes forward).
   - Wallet HTTP takes WebKit's route per request (CFNetwork plus PAC, no global switch, no
     silent direct).
   - The load watchdog trusts WebKit's own `isLoading` before restarting anything.
   - The address bar leaves edit mode on Enter. [RD1–RD8, RD14]
8. **"Proxy" means the proxy itself could not be used.** A proxy that answered (a 502, or a
   CONNECT closed with no reply) speaks for the host. This one rule covers desktop RD9 and iOS
   −1000 → offline. [RX, RD9, RE4]
9. **The address bar names the committed page.**
   - A pending host shows only in an empty tab, and without a lock. A failed host shows
     without a lock.
   - Every client gives up a silent load at 20 s unless bytes are flowing.
   - Stop and a busy Retry.
   - Recovery when the network comes back. [RE1–RE5]
10. **Shared small rules move to core.**
    - `site_label` replaces five F14 copies plus the Recents rule.
    - `HoldingsMoved` refreshes balances after the app's own transaction.
    - `read_plan` gives iOS the stablecoins the others already read.
    - `remote_mark` puts a TTL on logo misses. [RE7–RE10]
11. **A chain notice appears after one failed pass** (`unreached_chains`), not three. The
    extension's reads use the core's 8 s timeout and cooldown and answer in plain words. The
    extension translates receipt reads for the op hashes it handed out. [RF1–RF3]
12. **Activity lists dApp transactions** with the core's kind, status and site. It is poked
    after every record write, and a may-have-been-sent op is a pending row. [RG1–RG4]
13. **Simulation severity is one core rule.** Could-not-check is caution; will-revert is danger,
    with a sanitised reason. `eth_simulateV1` errors never mark a chain down. [RG6–RG9]
14. **One address spelling (EIP-55).** It is written at grant time, on load and on account
    switch. `inpage.js` keeps the wallet's spelling. [RG10]
15. **Logs.** Desktop `vlog!`, iOS `os.Logger`, and an extension worker ring. Hosts only; the
    catalogue is in contract §15. [RB14, RD12, RE11, RH7]

## Project structure (touched)

```text
rust/crates/vela-core/src/user_op.rs
rust/crates/vela-core/src/app/{sign_request,tx_tracker,send,rpc_pool,clear_signing,activity_feed,
                               dapp_permissions,dapp_browser,browser_load,balance_dashboard}.rs
rust/crates/vela-core/src/app/{sim_outcome,net_health,remote_mark}.rs        (new, pure)
rust/crates/vela-core/provider/inpage.js
rust/crates/vela-core/tests/app_*.rs                                          (+ app_sim_outcome, app_net_health, app_remote_mark)
rust/crates/vela-core/i18n/locales/*/{componentsUi,send,explore}.json; scripts/gen-i18n.mjs (pins)
rust/crates/vela-core-uniffi/src/lib.rs; rust/crates/vela-core-wasm/src/lib.rs
app-desktop/vela-wallet/src/{executor/{proxy,proxy_macos,pool,relay,user_op,chain,sim,tracker,trusted_signer,sign_request,send}.rs,
                             explore/{probe,live,components,mod}.rs, signing/*, wallet/{page,browser_host,signing_host,live,money}.rs,
                             webview.rs, diag.rs (new), main.rs}
app-web/vela-wallet/extension/{background,content}.js, extension/lib/{protocol,request-life (new),swlog (new),op-receipt (new)}.js
app-web/vela-wallet/src/lib/{dapp,signing,services,wallet,flows,tokens}/**, src/routes/**; e2e/extension-*.e2e.ts
app-ios/VelaWallet/VelaWallet/{Core,Features/Explore,Features/Signing,Features/Send,Features/Wallet,Components}/**; Core/VelaLog.swift (new)
app-android/vela-wallet/app/src/main/java/app/getvela/wallet/{feature/{browser,explore,signing,send,wallet,flows},core}/**
app-web/trusted-signer/src/lib/{resolve,render}.js; samples/{origin-line,plain-send}-test.mjs (new)
scripts/device/chaos-proxy.py (optional `stall`)
```

## Risks

**High**
- **Residual double pay** (RA5) — **closed by T019** (ruling 8 answered Q1). The path was: the op
  lands while the relay stays mute, the person ignores "don't send it again" and re-signs after
  the 10 s nonce cache expires (desktop, web), the chain nonce is then N+1 and a second payment
  goes out. The tracker now also looks for the op's own `UserOperationEvent` on chain by the local
  hash (T019, with T180 so range errors reach it), so a landed op turns Confirmed (or Reverted) on
  the sheet and in Activity without the relay, instead of saying "may have been sent" until the
  24 h give-up. The caption and the pending Activity row stay as the first guard.
- **A wrong local userOpHash** would track a hash that never exists: the op lands while the
  sheet says "may have been sent" for 24 h. Mitigations: pin the Gnosis chain vector (phase 0
  gate), log `userop.hash_mismatch`, and compare on the parallel-space rows.
- **Wire drift.** A new enum variant is not additive for the hand-written Swift and Kotlin
  wires; one unknown `TrackOutcome` value makes kotlinx refuse the whole view. Mitigations: all
  clients ship in the same change, each wire gets a round-trip test, and
  `check-ios-core-fresh.sh` runs before any device row.
- **Extension lifecycle.** These Chrome facts come from the docs and were not re-verified on
  Chrome 151 / 154: `documentId` messaging, `runtime.getContexts`, port liveness, bfcache.
  G23(d) is not explained by reading the code. The claim before signing is the backstop: a false
  "gone" costs one re-approval, never money. The logs settle G23(d) on the device (EX8b).
- **The web and the extension cannot prove a refusal**, so a plainly refused relay reads "may
  have been sent" until the relay's `not_found` ×2 (≥ 60 s). The native clients say "not sent"
  at once. Row expectations differ on purpose (EX-S5).
- **Removing `sign_request`'s Confirmed patch** means on-chain dApp records close only when the
  tracker runs. RA8 also changes the web's answer for a reverted-in-window op from -32603 to the
  tx hash (Q2).
- **Signing presentation.** A plain send to a hostile contract reads as a calm "Send 0" on the
  web, which runs no simulation (C risk, accepted under the G14 ruling).

**Medium**
- **RD2's CFNetwork FFI** is hand-written, so a mistake crashes at run time. PAC must run on its
  own thread with its own run loop: touching CFRunLoop from a gpui dispatch worker has crashed
  this app before (the hidapi precedent).
- **Removing the silent direct route** means a stale or dead system proxy now visibly breaks
  wallet calls. The words and the log name the proxy.
- **WebKit semantics are unverified on these builds**: that `isLoading` tracks the main frame
  only, that progress starts at 0.1, and that the provisional URL behaves as assumed. Rows DX1,
  DX4, DX5 and E-G28b confirm them before `ENGINE_LIVE_PROGRESS` is tuned.
- **A 20 s give-up** can cut a site whose first byte comes later still. It is one core constant.
- **`read_plan` touches the balance total on four clients.** The iOS hero total rises on first
  run because stablecoins are now counted. That is correct, but visible.

**Low**
- 71 B of i18n headroom remain after 082.
- Line anchors drift: concurrent sessions edit this tree, so re-grep function names before
  editing.
- `stall` and TLS-reset chaos shapes stay unit-tested only.

## Out of scope

- **Relay repo (another repo, hand-offs with evidence):**
  - L-D2(a): Arbitrum in-band operations are accepted and never mined.
  - L-D4: the relay's pricing and the 0.01 native floor (ruling 2: on Gnosis and Arc the
    native coin is the stablecoin).
  - An `eth_getUserOperationStatus` alias for builds already shipped.
  - Confirm that admission stores an op before `not_found` could be read (the 60 s grace), and
    that `AlreadyQueued` covers included ops.
- **Deploying the signer page** needs the owner (L-HOST). 082 ships the code, the sample tests
  and the `BUILD_ALLOWED` entry; `LAUNCH` moves only after the deploy is verified.
- **Deferred:**
  - G4 sign-in sheet wording (RD15).
  - W9 TLS-reset class, W21 search engine, W22 hairline (RF6).
  - PAC on Windows and Linux.
  - os_log on the desktop.
  - One webview per desktop tab.
  - Moving the `tx_call_of` reader into core for the submit builders (RC6).
  - Per-leg batch display (CS26).
  - Android fault rows (RH3).
  - ~~A relay-independent landing check (Q1).~~ Not deferred: ruling 8 put it in 082 (T019).

## Questions for the owner — answered 2026-09-28

1. **Relay-independent landing check: in 082** (spec ruling 8). The tracker, for an entry with
   `maybe_sent ∧ ¬acknowledged`, also reads `eth_getLogs` on the EntryPoint for
   `UserOperationEvent` with `topics[1] = userOpHash` from the block recorded at submit (bounded
   ranges, stepping forward; a range error halves the window). A found event is proof: success →
   Confirmed with its tx hash, failure → Reverted. This adds a `TrackOperation::FindOpEvent` and
   its result event; `submit_block` is stored with the entry. Workstream A.
2. **Revert inside the wait → the tx hash on all four** (spec ruling 9), as RA8 recommends.
3. **The desktop hold hint is shown** (spec ruling 10): `explore.requestOpen`, +120 B, as RD1.

## Complexity tracking

| Choice | Why | Simpler alternative rejected because |
|---|---|---|
| Local userOpHash + a delivery flag through the pool | "Not sent" must be provable before saying "try again" (G21 paid twice) | Treating every transport failure alike either lies "not sent" or says "may have been sent" for a plainly refused relay |
| A JS lifecycle module in the extension, pinned to core by tests | The MV3 worker cannot load the ~3.6 MB core on each wake (`protocol.js:329-338`) | A core machine in the panel dies with the panel and cannot see page reloads |
| CFNetwork FFI on the desktop | Ruling 3: WebKit's own route, including PAC | `system-configuration` has no PAC; a second PAC engine would drift from WebKit |
| Moving `LoadWatch` into core | The phones need the same give-up rule now (G32) | Keeping it in the desktop starts a second copy the day iOS adds its watchdog |
| New tracker ends (`NotSent`, `MaybeSent`) | Ruling 1: track to the end; without them a refused web submit reads "may have been sent" for 24 h | Reusing `Rejected` words it as a fee problem |
