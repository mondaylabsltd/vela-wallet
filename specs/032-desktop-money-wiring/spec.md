# Feature Specification: Desktop Money Wiring — the Wallet Moves Money

**Feature Branch**: `032-desktop-money-wiring` (stacked on `031-desktop-read-wiring`)

**Created**: 2026-09-05

**Status**: Draft

**Input**: Founder direction (2026-09-05): desktop only; Android / iOS / web are
other people's. The 031 handover names the work: group A (`send` + `tx_tracker`
+ `batch_import` + `fee_policy` wired for real), and before any of it, the fixed
keyset signer in `vela-core`.

## Why

After 031 a signed-in person's desktop wallet is honest about what it HAS. It
still cannot do the one thing a wallet exists for. Every send screen in
`src/flows/` is a picture, and the signing sheet in `src/signing/` is a gallery
of 33 hand-written scenarios with no request behind it.

The web tier landed this path first (spec 026) and recorded the order that
worked, in its own words: kernels and `safe-transaction` verbatim, then the
relay client, **then the parallel space before any flow**, then `fee_policy`
and `send` together with ONE live fee session, then the tracker as an
app-resident, then the sheet, then the batch importer last. This cut repeats
that order on the desktop.

## What is different on the desktop, and why it changes the first phase

Every desktop signing path ends at a **real authenticator** — a USB key over
CTAP2, a phone over caBLE, the macOS platform vault, `webauthn.dll` on Windows.
None of them can be handed a private key. The parallel space's three P-256
scalars therefore cannot sign on the desktop through any existing path, which
means **no acceptance test in this cut can run until a software signer exists**.
That signer goes in `vela-core` behind a `dev-fixtures` feature, as the first
phase, so that every later phase is testable without a device.

## Scope

| Group | Machines | Core lines | This cut |
|---|---|---|---|
| A · spending | `send` `tx_tracker` `batch_import` `fee_policy` | 9,152 | **yes** |
| B · the sheet | `clear_signing` `approval_guard` `sign_request` | 9,362 | only if A closes with room; it needs a request source |
| C · dApp browser | `dapp_session` `dapp_permissions` `browser_history` | 3,771 | **no** — the desktop has no web engine (platform decision, not wiring) |

## Out of Scope

- A web engine for the desktop. `explore` stays a picture.
- A camera pipeline. `DS1` (scan) stays a picture.
- Any change to a machine's rules under `rust/crates/vela-core/src/app/`.
- Corpus regeneration: new live words reuse existing keys or are recorded as gaps.

## User Scenarios & Testing

### User Story 0 — The parallel space on the desktop (Priority: P1, enabling)

A developer runs the real desktop app with one substitution — the passkey
ceremony signs with the fixed keyset — and can create, sign in to, and sign for
the golden multi-key Safe with no authenticator on the desk.

**Independent Test**: with the space active, the three fixture accounts derive
their frozen addresses; an assertion built for the golden Safe verifies against
its own public key under the exact WebAuthn digest the on-chain verifier
checks; the registration attestation round-trips through the core's parser.

**Acceptance Scenarios**:

1. **Given** the fixture feature is off, **Then** no fixture scalar is in the
   binary and no path can reach a software signer.
2. **Given** the space is active, **Then** a badge says so on every screen, and
   every ceremony the onboarding and money machines request is answered by the
   fixture signer with the same shapes a real authenticator produces.
3. **Given** the golden fixture Safe (`0x88cC…6894`), **Then** the desktop can
   sign in to it and the home shows its real Gnosis balance, as 031 did by
   seeding storage directly.

### User Story 1 — Send a token to someone (Priority: P1)

A signed-in person picks a token, names a recipient, types an amount, sees the
relay's real fee in the fee coin the core chose, confirms, signs, and watches
the receipt settle — on the desktop's drawn screens.

**Acceptance Scenarios**:

1. **Given** a token with balance, **When** recipient and amount are typed,
   **Then** the core's validation rules the form and the CTA arms only when the
   core says so.
2. **Given** a valid draft, **When** the fee is quoted, **Then** the fee card
   shows the relay's quote in the core-chosen fee coin, requotes on the core's
   TTL, and a stale quote can never be confirmed.
3. **Given** the relay's treasury is empty or the network uncovered, **Then**
   the person is told what to do in the corpus's words.
4. **Given** confirm, **When** the signature is cancelled, **Then** nothing is
   submitted and the draft is intact; **When** signed, **Then** the operation
   is submitted once, persisted pending before the answer returns, and tracked
   to confirmed / failed by the core's polling rules.
5. **Given** the receipt confirms, **Then** the activity feed shows the send
   and the balance refreshes.

### User Story 2 — Money in flight outlives every window (Priority: P1)

A submitted operation is persisted as pending at submit time; the tracker runs
from boot, not from a screen, and settles what it finds.

### User Story 3 — Pay many at once (Priority: P2)

A pasted or picked table of recipients and amounts becomes one batched user
operation, with the core's parsing and rate rules.

## Requirements

- **FR-301 (One spine)**: quote (`fee_policy`) → sign → submit → persist
  pending → track (`tx_tracker`), driven by the core's events; the shell never
  composes a fee.
- **FR-302 (The signer is a seam)**: the passkey ceremony is obtained by the
  same `executor::passkey` entry points as login; the parallel space
  substitutes the fixed keyset behind them. The feature is compile-time
  (`dev-fixtures`) AND runtime (an explicit switch), and a badge renders
  unconditionally whenever the space is active.
- **FR-303 (Kernels in Rust, once)**: the Safe user-operation assembly the web
  ported verbatim into TypeScript (`safe-transaction.ts`, 3,068 lines) is
  written in Rust where the three native tiers can share it. Pure assembly
  goes to `vela-core`; the fetches stay in the desktop executor.
- **FR-304 (Ported, not reinvented)**: shell code ported from web carries a
  provenance header naming the source file and commit.
- **FR-305 (Fixtures stay canon)**: no `fixtures.rs` loses a constant; the
  galleries render unchanged.
- **FR-306 (Storage bytes)**: `vela.transactionHistory` and the account file
  keep their cross-client key and field names.
- **FR-307 (Every operation answered exactly once)**, with the core's own
  failure variants — the 030/031 rule, unchanged.
- **FR-308 (No machine changes)** under `rust/crates/vela-core/src/app/`.

## Success Criteria

- **SC-301**: the fixture keyset derives its three frozen single-key addresses
  and the frozen multi-key address in Rust; its assertions verify under the
  exact WebAuthn digest; its registrations parse. Proven by test.
- **SC-302**: with the space active, the desktop signs in to the golden Safe
  through the real onboarding machines — no storage seeding.
- **SC-303**: a live dust send from the golden Safe on Gnosis lands and its
  receipt is shown; the amount and fee agree with the explorer to the unit.
- **SC-304**: a window closed after submit shows the operation as pending on
  reopen and settles it.
- **SC-305**: relay faults (treasury empty, uncovered network, quote failure,
  submit rejection, receipt silence) each show a designed presentation; no
  raw relay text reaches a screen.
- **SC-306**: `cargo test` counts strictly increase in both crates; fmt and
  the CI jobs green; `rust/pkg-web` byte-identical unless a kernel was added
  on purpose, in which case it is rebuilt and committed.
