# Feature Specification: iOS dApp Browser and Signing — the Explore Tab Becomes a Browser

**Feature Branch**: `053-ios-dapp-browser-signing` (stacked on `052-ios-money-wiring`)

**Created**: 2026-09-14

**Status**: Draft

**Input**: Founder description: "desktop 做了这么多 speckit 来实现功能，我们的 iOS 才
两刀。继续用 speckit 方式一直做到 057 来完成所有功能实现，可以参考 app-web /
app-desktop / app-android，以及考虑当前进展。"

Program context: the table in [052's spec](../052-ios-money-wiring/spec.md#program--where-052-sits);
`docs/ios/wiring-program-052-057.md` is written in 056.

## Why

After 052 the iPhone can move its own money. It still cannot let anybody
*else* ask it to. The 探索 tab renders `DemoPageModel` — a drawn picture of a
web page — and the signing sheet renders `SigningFixtures`. Between them sits
the entire reason a wallet is on a phone at all: **a dApp the person did not
write, asking for something the person has to understand before they agree.**

Six machines are exported, in the committed Swift bindings, and referenced by
**zero** Swift:

| | |
|---|---|
| `explore_sites.rs` | 733 lines: favourites, groups, the tab strip, the two caps |
| `browser_history.rs` | 471 lines: visits, the title/favicon merge rule, clear |
| `dapp_permissions.rs` | 1,341 lines: per-origin grants, the consent queue, the page events, the popup verdict |
| `sign_request.rs` | 2,096 lines: arrival, the funding pre-check, sponsorship, sign-and-submit, the record |
| `clear_signing.rs` | 5,038 lines: what a transaction DOES, in words, with six degradation rungs |
| `approval_guard.rs` | 2,436 lines: an unlimited approval never leaves this wallet |

**12,115 lines of decided behaviour, and nothing on iOS asks any of it a
question.** This cut adds no Rust: like 052 it is executors, live builders,
and — the part that is new — one platform engine.

**What is genuinely new here, and why it is the risky part.** Every cut so far
has been "the shell performs what the core decided". This one adds a surface
the wallet does not author: a `WKWebView` running somebody else's JavaScript.
Three rules fall out of that and are stated as requirements rather than left
to care:

1. **The origin is never something the page said.** It is read natively from
   the web view on the main thread at the moment of the request and passed
   through `dappOriginOf`. A page that can name its own origin can sign as
   any site (FR-009).
2. **Routing is an allowlist.** A method outside the list is refused, never
   forwarded. A denylist fails open, and `eth_signTransaction` is exactly the
   method it fails open on (FR-011).
3. **Displayed is signed.** The calldata shown, the SafeOp hash the core
   computed and the WebAuthn challenge are the same bytes, checked by a test
   that would fail if any layer re-derived one of them (FR-024).

**The provider script is not ported.** `extension/inpage.js` and
`extension/lib/protocol.js` are copied into the app bundle at build time from
the web tree, byte for byte, with a test that fails when they drift. A second
hand-written EIP-1193 provider is a second set of answers to the same
questions, and the questions are about somebody's money.

## Program — where 053 sits

| Cut | Mirrors | Machines |
|---|---|---|
| 052 ✅ | Android 043, web 026, desktop 032 ph1–6 | `send`, `fee_policy`, `tx_tracker` |
| **053 (this)** | Android 044, web 027, desktop 032 ph13–33 | `explore_sites`, `browser_history`, `dapp_permissions`, `sign_request`, `clear_signing`, `approval_guard` |
| 054 | Android 045, desktop 033+034 | `send` split/sweep, `batch_import`, `contacts`+`contacts_io` |
| 055 | Android 046, desktop 035+036+037 | `token_trust::SimDeltasComputed`, `clear_signing` message rungs, the scanner |
| 056 | Android 047, web 028, 038 | `payment_request`, the preferences with no machine, the two rulers |
| 057 | Android 048+049 | no new machines — the founder pass, the audits, the send proof |

## Scope

**In**: the 探索 tab becomes a real browser with memory; a dApp connects with
the person's consent; reads are proxied through the wallet's own pool; the
chain switches; a transaction is signed, submitted and answered with a real
hash; an unlimited approval is stopped and edited; `personal_sign` produces an
EIP-1271 signature the page can verify on-chain.

**Out of scope, and named so nobody has to guess**:

- **The camera scanner and the simulation block.** Spec 055.
  `clear_signing`'s `balances` section renders what the core gives it with no
  simulation behind it; `token_trust::SimDeltasComputed` is not dispatched.
- **`eth_sign` message depth and SIWE phishing wording.** The rung exists in
  the core and is reached; the *depth* (`MessagePresented` variants, the
  phishing panel's full copy) is 055. Here `eth_sign` is **refused** — the
  same answer the extension, the desktop and Android give.
- **Split and sweep, the payroll importer, contacts I/O.** Spec 054.
- **`dapp_session` (WalletPair).** Founder ruling 2026-09-08: the in-app
  browser's injection is the answer; it is not a debt.
- **`ext_cache`.** Safari-extension only, and the Safari extension is its own
  spec (founder ruling 2026-09-14).
- **The web popup entry** (`dapp_permissions::PopupRequest`). It is a
  browser-extension window shape; iOS has no such window. The arm stays
  undispatched and the reason is recorded, not hidden.
- **Any machine change under `rust/crates/vela-core/src/app/`.** Zero lines.
  Zero `vela-core-uniffi` changes: every export this needs is already in the
  committed bindings.
- **No corpus regeneration.** Specs 021 and 023 drew these screens with their
  text.

## User Scenarios & Testing

### User Story 1 — A real page, with the wallet inside it (Priority: P1, enabling)

A person opens 探索, types an address, and a real web page loads. The page's own
scripts find `window.ethereum` before they run, and a dApp that uses the modern
discovery event finds Vela by name.

**Why first**: nothing else in this cut can be observed until a page can run.
The provider announcing itself is also the cheapest possible proof that the
injection world, the injection time and the bundled bytes are all right.

**Acceptance**
1. A page loads over the network and the address bar shows its host, with the
   padlock reflecting the scheme.
2. The page reports the discovery announcement carrying Vela's name, and finds
   the legacy global.
3. The bundled provider bytes equal the web tree's.
4. Requests from a subframe never arrive.

### User Story 2 — The browser remembers (Priority: P1)

Favourites, groups, tabs and recents survive the app being killed. The start
page is the person's own, not a catalogue.

**Acceptance**
1. Pinning a site puts it on the start page and it is still there after a
   force-quit.
2. A group can be made, renamed, hidden and deleted; deleting it leaves its
   sites pinned.
3. Tabs open, switch and close; the strip is never left empty and never
   without a selection.
4. A visit appears under 最近; 清除 removes the stored key rather than writing
   an empty list.
5. Nothing is recorded before the history store has answered.

### User Story 3 — A dApp connects (Priority: P1)

A site asks for an account. The person sees which origin is asking and which
account would be given, and decides. A site already granted is answered without
a sheet.

**Acceptance**
1. `eth_requestAccounts` opens the consent surface, which leads with the
   origin as a **fact** and the site's name and icon as **claims**.
2. Approving returns that address; the connected chip appears.
3. A second `eth_accounts` from the same origin is answered from the grant
   mirror with no sheet.
4. `wallet_switchEthereumChain` to a known chain moves the wallet and the page
   hears `chainChanged`; to an unknown chain it answers 4902; to an
   unparseable parameter, -32602.
5. `eth_blockNumber` is answered by the wallet's own pool for the browser's
   chain.
6. 断开连接 emits `accountsChanged []` and `disconnect`, and the next request
   asks again.
7. A window closed without an answer settles as **4900 unknown-pending**.

### User Story 4 — A dApp asks for a transaction, and the person can read it (Priority: P1)

The sheet says what the transaction does in words before it says what it is in
hex. The person slides to confirm, signs once, and the page gets a hash.

**Acceptance**
1. The sheet names the origin, the network and the account, and describes the
   call in the core's words — not a decoded-hex dump with a friendly header.
2. The technical details are present, collapsed, and never removed.
3. One passkey prompt. A dismissal is a rejection and the page hears 4001.
4. At submit the row is written **pending**; when the receipt lands it is
   patched to confirmed.
5. The page is answered with a **transaction hash**, not a user-operation hash,
   whenever the receipt arrives inside the wait.
6. A receipt poll for a user-op hash this wallet minted is translated.

### User Story 5 — An unlimited approval never leaves (Priority: P1)

A site asks to spend everything, forever. The wallet does not let that happen,
and does not make the person read hex to find out.

**Acceptance**
1. An approval with no ceiling is shown as the danger it is; the "as requested"
   chip is **disabled**, not merely unselected.
2. The person sets a finite cap — their own figure, the balance, or revoke.
3. **The calldata that is signed carries the edited cap.** Displayed equals
   signed.
4. An `increaseAllowance` shows what the resulting total would be, and says so
   even when the current allowance could not be read.

### User Story 6 — A message signature the page can verify (Priority: P2)

A sign-in message is answered, and the signature verifies on-chain for the
Safe's own address.

**Acceptance**
1. `personal_sign` shows the readable text, or an honest binary preview.
2. The answer is the EIP-1271 envelope; `isValidSignature` through the
   wallet's own read proxy answers `0x1626ba7e`.
3. `eth_sign` is refused — the same answer the extension, the desktop and
   Android give.

### Edge Cases

- **A page that asks before anything is ready.** The provider's warm-up
  `eth_chainId` / `eth_accounts` arrive at document start, before any
  navigation callback. They must carry the real origin, not an empty one.
- **A page that posts a megabyte.** Bounded at 256 KiB before the core sees it.
- **A second request while one is open.** Refused `-32002`, not queued.
- **A fee quote that expires while the person reads.** Re-quoted under an open,
  idle sheet rather than shutting the slide.
- **A tab closed while its request is in flight.** The answer is routed by the
  request's own owner, and a torn-down document's answers are dropped by
  attempt.
- **The app leaves 探索 with a page running.** The engine survives; its
  presentation does not.
- **A grant made for an address that is no longer the active one.** The core
  refuses with `stale_authorized_address`; the shell does not guess.

## Requirements

- **FR-001 (The bridge)** The six machines MUST be driven from the shell as the
  seventeen before them, each with a wire family that passes the drift gate:
  views a subset, operations and results exhaustive.
- **FR-002 (One provider script)** The page-side provider MUST be the bytes the
  extension ships and the desktop and Android inject, in place before the
  page's own scripts run, with a test that fails on drift.
- **FR-003 (One request channel)** Each request MUST be bound to the tab and
  origin that made it; identifiers are single-use; an answer reaches only the
  asking page. Events reach only the page in front of the person.
- **FR-004 (Who answers)** Routing MUST be the allowlist the other three
  clients use, checked against the shared script by a test. A method outside it
  is refused, never forwarded.
- **FR-005 (Reads through the person's endpoints)** Chain reads MUST go through
  this wallet's own pool for that chain, with its bans and cooldowns — never an
  endpoint the page named.
- **FR-006 (Permissions are the core's)** Grants, when to ask, and what a
  revoke undoes are the permissions machine's. The consent surface leads with
  the origin as the fact.
- **FR-007 (The instant answers)** An already-granted origin's account and
  chain MUST come from the grant mirror without opening a sheet.
- **FR-008 (Events to the page)** `accountsChanged`, `chainChanged` and
  `disconnect` MUST be emitted when — and only when — the core says they
  happened.
- **FR-009 (Signing is 052's spine)** A dApp transaction MUST be signed and
  submitted by `UserOpSpine`, the same pipeline a person's own transfer runs.
  **No second signing path.**
- **FR-010 (Unlimited never leaves)** An approval without a ceiling MUST be
  blocked as asked and may leave only bounded by the person's own figure.
- **FR-011 (The allowlist fails closed)** An unknown method answers 4900. A
  denylist is forbidden: `eth_signTransaction` is exactly the method it fails
  open on.
- **FR-012 (The browser's memory)** Favourites, groups, tabs and recents MUST
  persist in the store shapes the other clients read, and survive process
  death.
- **FR-013 (The page lives, and stays inside 探索)** The engine survives a
  sheet and a brief absence; nothing of a page is painted on any other surface.
- **FR-014 (No Swift judgement)** No shell code decides what the core decides:
  whether an origin may be told something, what a request means, how dangerous
  it is, an approval's ceiling, or what a refusal says.
- **FR-015 (Words from the corpus)** Every word on the consent, connection and
  signing surfaces resolves from the core's corpus. Zero corpus change.
- **FR-016 (The origin is never the page's claim)** `origin` MUST be read
  natively from the web view at the moment of the request and passed through
  `dappOriginOf`; `is_main_frame` MUST be a property of the injection, not a
  field in the envelope.
- **FR-017 (Displayed is signed)** The calldata shown, the SafeOp hash the core
  computed and the WebAuthn challenge MUST be provably the same bytes.
- **FR-018 (Verified on the device)** Every phase is verified on the connected
  iPhone, driven through the parallel space; results.md says "device-verified"
  or "test-only" for every criterion, and a step that needs a finger says so.

## Key Entities

- **Engine** — one `WKWebView` and its navigation state for one tab. Owned by
  the controller, not by a view.
- **Envelope** — `{ ch, dir, id, method, params }` in, `{ dir, id, result |
  error }` or `{ dir, event, data }` out. See
  [contracts/page-envelope.md](./contracts/page-envelope.md).
- **Grant** — `vela.perm.<origin>`: origin, address, chain, granted-at. Pinned
  to the address it was made for.
- **Route** — which of six answers a method gets. The allowlist.
- **Signing controller** — `sign_request` + `clear_signing` + `approval_guard`
  + a fee session, born with a request and dying with its sheet.

## Success Criteria

- **SC-001** On the iPhone, the 探索 tab loads a real page over the network
  and the address bar shows that page's host.
- **SC-002** A dApp calls `eth_requestAccounts`, the consent sheet names the
  origin and the account, approving returns that address and the connected
  chip appears.
- **SC-003** `eth_blockNumber` from the page is answered by the wallet's own
  RPC pool — proved by a chain whose endpoint only this wallet knows.
- **SC-004** `wallet_switchEthereumChain` moves the wallet and the page hears
  `chainChanged`.
- **SC-005** A dApp transaction is signed and lands on Gnosis; the page
  receives a **transaction hash**, not a user-operation hash.
- **SC-006** An `approve(spender, MAX)` is stopped: the sheet shows the
  unlimited warning and the person sets a finite cap, and **the calldata that
  is signed carries the edited cap**.
- **SC-007** `personal_sign` returns a signature that the page verifies with
  `isValidSignature` through the wallet's own read proxy, answering
  `0x1626ba7e`.
- **SC-008** Disconnecting emits `accountsChanged []` to the page, and the
  next request re-asks for consent.
- **SC-009** A request whose window is dismissed without an answer is settled
  as **4900 unknown-pending**, never 4001 — the web 027 rule.
- **SC-010** The browser remembers: a favourite survives relaunch, a visit
  appears in history, and 清除 removes the stored key.
- **SC-011** The bundled provider scripts are byte-identical to the web tree's.
- **SC-012** Displayed equals signed: one test proves the calldata on screen,
  the SafeOp hash and the WebAuthn challenge agree.
- **SC-013** `eth_sign` and any method outside the allowlist are refused
  without reaching a node.
- **SC-014** No core, corpus, bindings or sibling-client change.
