# Feature Specification: Android Signing Depth — Simulation, Message Signing, Scanner

**Feature Branch**: `046-android-signing-depth` (stacked on `045-android-send-contacts-parity`)
**Created**: 2026-09-12
**Status**: Draft
**Input**: The program doc's 046 (mirrors desktop 035 + 036 + 037): the one
block a site cannot author (the simulated balance changes, judged by the
core's asymmetric trust and never written from), the rung the ladder was
missing (`eth_sign` presented as the danger it is, SIWE with its domain
binding), and the scanner (a camera the app can actually read, a picked
photo, EIP-681 with both refusals, add-network by QR).

## User Scenarios & Testing

### US1 — The balance-change block (P1)
A page asks the wallet to send or to call a contract. Before signing, the
sheet shows what the transaction would move: `−0.001 XDAI`, `+12 USDC`,
"Unverified token" for an ERC-20 the wallet does not trust — from an
`eth_simulateV1` run through the pool, judged by the core's `token_trust`
machine (`SimDeltasComputed` → `sim.judgments`). When the RPC refuses, the
sheet says so ("couldn't be simulated") and the intent block stands alone.
Nothing is ever written from a simulation (founder ruling, spec 017).
**Acceptance**: a page-initiated XDAI transfer shows `−0.001 XDAI` before
signing; after it lands the feed row says `−0.001 XDAI` — the block agreed
with the receipt. A simulate failure shows the unavailable line and still
signs.

### US2 — Message signing depth (P1)
`personal_sign` shows the message; a SIWE message shows "Sign in", the
domain, the statement, the URI, and the binding verdict (OK, or the
mismatch warning when the message's domain is not the page's origin);
`eth_sign` is presented — not refused silently — as the danger surface
(EthSign class, its warning, the danger tone), and can still be signed by
someone who reads it. **Acceptance**: the test dApp's SIWE message binds
OK on its own origin; a SIWE message naming another domain warns;
`eth_sign` shows the danger surface.

### US3 — The scanner (P1)
The send form's scan icon and the home's 扫码 open the camera; a QR in the
frame resolves (`send::ScanResolved`): an EIP-681 request locks the Send
(chain, token, amount; the two refusals: a non-payment call is refused, a
`/transfer`'s `value` is not the token amount), a bare address fills the
recipient, a chain the wallet lacks goes through `AddByChainIdRequested`
(the scan path). A picked photo decodes the same way. **Acceptance**: a QR
image picked from the phone's storage resolves into a locked Send; the
camera preview opens, analyses frames and closes cleanly; a QR carrying a
chain the wallet lacks adds the network.

### Edge cases
- No camera permission: the drawn permission line, the photo path still works.
- A QR that is not a payment: the text lands in the recipient (a bare
  address) or is refused with the scanner staying open.
- Simulation of a message: none (off-chain); of a batch (`wallet_sendCalls`):
  all calls in one block-state call.

## Requirements
- FR-001 The simulation runs through the RPC pool with `traceTransfers`, the
  deltas derived as the desktop's `executor/sim.rs` (Transfer logs, the
  native sentinel), dispatched to the wallet's `token_trust` host; the sheet
  reads `sim.judgments` once `sim.ready`. Never a write from it.
- FR-002 The Balances block sits on the signing sheet for
  `eth_sendTransaction`/`wallet_sendCalls`; the unavailable line replaces it
  when the simulation fails; "no asset changes" when it is empty.
- FR-003 `eth_sign` routes to the signing sheet with the EthSign danger class
  (the extension's provider keeps refusing it — the in-app browser presents
  it, as the desktop does).
- FR-004 The scanner: CameraX preview + ZXing decode (no Google services), a
  picked photo through the document picker, torch, close; decoded text goes
  through a Kotlin EIP-681 tokenizer pinned to the web's by a parity test;
  the core decides (`scan_resolved`).
- FR-005 A scanned chain the wallet lacks: the send executor's `AddNetwork`
  arm asks the settings machine (`AddByChainIdRequested`) and answers the
  core's outcome.
- FR-006 Every phase verified on the connected Xiaomi in the parallel space.

## Success Criteria
- SC-001 Balance block `−0.001 XDAI` before signing; feed `−0.001 XDAI` after.
- SC-002 SIWE OK on origin; mismatch warned; eth_sign danger surface shown.
- SC-003 A picked QR photo locks the Send with the scanned chain/amount.
- SC-004 The camera preview opens and closes without a crash on the Xiaomi.
- SC-005 A scanned unknown chain adds the network (or says not found).

## Out of scope
Deep-link `/pay` (047), the receive share image (047), the extension's
eth_sign policy (unchanged), Tevm/second engines.
