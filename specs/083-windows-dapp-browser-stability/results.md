# Results: 083 — the dApp browser works, and tells the truth, on Windows

**Status**: done on the device, 2026-09-28. Branch `083-windows-dapp-browser-stability`, final build
`24578479`, installed with the real installer into `C:\Program Files\Vela Wallet` and exercised with the
owner's own account and iPhone. Before/after images: `evidence/` and `evidence/after/`.

**Second pass, 2026-09-29** — the owner's question "does a Uniswap swap really work on Windows?" (ETH → USDC
had worked, USDC → ETH had not, and the feedback was slow and poor), then the hand-off items H1–H9. All of
it merged as `ffc9ac9f` and re-run on Base with the parallel-space build: max USDC → ETH, ETH → USDC and
0.1 USDC → ETH all landed. See "Uniswap on Base" below.

## Success criteria

| SC | Verdict | Evidence |
|---|---|---|
| SC-001 installed / read-only exe opens the dApp; a failed engine start is one panel | **Pass.** Installed build from Program Files, no overrides: the test dApp opened, profile in `%LOCALAPPDATA%\VelaWallet\WebView2`, 0 engine errors. Read-only folder: opens. Missing runtime: panel "WebView2 0x80070002", 系统浏览器 first, 1 log line, 0.12 CPU-s/10 s. Application log: 14 WER reports in 22 s before; **0 crash events** from 15:00 to the end of the pass | `after/us1-*.jpg`, `08-*.jpg` (before) |
| SC-002 typed navigations name the new site, typing replaces | **Pass.** 3/3 new host at 0.35 s and 4 s; 4/4 click-type-Enter loaded exactly the typed site (was: previous site named, text appended) | `after/us2-bar-names-new-site.jpg`, `03-typed-url-appended.jpg` |
| SC-003 0 frames of Edge's pages across the fault matrix | **Pass.** drop: Vela panel 0.3–25 s; black hole: panel 12–80 s (Edge's timeout at ~40 s no longer replaces it); certificate: Vela panel, no interstitial | `after/us3-*.jpg`, `04/05/06-*.jpg` (before) |
| SC-004 recovers without a tap | **Pass.** Network restored at 4 s → Sushi loaded by itself at ~9 s | `after/us3-recovers-by-itself.jpg` |
| SC-005 renderer crash → Vela panel, Reload restores | **Pass.** Killed renderer process → "此页面已停止运行"; 重新加载 restored the page | `after/us3-renderer-crash-panel.jpg` |
| SC-006 `target=_blank` / `window.open` → new tab | **Pass** for a person's tap (new selected tab, Back disabled on it); script-initiated popups and `location='mailto:'` refused; a tapped `mailto:` handed to Windows once, escaped, no "选取应用" | `after/w6-new-window-is-a-tab.jpg` |
| SC-007 a native-coin send reads as a transfer | **Desktop pass** ("发送 0.001 xDAI · 接收方"). Web still draws its blind branch — hand-off H3 | `after/w10-plain-transfer-reads-as-send.jpg` |
| SC-008 no regression | **Pass.** 2026-09-29 on `ffc9ac9f`: desktop 779 passed / 0 failed, core (`crux,bindings,i18n-all`) 1896 / 0, web unit 1647 passed (4 fail on this Windows checkout only: CRLF line endings and an unbuilt extension — they fail the same way on the E2 branch). 2026-09-28: desktop suite 718 passed / 0 failed; core `app_browser_load` 15, `app_sign_request` 58, `app_dapp_browser` 62, `app_self_call_guard` 14, `app_clear_signing` 74, `cable::` 38; rustfmt clean. Happy path on Windows: connect, sign, EIP-1271 verify, switch chain, a dust send landed (`0xe2e2527a…`) | — |

## What was found and fixed

| # | Finding | Fix | Commit(s) |
|---|---|---|---|
| W1 | Installed browser never started (profile beside the exe) | profile via `wry::WebContext` under local app data / `VELA_STATE_DIR` | dbf5a48c |
| W1b | Failed start retried per paint, WER report each | `EngineFailure` state, panel, Retry only on request | dbf5a48c |
| W1c, W9 | Demo host "app.uniswap.org" / demo tabs in a live session | the host/tab being opened, else none | dbf5a48c |
| W2 | Enter reopened the bar on the previous site; typing appended | ignore keyboard clicks; Enter/Esc give focus back; `named_url` | 885867f4 |
| W3–W5 | Edge's error / certificate / crash pages | `webview2_events.rs` + core `LoadPlatform::WebView2` | 9e0fe081 |
| — | Lookup failures on a Chinese Windows read as generic | Winsock 11001/11004 in the probe and the proxy | 9fb9d5ce, 3cd27313 |
| D2 | Menus hid the whole page | region hole on Windows | 5c9b42a4 |
| W10 | Plain transfer drawn as "合约交互 / 无法解码" | desktop transfer rows; a stray `calls` key never becomes the headline (pre-existing, security) | 25ed87f6, d5661249 |
| D3 | One timeout flipped all traffic to Direct | per-host route; a proxy is down for all only when it fails for all | 4bc67c85, 3cd27313, 3d22bcd1 |
| W19, D1, W11 | No phone QR in dApp signing; Esc rejected; "等待生物识别" before any prompt | QR/touch/timeout cards in the column; exactly-once close; core safety net answers a dismissed + cancelled request 4001 | 545621d7, 4198ec6b |
| W6, W7, W14, W15 | New windows dropped; schemes; consent without account/network; toolbar jump; Back crossing tabs | new tab on a gesture; mailto/tel only on a tap, escaped; consent rows; per-tab back floor; chrome never shrinks | e6285fc9, a6b1b2fb, 84e09838 |
| W20 | iPhone over caBLE: Apple's tunnel closed with "Policy violation" | tunnel ids upper-case, as Chromium | 24578479 |

