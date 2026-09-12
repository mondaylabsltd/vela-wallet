# Feature Specification: Desktop Send Parity — what the web can do with money that the desktop cannot

**Feature Branch**: `033-desktop-send-parity` (stacked on `032-desktop-money-wiring`)

**Created**: 2026-09-09

**Status**: Draft

**Input**: Founder direction (2026-09-09): "the web has a desktop layout too —
what does native still lack?", then "whatever the web can do, native should do
too", then "use new spec numbers, stack the branches, stop at 039".

## Why

032 ended with a handover that said the desktop had no pure wiring left. It was
wrong, and the way it was wrong is the reason this spec exists: the list came
from memory rather than from a comparison. Running the comparison mechanically —
every `Event` variant of every core machine, grepped in both shells — found
**forty-two events the web dispatches and the desktop never has**.

032's phases 44–47 closed the first four groups (the hero that never refreshed,
tap-to-hide, the network filter that was a picture, the network-settings
cluster). This cut takes the money ones.

## The method, and its two mandatory checks

1. **Name collisions lie.** `Abort` matches twenty web files that have nothing
   to do with the machine that declares it; `RejectTapped` reads as missing
   while the desktop rejects through `SwipeDismissed`. Every candidate is
   re-grepped before it is believed.
2. **Reachability on the web side.** An event dispatched inside a function
   nobody calls is not a capability. Phase 47 retired three planned items this
   way (delete a transaction, delete a custom token, receive with an amount) —
   all three are missing from BOTH shells, so building them is a new feature and
   not parity, and they are out of scope here.

## Scope

| # | What | Core events / view fields the desktop does not read |
|---|---|---|
| 1 | **Sweep** — send several tokens to one address in one operation | `ToggleMultiToken` `ToggleAllMultiTokens` `SetMultiNetwork` `ConfirmMultiSelection`; `multi_select_mode` `multi_selected_ids` `multi_valuable_ids` `multi_chain_id` `multi_specs` |
| 2 | ~~A fee quote that went stale~~ | **撤下(phase 2 复核)**:`FeeQuote.requote()` 在 web 里没有任何调用点,`FeeView.stale` 也没有任何 live 模型读它 —— 两端都没有,做就是新功能 |
| 3 | **The relayer's treasury is empty** | `send::DismissTreasurySheet` and the sheet it dismisses |
| 4 | **Split rows edited as a set** | `send::RecipientsChanged` |

## Out of Scope

- Deleting a transaction record, deleting a custom token, receiving with an
  amount: **neither shell can do these** (verified, phase 47). New features.
- The signing sheet's simulation and recipient risk — spec 034.
- A camera pipeline, a web engine: platform work, not wiring.
- Any change to a machine's rules under `rust/crates/vela-core/src/app/`.
- New corpus keys. Every word this cut needs already ships in fifteen locales,
  because the phone drew these screens first.

## User Scenarios & Testing

### User Story 1 — Sweep several tokens to one address (Priority: P1)

A person leaving a chain selects three tokens on it and sends them to one
address in one operation, without repeating the send flow three times.

**Independent Test**: with the golden Safe's holdings on screen, entering
multi-select, ticking rows, and confirming produces one batch whose per-token
amounts are the core's `multi_specs` — no amount the shell computed.

**Acceptance Scenarios**:

1. **Given** multi-select is on and a token on Gnosis is ticked, **When** the
   list is read, **Then** rows on every other chain are dimmed rather than
   removed — the person still owns them, and a list that silently shortened
   would read as a bug.
2. **Given** rows are ticked, **When** "select all valuable" is pressed,
   **Then** the scope is what the picker is SHOWING, and which of those count
   as valuable stays the core's answer.
3. **Given** a selection is confirmed, **Then** the form's per-token rows are
   `multi_specs`, and the fee is quoted once for the whole batch.

### ~~User Story 2 — The quote went stale~~ (retired in phase 2)

The reachability check retired it: the web's `FeeQuote` class HAS a `requote()`
method and nothing calls it, and no live model reads `FeeView.stale`. So a
stale-quote affordance is missing from both shells. Recorded as a shared gap,
not built here.

### User Story 3 — The relayer cannot pay (Priority: P2)

A submit that fails because the relayer's treasury is empty says so in the
designed words and offers the way out the core names, rather than a raw error.

**Independent Test**: with the treasury probe reporting empty, the sheet the
core raises is drawn, and dismissing it dispatches `DismissTreasurySheet`.

## Requirements

- **FR-331 (The core decides, the shell shows)**: every amount, every gate and
  every "which of these is valuable" answer is read from the view. The shell
  states scope (what is on screen) and nothing else.
- **FR-332 (Drawings do not move)**: no existing gallery state changes by a
  pixel; new states are new chips.
- **FR-333 (Zero new corpus keys)**: reuse the keys the phone's screens already
  resolve; a missing word is recorded as a gap, never invented.
- **FR-334 (Every operation answered exactly once)** — the 030/031 rule.
- **FR-335 (No machine changes)** under `rust/crates/vela-core/src/app/`.

## Success Criteria

- **SC-331**: a sweep of two or more tokens on one chain reaches the confirm
  screen with per-token rows from `multi_specs` and one fee for the batch.
- **SC-332**: off-chain-filter rows are dimmed, not hidden, and the master tick
  covers exactly the visible ∩ valuable set.
- ~~**SC-333**: a stale quote is visible and re-quotable~~ — retired with User
  Story 2 (neither shell reaches it).
- **SC-334**: the treasury-empty sheet renders from the core's presentation and
  dismisses through its event.
- **SC-335**: `cargo test` counts strictly increase; fmt, gallery sweep and the
  Windows check stay green.
