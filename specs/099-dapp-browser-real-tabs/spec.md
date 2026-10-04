# Feature Specification: The dApp browser — real tabs, and layers you can see

**Feature Branch**: `099-dapp-browser-real-tabs`
**Created**: 2026-10-04
**Status**: Draft
**Input**: Owner, 2026-10-03/04:

> 「dapp browser 没有维持真正的多标签页，现在的体验很差，切换标签页时，会重新刷新，这不对吧」
>
> 「并且 dapp browser 要做清晰一点：1. 首先它是一个 browser 对吧，支持多标签页 2. 其次它被 inject vela
> wallet eip6963 eip1193 provider 3. 签名 UI 4. 签名器 5. 客户端请求 6. 网络请求 等等，要清晰，好定位问题，
> 以及 UI 提示友好，便于排查问题，最重要的是，在不稳定的环境构建持续的稳定的用户体验，性能要好，程序要健壮，
> 资源占用尽量少。」

## Where it stands (measured 2026-10-04)

- **The desktop has ONE system webview for every tab** (`app-desktop/.../webview.rs:366-389`, "one
  system webview"). Selecting a tab re-navigates it to that tab's saved URL
  (`page.rs:12754-12777` → `webview::navigate` → `load_url`), so **the page reloads**: form input,
  scroll, a connected session, an open WebSocket are gone, and the core retires the old document —
  its open requests settle 4900 "The page navigated away" (`dapp_browser.rs:984-1029`).
  Spec 082 knew (W14) and chose the minimum: a *hold* while a request is open, a *veil*, a
  per-tab Back floor; "one webview per desktop tab" was left as a follow-up (082 plan.md:295).
- **iOS and Android already keep one live webview per tab** (iOS `BrowserController.engines`,
  Android `engines` with `onPause` for background tabs); neither evicts under memory pressure.
- **The core is already per tab**: `dapp_browser` keys tabs and documents by id
  (`dapp_browser.rs:367-391`); the desktop uses only the one id `"browser"`
  (`browser_host.rs:41-48`).
- **What the person can see of the layers today**: a loading hairline, a lock, a connection dot,
  a chain-unreachable notice, load-failure / engine-failure / crash panels. There is **no
  per-request record** of what a page asked and how it ended (method, layer, outcome, time), on
  screen or in the log; the core's per-tab view carries no request status
  (`DbrTabView`, `dapp_browser.rs:465-476`); the in-page provider has no deadline of its own.
- Found on the same device pass (2026-10-04), in the layers this spec names:
  - after a failed submit the signing sheet's slider stayed locked because the fee had gone stale
    — nothing said "refresh the fee";
  - the landing screen asked the relay for the op's status only every 12 s, and ran the chain's
    "confirms in ~4 s" countdown before the relay had sent anything, so it read "taking longer
    than usual" within seconds;
  - an operation the relay accepted and later forgot (its record expired) stays "pending" for
    24 h, because the tracker cannot tell "forgotten" from "still on its way".

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Tabs are real (Priority: P1)

A person has Uniswap open in one tab with a half-entered swap and Aave in another. They switch
between them as often as they like; each page is exactly as they left it — no reload, inputs and
scroll kept, the site still connected. A signature request raised by the tab they are not looking
at is not lost: the signing sheet says which site and tab asked.

**Why this priority**: it is the owner's first complaint, and a reload on switch is data loss
(form state, sessions) and a cancelled request each time.

**Independent Test**: open two dApps, type into one, switch back and forth twenty times; the
typed value is still there and the page's navigation count is unchanged. Raise a signature in
tab A, switch to tab B, sign; tab A receives the result.

**Acceptance Scenarios**:

1. **Given** two open tabs with live pages, **When** the person switches tabs, **Then** neither
   page reloads, and the shown page appears within 100 ms with no network activity caused by the
   switch.
2. **Given** a signing request open from tab A, **When** the person switches to tab B, **Then** the
   switch happens (no hold), the request stays open, and the sheet names tab A's site.
3. **Given** a request open from tab A, **When** the person closes tab A, **Then** the request ends
   as the page leaving ("navigated away"), exactly as today.
4. **Given** many tabs open, **When** the wallet has to free memory, **Then** only background tabs
   with nothing open are suspended, and a suspended tab says it was reloaded to save memory when
   it comes back — never silently.

---

### User Story 2 — Each layer says what it is doing (Priority: P1)

When something does not work, the person (and the developer they report to) can see **which
layer** stopped: the page did not load (browser), the wallet was not offered to the page
(provider injection), the wallet refused a request (client request), the chain's RPC did not
answer (network request), the relay could not quote, simulate or send, or the passkey ceremony
failed (signer). The words are the person's, the details are one tap away, and a bug report
carries them.

**Why this priority**: the owner's second ask — "要清晰，好定位问题" — and what took hours this
week: an Arbitrum swap that read "无法连接 Vela 服务" when the network was fine.

**Independent Test**: inject one fault per layer (offline page, insecure page, unsupported method,
blackholed chain RPC, relay refusal, cancelled passkey). Each shows a message naming that layer
and an action, and the tab's request record lists the request with that layer and outcome.

**Acceptance Scenarios**:

1. **Given** a tab, **When** the person opens its status, **Then** they see the page state
   (loading / loaded / failed), the wallet state (offered to the page or not, connected account,
   chain), and the latest requests with method, outcome and duration.
2. **Given** any page request, **When** it ends, **Then** one log line records tab, request id,
   method, layer, outcome and duration — no parameters, signatures or keys.
3. **Given** a failed request, **When** the person sees the message, **Then** it names the layer
   in plain words ("this network's RPC didn't answer", "Vela's relay couldn't simulate this",
   "the passkey prompt was cancelled") with the action that helps, never a generic "error".
4. **Given** a bug report sent from the browser, **Then** it includes the tab's recent request
   record and states (redacted as above).

---

### User Story 3 — Steady in an unsteady environment, and light (Priority: P2)

On a bad network, with an RPC that times out, a relay that is slow, or a page that misbehaves, the
browser keeps working: every request ends with an answer, one tab's crash or hang does not touch
the others, and the wallet uses no more memory than the tabs actually need.

**Why this priority**: the owner's "最重要的是" — continuous stable experience, good performance,
robustness, low resource use.

**Independent Test**: with an RPC endpoint blackholed and the network toggled off and on, ten
pages issuing reads and one signing flow all settle; killing one tab's web content process
leaves other tabs working; with ten tabs open, live webviews never exceed the cap.

**Acceptance Scenarios**:

1. **Given** a chain RPC that never answers, **When** a page reads, **Then** the read settles with
   an error naming the network within the read deadline.
2. **Given** a tab whose web content process dies, **Then** that tab shows its crash panel and
   every other tab keeps working.
3. **Given** the network drops and returns, **Then** each tab recovers on its own (its load watch,
   its chain notice) without a wallet restart.
4. **Given** more tabs than the live cap, **Then** the least recently used background tabs with
   nothing open are suspended, and memory falls accordingly.

---

### User Story 4 — The signing sheet and the landing say what they wait for (Priority: P2)

The sheet never has a dead control without a reason, and the landing screen's progress follows
what is actually happening — the relay's queue, its funding, the network — without stalls that
look like hangs.

**Why this priority**: found on the 2026-10-04 device pass; it is the "签名 UI" layer of the ask.

**Independent Test**: let a fee go stale after a failed submit — the sheet says to refresh and
the refresh unlocks it; submit on a chain whose relay is funding — the landing says so within a
few seconds and starts the chain's countdown only once the relay has sent.

**Acceptance Scenarios**:

1. **Given** a sheet whose confirm is disabled, **Then** it states why and offers the unlocking
   action (e.g. refresh a stale fee).
2. **Given** a submitted operation, **Then** the relay's status is first asked within ~3 s, and the
   chain's typical-time countdown starts only when the relay reports the bundle sent.
3. **Given** an operation the relay accepted and later answers `not_found` for, with no receipt
   found on-chain, **Then** it ends as "not sent" within a bounded time instead of 24 h pending.

### Edge Cases

- A page in a background tab calls `window.open` / `target=_blank`: a new tab opens (082/083
  behaviour), it does not navigate the visible tab.
- Two tabs on the same origin: grants and `accountsChanged`/`chainChanged` reach both documents
  (core `emit_to_origin`), each tab keeps its own document and requests.
- A background tab raises a request while another request's sheet is open: it queues per the
  core's one-active-job rule; the queued count is visible.
- A suspended tab had an open request: it is never suspended (FR-004).
- Debug mode toggled: today the single view is rebuilt; with per-tab views every live view is
  rebuilt at its URL, saying so.
- Windows (WebView2) and macOS (WKWebView): both get per-tab views; Linux has no in-app browser.
- The person quits with requests open: unchanged (the quit guard for in-flight submits).

## Requirements *(mandatory)*

### Functional Requirements

**Browser (tabs)**

- **FR-001**: On desktop, each open tab owns its own system webview, created when the tab is
  first shown and kept alive across switches. Selecting a tab shows its view and hides the
  others; it never re-navigates.
- **FR-002**: Each desktop tab uses its own core tab id (the explore tab id) for routing, delivery,
  events and per-tab state; `"browser"` as the single id goes away.
- **FR-003**: A switch is no longer held while a request is open (082 RD1's hold existed only
  because a switch reloaded). The signing sheet names the requesting tab's site; closing the
  requesting tab still ends its requests as "navigated away".
- **FR-004**: At most `LIVE_TABS_CAP` webviews are live (default 6). Beyond it the least recently
  used background tab with no open request, consent or signing job is suspended (view destroyed,
  URL and title kept); bringing it back reloads it and says "reloaded to save memory". Tabs with
  anything open are never suspended.
- **FR-005**: Load watch, chain-unreachable notice, crash/engine-failure panels and the Back floor
  are per tab (today they assume one document).

**Provider injection**

- **FR-006**: Each tab reports whether the wallet was offered to its current document (provider
  script ran in a secure top frame) and why not when it was not (insecure context, sub-frame,
  injection failed), visible in the tab's status.

**Client requests**

- **FR-007**: Every page request is recorded per tab — id, method, class (answered locally /
  network read / relay / signing), start, end, outcome (result or EIP-1193 error code), and the
  layer that decided it — in a bounded ring (last 200 per tab), with no parameters, signatures or
  keys. The core owns the record and its vocabulary; shells render it.
- **FR-008**: Every read settles within `READ_DEADLINE` (30 s) with an error that names the
  network layer if no endpoint answered; signing requests wait for the person, never a timer.

**Network requests**

- **FR-009**: A read's failure names which: no endpoint answered, rate-limited, endpoint error
  passed through — reflected in the request record and in the chain notice.

**Signing UI**

- **FR-010**: A disabled confirm always shows its reason and its unlocking action (stale fee →
  refresh; insufficient balance; request still being simulated).
- **FR-011**: The landing asks the relay's status within ~3 s of acceptance (then at the tracker's
  pace) and starts the chain's typical-time countdown only after the relay reports the bundle sent;
  before that it says what the relay is doing (queued / funding / sending).
