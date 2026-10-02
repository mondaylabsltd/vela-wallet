# Feature Specification: real dApp interop pass

**Feature Branch**: `096-dapp-native-value` (part A), `096-clear-signing-readable` (part B), `096-fee-coin-guard` (part C)

**Created**: 2026-10-02

**Status**: In progress (beta submission 2026-10-03)

**Input**: the lead's real-dApp pass of 2026-10-02 on the extension build
(main + 090–095), golden Safe on BNB Chain — PancakeSwap, Uniswap, Aave, CoW,
Curve, Sky. Twelve findings, [findings.md](findings.md).

## Scope

The findings a person meets on the first day of the beta: a dApp call that
moves native coin must reach a signature; a failure must be said, not
vanish; what the sheet says must be what is signed; a fee coin must not be
one the batch spends; a page polling a batch must hear how it ended; the
connect consent must say what it shares. Every rule is decided once in the
core; every shell with the surface gets it (web, extension, desktop, iOS,
Android). F10 (PancakeSwap lists Vela as MetaMask) is the site's, not ours.

| Part | Branch | Findings |
|------|--------|----------|
| A | `096-dapp-native-value` | F1, F3, F8, F11, and the cross-shell sweep of F1's class |
| B | `096-clear-signing-readable` | F4, F5, F6, F7, F9 |
| C | `096-fee-coin-guard` | F2, F12 |

Each part writes its own `results-<part>.md`.

## User Scenarios & Testing

### US1 — A dApp call that sends native coin reaches the signature (P1, F1)

PancakeSwap's BNB → USDC asks `eth_sendTransaction` with
`value: "0xaa87bee538000"` (0.003 BNB).

**Acceptance**:

1. **Given** that request on BNB Chain (in-band, native or stablecoin fee) or
   on Tempo, **When** the person slides, **Then** the gas floor measures the
   call with its value, the passkey signs, the op carries `0xaa87bee538000`,
   and the page gets the transaction.
2. **Given** a batch with a native leg, or Aave's `depositETH` with a value,
   **Then** the same.
3. **Given** a value no reading can be sure of (`"1000"`, `"aa87bee538000"`,
   a JSON number other than 0, `"0X1f"`, an overflow), **Then** the request is
   refused `-32602` before any sheet, on every shell — never guessed, never
   signed as one amount here and another there.

### US2 — A failure before anything was sent is said, and can be tried again (P1, F8)

**Acceptance**:

1. **Given** the submit fails before the op leaves (an estimate, a nonce
   read, the relay unreachable), **When** the sheet is up, **Then** it shows
   "Failed — The transaction couldn't be submitted. Your funds are safe —
   please try again." with **Done** and **Try again**; the request window
   stays open (the page is not answered yet).
2. **When** the person taps Try again, **Then** the request is back on review,
   unanswered, and a slide submits it afresh.
3. **When** the person taps Done (or the ✕), **Then** the page gets the
   JSON-RPC error (`-32603`, the failure's words) once.
4. **Given** the relay refused it, **Then** "The network refused it — nothing
   was sent." and Done only.
5. **Given** nobody is looking (the sheet was closed while it ran, or a new
   request replaced it), **Then** the page is answered at once.

### US3 — A batch's status says how it ended (P2, F3)

**Acceptance**: **Given** a `wallet_sendCalls` batch the relay rejected
before any block, **When** the page asks `wallet_getCallsStatus`, **Then**
`status: 400` (EIP-5792: not included, the wallet will not retry) — never 100
for as long as it asks. A landed batch stays 200/500 from its receipt; a
`rejected` that names a bundle transaction waits for that transaction (100),
as the tracker does.

### US4 — The connect consent says what it shares (P2, F11)

**Acceptance**: **Given** a site asks to connect, **Then** the request window
and the side panel name the account (artwork, name, short address) the core
says a Connect pins the grant to, and the network the grant is written on —
the facts every in-app browser's consent already names.

### US5–US7 — parts B and C

Readable signing (F4–F7, F9) and the fee-coin guard (F2, F12): see those
branches' results.

## Requirements

- **FR-001** (core): one reading of a call's `value` — `tx_request` —
  used by the card (RC4), at arrival (refuse or canonicalize), and by every
  shell's submit and fee quote (`calls_of`: every call or none).
- **FR-002** (core): a failure while its sheet is up is held and answered on
  the close; `RetryTapped` returns a not-sent, not-refused failure to review;
  `SignView.failure_retryable` tells the shells.
- **FR-003** (core): `calls_status` takes the relay's status when there is no
  receipt; its refusal before any block is 400. The in-app browsers ask the
  relay as a second read; the extension worker's twin does the same.
- **FR-004** (core): `DpermPopupView.consent_address` — the account a
  Connect would share.
- **FR-005**: no new strings in part A (Send's `send.txRetryBtn`,
  `componentsTx.receipt.done`; `explore.account`, `explore.network`).

## Success Criteria

- **SC-001**: the PancakeSwap / Aave / batch requests of the pass reach a
  signature on BNB Chain in the real submit path (test fails on the old code
  with the console's exact line).
- **SC-002**: one value table, pinned in the core and in each shell's tests.
- **SC-003**: no request window closes over a failure; every failure is
  answered exactly once.
- **SC-004**: a relay-rejected batch reads 400 in the extension and in every
  in-app browser.
