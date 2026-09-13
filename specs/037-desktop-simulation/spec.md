# Feature Specification: Desktop Simulation — the one block a site cannot author

**Feature Branch**: `037-desktop-simulation` (stacked on `036-desktop-scanner`)

**Created**: 2026-09-09

**Status**: Draft

**Input**: Founder: 按推荐来. The comparison table is empty of wiring; this is
the second of the three real features it left, and the one with the most
security value.

## Why

Every other line on a signing sheet begins as something the dApp said — its
calldata, its intent, the token it names, the amount it claims. The balance
preview begins as what the CHAIN says would happen if the call ran. That is the
whole reason the deeper degradation rungs promote it from a footnote to the
protagonist.

It is drawn in both galleries (`kind: 'balances'` on the web,
`Block::Balances` here) and wired in neither — 034 retired it as parity work
for exactly that reason and recorded it as a feature worth building. This
builds it, on the desktop.

## The asymmetry is the core's, and this cut does not re-decide it

`token_trust`'s invariant ⑥:

- an **outflow** renders its amount whenever metadata resolved — the real token
  emits its own log, so what leaves cannot be understated;
- an **inflow** renders a number only when the token is trusted. A site can
  emit any `Transfer` it likes from a contract it controls, so an unverified
  receipt shows direction and name and **no figure**. "+1,000,000 SAFEMOON" on
  a signing sheet would be the attacker writing the wallet's own reassurance.

The shell simulates, hands the deltas over, and draws what comes back.

## Scope

1. `eth_simulateV1` through this person's own RPC pool — no third-party
   simulation service (ported from the web's `sim-engine-rpc.ts`).
2. Deltas from the logs, native and ERC-20 through ONE parser
   (`traceTransfers` reports native moves as synthetic `Transfer`s).
3. `token_trust::SimDeltasComputed` for the judgment.
4. The `Balances` block on the live signing sheet, plus the corpus's own
   sentence for a simulation that could not run.

## Out of Scope

- The send flow's `sim_json` (unread on both shells; the sheet is where this
  belongs).
- Any write from a simulation. The founder's standing ruling: token admission
  comes from confirm-time receipt logs, never from a sign-time simulation.
- A second engine (the web keeps a disabled Tevm seam; this has one engine and
  a degradation).

## Success Criteria

- **SC-371**: a swap's deltas net to one signed line per asset; other people's
  transfers and self-transfers produce none.
- **SC-372**: an unverified inflow carries a direction and no digits.
- **SC-373**: "could not be checked" is a different block from "checked,
  nothing moves".
- **SC-374**: proven against a real chain — a live simulation on Gnosis returns
  the delta the call would make.
