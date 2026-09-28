# Feature Specification: The dApp browser and signing hold up on a bad network — on every client

**Feature Branch**: `079-android-dapp-browser-stability` (worktree `vela-wallet-079`, off `main` a2c46438)

**Created**: 2026-09-28

**Status**: Draft

**Input**: Owner, 2026-09-28:

> 在连接的安卓设备上 安装最新的 apk, 然后 来测试 dapp browser … 你要看界面视觉，文案，UI/UX, 以及安卓日志，等等，来判断是否需要优化，目标是要达到很好的体验，在一个不稳定的环境里构建稳定的用户体验

This spec is the result of that device pass: v0.9.5 (main a2c46438) on the Xiaomi alioth
(Android 13, System WebView 153) with the owner's own wallet, the phone routed through a
fault-injecting proxy (latency, drop, black hole and cut tunnels, per host) so every fault
below was produced on purpose and repeated. The owner pressed the fingerprint and watched
the phone. Nine rulings below are theirs, given during the pass. Screenshots are in
`evidence/`.

## Rulings the owner gave during the pass (2026-09-28)

1. > 签名框不小心容易关掉，关掉后就必须由 dapp 重新发起了，我觉得这很不合理，除非用户明确关掉，不应该很容易误操作，比如下滑就关掉了

   The pass recorded it twice: a touch at 08:29:26 closed a send sheet and the page got
   `4001 User rejected` (`evidence/strip-w.jpg`). This **overturns** the sheet's written
   rule "Dismissal is rejection" (`SigningSheet.kt`). The rule's other half, "no big Reject
   button", is kept: the explicit close is one quiet ✕.

2. > 可信签名器签完后，回到签名提示框，似乎没有任何提示

   After the signature, the sheet keeps the whole form and the greyed-out slide
   ("滑动以确认 · 确认发送"); the only change is a small line at the top
   (`evidence/strip-postsign.jpg`).

3. > 似乎没有刷新网络费的按钮呀

4. > 可信签名器页面看起来也很复杂，小白都不敢用了 (see **Decisions left to the owner**, D1)

5. > 你标记的安全站点 只是https 而已，并不代表这个站点真的安全，因为它们可能让你签恶意交易
   > — 连接已加密 我感觉不需要 — 用一把锁代表 https 和非https 就行了，不文字标记

6. > 连接时 切换网络，没有网络logo呀

7. > 切换账户也缺少了 nimiq 账户logo

8. > 而且看不到余额 — 需要能看到这个网络上的余额吧

9. Asked whether the trusted signer page joins 079 (D1 below, now User Story 7), the owner chose
   **yes, in 079**: one slide instead of two (the app hands off with a button; the page's slide is the
   one consent), a page a beginner can read, no "未知站点" alarm for the app's own browser, plain
   failure words, and a page that opens without the network.

## What the pass found, in the order a person meets it

| # | Where | Found | Evidence |
|---|---|---|---|
| F1 | Recents | A failed load is recorded as a visit, titled with the engine's error page ("网页无法打开") | `02-explore.jpg`, store dump |
| F2 | Recents | A visit is recorded with the PREVIOUS page's title and icon (bscscan.com saved as "Buy, sell & trade … on Uniswap" with Uniswap's favicon) | store dump |
| F3 | Address bar | After Go / Retry on a slow network nothing moves for seconds: old page, old host, no progress, because progress starts only when the engine commits the new page | `strip-slow2.jpg` |
| F4 | Load failure | Retry removes the panel at once and shows the engine's raw error page (green robot, `net::ERR_…`) for the whole retry | `strip-retry2.jpg` |
| F5 | Load failure | The panel never says why (no network / site not found / certificate / too slow) and never retries on its own | `10-uni-drop-3.jpg` |
| F6 | Page chain unreachable | With every node of the site's chain down, the page waits 24.5 s per read for `-32603`; the browser shows nothing, the chip stays green | log, `08-rpc-down` |
| F7 | Consent | The sheet has no title (the model sets `connect.browser.title`, the panel never draws it); Connect is drawn in the outlined Disconnect style | `05-consent.jpg` |
| F8 | Everywhere | "安全站点" in green for any https page, including one that asks for a malicious signature | `05-consent.jpg`, `14-sitemenu.jpg` |
| F9 | Pickers | Network and account pickers are text only: no network logos, no balance per network, no account identicons | `30-network-picker.jpg` |
| F10 | Signing sheet | Closes on a swipe down, a tap on the scrim or Back; the page gets 4001 and must ask again | `strip-w.jpg`, log 08:29:26 |
| F11 | Signing sheet | After signing: no progress, no landed moment; the slide stays on screen, greyed | `strip-postsign.jpg` |
| F12 | Signing sheet | A relay that does not answer turns the fee into "点击重试" with no reason; nothing retries when it comes back; no way to refresh a quote that is still showing | `strip-relayfail.jpg` |
| F13 | Signing sheet | A transaction the relay accepted but never mines: the page is answered at 120 s with the operation hash, the sheet says nothing more, and the tracker keeps polling it every few seconds for up to 24 h (two Arbitrum operations, one from 2026-09-27, polled all morning) | log, bundler replies |
| F14 | Signing sheet | Header repeats the host ("127.0.0.1:8137" over "127.0.0.1:8137") | `16-sign-sheet` |
| F15 | Tabs | Every tab thumbnail is the same drawing (two fields and a green button), not the page | `09-tabs.jpg` |
| F16 | Site avatars | `app.uniswap.org` is the letter "A"; the favicon the browser already records is never shown | `02-explore.jpg` |

