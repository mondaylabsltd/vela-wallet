# Feature Specification: dApp pass two — what the second real-money pass found

**Feature Branch**: `097-signing-readable` (part A), `097-dapp-activity-amounts` (part B), `097-refusal-after-submit` (part C)
**Created**: 2026-10-03
**Status**: In progress
**Input**: The second real-money dApp pass on the Chrome extension (main `cca03b56`, which holds 096 A + B + C), golden Safe `0x88cC…6894` on BNB Chain. It covered PancakeSwap, Uniswap, 1inch, Aave (supply, borrow, repay, withdraw), CoW and Curve. 8 of 9 flows landed. F1, F2, F4, F6, F7 and F11 of 096 are confirmed fixed. Findings N1–N8 below; evidence in [findings.md](findings.md).

## User Scenarios & Testing

### User Story 1 — The sheet never states something false or unknown as certain (Priority: P1)

A person signs a 1inch native order, an Aave borrow or a single-call swap and reads the sheet.

**Acceptance Scenarios**:

1. **Given** a field the reading formats as an address but whose value is a `uint256` holding an address (1inch `Beneficiary`), **When** the sheet draws it, **Then** it shows that address (`0x…`, short form with the full form available), never a decimal number. **(N1)**
2. **Given** an amount of a token the wallet cannot identify (no registry entry, no metadata read), **When** the sheet draws it, **Then** the figure is not presented as certain at a guessed 18 decimals. It is either read correctly or shown as unknown, and the reading is marked incomplete. **(N1)**
3. **Given** an Aave `borrow`, **When** the sheet draws the amount, **Then** it reads as money coming in (`+0.3 USDC`), not leaving. **(N2)**
4. **Given** a single-call swap whose received amount is a minimum, **When** the sheet draws the hero, **Then** it says it is a minimum, the same way batch legs already do. **(N3)**
5. **Given** a 1inch limit-order signature (`Order`), **When** the sheet draws it:
   - **Then** a zero `receiver` reads as the account itself, not as `0x0000…0000`;
   - the order's expiry is shown;
   - an unwrap-to-native flag names the native coin as what is received;
   - a minimum is labelled as one. **(N6)**
6. **Given** the Curve router on BNB Chain, a known token contract as a batch leg's target, or a token on an approve/permit row, **When** the sheet draws it, **Then** each is named. A token whose symbol is not from the registry also shows its short address. **(N8, sheet part)**

### User Story 2 — Activity says what happened, with the figures the chain proved (Priority: P1)

**Acceptance Scenarios**:

1. **Given** a dApp call that sends no native coin (USDC→BNB swap, Aave borrow/repay/withdraw, CoW, Curve), **When** it lands, **Then** its Activity row shows the amounts the receipt proves moved in and out. **(N5)**
2. **Given** a "Received" transfer that the feed merges into a dApp row, **When** the row is drawn, **Then** its `+` figure is shown even when the row has no outgoing amount. **(N5)**
3. **Given** a dApp interaction, **When** its row is titled:
   - **Then** the intent is capitalised;
   - the place is the protocol when a known contract names it: Permit2 approvals take their spender's protocol, and Curve is "Curve", not a host name;
   - a typed-data signature is titled by what it is, not "Sign structured data". **(N7)**
4. **Given** amounts with no known price, **When** the detail sheet draws them, **Then** it shows no fiat figure rather than "≈ $0.00". **(N7)**
5. **Given** a failed dApp row, **When** it is drawn or opened, **Then** it says why it failed. **(N4, Activity part)**
6. **Given** a contract the sheet named (e.g. 1inch NativeOrderFactory), **When** Activity shows it, **Then** it uses the same name, not a raw address. **(N8, Activity part)**

### User Story 3 — A refusal after "Submitted" is shown, not swallowed (Priority: P1)

**Acceptance Scenarios**:

1. **Given** a transaction the sheet has shown as Submitted, **When** the network refuses it ("nothing was sent"), **Then** the sheet shows the failure in its own words with Done. It stays until the person closes it; the dApp is answered once. **(N4)**
2. **Given** the same on the in-app browsers (desktop, iOS, Android), **Then** the same happens.

### Edge Cases

- An address-formatted field holding a number larger than 160 bits is shown raw, never truncated into an address.
- A token whose decimals cannot be read stays unknown; a later read upgrades the reading.
- The extension window closed by the OS while a failure is shown answers the page once (096 A behaviour stays).

## Requirements

- **FR-001**: Every rule is decided once in `vela-core`; the four shells and the extension only draw.
- **FR-002**: Parity on every shell that has the surface; a shell without it is named in results.
- **FR-003**: The ja + en residency budget stays at 141,800 bytes. New wording reuses existing keys where one fits; any new key is offset by trimming, so ja + en does not grow.
- **FR-004**: Each finding gets a test that fails on the old code, using the pass's real requests (`scratchpad/dapp096/logs/r2-req-*.json`, copied into core fixtures).

## Success Criteria

- **SC-001**: The 1inch native-order, Aave borrow and single-call swap requests from the pass read correctly through the real core on all four shells.
- **SC-002**: Every dApp row of the pass shows its proven figures and a protocol place.
- **SC-003**: A post-submit refusal is visible on every shell until dismissed.
- **SC-004**: All suites green; CI green on the stacked PR.
