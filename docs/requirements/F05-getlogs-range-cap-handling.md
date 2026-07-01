# F05 · `eth_getLogs` Range-Cap Handling

| | |
|---|---|
| **Epic** | F — Networks & RPC Infrastructure |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | F04, D05, D06 |

## 1. Summary

When an RPC rejects an `eth_getLogs` request because the **block range is too large**, the pool
**parses the cap and returns it** to the caller (so it can split the range) — it does **not** fail over
or ban the endpoint. This lets deposit detection (D05) and history (D06) adapt to each provider's limits
without churning the pool.

## 2. Background & context

Range-cap errors are not endpoint failures — they're policy limits that differ per provider. Treating
them as failures would ban good endpoints and thrash failover. Returning the cap lets the caller do the
right thing (chunk the query).

## 3. Users & stories

- As a **user**, I want deposit/history queries to work across providers with different limits, so that data is complete.
- As a **maintainer**, I don't want range-cap errors to poison the RPC pool, so that healthy endpoints stay usable.

## 4. Functional requirements

- **FR-1** — Detect `eth_getLogs` range-cap errors and **parse the maximum allowed range** from the error.
- **FR-2** — **Return** the cap to the caller instead of failing over or banning (F04).
- **FR-3** — Callers (D05/D06) split the requested range into cap-sized chunks and retry.
- **FR-4** — Non-range-cap `getLogs` failures still follow normal failover/banning (F04).

## 5. Non-functional requirements

- **NFR-1** — Parsing is robust to differing provider error formats.
- **NFR-2** — No endpoint is banned for a range-cap response.

## 6. UX / flow notes

No direct UI; makes deposit detection and history reliable across heterogeneous RPCs.

## 7. Acceptance criteria

- [ ] **AC-1** — A range-cap error returns a usable cap; the caller chunks and completes the query.
- [ ] **AC-2** — The endpoint that returned a range-cap is **not** banned.
- [ ] **AC-3** — A genuine getLogs failure still fails over normally.

## 8. Out of scope / non-goals

- Deposit detection — **D05**; history — **D06**; general failover — **F04**.

## 9. Dependencies, risks & open questions

- **Risk:** unparsed provider error variants; fall back to a conservative default chunk size.
- **Open question:** None.

## 10. Source anchors

- `src/services/rpc-pool.ts:63-488` — range-cap parse-and-return.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 56.
