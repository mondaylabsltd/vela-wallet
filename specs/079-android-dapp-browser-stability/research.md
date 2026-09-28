# Research — spec 079

## R0 — Cross-client audit (2026-09-28, a2c46438)

Read-only code audits of iOS, macOS desktop and the extension + trusted signer page against the
Android device findings F1–F16/T1. Paths relative to each client's source root; `core:` =
`rust/crates/vela-core/src/app/`. Verdicts are from code; the iPhone rows are then confirmed by the
079 probe (`app-ios/VelaWallet/VelaWalletUITests/DappBrowserStabilityProbeTests.swift`).

### iOS (`app-ios/VelaWallet/VelaWallet/`)

| Item | Verdict | Evidence |
|---|---|---|
| F1 | different | `Features/Explore/Core/BrowserEngine.swift:309` visit only from `didFinish`; `didFail` `:319-326` / `didFailProvisional` `:328-341` record nothing. A server's own 404/500 page still counts. |
| F2 | same pattern | `BrowserEngine.swift:213` `title = webView.title ?? title` (keeps the previous title on nil); `:233-243` icon from a separate async `evaluateJavaScript`, url/title read in its callback. |
| F3 | partly | `BrowserEngine.swift:126-129` `load()` sets no loading state; `:279-281` loading from `didStartProvisionalNavigation`; a new tab waits a core round trip, `ExploreScreen.swift:468-473` draws `DemoPageView` in the gap. |
| F4 | partly | `ExploreScreen.swift:459-465` panel while `engine.failure`; `BrowserEngine.swift:279` failure cleared on provisional start → blank white or previous page during retry. |
| F5 | different/same | `BrowserEngine.swift:377` `"\(nsError.localizedDescription) (\(nsError.code))"` shown verbatim (`BrowserWebView.swift:57`); no automatic retry. |
| F6 | same | `App/RootView.swift:2415` `pool.failedChains` only in the feedback report; core `dapp_browser.rs:1515` -32603. |
| F7 | title same / button different | `ExploreLive.swift:286-288` builds the title; `ConnectionPanelView.swift:33` never reads it; `:175` Connect is filled accent. |
| F8 | sheets same | `AddressBarView.swift:107-109` grey lock, "Secure site" only as VoiceOver label; `ConnectionPanelView.swift:44-47`, `SiteMenuSheetView.swift:39-42` visible green `statusLine`. |
| F9 | networks same / accounts different | `ConnectionPanelView.swift:127-131` dot + name only; account picker `SettingsSheet.swift:382,393` identicon + amount. |
| F10 | same (swipe) | `ExploreScreen.swift:300` dismiss → `onSigningDismissed()`; `:315` `.presentationDetents([.large])`, no `interactiveDismissDisabled`; core `sign_request.rs:1044-1048` 4001 before signing. |
| F11 | same | `SigningLive.swift:343-346` `.positive("submitted")`; `:233-238` slide stays, disabled; `StatusHeroView` only in send (`FlowBodies.swift:1306`). |
| F12 | same | `SigningLive.swift:808-809` "Tap to retry"; `:844` row tappable; `SigningController.swift:400` `refreshFee()` has no caller; failed quote never auto-retried (only stale re-quote, core `fee_policy.rs:158`). |
| F13 | same (core) | `SignExecutor.swift:88` `receiptWaitMs 120_000`; core `tx_tracker.rs:68,72,856-857`. |
| F14 | same | `SigningLive.swift:205-206` host as name and host; `SigningAtoms.swift:57,62`. |
| F15 | same | `TabCardView.swift:32-37` two rounded rects + capsule. |
| F16 | same | `ExploreLive.swift:381-382` first letter; `LetterAvatarView.swift:6-8` "Deliberately NOT a fetched favicon"; signing header does fetch icons (`SigningLive.swift:63-70`). |
| T1 | same | `SigningLive.swift:233-238` slide independent of route; `Core/UserOpSpine.swift:475,494` trusted signer runs after `approve_tapped`. |

