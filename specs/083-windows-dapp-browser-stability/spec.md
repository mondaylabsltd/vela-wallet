# Feature Specification: The dApp browser works, and tells the truth, on Windows — installed, on a bad network

**Feature Branch**: `083-windows-dapp-browser-stability` (off `main` de93634f, v0.9.5 + 079)

**Created**: 2026-09-28

**Status**: Draft

**Input**: Owner, 2026-09-28:

> 在当前windows 上安装最新的vela wallet , 然后 来测试 dapp browser ，遇到需要我按指纹的地方，提醒我，我来帮助你， 你要看界面视觉，文案，UI/UX, 以及windows 日志，等等，来判断是否需要优化， 目标是要达到很好的体验，在一个不稳定的环境里构建稳定的用户体验， 使用github speckit 083 来完成这个问题

> 如果用的是平行空间账户的，所有过程都能你自己完成吧

This spec is the result of that device pass: the Windows desktop (Rust + gpui, WebView2 141.0.3537.99
through wry 0.56.1) on this machine — Windows 11 Pro 26200, 1.75× display, in China behind v2rayN +
sing-box (system proxy `127.0.0.1:10808`, TUN). Two builds of main de93634f: the release exe the
installer ships (`VelaWallet-Setup-0.9.5-x64.exe`), and the same build with `dev-fixtures` so the
parallel space signs without a fingerprint (account MultiTest `0x88cC…6894`, Gnosis). Every page and
the app's own traffic went through the fault proxy `scripts/device/chaos-proxy.py` (drop, black hole,
latency, per host, upstream the machine's own proxy), so each fault below was produced on purpose and
repeated. The page inside WebView2 was driven and read over the DevTools protocol
(`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port`); the wallet window by posted input,
captured with PrintWindow; Windows' Application log was read after each run. The upgrade itself needs
an administrator prompt, which timed out unattended; the installed product was exercised as the
installed v0.9.2 and as the 0.9.5 release exe run from a folder its user cannot write (what
`C:\Program Files` is). Screenshots are in `evidence/`.

079 fixed the same class of problems on every client, but its desktop column was verified on the Mac
only (079 `plan.md:12`, `results.md:14`). WebView2 behaves differently from WKWebView in exactly the
places 079's desktop design relied on, and the installed Windows app had never been run.

## What the pass found, in the order a person meets it