Every group was implemented, adversarially reviewed and fixed until a verifier returned MERGE.
The reviewer's risk that `Page.resetNavigationHistory` could crash WebView2 during a pending reload
was tested: 5/5 races answered "History cannot be pruned" (-32000), no crash.

## Uniswap on Base (2026-09-28/29)

Traced with the parallel-space account (MultiTest `0x88cC…6894`) on Base, where Uniswap runs the classic
path on the desktop: `wallet_getCapabilities` answers 4200, so it is Permit2 approve, a `PermitSingle`
typed-data signature, then Universal Router `execute`.

| # | What the person saw | Cause | Fix |
|---|---|---|---|
| U1 | **Max** USDC → ETH: 无法连接 Vela 服务 — 请检查网络, retried forever, slide shut, the fee row's ">" dead | the fee coin was auto-picked as USDC; the fee leg runs after the swap, which had taken every USDC; the relay's "simulation failed" was drawn as an outage | the fee pays from what the operation LEAVES (the column's simulation feeds the fee machine); a relay refusal is "would fail", not the network; the coin list opens over a failed quote — 380d9014, 5825e443 |
| U2 | (S2) a swap that reverted inside the op was answered as done | the receipt's `success` was dropped; the bundle tx's status is 0x1 either way | one error to the page, the record failed, the send receipt's failure in the column — ddb52da8 |
| U3 | (S3) after 90 s Uniswap got the userOpHash and waited on "pending" forever | no node knows an op hash | the page waits on an outcome (up to 10 min, the tracker's slow line), then gets "not confirmed yet", never the op hash — ddb52da8 |
| U4 | (S3b) Uniswap called a swap done when only its approval had landed | a relay "already pending" answer handed back the OTHER op's hash | wait for that op, then send this one; never another op's hash — ddb52da8 |
| U5 | (review) another account's Safe failure in the same bundle would have failed our swap | the receipt's logs are the whole bundle's | only the op's own execution logs count, in the core, for the tracker and the page alike — 2448738a |
| U6 | (review) a reverted swap first read "couldn't be submitted — your funds are safe" | the generic sentence until the tracker caught up | the send receipt's revert at once: sentence, hash, explorer — 2448738a |
| U7 | the swap was nowhere in 活动 | the feed kept only sends and receives | H2 — 43fd67a4, ff660c2d |
| U8 | fee ≈ $0.08–0.10 on a sub-dollar swap | measured 14.2× the chain cost: ×3 markup (`INBAND_MARKUP`), ×3.02 priced on padded limits not gas used, ×1.57 the default *fast* tier; conversion and L1 fee are fine | README corrected (ffc9ac9f); pricing is the owner's decision (below) |

Device re-run on `ffc9ac9f` (2026-09-29, Base):

| Swap | Vela | On chain |
|---|---|---|
| **Max** 0.271741 USDC → ETH | fee auto-picked in **ETH** (0.000031 ETH ≈ $0.08), no network sentence, slide live at 9 s; a manual refresh kept ETH; 准备 2 s → 已提交 10 s → **已确认 16 s** with the tx hash | `0x01a5b41f…79d831bd`: status 1, `UserOperationEvent.success` 1, 0.271741 USDC out, ETH in |
| 0.0001 ETH → USDC | Uniswap's own "low native balance" caution first; sheet ready 8 s; **已确认 18 s** | `0xfdc8a557…19a1dd20`, +0.269487 USDC |
| 0.1 USDC → ETH | fee stays **USDC** (0.084558) — 0.17 USDC is left after the swap, so the usual order stands; landed at ~15 s, Uniswap's toast 已兑换 0.100 USDC | `0xbba4aa9f…01e96b1`: status 1, success 1, 0.1 USDC to the pool, 0.084558 USDC fee |

Uniswap listed each as confirmed on its own node — the hash Vela answered is the transaction's. In 活动 all
three are rows under app.uniswap.org; see F1 below for what those rows still did not say.

Found on this run and fixed (F1–F3, see below): a dApp row said 合约交互 with no amount although the sheet
had shown 余额变化; the detail's hash ran off the panel; the Universal Router was labelled 接收方.
Not fixed: the sheet's headline for `execute` is the English function name ("Execute") from the selector
database — the Universal Router has no clear-signing descriptor yet (it would read "兑换 0.1 USDC → ETH").

## Owner decisions (2026-09-28)

D1 Esc never answers a pending request — done. D2 region hole — experiment passed, done. D3 per-host
route — done. D4 consent shows account + network — done. D5 (per-user installer) — not asked;
"Install for me only" is offered (H9); moving existing per-machine installs is still open.

**Open for the owner (2026-09-29):**

1. **Fee pricing** (U8). Price on simulated gas instead of the padded limits (≈30% lower, still funds
   the relay: 3 × R / (1.4 × cap) ≈ 1.39× the priced gas); default speed *standard* on chains whose
   base fee sits at its floor, e.g. Base (34–45% lower); the ×3 markup itself (also the only buffer
   against gas drift). A 0.1 USDC swap paid a 0.085 USDC fee.
2. **H4, what a dApp sees**: a phone that never connected now leaves the request open under 重试 / 关闭
   (before: an immediate -32603).
3. **H6 desktop-first**: hedged reads ship in the desktop pool (macOS and Linux too); web, iOS and
   Android still wait one endpoint's 8 s.
4. **i18n budget**: "would fail" reuses the first clause of an existing sentence; a sentence of its own
   needs the residency budget raised.
5. **D5**: move the per-machine install to per-user (one administrator uninstall, then `/CURRENTUSER`).
6. **H4 remainder**: retrying a relay failure after the transaction was signed could sign the same
   nonce twice — decide before it is offered.

## Where the clients differ after 083

- Core change reaching every client: a request whose sheet was dismissed and whose passkey was then
  cancelled now gets one 4001 (before: no answer). Android, iOS and web (`assets/wasm` next build).
- Core changes every client gets with its next core build: an op fails only on its OWN execution logs
  (U5); dApp transactions are Activity rows (H2); the fee machine weighs what an operation leaves (U1)
  — but only shells that report balance changes and relay refusals get U1 (the desktop today).
- **Follow-up for web, iOS and Android**: they still answer a reverted op as success, the op hash after
  120 s, and a pending op's hash (S2/S3/S3b) — adopt `Reverted` / `NotConfirmed` / `existing_op`; hedge
  reads in their pool drivers (H6); Android and iOS map `dappOrigin` / `intent` into their feed rows (H2;
  the web does).
- macOS gets: the address-bar fix, the signing-column close rule and QR card, Esc, preparing label,
  consent rows, back/forward state, no downloads, `mailto:`/`tel:` refused (no gesture flag there).
  macOS keeps hiding the page under menus, and has no WebView2 events.

## Hand-off items H1–H9 (2026-09-29)

| # | What | State | Device |
|---|---|---|---|
| H1 | sign-in sheet copy | **done** — the phone row says 扫码; "this device" names Windows Hello on Windows, Touch ID on a Mac (d9ab6e80, 5c79c6b3) | **pass**: 手机或平板 · 扫码, 这台设备 · Windows Hello |
| H2 | a dApp transaction in 活动 | **done** in the core + desktop + web (43fd67a4, ff660c2d): pending until it lands, under the site, a page cannot put a counterparty or a figure there | **pass**: the three swaps are rows under app.uniswap.org; detail 已确认 with the real tx hash. What they moved: F1 |
| H3 | web: a plain coin send reads as a send; "preparing" until the passkey is asked | **done** on the web (3707e196, 1618a9f6, 4d4c76c4) | web unit + e2e by the agent; not run in a browser here |
| H4 | phone stops told apart; a failed message is not a failed transaction | **done on desktop** (a22e1b30, 95b8e573, 1063909f) — owner items 2 and 6 above | needs the owner's iPhone |
| H5 | "check your phone" once scanned, with Cancel; handshake waits seconds | **done on desktop** (a22e1b30, 95b8e573) | needs the owner's iPhone |
| H6 | hedged reads | **done on desktop** (8872b915, e6b7469e) — owner item 3 | **not reproduced**: the pool ranks endpoints by speed, so a slow or silent user node (a local node that holds every request) was never asked once faster public nodes answered — reads stayed at ~220–330 ms. The 1.5 s hedge only matters when the fastest node falls silent, which this network could not stage without faking chain data; covered by unit tests |
| H7 | a profile folder Windows will not let Vela write | **done** (672d1dbb, 5c79c6b3): per-process write probe → engine panel | **pass**: write-denied folder → 无法加载此页面 · WebView2 0x80070005 with 重试 / 在系统浏览器中打开 at 2 s, one log line (was: blank forever) |
| H8 | after a failed load the tab keeps the previous title | **done** (0ff7fbed) | **pass**: expired.badssl.com after Uniswap → tab "expired.badssl.com", warning lock, 网站证书有问题，Vela 已阻止打开。 |
| H9 | D5 per-user install | **partly done**: "Install for me only" (`/CURRENTUSER`) beside per-machine (d344d847, 5adf0c17); moving an existing per-machine install waits on owner item 5 | the installer compiles from the merged tree (Inno Setup, 2026-09-29); running it over the owner's per-machine install (quickstart F4) needs the owner's UAC — not run |

Relay faults still cannot be held on the app's own traffic on this machine (its route falls back to
Direct around the fault proxy, and the TUN makes Direct work) — S5/S7 were not re-run.