Files: engine `Features/Explore/Core/{BrowserEngine,BrowserController,DbrExecutor,ProviderBridge}.swift`,
`Components/Explore/BrowserWebView.swift`; chrome `Features/Explore/{ExploreScreen,ExploreLive}.swift`,
`Components/Explore/{AddressBarView,BrowserToolbarView,ConnectionPanelView,SiteMenuSheetView,TabCardView,LetterAvatarView}.swift`;
signing `Features/Signing/{SigningSheet,SigningLive}.swift`, `Core/{SigningController,SignExecutor,UserOpSpine}.swift`,
`Components/Signing/{SigningAtoms,SlideToConfirmView,SigningFooter}.swift`; tracker `Features/Send/Tracker*.swift`.

### Desktop (`app-desktop/vela-wallet/src/`)

Trusted signer handoff is the `velawallet://` scheme since 076 (`executor/trusted_signer.rs:21-31`), not loopback.

| Item | Verdict | Evidence |
|---|---|---|
| F1 | different | `wallet/page.rs:12871` visit when the document settles (injected `DOMContentLoaded`/`load`, `webview.rs:71-72`); WKWebView draws no error page. Server 404/5xx recorded. |
| F2 | narrower | `webview.rs:67-68` title + icon in one message; `:459` URL read from the webview later → a late `load` from the page being left can land under the new URL. |
| F3 | worse | no loading/progress anywhere in `explore/*`, `webview.rs`, `browser_host.rs`. |
| F4 | n/a | no load-failure panel; only the renderer-crash screen (`page.rs:12615-12621`). |
| F5 | worse | no didFail handling; only the toolbar reload (`page.rs:12605`). |
| F6 | same | `executor/dapp_browser.rs:252` failure → None → -32603; `executor/pool.rs:358` exposes only `rate_limited_chains`. |
| F7 | different (right) | `page.rs:13583` title drawn; `:13623` Connect is `accent_button`. |
| F8 | partial | `explore/components.rs:497-498` grey lock; `page.rs:13699-13703` connection panel shows `secure_site`, green when connected (`:13791`). |
| F9 | same + bug | `explore/fixtures.rs:355` Check/Network icons; `page.rs:13732` network dot always `chain_ethereum()`; "switch account" has no handler (`page.rs:13836`). |
| F10 | different (right) | third column, no outside click; Esc and ✕ → `SwipeDismissed` (`page.rs:16844-16856`, `5328-5337`). |
| F11 | same for ≤90 s | `executor/sign_request.rs:156,349-355`; `signing/live.rs:870-871` "Submitted"; slide dimmed 45% (`page.rs:14592`); status view only after the core closes the sheet (`page.rs:15156`). |
| F12 | same | `signing/components.rs:877` chevron only; `flows/live.rs:1089-1090` "—"; `signing_host.rs:444-446` tap re-quotes; `stale` ignored on this sheet (send uses it, `flows/live.rs:1261`). |
| F13 | same (core) | `page.rs:15199` every other status reads Submitted; `executor/sign_request.rs:416-420` page gets tx hash or userOp hash after 90 s. |
| F14 | same | `signing/live.rs:1328-1329`; `signing/components.rs:90-97`. |
| F15 | n/a | tab strip (`explore/components.rs:226-229`). |
| F16 | same | `explore/live.rs:152-154`; `components.rs:28-31`; favicon saved, never fetched (`webview.rs:110-113`). |
| T1 | same | `page.rs:14240` slide → `host.approve`; `executor/sign_request.rs:322-331` → `Signer::TrustedSigner`; `signing_host.rs:170-171` `open_url`. |

Files: engine `webview.rs`, `wallet/browser_host.rs`, `executor/dapp_browser.rs`; chrome `explore/{components,live,fixtures}.rs`,
`wallet/page.rs` (12289 explore, 13573-13874 consent/connection, 13882 networks); signing `signing/*.rs`,
`wallet/signing_host.rs`, `wallet/page.rs` 14002/15156, `executor/{sign_request,trusted_signer,user_op}.rs`;
fee `signing/components.rs:809`, `signing/live.rs:1203`, `wallet/speed_control.rs`; tracker `executor/{tracker,relay}.rs`.

### Rules that are already shared, and where they live