| # | Sev | Where | Found | Evidence |
|---|---|---|---|---|
| W1 | P0 | Install → Explore | WebView2 is given no user-data folder, so it uses `<exe dir>\vela-wallet.exe.WebView2`. The installer puts the exe in `C:\Program Files\Vela Wallet`, which the user cannot write: creation fails `0x80070005 拒绝访问`. **The installed app's dApp browser has never opened** — reproduced with the installed v0.9.2 and with the 0.9.5 release exe | `08-installed-browser-blank-fixture-host.jpg`, `08b-installed-092-blank.jpg`, `08-webview2-access-denied-stderr.txt` |
| W1b | P0 | Explore | The failed creation is retried on every paint (~6/s, 8 % of a core), reported only to stderr, and each attempt makes WebView2's loader file a Windows Error Reporting event (Application Error 1000, `EmbeddedBrowserWebView.dll`, 0x80000003) every ~1.7 s until Windows throttles it | Application log 14:50:45–14:51:07 |
| W1c | P0 | Address bar | With nothing loaded, the bar says **"🔒 app.uniswap.org"** — the gallery's demo host (`page.rs:12511-12514`): a lock beside a site that is not there | `08-…jpg` |
| W2 | P0 | Address bar | Enter's key-up becomes a keyboard click on the focused bar (gpui `div.rs:2800-2850`), which re-opens editing with the PREVIOUS page's URL selected. After any typed navigation the bar never shows the lock and the site; it names the previous site; the next click drops a caret into that stale URL and what is typed is appended — "example.com" typed on PancakeSwap loaded `https://pancakeswap.finance/example.com`, while the tab said "Uniswap Interface" and the bar `app.uniswap.org/not-found` | `02-bar-names-previous-site.jpg`, `03-typed-url-appended.jpg` |
| W3 | P0 | Load failure | WebView2 shows its own error pages and wry reports them as a page arriving: a refused connection shows Edge's "当前无法使用此页面 … ERR_EMPTY_RESPONSE" from 0.3 s and never Vela's panel; a black hole shows Vela's panel at 9 s and 正在重试…, then at ~40 s Edge's "嗯… 无法访问此页面 … ERR_TIMED_OUT" replaces it and the retries stop. When the network came back nothing reloaded | `04-drop-edge-error-page.jpg`, `05-blackhole-panel-then-edge.jpg` |
| W4 | P1 | Certificate | `expired.badssl.com`: Edge's interstitial with **高级 → continue anyway** inside the wallet | `06-cert-interstitial-advanced.jpg` |
| W5 | P1 | Renderer crash | Edge's "此页存在问题 STATUS_BREAKPOINT" page; Vela's crash panel is macOS-only; the chrome keeps the green "connected" dot and the old title | `07-renderer-crash-edge-page.jpg` |
| W6 | P1 | Links | `window.open()` returns `null`; a `target=_blank` link does nothing | CDP |
| W7 | P2 | Links | `mailto:` goes straight to Windows' "选取应用" dialog, with nothing from Vela | — |
| W8 | P1 | Signing | Esc closes a pending signing column and answers the page `4001` — one key (see D1) | `13-esc-rejects-signing.jpg` |
| W9 | P1 | Tab strip | Signed in with no recorded tab (fresh profile; first navigation), the strip shows the demo tabs "Uniswap · Polymarket", which do nothing | `01-fixture-tabs-flash.jpg` |
| W10 | P2 | Send sheet | A plain 0.001 xDAI transfer (no calldata) is titled "合约交互", with an amber "⚠ 无法解码 — 无 ERC-7730 描述符（0 字节）" and "交互合约 未验证" | `11-send-contract-interaction.jpg` |
| W11 | P2 | Send receipt | "等待生物识别…" for 2 s and more while nothing asks for a fingerprint: the label covers the network steps before the signature | `12-send-receipt-biometric-label.jpg` |
| W12 | P2 | Menus | Opening ⋯ (or any menu over the page) hides the whole page | `09-menu-hides-page.jpg` |
| W13 | P2 | Reads | With the chain's first node black-holed, each dApp read waits 8.5 s (8 s per node, no hedging); the one timeout also flips the whole app's traffic to Direct for 60 s | `chaos.log` 14:40 |
| W14 | P2 | Consent | Connect names the site but not the account or network it will get | `10-consent.jpg` |
| W15 | P3 | Chrome | The toolbar moves ~3 px between the start page and a page; Back looks enabled on a new tab (078 E-04) | — |
| W16 | P3 | Sign-in sheet | "手机或平板 — 扫码，用附近设备**创建**" in the sign-in sheet; "Touch ID 或 Windows Hello" on Windows | `15-login-sheet-copy.jpg` |
| W17 | P3 | Activity | A dApp send that landed is not in 活动 (079 D3, every client) | — |

What held up on Windows and must not regress: EIP-6963 announce and the legacy provider on every
load; a framed origin gets no provider; consent → connected, with account and network in the
connection panel; `personal_sign` → ✓ 已签名 → the column closes and the page has the signature
(EIP-1271 `isValidSignature` valid); `wallet_switchEthereumChain`; a real send landed on Gnosis
(`0xb9f632c5…4173`), receipt with ring and countdown; the fee quote in ≤ 4 s with refresh and the
stale line at 15 s; the progress hairline within 0.3 s of Enter on a 6 s first byte; Vela's failure
panel and its retries (while they last, W3); `alert()` names the origin; Reload recovers a dead
renderer; the app's own traffic routes around a dead proxy.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — The installed app opens the browser, or says plainly why it cannot (Priority: P1) 🎯 MVP

A person installs Vela with the installer and opens Explore. Today the page stays blank under a
lock and "app.uniswap.org", forever, while Windows logs a crash report every two seconds.

**Why this priority**: every Windows user of the installer has no dApp browser at all. Nothing else in
this spec matters until the engine starts.

**Independent Test**: run the release exe from a folder the user cannot write (and, once the owner
approves the prompt, the installed build from `C:\Program Files`); open the test dApp.

**Acceptance Scenarios**:

1. **Given** the app installed under `C:\Program Files`, **When** Explore opens a site, **Then** the site loads, and its browser profile lives in the user's own app-data folder.
2. **Given** the engine cannot start (runtime missing, folder refused), **When** Explore opens, **Then** one Vela panel says so in plain words with Retry and "在系统浏览器中打开", the app does not retry on its own more than a bounded number of times, and Windows' log gains no crash report per attempt.
3. **Given** nothing has loaded, **When** the bar is drawn, **Then** it names no site (never a demo host) and draws no lock.

