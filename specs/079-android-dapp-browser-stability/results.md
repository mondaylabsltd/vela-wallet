# 079 — Results

**Status: 2026-09-28, in progress.** Branch `079-android-dapp-browser-stability`, with `079-ios`,
`079-desktop` and `079-ext` merged into it. Rows marked *pending* are waiting for a person (a
fingerprint, the iPhone's developer-certificate trust, or the owner's release of the signing page).

## Success criteria

| SC | Verdict | Evidence |
|---|---|---|
| SC-001 accidental closes | **Pass** on Android (Xiaomi: 9 attempts of swipe, scrim and Back on the signing sheet, 0 closed; the consent sheet is also non-dismissible). The web/extension e2e checks that Escape and a scrim tap leave the request open. Desktop already had no outside close. iOS: `interactiveDismissDisabled` + ✕, unit-tested, *device pending* | `evidence/android-after/us1-sheet-explicit-close.jpg`; web e2e `extension-signing` |
| SC-002 a named status after approval | **Pass** on Android: signing → submitting → waiting with the chain's ring → landed tick (Gnosis `0xbf66a6d7…`); past the window, the still-confirming sentence, which flipped to landed when the relay caught up (`0xe3f09574…`); a message ends on "已签名". Desktop shows the same states (screenshots, driven by made-up core states). Web shows status (never a dimmed slide, e2e). iOS unit-tested, *device pending* | `us1-tx-landed.jpg`, `us1-still-confirming.jpg`, `us1-message-signed.jpg`; desktop `sign-states.png` |
| SC-003 progress within 0.5 s | **Pass** on Android (progress from the tap with `latency=6000`). Desktop: yes for loads the wallet asks for; wry reports nothing before a commit for links inside a page. iOS *device pending* | `us3-progress-from-the-tap.jpg` |
| SC-004 no raw engine error page | **Pass** in the app on Android (the panel stays through Retry; certificate, offline and not-found each have their own sentence), desktop (watchdog + probe; WKWebView's empty `about:blank` no longer counts as the page arriving). iOS draws the panel until the next commit (WebKit never draws its own page), *device pending*. The signing page opens in Chrome on Android, so a failed first load there is Chrome's own page until the person closes the tab; then the app says "签名页没能打开" with Retry. On iOS the tab closes itself as soon as the first load fails | `us3-retrying-keeps-the-panel.jpg`, `us3-l5-certificate.jpg`, `t5-back.jpg`; desktop `load-retrying.png` |
| SC-005 fee back without a tap ≤ 15 s | **Pass** on Android (8 s after the relay came back). Web: re-quote schedule unit + e2e `relay-faults`. Desktop, iOS: unit-tested | `us2-fee-refresh-and-retry.jpg` |
| SC-006 Recents only successful loads | **Pass** on Android, on the loads of this pass (failed, refused, certificate, and successful loads; the store read back held only the pages that loaded, each with its own title and icon). Fewer than the 10 + 10 the criterion names | store read in the device log |
| SC-007 no "安全站点"/"不安全"/"已加密" text | **Pass** on Android (UI dump sweep, zh). Desktop: lock only (screenshot). iOS: removed in code and gallery, *device pending*. The en sweep was not run | `us5-lock-only-and-network-picker.jpg` |
| SC-008 one slide per signature (trusted-signer route) | **Partial**: the app shows "去签名页确认" instead of its own slide on Android (device), iOS and desktop (unit). The end-to-end signature with the owner's fingerprint is *pending* | quickstart T1 |
| SC-009 signing page opens with its host unreachable | **Pending the release** (T059): the offline open needs the content-addressed page and its `immutable` headers to be live on `sign.getvela.app`. Until then only the fallback is proven: with the host dropped, the app says "签名页没能打开，请检查网络。" with Retry, and the request stays open (Android device; iOS unit) | `t5-back.jpg`, `t5-cancelled.jpg` |
| SC-010 no regression | **Pass**: 070 rows A1–A18 on the Xiaomi (A5 checked with chain 1337, since Polygon is configured on this device; A13 needs a tapped link, since Chromium skips history a script made without a gesture; A16's events reach the page when it is back on screen). Android unit suite 786/786 | `a11-crash.jpg`, `a12-alert.jpg`, `a16-connections.jpg` |

## The client matrix after the fix

✓ = fixed (or already right), — = the client has no such surface. Rows as in the spec.

| # | Android | iOS | Desktop | Extension |
|---|---|---|---|---|
| F1 failed load in Recents | ✓ | ✓ | ✓ (a 404 page still counts on macOS: WKWebView gives no status) | — |
| F2 visit with the previous page's title/icon | ✓ | ✓ | ✓ | — |
| F3 no progress until commit | ✓ | ✓ | ✓ for loads the wallet asks for | — |
| F4 Retry shows the engine's page | ✓ | ✓ | ✓ | — |
| F5 no reason, no retry | ✓ | ✓ | ✓ | — |
| F6 chain down: nothing shown | ✓ | ✓ | ✓ | — |
| F7 consent title / primary Connect | ✓ | ✓ | ✓ | ✓ |
| F8 "安全站点" | ✓ | ✓ | ✓ | ✓ |
| F9 pickers: logo, balance, identicon | ✓ | ✓ | ✓ (and "切换账户" works) | — |
| F10 closes by accident | ✓ | ✓ | ✓ | ✓ |
| F11 greyed slide after approval | ✓ | ✓ | ✓ | ✓ |
| F12 fee: refresh, reason, re-quote | ✓ | ✓ | ✓ | ✓ |
| F13 unlanded op silent | ✓ | ✓ | ✓ | ✓ |
| F14 host twice in the header | ✓ | ✓ | ✓ | ✓ |
| F15 tab thumbnails a drawing | ✓ | ✓ | — | — |
| F16 favicons unused | ✓ | ✓ | ✓ | — |
| T1 two slides on the trusted-signer route | ✓ | ✓ | ✓ | — |
| S1 signing page: "未知站点" for every dApp | the page now names the host Vela's browser saw; the "self-reported" warning stays until the owner decides T048 | | | |
| S2 signing page: jargon, stuck slider | ✓ plain words; the slide comes back after a cancelled prompt | | | |
| S3 signing page unverified / never cached | ✓ opened by content hash (`LAUNCH`); cached once T059 is released | | | |

Also on the signing page: a send reads as the send (the fee leg is the fee row, not a second
"批量" leg), and the fee's explanation folds under its row with the "自述" tag in sight.

## Where the clients differ (accepted)

- **Closing after approval** is respected everywhere: the operation continues and the page gets
  its answer, and the ending does not come back (iOS first; Android `002f5a5c`; web `b4a0c67f`).
- **The web ✕** opens only once the signature exists (a cancelled prompt would leave the page
  unanswered); on the phones the ✕ works in every state.
- **Desktop** retries pause when you leave Explore or the tab, not on window focus; a reverted
  transaction shows the existing on-chain-revert sentence with an explorer link.
- **iOS** opens the signing page in an in-app Safari view, so it sees a failed first load and
  closes the tab itself; Android (a Chrome Custom Tab, no callbacks) finds out when the person
  comes back.
- **The web fallback request window** closes as soon as the worker answers, so the signed tick
  shows in the side panel and wallet pages only.

## Gates

| Suite | Result |
|---|---|
| Android JVM | 786/786 on the merged tree |
| iOS hermetic (simulator clone) | 928/928 (on `079-ios`) |
| Desktop `cargo test` | 651 passed, 49 ignored, on the merged tree |
| Web unit (vitest) | 1906 passed, 5 skipped; `pnpm check` 0 errors (on `079-ext`) |
| Web/extension e2e | 45/45 (own preview port, on `079-ext`) |
| Signing page | fee-leg 8/8, hostile 32/32, takeover 18/18, unlimited-line 11/11 |
| Core | `cargo test -p vela-core --features i18n-all,crux` 1845/1845; `cargo clippy --workspace --all-targets -D warnings` clean; rustfmt clean on the 30 changed core files and 21 desktop files. (`cargo test --workspace` without those features fails the two i18n corpus suites, the known feature gate) |
| i18n | lint: no new defects; verify-parity: 75,395 comparisons, 0 divergences |
| Parity rulers | same counts as `main` (dropped-judgement 21, event parity 6, machine parity 6/7, dead controls 0), except copy parity iOS-only 36 → 37: `onboarding.common.close` and `componentsUi.signing.signerDown` read as iOS-only because Android reaches them through a constant and a key-building helper the script cannot follow; `connect` iOS-only fell 2 → 1 |

## Not done, and why

- **iPhone device pass (T077)**: UI Automation now starts; the app will not launch until the
  developer certificate is trusted on the phone (a person's step).
- **Fingerprint end to end (quickstart T1–T3)** on the Xiaomi with the owner's account.
- **Signing page release (T059)**: deploy `app-web/trusted-signer/dist/` (build
  `e3ef90a6…`), check the headers with `curl -I`, then set `LAUNCH` to that hash. SC-009 follows.
- **T048**: how the signing page's "self-reported site" warning should read. It cannot tell who
  opened it, so it cannot drop the warning; the recommendation is to keep it in plain, neutral words.
- **Desktop at phone width**: the window cannot go below 1280×800.
- **Extension side panel**: the signed tick is not e2e-tested there (the panel test helper looks
  for a page the panel no longer uses; it fails the same way on main).

## Hand-off to the owner (outside the apps)

- **D2** Arbitrum in-band operations are accepted by the relay and never mined (treasury
  0.0397 ETH; the same shape lands on Gnosis in ~30 s). Relay repo.
- **D3** dApp transactions never appear in Activity on any client.
- **D4** ~0.01 xDAI fee quote for a plain Gnosis transfer (the in-band overcharge work).
- **D5** "无法模拟这笔交易…" shown as danger when the chain's public node has no simulation.
- **D6** `accountsChanged` lower-cases the address; `eth_requestAccounts` checksums it.
- **D7** History "全部网络" when empty reads "此网络暂无交易".
- **i18n budget**: ja + en residency is at 138,750 of 138,800 bytes. The next new word needs the
  cap raised or something trimmed.
