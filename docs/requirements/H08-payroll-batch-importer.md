# H08 · Payroll Batch Importer (Recipient Table Paste / CSV)

| | |
|---|---|
| **Epic** | H — Send & Receive |
| **Status** | 🚧 In progress (branch `feat/contacts-groups-payroll-batch`) |
| **Owner** | Shelchin |
| **Depends on** | H06 (contact groups), H07 (split send), E07 (fiat entry) |
| **Related** | G01 (UserOp), J05 (approval guard), H03 (recipient risk) |

## 1. Summary

Let a user pay many recipients at once by **pasting or uploading a table** (CSV / TSV / pasted rows /
`.xlsx`) of address + amount (+ optional name). The importer parses it into recipient rows, applies
fiat→token conversion where amounts are fiat, and hands the result to the split-send path (H07) so the
whole payroll settles as **one atomic MultiSend UserOp**. Rows can be sourced from a saved contact
group like "Payroll" (H06).

## 2. Background & context

Sending salaries or grants one-by-one is slow and error-prone. A batch importer plus atomic MultiSend
turns "N transactions, N signatures, N gas payments" into one. Amounts default to **fiat** (a founder
paying "$5,000") and are converted per current price (E07), so the payer thinks in money, not wei.
Column order is not fixed — real spreadsheets vary — so the parser infers roles instead of demanding a
schema.

## 3. Users & stories

- As a **team lead**, I want to paste a payroll table and send everyone at once, so that payday is one action.
- As a **DAO operator**, I want to reuse my "Payroll" contact group as the recipient list, so that I don't re-paste addresses.
- As a **careful payer**, I want bad rows flagged (not silently dropped), so that no one is skipped without me knowing.

## 4. Functional requirements

- **FR-1** — Parse CSV / TSV / semicolon / pasted-table text via a **pure, synchronous** path; sniff
  the delimiter, preferring the one that yields an address cell (so `addr;¥5,000.50` picks `;` over the thousands comma).
- **FR-2** — Infer columns by role: address = the cell that `isAddress()`; amount = first positive-number
  cell that isn't the address; remaining text cell = name. Support `address,amount`, `amount,address`, and `name,address,amount`.
- **FR-3** — `.xlsx` is supported by **lazy-loading `xlsx`** only when a user opens a spreadsheet — SheetJS (~1MB) must never sit on app startup.
- **FR-4** — Return both `rows` (valid) and `errors` with a **1-based source line** and a reason (`no-address` | `no-amount`); never silently discard a row.
- **FR-5** — Amounts are fiat by default; the importer applies fiat→token conversion (E07) before building calls. A `name` column becomes the contact label.
- **FR-6** — Valid rows feed the **split** path (H07) → one `sendBatchCalls` MultiSend UserOp; max-send reserves native gas so the batch can't revert with AA21.

## 5. Non-functional requirements

- **NFR-1** — Parsing a few-hundred-row table is instant (synchronous, no RPC) and must not block the UI thread noticeably.
- **NFR-2** — Startup bundle unaffected by spreadsheet support (lazy import verified).
- **NFR-3** — Amounts use BigInt base-unit math; no float rounding drift across a large payee list.

## 6. UX / flow notes

Paste box + file picker → preview table with valid rows and an **errors panel** (line, raw text, reason).
Group picker offers saved groups (H06). Confirm routes through the standard signing sheet (I01) with the
batch shown as one transaction. Duplicate/first-interaction recipient warnings surface per H03.

## 7. Acceptance criteria

- [ ] **AC-1** — `address,amount`, `amount,address`, and `name,address,amount` all parse to the same rows.
- [ ] **AC-2** — A row with no valid address yields a `no-address` error carrying its 1-based line; parsing continues.
- [ ] **AC-3** — Opening a `.xlsx` triggers the lazy `import('xlsx')`; a cold start that never opens one does not load it.
- [ ] **AC-4** — A parsed table with fiat amounts converts to correct token base units and sends as a single MultiSend UserOp.
- [ ] **AC-5** — Selecting the "Payroll" group prefills recipients without manual paste.

## 8. Out of scope / non-goals

- The MultiSend encoding itself — see **H07 / G01**.
- Fiat conversion math — see **E07**.
- Per-recipient risk scoring — see **H03**.

## 9. Dependencies, risks & open questions

- **Depends on:** contact groups (H06), split send (H07), fiat conversion (E07).
- **Risk:** locale-formatted amounts (thousands separators, currency glyphs, full-width digits) must be
  normalized before `Number()`; a mis-sniffed delimiter could merge address+amount. Guarded by the delimiter sniffer + explicit error rows.
- **Open question:** cap on rows per batch (MultiSend gas ceiling) — define an upper bound and surface it before send.

## 10. Source anchors

- `src/services/recipient-table.ts:1-15` (contract), `:19-37` (`ParsedRow`/`ParseError`/`ParseResult`), `:46-58` (delimiter sniff), `:143` (`parseRecipientTableText`), `:169` (`parseRecipientTable`, lazy xlsx).
- `src/services/batch-send.ts:82` (`buildSplitCalls`), `:90` (`sumSplitBaseUnits`), `:148` (`reserveNativeGas`).
- `src/services/contacts.ts:52` (`ContactGroup`), `:337` (`saveGroup`).
- `src/services/fiat-convert.ts:39` (`fiatToTokenAmount`).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 98, 99.
