# Feature Specification: 090 — Receive code: opt-in network (ERC-681)

**Feature Branch**: `090-receive-network-qr`
**Created**: 2026-10-02
**Status**: Implemented (see `results.md`)
**Input**: Owner ruling 2026-10-02 (issue #312 follow-up; #208 had been closed as "not supported"):
support ERC-681 on the Receive code, "but the hints must be friendly and the visual interaction
friendly, because many wallets that scan it don't support ERC-681". Default = bare address. Under the
code, a switch "include network", off by default. When on, the code is `ethereum:<address>@<chainId>`
for the network on screen, and a short, calm hint sits under the switch.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Show a code any wallet can scan (Priority: P1)

A person opens Receive, taps a network (or a token's 收款), and shows the code. With nothing touched,
the code is their bare address — exactly as before — so any wallet, ERC-681-aware or not, reads it.

**Why this priority**: this is today's behaviour and the one that never fails a payer. It must not
regress.

**Independent Test**: open any network's code; scan it with a plain-address scanner; it reads the
address. The switch under the code is off; no hint is shown.

**Acceptance Scenarios**:

1. **Given** the receive code for any network or token, **When** it opens, **Then** the code is the bare
   address and the "include network" switch is off with no hint under it.
2. **Given** the receive flow was closed with the switch on, **When** it is opened again, **Then** the
   switch is off again (session-scoped).

---

### User Story 2 — Tell the payer which network, when their wallet understands it (Priority: P1)

The person turns on "include network". The code now says `ethereum:<address>@<chain>` for the network
on screen; a Vela payer (or any ERC-681 wallet) lands on Send to that address, on that network. A quiet
line under the switch says some wallets can't read this code, and to switch it off if the payer's
can't.

**Why this priority**: it removes the "which network?" question that made people send on the wrong chain
(issue #312), without forcing a format many wallets can't read.

**Independent Test**: switch on, scan with Vela's own scanner: Send opens to the same address on the
same network. Switch off: the code is the bare address again and the hint goes away.

**Acceptance Scenarios**:

1. **Given** the Gnosis code, **When** the switch is turned on, **Then** the code encodes
   `ethereum:<address>@100` and the hint appears under the switch, in the normal subtle text colour.
2. **Given** a token's code (e.g. USDC on Base) with the switch on, **When** it is scanned, **Then** it
   names only the network (`@8453`), no token and no amount; the payer picks the asset.
3. **Given** the switch on, **When** the person taps Copy, **Then** the bare address is copied
   (people paste addresses).
4. **Given** the switch on, **When** the person taps Save image, **Then** the saved card encodes
   exactly the code on screen.

### Edge Cases

- Request mode (amount builder) already encodes a URI with its network: no switch, no hint there.
- Before the machine has a recipient: no code, no switch.
- Switching network while the switch is on: the code follows the new network; the switch stays on.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The core (`payment_request`) MUST decide the address-mode code: the bare recipient with the
  switch off, `ethereum:<recipient>@<chain_id>` (no token, no amount) with it on, where `chain_id` is the
  asset/network on screen.
- **FR-002**: The switch MUST default to off and MUST reset to off on every `Start` (session start).
- **FR-003**: The copy payload in address mode MUST stay the bare address, switch on or off.
- **FR-004**: The view MUST say whether the switch is offered (`network_switch`), its position
  (`include_network`) and whether the hint shows (`network_hint`); shells draw exactly that.
- **FR-005**: Every shell with a receive code (web + extension, desktop, iOS, Android) MUST draw the
  switch and hint from the core view, and MUST encode the core's `qr_value` on screen AND in the saved
  share image.
- **FR-006**: The URI MUST round-trip through Vela's own scanner (each shell's `parseEIP681`) and the
  send machine to the same address and `request_chain_id`.
- **FR-007**: The two strings live only in the corpus (`receive.includeNetwork`,
  `receive.includeNetworkHint`), in all 15 locales; the hint is calm (subtle text colour, not a warning
  colour).

### Key Entities

- **PaymentRequestView** — gains `network_switch`, `include_network`, `network_hint`; `qr_value`
  switches in address mode.
- **Event::IncludeNetworkChanged { include }** — the switch.

## Success Criteria *(mandatory)*

- **SC-001**: With the switch untouched, every shell's code and share image encode the bare address
  (byte-identical to before this spec).
- **SC-002**: With the switch on, every shell's code and share image encode the same
  `ethereum:<address>@<chain>` string the core view carries, and the shell's own scanner parser reads it
  back to the same address and chain.
- **SC-003**: All suites green; ja + en residency within the raised 139,800 budget.

## Assumptions

- Session-scoped (not persisted) is the friendlier default: a code shown tomorrow is one every wallet can
  read. Persisting the choice is an open question for the owner.
- A token's code names only its network (owner: "`ethereum:<address>@<chainId>` for the network being
  shown"), never the ERC-20 `transfer` form, which even fewer wallets read.