| Rule | Where |
|---|---|
| 4001 on dismiss before signing, silent after | core `sign_request.rs:1044-1048`, `1161-1190` |
| 120 s wait, 24 h abandon, never "failed" on timeout | core `tx_tracker.rs:14-72` |
| 30 s stale fee quote | core `fee_policy.rs:158` |
| -32603 "No endpoint answered" | core `dapp_browser.rs:1515` |
| `secure` of a tab (scheme judgement) | core `DbrTabView.secure` |
| Chain failed / rate-limited sets | core `rpc_pool.rs:886,1644,1843` |
| Address → URL or search | core `dapp_rpc::browser_input` |

Everything else in the table above — when a visit is recorded, the loading state, the failure
panel's words, the retry, the fee refresh control, the sheet's dismissal, the post-approval view,
the pickers, the letter — is decided separately in each shell. That is why the same defect exists
three times, and why the fix moves those decisions into the core as rules each shell calls.

## R1 — One failure classifier, per-platform tables in the core

- **Decision**: `browser_load::classify(platform, code, domain, certificate)` in vela-core, with the
  tables below. Shells pass the raw platform facts; they never pick words.

  | Class | Android `WebViewClient` | Apple `NSURLErrorDomain` | Desktop probe (reqwest) | Auto-retry |
  |---|---|---|---|---|
  | `offline` | `ERROR_IO` (-7), `ERROR_UNKNOWN` (-1, seen as `ERR_EMPTY_RESPONSE`), `ERROR_CONNECT` when the device has no route | -1009 not connected, -1005 connection lost, -1018 roaming off, -1020 data not allowed | connect error with no route | yes |
  | `timeout` | `ERROR_TIMEOUT` (-8) | -1001 | timeout | yes |
  | `not_found` | `ERROR_HOST_LOOKUP` (-2), `ERROR_BAD_URL` (-12) | -1003, -1006, -1000 | DNS error | no (a typo does not heal) |
  | `refused` | `ERROR_CONNECT` (-6) | -1004 | connection refused | yes |
  | `certificate` | `ERROR_FAILED_SSL_HANDSHAKE` (-11), `onReceivedSslError` | -1200…-1206, -2000 | TLS error | never |
  | `other` | anything else | anything else | anything else | once |
  | (not a failure) | — | -999 cancelled, `WebKitErrorDomain` 102 frame load interrupted | — | — |

- **Rationale**: the audit found three different texts: nothing on Android, the system string plus
  a code on iOS, and no panel at all on desktop. One table gives the same words on every client and
  one tested place to fix a misclassification.
- **Alternatives**: a per-shell `when`/`switch` (the status quo that drifted); passing the engine's
  description string (localized by the OS, not by Vela; untestable).

## R2 — Retry schedule

- **Decision**: `retry_delay_ms(class, attempt)`: attempts 1–3 at 2 000 / 5 000 / 10 000 ms for
  `offline | timeout | refused`; 1 attempt at 3 000 ms for `other`; none for `not_found` and
  `certificate`. The timer runs only while the tab is in front and the app is foreground; a new
  navigation cancels it. The panel shows "retrying" during an attempt and the manual Retry
  resets the count.
- **Rationale**: no connectivity callback is available on Android (047's manifest decision: no
  ACCESS_NETWORK_STATE), so a short capped schedule is the honest substitute.

## R3 — The visit rule, and reading one document

- **Decision**: `browser_load::visit_to_record(LoadFinished { url, title, icon, main_frame_failed,
  http_status })` returns `Some(visit)` only for a finished load with no main-frame failure and a
  status below 400 where the platform reports one; the url is the one the page script reported, not
  the web view's current url. Each shell reads `{href, title, icon}` in a single script evaluation.
- **Rationale**: F1 (Android error page recorded), F2 (Android: title and icon from the previous
  document; iOS: `?? title` and an async icon lookup; desktop: url read later than title).

## R4 — Loading and failure on desktop (wry 0.56)

- **Finding**: wry's navigation delegate reports `Started` at commit and `Finished` at finish and
  implements no `didFail*` (`wry-0.56.1/src/wkwebview/navigation.rs`): a failed navigation is
  silent.
