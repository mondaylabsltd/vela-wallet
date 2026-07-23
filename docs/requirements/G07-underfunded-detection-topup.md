# G07 · Underfunded Detection & Top-Up Modal (Cross-Repo Coupling)

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G06 |
| **Related** | G05, F07 |

## 1. Summary

When the dedicated gas account (G06) is underfunded, the bundler returns an error that Vela recognizes
via `parseBundlerUnderfunded` — matched by **stable signals** (e.g. `/dedicated bundler (gas account|
EOA)/` + `Deposit to:` + `required:`), **not exact wording** — and surfaces a **gas-account top-up
modal** (`BundlerFundingModal`). This parser must stay in sync with the `vela-relay` repo's error
messages.

## 2. Background & context

A relayer EOA that runs dry can't submit ops. Rather than a cryptic failure, Vela detects the specific
underfunded condition and guides the user to deposit. Because the signal crosses two repos, matching on
robust substrings (not literal strings) prevents breakage when wording changes slightly.

## 3. Users & stories

- As a **user** whose gas account ran low, I want a clear "top up here" prompt, so that I can fix it in one step.
- As a **maintainer**, I want resilient error matching, so that a bundler copy-edit doesn't silently break the modal.

## 4. Functional requirements

- **FR-1** — `parseBundlerUnderfunded` detects the underfunded condition by stable signals (`dedicated bundler gas account|EOA` + `Deposit to:` + `required:`), tolerant of wording changes.
- **FR-2** — On match, present `BundlerFundingModal` with the deposit address and required amount.
- **FR-3** — After top-up, allow retrying the send.
- **FR-4** — Distinguish underfunded (top-up) from other bundler errors (which follow their own handling).

## 5. Non-functional requirements

- **NFR-1** — Matching is substring/regex-based, not exact-string, to survive minor bundler edits.
- **NFR-2** — Coupling documented so the wallet parser and bundler handlers evolve together.

## 6. UX / flow notes

`BundlerFundingModal` shows deposit-to address + required amount; copy explains the gas account is non-refundable and tops up from refunds (G06).

## 7. Acceptance criteria

- [ ] **AC-1** — A representative underfunded error triggers the top-up modal.
- [ ] **AC-2** — A reworded-but-equivalent error still matches (stable-signal test).
- [ ] **AC-3** — A non-underfunded error does **not** trigger the modal.

## 8. Out of scope / non-goals

- Gas account activation — **G06**; general receipt handling — **G09**.

## 9. Dependencies, risks & open questions

- **Risk:** **cross-repo coupling** — `parseBundlerUnderfunded` (wallet) must stay in sync with `vela-relay` `handlers.ts` wording.
- **Open question:** a shared contract/enum for this signal to remove string coupling entirely.

## 10. Source anchors

- `src/services/bundler-service.ts:367-385` — `parseBundlerUnderfunded`; `src/components/ui/BundlerFundingModal.tsx`.
- memory `project_bundler_underfunded_coupling`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 97.