### User Story 2 — The address bar says which site is open, and typing replaces it (Priority: P1)

After typing an address and Enter, the bar goes back to editing the previous site's URL; the next
address typed lands inside it.

**Why this priority**: the bar is the one place a wallet user checks before connecting or signing.
It currently names the wrong site, and turns ordinary typing into a navigation to a different page.

**Independent Test**: type three addresses in a row with Enter; after each, capture the bar at 0.5 s
and when the page has loaded; read the page's `location.href`.

**Acceptance Scenarios**:

1. **Given** an address typed and Enter, **When** the load starts, **Then** within 0.5 s the bar shows the lock rule and the host being opened, not the previous site, and is no longer editing.
2. **Given** a page open, **When** the person clicks the bar and types, **Then** the typed text replaces the address (the address is selected on the first click).
3. **Given** a signed-in person with no recorded tab, **When** the first page opens, **Then** the strip shows that page's tab, never the demo tabs.

### User Story 3 — On Windows a page that fails never shows Edge's page, and comes back by itself (Priority: P1)

**Why this priority**: 079 promised "never the engine's raw error page" (SC-004) and automatic
recovery; on WebView2 both are lost, and a certificate error can be clicked through.

**Independent Test**: 079 quickstart rows L2–L5 on Windows with the fault proxy, 1 s frames over 60 s
per fault; a renderer crash via DevTools `Page.crash`.

**Acceptance Scenarios**:

1. **Given** a refused, reset, unresolvable or silent host, **When** the page is opened, **Then** Vela's panel names the reason (no network / site not found / too slow) and retries on the 079 schedule; Edge's error page is never on screen, before, during or after a retry.
2. **Given** the fault is removed while the panel is up, **When** the next retry fires, **Then** the page loads without a tap.
3. **Given** an invalid certificate, **When** the page is opened, **Then** Vela's panel says the site's certificate is not valid, offers no way to continue, and does not retry by itself.
4. **Given** the page's renderer dies, **When** it happens, **Then** Vela's crash panel with Reload replaces the page within 1 s.

### User Story 4 — Links that open a new window open a new tab (Priority: P2)

**Independent Test**: the test dApp's `target=_blank` link and `window.open(url)`; a `mailto:` link.

**Acceptance Scenarios**:

1. **Given** a `target=_blank` link or `window.open('https://…')`, **When** activated, **Then** a new Vela tab opens on that address and becomes the tab on screen.
2. **Given** a link with a scheme other than http/https, **When** activated, **Then** the page never navigates to it; only `mailto:`/`tel:` are handed to Windows.

### User Story 5 — The signing column says what is happening, in words a person reads (Priority: P2)

**Acceptance Scenarios**:

1. **Given** a request that sends only the chain's own coin (no calldata), **When** the column opens, **Then** it reads as a transfer of that amount to that address, with no decode warning.
2. **Given** an approved request, **When** the wallet is still preparing (estimate, nonce, deployment), **Then** the status says it is preparing; the biometric line appears only when the authenticator is asked.

### Edge Cases

- WebView2 runtime absent (Windows 10 LTSC without Edge): US1 panel, with the runtime named.
- Two Vela processes (installed + a dev build) at once: each has its own profile folder, neither blocks the other.
- `VELA_STATE_DIR` set (tests, isolated runs): the browser profile follows it.
- An error page committed while a retry's timer is pending: the retry still runs; the engine page is never shown.
- A redirect chain that ends in an error: one failure, reported for the address the person asked for.
- `window.open` with no URL (`about:blank` popups used for OAuth): refused (see Assumptions).

## Requirements *(mandatory)*

**Engine (US1)**
- **FR-001**: The desktop MUST give WebView2 a user-data folder under the user's own app-data directory (`%LOCALAPPDATA%\VelaWallet\WebView2`), or under `VELA_STATE_DIR` when that is set.
- **FR-002**: A failed engine start MUST be shown as a Vela panel with the reason class (runtime missing / folder refused / other), Retry and 在系统浏览器中打开; automatic retries MUST be bounded (no retry per paint).
- **FR-003**: "Clear browsing data" and Erase MUST clear the folder of FR-001, including when Explore was not opened in the session.