- **Decision**: the shell marks `loading` when it asks for a load. If nothing commits within 3 s, it
  probes the URL natively (HEAD, 5 s budget) and feeds the probe's error through R1. A probe that
  succeeds keeps the progress up until 20 s, then shows `timeout`. A commit at any time clears the
  panel.
- **Alternatives**: patching wry's delegate (fork cost); leaving desktop with no panel (F4/F5 stay).

## R5 — Signing sheet: explicit close and the status body

- **Decision**: Android `VelaModalSheet` gains a `dismissible = false` mode (no drag, scrim tap
  swallowed, Back swallowed) used by the signing and consent sheets, with a ✕ in the header. iOS
  gets `.interactiveDismissDisabled()` plus a ✕. Desktop and extension already close only
  explicitly. The ✕ sends the core's existing dismiss event (reject before signing, plain close
  after). Post-approval, each client swaps the form for its existing `StatusHero` (Android
  `FlowBlocks.kt:593`, iOS `StatusHeroView`, desktop `dapp_receipt_body`, web `StatusHero`).
- **Rationale**: rulings 1 and 2; parity with Send (memory: dApp tx UX parity with Send).

## R6 — Tracker wording and cadence

- **Finding**: `TrackStatus::AcceptedNotLanded` already exists (window closed, not landed) and the
  entry view says `polling: false` once abandoned; no client shows either.
- **Decision**: add a receipt cadence that grows with age past the window (12 s until 10 min, 60 s
  until 1 h, 300 s until 24 h; abandon unchanged) and an `outcome` wording field
  (`landing | still_confirming | unknown`) on the entry view. Never a failure on time alone.

## R8 — Fee row

- **Decision**: the signing fee row on every client reuses its send flow's refresh control
  (Android `FeeRefreshButton`, iOS/desktop/web `refreshLabel`). `FeeFailure::QuoteUnavailable` reads
  `componentsUi.funding.denialNetworkError` ("无法连接 Vela 服务 — 请检查网络，稍后会自动重试。") and
  re-quotes on `fee_policy::requote_delay_ms(attempt)` = 3 s, 6 s, 12 s, then every 15 s, while the
  sheet is open and not approved.

### Extension (`app-web/vela-wallet/src/lib/`) and the trusted signer page (`app-web/trusted-signer/`)

| Item | Verdict | Evidence |
|---|---|---|
| F7 | right | `dapp/DappRequestHost.svelte:344` title; `:380` Connect filled, Cancel outlined `:377` |
| F8 | n/a | no lock/text in the live extension (`explore.secureSite` only in gallery routes) |
| F9 | n/a | no pickers; grant pins the active account (`dapp/request.ts:109`) |
| F10 | same | `wallet/ui/BottomSheet.svelte:29-34,475-477` scrim/Esc/drag/✕ → `reject_tapped` (`signing/SigningHost.svelte:376`) → 4001; locked only while signing/submitting (`:375`) |
| F11 | transactions right / messages close | `SigningHost.svelte:350-367,420-427` `DappReceipt`/`StatusHero`; message signature closes the sheet |
| F12 | worse | `signing/live.ts:468,477-478` "点击重试" only; stale never re-quoted (send has refresh + stale note: `flows/ui/FeeRow.svelte:68-73,97`) |
| F13 | "submitted" forever | `wallet/core/tracker-resident.ts:28-31,139-148` ticks only in the 120 s window; `SigningHost.svelte:129-135` |
| F14 | same | `DappRequestHost.svelte:252` `dapp: null` → `signing/live.ts:705,722-723` |

Signer page:

- **S1 "未知站点"**: `tag.unknownSite` (`src/lib/locales/zh.js:67`) when `ctx.dapp.name` is missing
  (`src/lib/resolve.js:176`); `warn.claimedOrigin` (`zh.js:136`) when `!originVerified && origin`
  (`resolve.js:1356-1357`). The URL channel hard-codes `originVerified: false`
  (`src/lib/intake.js:308`); the core never writes `context.dapp` (`vela-core/src/trusted_signer.rs:312-361`);
  Android builds `appName`/`appIcon` but never forwards them (`TrustedSignerChannel.kt:64-66`).
  The page knows when its answer goes to the Vela app (`answersToWallet`, callback `velawallet:`,
  `resolve.js:892-894`), used today only for key ceremonies.