### The same findings on every client (owner, 2026-09-28: "这些问题在 iOS iphone chrome ext desktop 都可能存在，你需要验证，如果可以，最好统一修复，保持一致性")

Code audit of each client at a2c46438 (file:line evidence in `research.md`). The iPhone probe
(`DappBrowserStabilityProbeTests`) could not run before the fix (UI Automation was off), so the iOS
column rests on the audit; the probe ran after the fix (T077). ✗ = the defect is there, ✓ = already
right, — = the client has no such surface. The extension hosts no pages, so page-load rows do not apply.

| # | Finding | Android | iOS | Desktop (macOS) | Extension |
|---|---|---|---|---|---|
| F1 | Failed load recorded in Recents | ✗ | ✓ (only on finish; a server 404 page still counts) | ✓ (same caveat) | — |
| F2 | Visit carries the previous page's title/icon | ✗ | ✗ (async icon lookup; `?? title` keeps the old title) | ✗ narrower (URL read later than title) | — |
| F3 | No progress until the engine starts the load | ✗ | ✗ partly (starts at provisional; none inside load/reload; demo page drawn in the gap) | ✗ worse (no indicator at all) | — |
| F4 | Retry exposes the engine's page | ✗ (raw error page) | ✗ (blank or previous page) | — (no failure panel) | — |
| F5 | Failure says no reason / never retries | ✗ | ✗ (system text + error code; no retry) | ✗ worse (no panel, no reason) | — |
| F6 | Chain unreachable: nothing shown | ✗ | ✗ | ✗ | — (Chrome tab) |
| F7 | Consent: no title / Connect not primary | ✗ both | ✗ title / ✓ primary | ✓ both | ✓ both |
| F8 | Visible "安全站点" / green lock | ✗ | ✗ in sheets (grey lock in bar) | ✗ in connection panel | ✓ (none; Chrome shows the URL) |
| F9 | Pickers: no network logo / balance / identicon | ✗ all | ✗ networks / ✓ accounts | ✗ networks (dot always Ethereum-coloured) and "switch account" does nothing | — (no pickers) |
| F10 | Signing surface closes by accident | ✗ swipe, scrim, Back | ✗ swipe | ✓ (no outside click; Esc and ✕ are explicit) | ✗ scrim, Esc, drag |
| F11 | After approval: form + small line, slide greyed | ✗ | ✗ | ✗ (up to 90 s, then a status view) | ✓ transactions (status view) / ✗ message signature just closes |
| F12 | Fee: no refresh, no reason, no auto re-quote | ✗ | ✗ | ✗ ("—", stale flag ignored) | ✗ worse (stale never re-quoted) |
| F13 | Unlanded op: silent, polled up to 24 h | ✗ | ✗ | ✗ | ✗ ("submitted" forever; stops ticking after 120 s) |
| F14 | Header repeats host | ✗ | ✗ | ✗ | ✗ |
| F15 | Tab thumbnails are a drawing | ✗ | ✗ | — (tab strip) | — |
| F16 | Letter "A" for app.uniswap.org; favicon unused | ✗ | ✗ | ✗ | — |
| T1 | Two slides on the trusted-signer route | ✗ | ✗ | ✗ | — (signs in the extension) |
| S1 | Signer page: "未知站点" + "cannot be verified" for every dApp request (core never sends `context.dapp`) | ✗ | ✗ | ✗ | — |
| S2 | Signer page: failure line in jargon; slider knob stuck at the end after a failure | ✗ | ✗ | ✗ | — |
| S3 | Signer page opened unverified (root `sign.html`) and never cached | ✗ | ✗ | ✓ verified / ✗ not cached | — |
| — | Consent's approve word | 连接 | 批准 | `consent_connect` | `connect.browser.connect` |

