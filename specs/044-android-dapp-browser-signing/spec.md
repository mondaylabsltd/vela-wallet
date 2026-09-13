# Feature Specification: Android dApp Browser and Signing — The Wallet Answers a Page

**Feature Branch**: `044-android-dapp-browser-signing` (stacked on `043-android-money-wiring`)

**Created**: 2026-09-12

**Status**: Draft

**Input**: User description: "Android dApp Browser and Signing — a signed-in
person opens a dApp inside the app's 探索 tab, the dApp finds the wallet,
connects to the signed-in account with the person's consent, reads the chain
through the person's own endpoint pool, switches chain, asks for a signature,
and the person sees what they sign before it goes out through 043's passkey →
relay → tracker spine. Mirrors web 027 and desktop 032 phases 13–14, with
027's unmet SC-304 ('connects but cannot sign') as the failure this spec
exists to not repeat. Every phase is verified on the connected Xiaomi."

## Why

After 043 a person on Android can move their own money. They still cannot
use it anywhere: the 探索 tab is a drawing (E2 with the signed-in identity
painted over it, a CS12 signing sheet that signs nothing), and every dApp
that could ask the wallet for something has no way to reach it.

Everything that decides is already in the core, and none of it is reachable
from the phone. Six machines cover the whole journey — what an origin may
be told and when it must ask, including the instant answers an
already-connected page expects (`dapp_permissions`), the browser's own
memory of favourites, groups and tabs (`explore_sites`) and of recent sites
(`browser_history`), the request lifecycle from arrival to answer
(`sign_request`), what a request means and how dangerous it is
(`clear_signing`, the largest machine in the core), and the rule that an
unlimited approval never leaves the wallet (`approval_guard`). Sixteen
machines cross the Android bridge today; these six do not. (Two more
machines in the same family belong to other doorways and stay out: the
extension's service-worker cache and the paired-session machine — the
desktop's in-app browser composes exactly these six.) The words a page sees are shared too: the provider script the
extension ships and the desktop injects into its system webview is one file,
and the desktop's table of who answers which request was ported from the
same script and is tested against it.

**Why SC-304 is the design brief.** Web 027 delivered a provider a page could
find and a consent a page could obtain, and then could not produce a
signature a page could use. The gap was between the request channel and the
signing seam. This spec is ordered so that the signing path is proven with a
real page asking, on the device, before anything is called done: the local
test dApp the web already keeps (a Connect button and a Sign button over the
standard discovery event) is served to the phone, and the first success
criterion that counts is dust leaving the Safe because a page asked.

**What is different on a phone.** One column: the signing sheet is a sheet
over the page, not a third column beside it, so the browser must stay alive
under a sheet and must vanish when the person leaves the tab. A request can
arrive while the app is backgrounded or the screen is locked; it waits for
the person within the process, and nothing is signed without them. The
browser is the system's, so the page runs the same engine as the person's
other browser — no second rendering stack — and the shell owns the origin,
never the page.

**Standing exclusions**: the deeper message-signing surface (sign-in
binding, danger classes), the balance-change simulation block, and the
camera scanner are 046; split, sweep and batch are 045; remote and paired
sessions (WalletPair) are out by the founder's ruling for the desktop and
the same here; adding a network from a page's request is answered with the
core's refusal until 046's add-network path exists.

## User Scenarios & Testing *(mandatory)*

### User Story 0 - The six machines cross the bridge, and a page can see the wallet (Priority: P1, enabling)

The six browser-and-signing machines are driven from the phone the way the
sixteen before them are, and a page opened in the app finds a wallet that
announces itself with Vela's own name.

**Why this priority**: nothing later exists without it. It is also the
cheapest place to catch drift: every wire family is checked against the
core's generated mirrors before a screen depends on it.

**Independent Test**: the drift gate covers every event, operation, result
and closed error family of the six machines; a page loaded in the in-app
browser reports the discovery announcement and the legacy global, with
Vela's name, on the device.

**Acceptance Scenarios**:

1. **Given** a debug build, **When** each of the six machines is created
   and sent its first event, **Then** it answers with a view and no fault.
2. **Given** the in-app browser opens a page that listens for wallet
   discovery, **When** the page loads, **Then** it hears one announcement
   naming Vela with a fresh per-page identifier, and the legacy global is
   present when no other wallet placed one.
3. **Given** a page that asks for the current chain before connecting,
   **When** it asks, **Then** it is answered without any sheet opening.

---

### User Story 1 - Browse: the tab is a browser with a memory (Priority: P1)

A person opens 探索, sees their favourites and groups, opens a site by name
or address, moves between tabs, and finds the same favourites, groups, tabs
and recent sites after the app is killed and reopened.

**Why this priority**: a request has to come from somewhere. The drawn
screens exist (E1–E7); the machines that remember exist; this joins them.

**Independent Test**: favourites, groups, tabs and recents written on the
device are read back after a force-stop, and the address bar navigates a
real site.

**Acceptance Scenarios**:

1. **Given** an empty explore, **When** the person adds a favourite and a
   group, **Then** the start page shows them, and after a force-stop and
   reopen they are still there in the same order.
2. **Given** a site open in a tab, **When** the person opens the tab
   switcher and a second site, **Then** both tabs exist, the switcher shows
   both, and closing one leaves the other.
3. **Given** a visited site, **When** the person returns to the start page,
   **Then** the site is in recents, once, by its origin.
4. **Given** the person leaves 探索 for the wallet tab, **When** they come
   back, **Then** the page is where they left it, and while they were away
   nothing of it was painted over the wallet.

---

### User Story 2 - Connect: consent, instant answers, reads and chain switches (Priority: P1)

A page asks to connect; the person sees the origin as the fact and the
site's claimed name and icon as claims, chooses to connect the signed-in
account, and from then on the page gets its account and chain instantly,
its chain reads through the person's own endpoints, and hears when the
account or chain changes.

**Why this priority**: this is what 027 proved on the web and what every
dApp does first; the parallel space's Safe is the account it sees.

**Independent Test**: the web's local test dApp, served to the phone,
connects and prints the Safe's address; a public dApp connects and shows the
same address in its own header.

**Acceptance Scenarios**:

1. **Given** a never-connected origin asks for accounts, **When** the
   request arrives, **Then** a consent sheet opens leading with the origin,
   naming the signed-in account and the current network; approving answers
   the page with that one address; dismissing answers the standard refusal
   exactly once.
2. **Given** a connected origin, **When** it asks for accounts or chain
   again, **Then** it is answered instantly from the cached grant with no
   sheet.
3. **Given** a connected origin asks for a balance or a block number,
   **When** the read is answered, **Then** the answer came through the
   person's own endpoint pool for that chain, never from a third-party
   endpoint the page named.
4. **Given** a connected origin asks to switch to a chain the wallet has,
   **When** the switch is granted, **Then** the page hears the chain-changed
   event and its next read goes to that chain; asked for a chain the wallet
   does not have, it hears the core's refusal.
5. **Given** the person switches the active account while a page is
   connected, **When** the switch completes, **Then** the page hears
   accounts-changed with the granted address, and a signature request is
   pinned to the grant's address, never to whichever account is active.

---

### User Story 3 - Sign: the person sees what they sign, and it lands (Priority: P1)

A connected page asks the wallet to send a transaction. The signing sheet
shows who is asking, what the transaction does in plain words, the fee that
will be paid, and the signer; the person slides to confirm; the operation is
signed, submitted, written to the activity as pending, tracked to a receipt,
and the page is answered with the hash.

**Why this priority**: SC-304. This is the story 027 could not finish.

**Independent Test**: the local test dApp asks for a dust transfer to the
founder's address; in the parallel space the operation lands on chain, the
feed shows it pending at submit and confirmed after, and the page prints the
hash it was answered with.

**Acceptance Scenarios**:

1. **Given** a connected page asks to send a native transfer, **When** the
   sheet opens, **Then** it names the origin, the recipient and amount in
   the core's words, the fee quoted by the fee policy, and the signing
   account; the slide is enabled only when the core says it may be.
2. **Given** the person slides to confirm, **When** the operation is signed
   (by the passkey, or by the parallel space's keyset), **Then** exactly one
   signature is produced, the pending row is in the activity before the
   tracker is handed the hash, the page is answered with the hash, and the
   receipt is tracked to confirmation like a send from 043.
3. **Given** the person dismisses the sheet, **When** the request is
   refused, **Then** the page is answered with the standard rejection once,
   nothing was signed, and nothing was written.
4. **Given** a page asks to sign a message for sign-in, **When** the sheet
   opens, **Then** it shows the message as the core reads it and the
   signature it answers verifies for the wallet's address.
5. **Given** the relay refuses the submit, **When** the refusal arrives,
   **Then** the sheet says so in the core's words and the page is answered
   with a failure, not left waiting.

---

### User Story 4 - The guard: unlimited never leaves (Priority: P2)

A page asks for an approval without a ceiling. The sheet says so, refuses to
sign it as asked, and offers the person a bounded amount instead.

**Why this priority**: a token drainer's first tool is an unbounded
approval, and the core already detects every approval-granting shape; the
phone only has to draw its verdict.

**Independent Test**: the local test dApp asks for an unlimited approval;
the sheet shows the guard's block and editor; the operation that goes out,
if any, carries the bounded amount the person chose.

**Acceptance Scenarios**:

1. **Given** a page asks for an unlimited token approval, **When** the
   sheet opens, **Then** the slide is disabled, the reason is the guard's
   words, and an editor offers a bounded amount.
2. **Given** the person enters a bounded amount, **When** they confirm,
   **Then** the signed operation carries that amount and the page is
   answered with its hash.
3. **Given** the request is a batch containing one unlimited approval,
   **When** the sheet opens, **Then** the whole batch is blocked until that
   leg is bounded.

---

### User Story 5 - Connections are visible and revocable (Priority: P2)

The person sees which sites are connected, to which account, and can
disconnect one; the page hears it.

**Why this priority**: consent that cannot be withdrawn is not consent.

**Independent Test**: after connecting the test dApp, the connection sheet
lists it; revoking it makes the page's next request open a fresh consent
and fires the disconnect event on the page.

**Acceptance Scenarios**:

1. **Given** two connected origins, **When** the person opens the
   connections list, **Then** both are listed with their granted account and
   chain.
2. **Given** a listed connection, **When** the person revokes it, **Then**
   the page hears disconnect, its cached answers are gone, and its next
   request for accounts opens consent again.

---

### Edge Cases

- A page's claimed name and icon never widen a grant; an icon is fetched
  from the page's own origin or not at all; the origin shown is the
  browser's, never the page's.
- A second request arrives while a sheet is open: it waits its turn or is
  refused with the standard busy answer, per the core; nothing is lost and
  nothing is doubled.
- The app is backgrounded with a request pending: the request waits within
  the process; the person finds the sheet where they left it. The process
  dies: the page is gone too, and nothing was signed.
- A page navigates to another origin mid-session: the old grant does not
  follow; the provider announces again on the new page.
- A request over a non-secure origin is refused with the core's reason.
- A malformed or oversized payload is bounded before it reaches a screen.
- The person switches the network in settings while a page is connected:
  the page hears the change; a request already on screen keeps the chain it
  was made for.
- The signing sheet is dismissed by a system back or a swipe: the same
  single refusal as a tap on close.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001 (The bridge)**: the six browser-and-signing machines MUST be
  driven from the Android shell the way the sixteen before them are, and
  every wire family MUST pass the drift gate against the core's generated
  mirrors: views may be subsets, operations and results exhaustive.
- **FR-002 (One provider script)**: the page-side provider MUST be the same
  script the extension ships and the desktop injects — announced through the
  standard discovery event with Vela's true identity, present as the legacy
  global when no other wallet placed one, carrying the compatibility markers
  single-wallet dApps gate on — and it MUST be in place before the page's
  own scripts run.
- **FR-003 (One request channel)**: requests MUST travel from the page to
  the wallet and answers back over a channel that binds each request to the
  tab and origin that made it; identifiers MUST be single-use; answers MUST
  reach only the asking page.
- **FR-004 (Who answers)**: each forwarded request MUST be classified as a
  signature, a fact about the wallet, or a read of the chain by the same
  table the extension and the desktop use, and that table MUST be checked
  against the shared script by a test that fails when they disagree.
- **FR-005 (Reads through the person's endpoints)**: chain reads asked by a
  page MUST be answered through the person's own endpoint pool for that
  chain, with the pool's bans and cooldowns, never through an endpoint the
  page named.
- **FR-006 (Permissions are the core's)**: what an origin is granted, when
  it must ask, and what a revoke undoes MUST be decided by the permissions
  machine; the consent surface MUST lead with the origin as the fact and
  present the site's name and icon as claims.
- **FR-007 (The instant answers)**: the account and chain an already-granted
  origin expects MUST come from the permissions machine's grant mirror
  without opening a sheet.
- **FR-008 (Events to the page)**: the page MUST hear account changes, chain
  changes and disconnects as the standard provider events, when and only
  when the core says they happened.
- **FR-009 (Signing is 043's spine)**: transaction requests MUST be
  answered through the request, clear-signing and guard machines on the
  drawn signing sheet, priced by the fee policy, signed by the passkey (or
  the parallel space's keyset), submitted through the relay, written to the
  activity at submit, and tracked by the tracker — no second signing path.
- **FR-010 (Unlimited never leaves)**: an approval without a ceiling MUST
  be blocked as asked and MUST leave the wallet only bounded by the
  person's own amount.
- **FR-011 (Message signing answers)**: sign-in style message requests MUST
  be answered through the request machine with the message shown as the
  core reads it, so a page's login works; the deeper surface is 046.
- **FR-012 (The browser's memory)**: favourites, groups, tabs and recents
  MUST be the explore and history machines', persisted in the store shape
  the other clients read, and MUST survive a process death.
- **FR-013 (The browser lives and hides)**: the page MUST keep running under
  a sheet and while the app is briefly away, and MUST NOT be painted over
  any surface that is not the browser.
- **FR-014 (No Kotlin judgement)**: no shell code may decide what the core
  decides — whether an origin may be told something, what a request means,
  how dangerous it is, what an approval's ceiling should be, or what a
  refusal says.
- **FR-015 (Words from the corpus)**: every word on the consent, connection
  and signing surfaces MUST resolve from the core's corpus; new keys follow
  the corpus procedure.
- **FR-016 (Verified on the device)**: every phase MUST be verified on the
  connected test device, driven over the cable in the parallel space, and
  recorded with what the device showed; the one step that needs a finger is
  recorded as done only when a person did it.

### Key Entities

- **Provider**: the object a page sees; announces itself, forwards requests,
  emits the events dApps listen for.
- **Request**: one call a page makes, bound to the tab and origin that made
  it and to a single-use identifier, carrying what is asked.
- **Grant**: what an origin has been given — account, chain, provenance and
  age. Owned by the permissions machine.
- **Cached answer**: the account and chain a granted origin may be told
  immediately. The permissions machine's grant mirror.
- **Site memory**: favourites, groups, open tabs and recents. Owned by the
  explore and history machines, persisted in the shared store shape.
- **Signing record**: a request's lifecycle from arrival to answer, its
  reading, its verdicts and its outcome; the row the activity shows.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001** (device): a page loaded in the in-app browser reports the
  discovery announcement with Vela's name and finds the legacy global.
- **SC-002** (device): the local test dApp connects and prints the parallel
  space's Safe address; a public dApp connects and shows the same address.
- **SC-003** (device): a chain read asked by the page returns the same value
  the wallet's own pool returns for that chain at that moment.
- **SC-004** (device): dust leaves the Safe because a page asked — the
  sheet showed the transfer in the core's words, one signature was produced,
  the row was pending at submit and confirmed after, the page printed the
  hash it was answered with.
- **SC-005** (device): an unlimited approval asked by a page is blocked on
  screen and can leave only bounded.
- **SC-006** (device): a message sign-in is answered and the signature
  verifies for the Safe's address.
- **SC-007** (device): connections are listed and one is revoked; the page
  hears disconnect and its next request opens consent again.
- **SC-008** (device): favourites, groups, tabs and recents survive a
  force-stop; leaving and re-entering 探索 finds the page where it was, and
  nothing of the page is visible outside 探索.
- **SC-009**: the drift gate is exhaustive for the six new wire families;
  the unit-test count grows by at least the number of new executors and
  live builders.
- **SC-010**: the classification table's parity test against the shared
  provider script passes.
- **SC-011**: a release build's package carries no fixture key material
  and no debug-only door.
- **SC-012** (device, founder): one signature from the founder's own passkey
  answers a page, with one prompt.

## Assumptions

- The system webview on the device is recent enough to run the same
  provider script the desktop injects; JavaScript is enabled for the
  browser only.
- The parallel space's Safe is the connected account for every scripted
  device pass; the founder performs SC-012 with a finger.
- The web's local test dApp page is served to the phone over the cable; a
  public dApp is used for the connect proof only, and money moves only
  through the test dApp's transfer request.
- A message signature from a smart account verifies by the contract's
  signature check through the pool, which is how the test dApp checks it.
- Sponsorship is off in the parallel space, as in 043; the treasury
  pre-check and the fee sheet behave as they did there.
- The drawn explore and signing screens from spec 022 gain callbacks and
  live models; no new screens are drawn. Text for them is in the corpus;
  if a key proves missing it is added by the corpus procedure.
- Answering a request for a chain the wallet lacks is the core's refusal;
  adding a network from a page is 046's add-network path.
