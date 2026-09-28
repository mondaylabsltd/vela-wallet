# Results: 083 — the dApp browser works, and tells the truth, on Windows

**Status**: done on the device, 2026-09-28. Branch `083-windows-dapp-browser-stability`, final build
`24578479`, installed with the real installer into `C:\Program Files\Vela Wallet` and exercised with the
owner's own account and iPhone. Before/after images: `evidence/` and `evidence/after/`.

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
| SC-008 no regression | **Pass.** Desktop suite 718 passed / 0 failed; core `app_browser_load` 15, `app_sign_request` 58, `app_dapp_browser` 62, `app_self_call_guard` 14, `app_clear_signing` 74, `cable::` 38; rustfmt clean. Happy path on Windows: connect, sign, EIP-1271 verify, switch chain, a dust send landed (`0xe2e2527a…`) | — |

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

## Owner decisions (2026-09-28)

D1 Esc never answers a pending request — done. D2 region hole — experiment passed, done. D3 per-host
route — done. D4 consent shows account + network — done. D5 (per-user installer) — not asked; still open.

## Where the clients differ after 083

- Core change reaching every client: a request whose sheet was dismissed and whose passkey was then
  cancelled now gets one 4001 (before: no answer). Android, iOS and web (`assets/wasm` next build).
- macOS gets: the address-bar fix, the signing-column close rule and QR card, Esc, preparing label,
  consent rows, back/forward state, no downloads, `mailto:`/`tel:` refused (no gesture flag there).
  macOS keeps hiding the page under menus, and has no WebView2 events.

## Not done, and why — hand-off

- **H1** W16 sign-in sheet copy ("扫码，用附近设备创建"; "Touch ID 或 Windows Hello" on Windows).
- **H2** W17 / 079 D3: a landed dApp transaction is not in 活动 (every client).
- **H3** SC-007 on the web: the web still draws a plain transfer on its blind branch; its signing
  status also says "waiting for biometric" while preparing.
- **H4** — **done on desktop** (a4ecf408 and its review fixes; no device run yet). A failed message
  signature says 链下签名 — 未向链上发送任何内容。, not the transaction's sentence. A phone that
  scanned but never connected (tunnel or handshake), or whose connection dropped once it was asked, no
  longer answers the page -32603 in transport English: the request stays open under 网络连接不稳定
  with 重试 / 关闭 (the body: the request never arrived; or, once asked, the transaction was not
  submitted / nothing went on chain), and 关闭 is the person's 4001. **Owner to confirm this change in
  what a dApp sees** — it follows W19's scan timeout; before, an immediate -32603. A tunnel the phone
  closes while connecting is the phone's cancel: back to the form, as mid-prompt (spec 038 finding
  18). macOS: a BLE channel that will not open falls through to the tunnel, as on Linux. Not done: a
  relay failure after a transaction was signed (a retry could sign the same nonce twice — owner
  decision); vi `networkBody` says "máy chủ" (server) — right where the line is shared (balance,
  onboarding), loose on the phone card; a phone-only line needs a new key and the translation pass.
- **H5** — **done on desktop**: the QR comes down the moment the phone's advert decrypts, and
  "查看你的手机" — with a Cancel, since the phone has been asked nothing yet — stands over the
  connection. The tunnel upgrade and the handshake's two frames wait 15 s each, not 130 s: the phone
  answers them with nobody touching it (confirm in the `[vela-cable]` log on the device).
- **H6** D3b hedged reads (a dApp read still waits one endpoint's 8 s before the next).
- **H7** A profile folder WebView2 accepts but cannot use makes the engine wait forever (only an
  artificial ACL produced it; research R1).
- **H8** After a certificate failure the tab keeps the previous page's title.
- **H9** D5: the installer is per-machine (admin); an unattended upgrade times out on UAC.
- Relay faults could not be held on the app's own traffic on this machine (its route falls back to
  Direct around the fault proxy, and TUN makes Direct work) — S5/S7 were not re-run.