What held up and must not regress: provider injection and EIP-6963 on every load; `vh`
units correct (679 = innerHeight); consent → connect → `eth_requestAccounts`; the connection
and chain survive a force-stop; navigation mid-sign closes the sheet; a closed signer tab
recovers on Reload; a renderer death does not take the app down (070); message signature
and a real send landed (Gnosis `0x09c35478…4c9f`, the page got the hash).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A signing request is never lost by accident, and the person sees it land (Priority: P1)

A dApp asks for a signature or a transaction. The sheet stays until the person decides:
they approve it, or they close it on purpose. After approving, the sheet stops being a form
and becomes a status: signing, submitting, waiting to land (with the chain's usual time),
landed, or failed with a reason. A transaction that takes unusually long says so, lets the
person close the sheet without cancelling anything, and keeps being followed; one that
never lands ends in a clear state instead of "pending" forever.

**Why this priority**: rulings 1 and 2. An accidental close throws away the dApp's request
(the person must redo the swap from the start). A silent sheet after a fingerprint is the
moment people press twice or give up. Both were hit several times in one hour.

**Independent Test**: Test dApp → Send dust → try every accidental close (swipe down, tap the
scrim, Back, an edge swipe): the sheet stays. ✕ closes and the page gets 4001. Approve → the
status runs submitting → waiting → landed ✓ and the sheet closes itself. With the relay
dropped after the signature, the sheet shows the failure reason and the page gets an error.

**Acceptance Scenarios**:

1. **Given** a signing sheet is open, **When** the person swipes it down, taps outside it, or
   uses the system Back gesture, **Then** the sheet stays open, unchanged, and no answer is sent
   to the page.
2. **Given** a signing sheet is open, **When** the person taps its ✕, **Then** it closes and the
   page gets exactly one rejection (4001).
3. **Given** the person has approved (and signed), **When** the wallet submits, **Then** the sheet
   shows only the request's one-line summary and a status: "signing", then "submitting", then
   "submitted, waiting to land" with progress against the chain's usual time; the slide and the
   fee controls are gone.
4. **Given** the transaction lands, **Then** the sheet shows landed ✓ with the hash's short form
   and a link to the explorer, then closes on its own after a short beat; the page gets the
   transaction hash.
5. **Given** the wait passes the chain's usual time, **Then** the sheet says it is still
   confirming and that the person may close it; closing it then does NOT reject — the wallet
   keeps following the transaction and the page still gets its answer.
6. **Given** the submission fails (relay unreachable, refused), **Then** the sheet shows why in
   plain words and the page gets one error; nothing is recorded as pending.
7. **Given** a transaction the relay accepted has not landed when the wait window ends, **Then** the
   sheet says in plain words that it was handed to the network but has not landed yet, that Vela keeps
   checking, and that it must not be sent again; it never says "failed" (the tracker's money rule: a
   timeout is not a failure — marking it failed invites a double spend). The wallet keeps checking at a
   slowing pace, and past the tracker's 24-hour limit the record shows as "unknown", not pending forever.

---

### User Story 2 — The network fee explains itself and can be refreshed (Priority: P1)

The fee row always offers a refresh. When the fee cannot be quoted, it says why (the service
cannot be reached — check the network) and tries again by itself when the connection comes
back, so the person does not have to guess that "tap to retry" is the only way forward.

**Why this priority**: ruling 3; F12. A sheet whose slide is disabled with no reason reads as
broken.

**Independent Test**: Open Send dust with the relay dropped: the fee row says the service
cannot be reached, the slide stays disabled. Restore the relay without touching the phone:
the fee appears within one retry interval and the slide enables. Tap refresh on a shown fee:
it re-quotes with a spinner.

**Acceptance Scenarios**:

1. **Given** a fee is shown, **When** the person taps the refresh control, **Then** the fee row
   shows it is re-quoting and then the new fee; the slide is disabled only while re-quoting.
2. **Given** the fee cannot be quoted because the service is unreachable, **Then** the row says so
   in words, and the wallet re-asks on its own at a growing interval while the sheet is open.
3. **Given** the service answers again, **Then** the fee appears without any tap and the slide
   enables.

---

### User Story 3 — A page on a bad network never looks frozen or broken (Priority: P1)

Typing an address or tapping Retry shows progress at once. A failed load shows a Vela panel
that says why, stays in place while a retry runs (never the engine's own error page), and
retries by itself a few times. Recents only ever hold pages that loaded, under their own
title and icon.

**Why this priority**: F1–F5. This is the "unstable environment" in its plainest form, and
every dApp session starts with a page load.

**Independent Test**: With the proxy's latency, drop and black-hole modes against one host:
open, fail, retry, recover; read Recents from the store afterwards.

**Acceptance Scenarios**:

1. **Given** the person submits an address (or taps Retry, or a link), **When** the site is slow,
   **Then** progress shows within a fraction of a second and stays until the new page commits or fails;
   the address bar keeps the committed host until then.
2. **Given** the main document fails, **Then** a Vela panel shows the host and a reason in plain
   words matched to the failure: no connection / too slow, site not found, certificate problem.
3. **Given** the failure panel is shown, **When** a retry runs (automatic or tapped), **Then** the panel
   stays, shows that it is retrying, and gives way only to the loaded page; the engine's own error
   page is never visible.
4. **Given** a load failed for a network reason, **Then** the browser retries by itself a few times at a
   growing interval while the page is in front; a certificate failure is never retried automatically.
5. **Given** a load failed, **Then** nothing is added to Recents; **given** a load finished, **Then** the
   Recents row carries that page's own title and icon, never the previous page's.

---

### User Story 4 — When the site's chain cannot be reached, the browser says so (Priority: P2)

When every node of the network the page is on stops answering, a quiet line under the address
bar names the network and offers a retry; it goes away by itself when the network answers
again. Rate limiting stays quiet (the wallet's standing rule).

**Why this priority**: F6. Without it the dApp spins or shows its own wrong error and the
person blames the dApp or the wallet at random.

**Independent Test**: Black-hole every node of Gnosis while the test dApp is on Gnosis → Block
number: the line appears; restore → the line clears on the next answer.

**Acceptance Scenarios**:

1. **Given** the page's chain has just failed on every node, **Then** a one-line notice names the chain
   and offers Retry; the connection chip stays as it is (the connection is fine).
2. **Given** the chain answers again, **Then** the notice disappears without a tap.
3. **Given** the chain is only rate-limited, **Then** no notice is shown.

---

### User Story 5 — The browser's chrome is honest and complete (Priority: P2)

A lock icon, and only an icon, says whether the connection is https (closed lock, neutral) or
not (open lock, warning colour) — never a word that promises the site is safe. The consent
sheet says what is being asked ("连接到 {host}") and makes Connect its primary action. The
network picker shows each network's logo and the person's balance on it; the account picker
shows each account's identicon. The
signing header shows a host once. Site avatars use the site's own icon, with a letter that
skips `app.`/`www.` when there is none.

**Why this priority**: rulings 5–8; F7–F9, F14, F16.

**Independent Test**: Walk address bar → consent → connection panel → site menu → network picker
→ account picker → signing header on an https site and on the http test dApp.

**Acceptance Scenarios**:

1. **Given** an https page, **Then** every browser surface shows a closed lock icon and no text claim
   about safety or encryption; **given** an http page, **Then** an open lock icon in the warning colour.
2. **Given** a site asks to connect, **Then** the sheet's title names the host and asks, and Connect is
   the primary (filled) button.
3. **Given** the network or account picker is open, **Then** every row leads with the network's logo or
   the account's identicon, as elsewhere in the wallet.
4. **Given** the network picker is open, **Then** each network row shows the account's balance on that
   network (in the display currency, as the home screen shows it), from what the wallet already knows —
   the picker opens at once and never waits on the network; a network whose balance is not known yet
   shows no figure rather than a zero.
5. **Given** the site's name is its host, **Then** the signing header shows it once.

---

### User Story 6 — Tabs can be told apart (Priority: P3)

The tab switcher shows a snapshot of each page as it was last seen, not a drawing.

**Why this priority**: F15; useful, not blocking.

**Independent Test**: Two tabs on two sites → the switcher shows two different snapshots.

**Acceptance Scenarios**:

1. **Given** a tab has shown a page, **Then** its card shows that page's last snapshot; a start-page tab
   shows the start page's drawing.

### User Story 7 — One slide, on a signing page a beginner can read, that works offline (Priority: P1)

When the account signs through the trusted signer, the app shows what is being asked and a single
button that goes to the signing page; the person slides once, on that page. The page leads with a
plain summary (what leaves the wallet, to whom, on which network, the fee), names the site the app's
browser verified without calling it unknown, keeps technical detail folded, and says what went wrong
in everyday words with a way to try again. The page opens and signs even when the network is down,
because a signature needs no network.

**Why this priority**: ruling 4 and 9. It is on the path of every signature this phone makes; two
slides, a warning that contradicts the app, and a page that will not load on a bad connection undo
everything User Story 1 fixes.

**Independent Test**: On the Xiaomi (owner's account, trusted signer): Sign and Send dust from the
test dApp — one slide in total; the page shows the site as the app's browser reported it, no
"未知站点"; cancel the passkey prompt → a plain message and the slide works again; with
sign.getvela.app dropped at the proxy after one visit, a message signature still completes.

**Acceptance Scenarios**:

1. **Given** the account signs through the trusted signer, **When** the signing sheet opens, **Then** its
   action is a button (not a slide) that opens the signing page; the page's slide is the only slide.
2. **Given** the request came from the app's own browser, **Then** the page shows that site's host as the
   request's origin with a neutral "from Vela's browser" note, and no "unknown site" warning.
3. **Given** the page opens, **Then** above the fold it shows only: the action in one sentence, amount and
   recipient (or the message), network, fee, account, and the slide; everything else is folded.
4. **Given** the passkey prompt is cancelled, times out, or finds no key, **Then** the page says so in
   everyday words (no "仪式"), the slide returns to its start, and the person can slide again without
   reloading.
5. **Given** the page has been opened once on this device, **When** the network is down, **Then** it still
   opens and completes a message signature; a transaction still signs and the app submits it when the
   network allows.
6. **Given** the page itself cannot be opened at all (first use, no network), **Then** the app — not a
   browser error page — says the signing page could not be reached and offers to try again, and the
   request is still open.

---

### Edge Cases

- A retry that succeeds after the person has typed a new address: the new address wins; the old retry
  is abandoned.
- Automatic retries stop when the page leaves the screen (another tab, leaving Explore, app in the
  background) and resume only on return.
- A certificate error is never auto-retried and never offers "continue anyway" (070's rule stands).
- The person closes the sheet while it is "submitting": the submission is not cancelled; the page still
  gets its answer and the wallet still tracks it (same as scenario 5).
- A second request queued behind the first (070 FR-004) opens only after the first sheet reaches a final
  state or is closed.
- The relay answers the fee but refuses the submission: the failure reason is the refusal's, not
  "unreachable".
- Back while the failure panel or the chain notice is up keeps its existing meaning (back in page
  history, then the start page); only the signing and consent sheets ignore Back.
- A page that sets its title late: the Recents title is the one it had when the load finished, taken
  from the same document as the address and icon.

## Requirements *(mandatory)*

### Functional Requirements

**Signing sheet (US1)**

- **FR-001**: The signing sheet and the connect consent sheet MUST close only through their explicit
  close control or through the request's own completion. Swipe-down, scrim tap and system Back MUST NOT
  close them. (Ruling 1.)
- **FR-002**: Closing before approval MUST send the page exactly one rejection (4001). Closing after
  approval MUST NOT reject or cancel anything.
- **FR-003**: Once the person approves, the sheet MUST replace the form's controls (fee, speed, slide)
  with a status showing, in order: signing, submitting, submitted-waiting (with progress against the
  chain's usual confirmation time), landed (with short hash and explorer link, then closing on its own),
  or failed (with a plain reason). The status MUST reuse the wallet's existing send-receipt visuals, so a
  dApp transaction and a send look the same. (Ruling 2.)
- **FR-004**: When the wait passes the chain's usual time, the sheet MUST say it is still confirming and
  that it may be closed; the wallet MUST keep following the operation after the sheet closes.
- **FR-005**: An operation still not landed after the wait window MUST be described honestly ("handed to
  the network, not landed yet, Vela keeps checking — do not send it again"), MUST NOT be called failed
  (the core tracker's invariant), MUST be checked at an interval that grows with its age instead of
  every few seconds for 24 hours, and past the tracker's abandon age MUST read "unknown" rather than
  pending. The rule lives in the core tracker so every client gets it.
- **FR-006**: A message signature (no chain involved) MUST show signing → signed and close on its own.

**Fee (US2)**

- **FR-007**: The fee row MUST offer a refresh control whenever a fee is shown or has failed; while
  re-quoting it MUST show that it is working. (Ruling 3.)
- **FR-008**: A quote that failed because the service is unreachable MUST say so in words and MUST be
  retried automatically at a growing interval while the sheet is open and not yet approved.

**Page loads (US3)**

- **FR-009**: Progress MUST show from the moment the browser is asked to load (address, Retry, link,
  reload), not from the engine's commit, and MUST end on commit or failure.
- **FR-010**: A main-document failure MUST show the Vela failure panel with a reason chosen from the
  failure's class (connection or timeout / name not found / certificate / other).
- **FR-011**: The failure panel MUST stay visible, in a retrying state, for the whole of a retry; the
  engine's own error page MUST never be visible.
- **FR-012**: Connection-class failures MUST be retried automatically (a few attempts, growing interval)
  while the tab is in front; certificate failures MUST NOT.
- **FR-013**: A visit MUST be recorded only for a load that finished without a main-document failure, and
  its address, title and icon MUST be read from the same document at the same moment.

**Chain reachability (US4)**

- **FR-014**: When the chain of the page in front is in the pool's failed set, the browser MUST show a
  one-line notice naming the chain with Retry; it MUST clear when the chain answers; rate-limited chains
  MUST NOT show it.

**Chrome (US5, US6)**

- **FR-015**: Security state MUST be an icon only: closed lock (neutral) for https, open lock (warning
  colour) for http, on every browser surface (address bar, consent, connection panel, site menu,
  signing header where drawn). No visible text may claim a site is safe, secure or encrypted.
  Screen-reader descriptions state the protocol only. (Ruling 5.)
- **FR-016**: The consent sheet MUST draw its title ("连接到 {host}", existing string) and make Connect the
  primary button.
- **FR-017**: The network picker MUST show each network's logo and the active account's balance on that
  network (display currency, the home screen's figure, never fetched by the picker and never shown as 0
  when unknown); the account picker MUST show each account's identicon. All from the wallet's existing
  sources. (Rulings 6, 7, 8.)
- **FR-018**: The signing header MUST NOT repeat the host when the name equals it.
- **FR-019**: Site avatars MUST use the recorded site icon when there is one; the letter fallback MUST
  skip a leading `app.`, `www.` or `m.`.
- **FR-020**: The tab switcher MUST show each tab's last snapshot of its page.

**Trusted signer (US7)**

- **FR-023**: When the account's signing route is the trusted signer, the signing sheet's confirm action
  MUST be a button that opens the page; there MUST be exactly one slide per signature, on the page.
- **FR-024**: For requests from the app's own browser, the page MUST show the origin the app reported,
  labelled as coming from Vela's browser, without the "unknown site / cannot be verified" warning. Requests
  whose origin the page genuinely cannot place keep today's caution.
- **FR-025**: The page's first screen MUST carry only the summary, network, fee, account and slide; the
  explanation paragraph and technical details MUST be folded. Its words MUST be everyday words.
- **FR-026**: Every failure of the passkey step MUST produce a plain message (WebAuthn reports cancel,
  timeout and "no key here" as one error by design, so the message covers them in everyday words
  rather than guessing which) and MUST reset the slide so it can be used again.
- **FR-027**: The page MUST open and sign with no network after its first successful load, without
  weakening 076's integrity guarantee (what runs is still exactly the published page).
- **FR-028**: When the app cannot get the page to open, the app MUST say so in its own sheet with Retry,
  keeping the request open.

**Guardrails**

- **FR-021**: Nothing in this spec may change which origin, frame, chain or account a request is
  attributed to, or any answer code the core decides (070's invariants).
- **FR-022**: New words MUST exist in all 15 locales and fit the ja + en residency budget (SC-005);
  existing strings are reused wherever one already says it (list in contracts/core-rules.md §4).

### Key Entities

- **Load attempt**: one requested navigation of a tab — started, committed, failed (with failure class),
  retrying (attempt n of N), abandoned.
- **Signing status**: the post-approval life of one request — signing, submitting, submitted-waiting
  (with expected time), still-confirming, landed (hash), failed (reason: only a definitive refusal or
  revert), unknown (past the tracker's abandon age).
- **Chain notice**: the page-in-front's chain is unreachable (from the pool's failed set), or not.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In 20 deliberate accidental-close attempts (swipe down, scrim tap, Back, edge swipe) on
  the signing and consent sheets, 0 close the sheet or answer the page.
- **SC-002**: After a fingerprint, the person sees a status change within 1 second, and every approved
  request shows a named status (submitting, waiting, still confirming, landed, failed) until the person
  closes it or it lands — never a sheet with no status.
- **SC-003**: After Go or Retry on a site with 6 s latency, progress is visible within 0.5 s in 10 of 10
  tries.
- **SC-004**: Across the fault matrix (drop, black hole, latency, cut tunnel — page host, relay, chain
  nodes, signer page), the engine's raw error page is visible in 0 frames of a 1 s-interval capture.
- **SC-005**: After a relay outage ends, the fee appears with no tap within 15 seconds.
- **SC-006**: After 10 failed and 10 successful loads, Recents holds exactly the 10 successful pages,
  each with its own title and icon (read back from the store).
- **SC-007**: No visible "安全站点"/"不安全站点"/"已加密" text anywhere in the browser (UI dump sweep in
  zh and en).
- **SC-008**: One slide per signature on the trusted-signer route, in 10 of 10 signatures (sign, send,
  approve) from the test dApp and Uniswap.
- **SC-009**: With the signing page's host unreachable after a first visit, 3 of 3 message signatures
  complete.
- **SC-010**: No regression: the 070 quickstart rows A1–A18 pass on the Xiaomi; Android unit suite green.

## Decisions left to the owner (outside this spec's reach)

Found in the same pass; each lives outside the Android app, so 079 records it rather than changing it.

- **D1 — The trusted signer page. Moved INTO this spec as User Story 7 (ruling 9).** Evidence:
  `18-signer-page.jpg`, `21-signer-down.jpg`, `23-signer-closed.jpg`.
- **D2 — Arbitrum operations are accepted and never mined.** Two in-band operations (`0xa8336304…`
  2026-09-27 Uniswap, `0xace642c7…` today) sit in the relay's mempool (`blockNumber: null`) with the
  Arbitrum treasury at 0.0397 ETH; the same shape on Gnosis landed in 33 s. Relay repo.
- **D3 — dApp transactions never appear in the wallet's Activity** (core `activity_feed` shows only
  sends and receives, on every client), so a stuck dApp swap is visible nowhere.
- **D4 — Fee quotes of ~0.01 xDAI for a plain Gnosis transfer** (10× the amount sent; the in-band
  overcharge line of work).
- **D5 — "无法模拟这笔交易…除非你完全信任该站点，否则请拒绝" on a zero-value transfer to self** when the
  chain's public node does not offer simulation (Arbitrum): an environment limit shown as a danger.
- **D6 — `accountsChanged` carries the address lower-cased while `eth_requestAccounts` returns it
  checksummed** (core, every client).
- **D7 — History "全部网络" with nothing in it reads "此网络暂无交易".**

## Assumptions

- Every client that has the surface: Android, iOS, macOS desktop, the Chrome extension (signing
  surface only — it hosts no pages), and the trusted signer page (owner, 2026-09-28: fix everywhere,
  keep them consistent). Shared decisions move into vela-core (contracts/core-rules.md); each client
  keeps its drawing. The directory keeps its first name (`079-android-…`) because that is how the
  pass began.
- Deploying the trusted signer page (including its cache headers) to sign.getvela.app is the
  owner's release step; 079 prepares it and verifies it once deployed.
- "The chain's usual time" and the receipt visuals are the send flow's existing ones (issue 199).
- Automatic page retries: 3 attempts at 2 s, 5 s, 10 s. The fee re-quotes at 3 s, 6 s, 12 s, then every 15 s.
  Tunable in planning, visible in results.
- The Back gesture is treated as accidental on the two sheets (ruling 1: "不应该很容易误操作"); the ✕ is
  the explicit close.
- The device pass uses the owner's wallet on the Xiaomi with the owner's fingerprint, and the
  parallel-space keyset where no fingerprint is needed. Money moved only as dust or zero-value
  self-transfers.
- The phone's global proxy is restored to none at the end of every device session.