- **FR-012**: An accepted operation that the relay later reports `not_found` and that has no
  receipt or on-chain event ends as "not sent" within a bounded time (proposed: 10 min after the
  first `not_found` past acceptance), with the same "send it again is safe" words as RA4.

**Signer**

- **FR-013**: A signer failure (prompt cancelled, platform authenticator unavailable — e.g. an
  unsigned build —, timeout, security key error) is classified and named in the sheet and the
  request record; the log line names the class.

**Diagnostics surface**

- **FR-014**: Each tab has a status entry in the browser chrome (page / wallet / last request
  issue) that opens a panel with the tab's request record and states, copyable, and included in
  a bug report from the browser.
- **FR-015**: One structured log line per request end and per tab state change (`dapp` area,
  tab id, request id), host-only URLs.

**Resources**

- **FR-016**: Background tabs keep running (dApps hold sockets) but do not render; a suspended tab
  holds only its URL, title and snapshot. Measured: idle CPU with ten tabs open ≈ one tab's.

**Phones**

- **FR-017**: iOS and Android keep their per-tab webviews; they take the core's request record,
  layer vocabulary and FR-010–FR-013 so every client says the same thing. Their memory eviction
  follows FR-004's rule.

### Key Entities

- **Tab**: id, URL, title, live/suspended, current document id, load state, provider-offered
  state, last-used time.
