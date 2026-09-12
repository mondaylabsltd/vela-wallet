# Feature Specification: Web Port Completion — Finishing What 024–027 Drew But Left Fixture

**Feature Branch**: `028-web-port-completion` (from `main` @ 28d25ae9, after PR #185)

**Created**: 2026-09-04

**Status**: Draft

**Input**: 创始人逐条核对后要求一次收干净：收款二维码是假的、扫码是画的、分享卡没生成逻辑、sweep 没接、manage_tokens 有机器没入口、设置里的显示偏好全是 fixture、抹除本机没接、通讯录没有导入导出、桌面端 send 没接线。

## Why

This is not a new feature. It is the port, finished.

024 through 027 each shipped a coherent slice and each left the same kind of
residue behind: a surface someone had DRAWN, sitting on fixture data, with the
machine or the service that would fill it either already present or waiting in
the Expo tree. Individually every one of those was a defensible "next feature,
not this one". Together they add up to a wallet whose screens promise more than
it does — and the audit that produced this list found one of them is not a
promise at all but a hole a person falls into:

**the receive QR code does not encode the address.** `qr-pattern.ts` says so in
its own header — *"It never encodes data"* — because spec 021 drew the receive
card before anything could fill it. Someone shows that code to a friend and no
money arrives. Nothing else on this list is that sharp, and it is why the list
gets closed rather than carried again.

The rest is the same shape at lower stakes: a scanner that cannot see, a share
card with no card behind it, a "many assets → one address" mode the core
supports and no screen drives, an add-token panel that was drawn and never
constructed, preference rows that do nothing when tapped, an address book that
cannot leave the browser, and a desktop layout that renders the send flow but
was never handed its actions.

**Standing rules this feature inherits, not re-decides**: the core decides and
the shell performs; fixtures stay the gallery's canon and live builders are
their siblings; components stay pure and gain callbacks; words come from the
corpus through the 5-step process; budgets do not move.

**Standing exclusions**: `browser_history` and `dapp_session` stay unwired
(spec 022 and 027's D43 already ruled); the TypeScript twins Rust owns —
`clear-signing.ts`, `approval-guard-editor.ts`, `local-descriptors.ts`,
`siwe.ts` — are not ported; neither are the WalletPair, webview and native
transports, the App Group sync, or `deployer-api.ts`.

**A dependency this feature does NOT contain**: 027 closed with SC-304 unmet —
approving a dApp request does not complete, and the extension it delivered can
connect but cannot sign. That is a defect, not a port gap, and folding it in
here would put two problems in one PR. It should be fixed BEFORE this feature
ships, because a wallet that cannot finish a signature makes everything below
moot. Its evidence is in `specs/027-web-extension-provider/results.md` and in
the `test.fixme` in `e2e/extension-signing.e2e.ts`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A receive code that actually works (Priority: P1)

Someone opens Receive, shows the code, and money arrives. They can also scan
someone else's code with their camera, or pick a screenshot of one, and send to
it. And they can share a card that carries the address in a form both a human
and a phone camera can read.

**Why this priority**: it is the only item on this list that is currently a
trap rather than a gap. A code that looks scannable and is not is worse than no
code at all, because the person has already told their friend to scan it.

**Independent Test**: render the receive code for a known address, decode the
rendered image with an independent decoder, and get that address back.

**Acceptance Scenarios**:

1. **Given** a signed-in wallet, **When** the receive code is shown, **Then**
   decoding the rendered image yields exactly the wallet's address (or the
   payment link, when an amount and asset were chosen).
2. **Given** the code carries a payment request, **When** it is decoded,
   **Then** the amount, asset and chain survive the round trip.
3. **Given** the camera is available, **When** the person opens the scanner and
   a code is in frame, **Then** the address is read and the send form is
   prefilled; **When** permission is refused, **Then** they are told so and
   offered the alternative rather than left with a dead viewfinder.
4. **Given** a screenshot of a code, **When** the person picks it, **Then** it
   is decoded the same way.
5. **Given** the share card, **When** it is produced, **Then** it carries the
   address, a scannable code, and the account's own identicon — the anti-forgery
   mark a doctored card cannot keep consistent.
6. **Given** any of the 15 locales, **Then** every word on these surfaces
   resolves from the corpus.

---

### User Story 2 - Preferences that do what they say (Priority: P1)

The rows in Settings — theme, language, number, date and time format, avatar
style — change what they name, and survive a reload. "Erase this device" erases
this device.

**Why this priority**: a control that does nothing when tapped is a lie told
repeatedly, and these are on the screen a person opens when something is already
not to their liking. The erase row is a data-safety promise made and not kept.

**Independent Test**: change each preference, reload, and see it hold; press
erase, confirm, and find nothing of the wallet left in this browser.

**Acceptance Scenarios**:

1. **Given** any preference row, **When** the person chooses a value, **Then**
   the app changes to match immediately and the choice survives a reload.
2. **Given** a chosen number, date or time format, **Then** every figure and
   timestamp in the app follows it — including money — and the presets are the
   product's own, not whatever the browser would have done.
3. **Given** a chosen avatar style, **Then** every account artwork follows it.
4. **Given** "Erase this device", **When** it is confirmed, **Then** every trace
   of the wallet in this browser is gone and the app returns to first-run; a
   cancelled confirmation changes nothing.
5. **Given** the passkey is what the wallet is founded on, **Then** erasing this
   browser is explained as losing local data, never as losing the wallet.

---

### User Story 3 - Many assets, one address (Priority: P2)

On one network, someone selects several assets at once and sends them all to a
single address — the "sweep" the core already supports and no screen drives.

**Why this priority**: it is a whole capability the wallet claims and cannot
perform, and the machine and the drawn screen both exist. It is not a P1 only
because nobody is misled by its absence.

**Independent Test**: in the parallel space, multi-select two assets on one
chain, send to one address, and see one operation carry both.

**Acceptance Scenarios**:

1. **Given** the picker in sweep mode, **When** several assets are selected,
   **Then** the total, the per-asset amounts and the CTA are all the core's
   ruling, and a master tick selects exactly what is visible.
2. **Given** a sweep draft, **When** it is confirmed, **Then** ONE operation
   carries every transfer, priced and signed once.
3. **Given** one asset cannot cover its own fee, **Then** the core's refusal is
   what the screen shows.

---

### User Story 4 - Add a token the wallet does not know (Priority: P2)

Someone pastes a contract address, sees what it is, and adds it — and it appears
in their assets and in the send picker.

**Why this priority**: the machine (22 tests), its executor, its session and the
drawn panel all exist; only the screen that constructs it is missing. Without
it, a token the registry has not heard of is invisible even when the person
holds it.

**Independent Test**: add a known ERC-20 by address in the parallel space; it
appears in the asset list and can be sent.

**Acceptance Scenarios**:

1. **Given** a contract address, **When** it is entered, **Then** the chain is
   read for the token's own name, symbol and decimals, and what is shown is what
   the chain said — never what the person typed.
2. **Given** a token that is not one, or one already known, **Then** the core's
   verdict is shown rather than a silent failure or a duplicate.
3. **Given** an added token, **Then** it survives a reload and appears wherever
   assets are listed.

---

### User Story 5 - The address book can leave (Priority: P2)

Someone exports their contacts to a file and imports them on another device or
browser, and importing never overwrites what is already there.

**Why this priority**: the book is per-browser, and the extension has its own
(027 D32). Without a way to carry it, every doorway starts empty.

**Independent Test**: export a book with groups, import it into a profile that
already has an overlapping entry, and see the existing entry survive.

**Acceptance Scenarios**:

1. **Given** a book with groups, **When** it is exported, **Then** the file
   carries every entry and its group, and re-importing it changes nothing.
2. **Given** an import that collides with an existing entry, **Then** the
   EXISTING entry wins and the person is told what was skipped.
3. **Given** a malformed or hostile file, **Then** it is refused with a reason,
   and nothing is written.

---

### User Story 6 - The desktop can send too (Priority: P3)

On a wide screen the send flow does what it does on a phone.

**Why this priority**: the desktop layout already renders the same live
overlays; only the actions were never handed to it. Small, and the last thing
anyone would miss.

**Independent Test**: at desktop width, complete a send end to end.

**Acceptance Scenarios**:

1. **Given** a wide viewport, **When** the person opens Send, **Then** every
   step behaves as it does on the phone layout, driven by the same sessions.

---

### Edge Cases

- **A code too dense to scan.** A long payment link may exceed what the drawn
  card can carry legibly; the code must stay readable or say it cannot.
- **No camera, or permission refused** — including the second time, after the
  browser has remembered a refusal.
- **A screenshot with no code in it**, or several.
- **A preference chosen while another surface is open** — the change must not
  strand a half-filled form.
- **Erase pressed with money in flight.**
- **An import file from a much older or newer version.**
- **Sweep where the same asset appears on two chains** — the mode is per network.
- **A token contract that answers slowly, or lies about its decimals.**

## Requirements *(mandatory)*

### Functional Requirements

- **FR-401 (A code is data, not decoration)**: the receive code MUST encode the
  address — or the payment request, when one is set — such that an independent
  decoder reads it back exactly. The decorative pattern MUST be gone from every
  surface a person could mistake for a real code, and MAY remain only where a
  placeholder is what is meant (the gallery's own canon).
- **FR-402 (Reading a code)**: the wallet MUST be able to read a code from the
  camera and from a chosen image, and MUST explain a refusal, an absence of
  camera, or an unreadable image rather than presenting a dead surface.
- **FR-403 (The share card)**: the card MUST carry the address, a scannable
  code and the account's identicon, so a doctored card cannot keep its parts
  consistent.
- **FR-404 (Sweep)**: selecting several assets on one network and sending them
  to one address MUST be possible, decided entirely by the existing core, and
  MUST result in ONE operation.
- **FR-405 (Custom tokens)**: a token MUST be addable by contract address, with
  its identity read from the chain, ruled on by the existing core, durable
  across reloads, and visible everywhere assets are listed.
- **FR-406 (Preferences are real)**: theme, language, number/date/time format
  and avatar style MUST take effect immediately, persist, and apply everywhere
  the app shows a figure, a date or an avatar. Number and date formatting MUST
  follow the product's own explicit presets rather than the platform's default
  behaviour, so the same wallet reads the same way everywhere.
- **FR-407 (Erase means erase)**: erasing MUST remove every trace of the wallet
  from this browser behind a confirmation, return the app to first run, and be
  described as losing local data — never as losing the wallet, which lives in
  the passkey.
- **FR-408 (The book travels)**: contacts MUST export to a file and import from
  one, with an existing entry always winning a collision, groups preserved, and
  a malformed file refused without a partial write.
- **FR-409 (Desktop parity for send)**: the wide layout MUST drive the send flow
  through the same sessions as the phone layout.
- **FR-410 (Nothing regresses)**: the galleries stay pixel-unchanged with
  fixtures untouched; the corpus grows only through the 5-step process; the
  budgets (zero-wasm Welcome, a wasm-free deploy bundle, one core artifact,
  artifact bytes) hold; no business rule is added to web code where a core owns
  it.
- **FR-411 (Say what is true)**: the settings route's own description of itself
  MUST match what it now does — it still claims everything but identity is
  fixture-driven, which stopped being true in 024.

### Key Entities

- **Receive code**: what the address or payment request looks like as an image a
  camera can read.
- **Scan result**: what a camera or a picked image yielded — an address, a
  payment request, or a refusal with a reason.
- **Share card**: the address, its code and the account's identicon, composed.
- **Sweep draft**: several assets on one network, one recipient, one operation.
- **Custom token**: a contract address and the identity the chain reported for
  it. Owned by `manage_tokens`.
- **Preferences**: theme, language, number/date/time format, avatar style —
  shell-side state, with no core to rule on them.
- **Address book file**: the contacts and their groups, in a form another
  browser can read.

## Success Criteria *(mandatory)*

- **SC-401**: the rendered receive code, decoded by an independent decoder,
  returns exactly the wallet's address; with a payment request set, the amount,
  asset and chain survive the round trip. Asserted, not eyeballed.
- **SC-402**: a code is read from a chosen image end to end, and a refused or
  absent camera produces a stated reason rather than a dead viewfinder.
- **SC-403**: the share card carries address, code and identicon together.
- **SC-404**: a sweep of two assets on one network produces ONE operation
  carrying both transfers.
- **SC-405**: a token added by contract address shows the chain's own name,
  symbol and decimals, survives a reload, and can be sent.
- **SC-406**: each preference takes effect and survives a reload; a chosen
  number and date format is visible in the app's own figures and timestamps.
- **SC-407**: erasing leaves nothing of the wallet in the browser and returns to
  first run; cancelling changes nothing.
- **SC-408**: an export re-imported changes nothing; an import colliding with an
  existing entry leaves that entry untouched and reports what it skipped.
- **SC-409**: a send completes at desktop width.
- **SC-410**: budgets identical, corpus gates green, galleries pixel-unchanged,
  and the whole e2e suite green on three engines.

## Assumptions

- **The Expo tree is the porting truth**, as in 024–027, with provenance
  headers: `qrcode.ts` (554 lines, dependency-free), `image-decode.ts` (74),
  `share-card.ts` (330), `locale-format.ts` (331), `avatar-style.ts` (54),
  `contact-io.ts` (341), `saved-contact.ts` (22), `erase-device.ts` (160), and
  `readonly-rpc-gate.ts` (109), `fiat-convert.ts` (231),
  `currency-catalog.ts` (158), `token-list-filter.ts` (98) as their consumers
  need them.
- **The surfaces are already drawn.** `AddTokenModel` / `AddTokenTab` /
  `AddTokenResult`, the sweep picker's multi-select, `ShareCardModel`,
  `ScanSurface`, and every preference row exist as fixtures. This feature fills
  them; it does not draw.
- **The machines are already aboard.** `manage_tokens` (22 Rust tests) and
  `send`'s sweep fields are in the shipped artifact, so wiring them costs zero
  bytes.
- **Preferences have no core**, and none is added: theme, language, formats and
  avatar style are shell state, persisted like the other small preferences.
  Currency already has its machine and is not touched.
- **The camera is a browser capability, not a core one**, and its absence is a
  normal state on desktop rather than an error.
- **Verification is 024–027's**: the parallel space as the harness, Playwright
  across three engines for anything that persists, and the corpus's own words
  in every assertion.
- **027's SC-304 is fixed before this ships.** It is not in this feature's
  scope and not in its tasks; it is a stated precondition.