- **S2 slide**: `src/sign.js:45-94` (commit at ≥88 %); no "already confirmed" flag; the one-slide
  change is app-only — a Continue button sends the same `ApproveTapped` (Android
  `SigningSheet.kt:210` → `VelaNavHost.kt:689`; iOS `SigningSheet.swift:67`; desktop `page.rs:14593`).
  Closing the tab never reaches the app (`sendBeacon` to `velawallet:` throws, swallowed
  `sign.js:426`); the app waits 5 min unless the person taps Cancel.
- **S3 failure words**: `ui.ceremonyFailed` (`zh.js:182`); only `NotAllowedError` is friendly
  (`sign.js:231-235`) and it covers cancel/timeout/no credential by WebAuthn design; `AbortError`
  and others show raw English `error.message` (`:237`). The knob is never reset (`:67`), so the
  retry is a tap on a knob parked at the end.
- **S4**: no auto-start; reload shows raw "channel url found no request" (`intake.js:431`,
  `sign.js:468-470`).
- **S5 integrity/offline**: 076 pins `b/<sha256>/sign.html` via `index.json` and `BUILD_ALLOWED`
  (`vela-core/src/trusted_signer/integrity.rs:62-110`, `ENFORCE=false` `:124`); only desktop checks
  (`app-desktop/.../executor/signer_integrity.rs:162-175`); Android/iOS open root `sign.html?ch=url`
  (`trusted_signer.rs:541-550`). No service worker. Hosting: Cloudflare (`server: cloudflare`),
  every path `cache-control: public, max-age=0, must-revalidate`, `.html` 308 → extensionless;
  deploy = copy `dist/` (`samples/build-single.mjs:35-47,267`), no CI in the repo.
- **S6 layout**: header (avatar, 未知站点, host, chain) → "批量" intent (a Vela op always has the
  call + the fee leg, `resolve.js:1284-1286`) → sentence → leg cards → rows → warnings → 技术细节 →
  fee with an always-open note → account → slider (not sticky, below the fold on a phone) → status.

## R7 — The trusted signer: one slide, an honest origin line, plain words, offline

- **One slide (app-side only)**: when the route is the trusted signer, the sheet's confirm is a
  button ("去签名页确认") that sends the existing approve event; the page's slide is the consent.
- **Origin**: the core fills `context.dapp = { name: host, origin, source: "vela_browser" }` for
  requests forwarded by an in-app browser; the page, when `answersToWallet` and `source ==
  vela_browser`, shows the host as the name with a neutral "来自 Vela 浏览器" line and no
  `warn.claimedOrigin`. Any other channel keeps today's caution. Rationale: the claimant in this
  channel is the Vela app, which observed the origin from the engine; calling it unknown contradicts
  the sheet the person just read.
- **Words and layout**: intent says what the person does ("发送 0.001 xDAI", not "批量"; the fee leg
  is shown as the fee row, not a leg); the explanation and technical details fold; the slider is
  sticky at the bottom; `ui.ceremonyFailed` becomes one everyday sentence and the knob resets;
  `AbortError` joins `NotAllowedError`; any other error shows the everyday line with the raw text
  inside 技术细节; a reload without a request says "这个签名请求已结束，请回到 Vela 重新发起".
- **Offline + integrity, without a service worker**: the apps open the content-addressed
  `/b/<hash>/sign` (the newest hash in `BUILD_ALLOWED`), and `dist/_headers` serves `/b/*` with
  `cache-control: public, max-age=31536000, immutable` (root stays must-revalidate). The browser's
  own HTTP cache (Chrome for the Custom Tab; SFSafariViewController's store on iOS — verify) then
  opens it with no network after one visit. The page change ships as a new hash added to
  `BUILD_ALLOWED` before the apps that open it.
- **Alternatives rejected**: a service worker (the page's hash-only CSP blocks registering it,
  `sw.js` would sit outside the hash and need its own pin — research S5); bundling the page in the
  app (a passkey for `getvela.app` must run on a `getvela.app` origin in the browser).
- **Owner action**: deploying the signer page (`dist/` incl. `_headers`) to sign.getvela.app is an
  outward-facing release — prepared here, deployed by the owner.
