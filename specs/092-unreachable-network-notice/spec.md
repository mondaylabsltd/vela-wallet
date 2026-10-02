# Feature Specification: 092 — Networks the wallet cannot reach

**Feature Branch**: `092-unreachable-network-notice`
**Created**: 2026-10-02
**Status**: Implemented (device check by the lead pending)
**Input**: Finding F08 of the 087 beta pass, and the owner's ruling of 2026-10-02.

## Background

F08 (Android, Xiaomi, mainland China): Home said 「4 个网络 RPC 不可用」. Tapping it opened the Settings tab and an RPC-fix sheet for **one** network (BNB Chain). The other three were never shown, and "RPC" is jargon on Home. In mainland China about 5 of the 24 networks' public RPCs are always unreachable, so the line never goes away.

The 087 proposal was to warn only about unreachable networks that hold the person's assets. **The owner rejected it (2026-10-02):** while a network cannot be read, nobody knows what it holds. A network never reached is unknown, and one last seen empty may have received funds since. So every unreachable network keeps the notice; the rest of F08 is fixed.

## User Scenarios & Testing

### User Story 1 — The Home line says what is wrong in plain words (P1)

A person in China opens the wallet. Under the balance they read 「3 个网络暂时连不上」 ("Can't reach 3 networks right now"), or 「暂时连不上 BNB Chain」 when there is only one. The tone is calm: it states a fact and does not raise an alarm. The word "RPC" does not appear.

**Independent Test**: block the RPCs of 1, then 3, networks; read the line.

1. **Given** one unreachable network, **Then** the line names it.
2. **Given** several, **Then** it counts all of them, held or not.
3. **Given** a network that is only rate-limited, **Then** it is not counted (it recovers on its own).

### User Story 2 — One tap shows every unreachable network (P1)

Tapping the line opens a list of **all** unreachable networks, in one place, one tap from Home. Each row shows what the wallet last knew about that network:
- "Last seen ¥8,640.00" — it held something the last time it was read;
- "Held tokens when last read" — it held tokens, none of them priced;
- "Held nothing when last read";
- "Not read yet" — not read since this account opened.

Networks with last-known holdings come first, by their worth; the rest follow in the wallet's network order. Each row has the existing per-network action (修复 / Fix), which opens that network's RPC fix. Closing the fix returns to the list.

**Independent Test**: one network read with funds, then blocked; two never read. Open the list.

1. **Given** the above, **Then** the list shows 3 rows: the held one first with its worth, then the other two in network order.
2. **Given** privacy is on, **Then** the worth is masked and the row stays.
3. **Given** a row's Fix, **Then** that network's RPC fix opens (not the first network's).

### User Story 3 — The list updates live (P1)

While the list is open, the wallet reads every network again (when it opens, then 10 s after each read). A network that comes back leaves the list and the count. A network fixed through its row leaves at once. When none is left, the title says 「所有网络都已连上。」 ("Every network is reachable again.").

**Independent Test**: open the list with 2 networks down; bring one back.

1. **Then** within one re-read the row is gone, and the title (and the Home line) count 1.
2. **When** the list closes, **Then** the re-reads stop.

## Requirements

- **FR-001** The core decides the set, the order, the count, the Home sentence (one vs several) and each row's sentence. Shells only draw (`BalanceView.unreachable_networks`, `unreachable_key`, `UnreachableNetwork.line_key`).
- **FR-002** The set is the networks the last read could not reach, minus the rate-limited ones. A network is never left out because it looked empty.
- **FR-003** "Last known" is per account and per run. It comes from the core's own record of each chain's last answer, plus the rows a shell carried over for an unanswered chain. Shells report every chain a round asked (`FetchSettled.read_chain_ids`), so "held nothing" can be told apart from "not read".
- **FR-004** While the list is open, the core re-reads (`UnreachableListOpened`/`Closed`, every `UNREACHABLE_RECHECK_MS` = 10 s after a read). Silent partial retries come first. It stops when the list closes, the account changes, or nothing is left.
- **FR-005** Copy has no jargon on Home. The strings are in all 15 locales. The old `assets.rpcUnavailable{Single,Multiple}` are removed.
- **FR-006** Parity: web (and extension side panel, which is the same route), desktop, iOS and Android.

## Success Criteria

- **SC-001** Every unreachable network can be reached from Home in one tap, on all four shells.
- **SC-002** The count on Home equals the rows in the list, and both drop when a network returns. The core tests and the web e2e check this.
- **SC-003** No "RPC" in the Home line in any locale.