- **Request record**: tab, document, request id, method, class, layer, started, ended, outcome.
- **Layer**: browser · provider · client request · network request · relay · signer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Switching between two loaded dApp tabs 20 times causes zero reloads (the pages'
  navigation entries unchanged; a typed form value survives) and shows each page in < 100 ms.
- **SC-002**: A signature raised in tab A survives switching to tab B and back and completes.
- **SC-003**: With 10 tabs open, at most `LIVE_TABS_CAP` webviews are live, and app memory with
  10 tabs stays within 1.5× of memory with 6 (baseline measured before the change).
- **SC-004**: With a blackholed chain RPC, every page read settles within 30 s with a network-layer
  error.
- **SC-005**: For each layer's injected fault, the person sees a message naming that layer and an
  action, and the tab's record shows the request with that layer — one test per layer.
- **SC-006**: Killing one tab's web content process leaves other tabs answering requests.
- **SC-007**: On a relay-funding submit, the landing names the relay's state within 5 s of
  acceptance and never shows "taking longer than usual" before the relay reports the bundle sent.

## Assumptions

- wry/gpui support several child webviews in one window on macOS (WKWebView) and Windows
  (WebView2); hidden views keep running. To be confirmed in research (Phase 0).
- `LIVE_TABS_CAP = 6` and `READ_DEADLINE = 30 s` are starting values, revisited with measurements.
- Desktop is first (the owner's machine, the reload bug); phone work is FR-017's shared core plus
  the four signing/landing fixes.
- The request record is for diagnosis, kept in memory and in bug reports only — not persisted,
  not sent anywhere else.
- The tab strip's 24-tab cap (`TABS_CAP`) stays.
