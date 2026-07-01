# C05 · Multiple Accounts & Account Switcher

| | |
|---|---|
| **Epic** | C — Onboarding & Wallet Lifecycle |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | C02 |
| **Related** | C03, A06, D01, D02 |

## 1. Summary

A user can hold **multiple accounts** and switch between them from an account switcher. Each account is
its own passkey-derived Safe (C02) with its own name (C03); the active account drives balances,
activity, send, and dApp connections. Selection persists locally (A06).

## 2. Background & context

Users separate concerns (personal / work / payroll). Because each account is an independent
passkey-controlled Safe, the switcher is a selection layer over otherwise-independent identities — no
shared key, no derived subaccounts.

## 3. Users & stories

- As a **user**, I want several accounts and quick switching, so that I keep contexts separate.
- As a **user**, I want the active account to drive the whole app consistently, so that I never act from the wrong one.

## 4. Functional requirements

- **FR-1** — List all local accounts with name (C03) and address; allow selecting the active account.
- **FR-2** — Persist the active account (A06); restore it on next launch.
- **FR-3** — Switching the active account re-scopes balances (D02), activity (D08), send (H01), and any dApp session (K03) to that account.
- **FR-4** — Support adding another account (new passkey via C02) and removing a local account reference (without touching on-chain state).

## 5. Non-functional requirements

- **NFR-1** — Switching is fast; balances for the newly active account load with cached-then-fresh (A06/D02).
- **NFR-2** — The active account is unambiguous in the UI at all times.

## 6. UX / flow notes

`AccountSwitcherModal` presents accounts; the active one is clearly marked and shown in the home header. A single dApp session belongs to one account (K03) — switching accounts affects the connection.

## 7. Acceptance criteria

- [ ] **AC-1** — Adding a second account and switching re-scopes balances/activity to it.
- [ ] **AC-2** — The active account persists across relaunch.
- [ ] **AC-3** — Removing a local account reference does not affect on-chain funds or other accounts.

## 8. Out of scope / non-goals

- Creating the account/passkey — **C02**; naming — **C03**.

## 9. Dependencies, risks & open questions

- **Risk:** an in-flight dApp session or pending tx during a switch — must scope cleanly (K03/L01).
- **Open question:** whether removing a local account should warn if it's the only reference to that wallet.

## 10. Source anchors

- `src/components/ui/AccountSwitcherModal.tsx`, `src/services/accounts.ts`, `src/models/wallet-state.ts`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 6.