**Truthful chrome (US1, US2)**
- **FR-004**: In a signed-in session the bar MUST NOT show the demo fixture host, and the strip MUST NOT show the demo fixture tabs.
- **FR-005**: Enter in the address bar MUST NOT re-open editing; after Enter the bar MUST show the host being opened.
- **FR-006**: Opening the bar for editing MUST start from the address being loaded when a load is pending, selected.

**Loads (US3)**
- **FR-007**: On Windows, an engine error page MUST NOT count as a page arriving; a navigation that completes unsuccessfully MUST be reported as a failure at once, with its reason from WebView2's error status.
- **FR-008**: While a failure or a retry is in progress the webview MUST stay hidden under Vela's panel.
- **FR-009**: A certificate error MUST end the navigation with no option to continue, and MUST NOT be retried automatically.
- **FR-010**: A renderer or browser-process failure MUST show Vela's crash panel on Windows as it does on macOS.

**Links (US4)**
- **FR-011**: A new-window request for an http(s) address MUST open a new Vela tab on it; any other new-window request MUST be refused.
- **FR-012**: A navigation to a scheme other than http, https, about, data or blob MUST NOT happen in the page; `mailto:` and `tel:` MAY be handed to Windows.

**Signing words (US5)**
- **FR-013**: A request that only moves the native coin (empty calldata) MUST be summarised as a transfer, on every client (core).
- **FR-014**: The status MUST NOT say it waits for a biometric before the authenticator is asked.

**Guardrails**
- **FR-015**: No change to what a page may ask, which origin signs, or the answer codes (079 FR-021).
- **FR-016**: New words go into the core catalog in all 15 locales, within the i18n budget; reuse keys first (079 `contracts/core-rules.md:101-105`).
- **FR-017**: macOS behaviour is unchanged unless stated; Linux builds (no webview) still compile.

### Key Entities

- **Engine start state**: not started / starting / ready / failed(reason, attempts).
- **Load outcome (WebView2)**: committed page / engine error page (not a commit) / failed(WebErrorStatus → reason) / crashed.

## Success Criteria *(mandatory)*

- **SC-001**: From a folder its user cannot write, the release build opens the test dApp in 10 of 10 launches; with the engine made to fail, one panel, ≤ 3 automatic attempts, and 0 new Application-log crash events per minute afterwards.
- **SC-002**: 10 typed navigations in a row: after each, within 0.5 s the bar shows the new host (0 of 10 name the previous site) and the page's `location.href` is the typed site (0 appended).
- **SC-003**: Faults drop / black hole / DNS / certificate on Windows, 1 s frames for 60 s each: 0 frames of an Edge error page or interstitial.
- **SC-004**: With the fault removed mid-panel, the page loads without a tap in 3 of 3.
- **SC-005**: Renderer crash: Vela's crash panel within 1 s and Reload restores the page, 3 of 3.
- **SC-006**: `target=_blank` and `window.open`: a new tab on the address, 3 of 3 each.
- **SC-007**: A native-coin send reads as a transfer (no "合约交互", no decode warning) on desktop and web.
- **SC-008**: No regression: the happy path above on Windows (connect, sign, verify, switch, send); desktop, core and web suites green; macOS builds.

## Decisions left to the owner

- **D1 — Esc on a pending signing request (W8).** 079 counted Esc as an explicit close on desktop (`079 spec.md` F10 ✓). On Windows it is also the key that leaves the address bar, and one press answered the page 4001. Recommendation: Esc leaves the address bar and closes menus, but never answers a pending request; only ✕ does.
- **D2 — Menus over the page (W12).** Today the page disappears while a menu is open. Options: a still snapshot of the page behind the menu (WebView2 `CapturePreview`), or keep as is.
- **D3 — Slow nodes (W13).** Hedged reads (ask the next node after ~1.5 s, first answer wins) and per-host route choice instead of one process-wide switch to Direct. Touches the core pool on every client.
- **D4 — Consent names account and network (W14).**
- **D5 — The upgrade needs an administrator prompt.** Unattended, the UAC prompt timed out twice and the upgrade was cancelled. A per-user install needs no prompt and would also have avoided W1 — but moves existing installs.

## Assumptions

- `window.open` without an http(s) URL (OAuth-style popups that script `window.opener`) is refused; a dApp that needs a popup still works in the system browser (在系统浏览器中打开).
- The parallel space stands in for the passkey everywhere except the one row that must see Windows Hello (the installed build's signing), which waits for the owner's fingerprint.
- Money moved is dust on Gnosis from the parallel-space account.
