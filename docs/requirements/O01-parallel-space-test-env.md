# O01 · Parallel-Space Test Environment

| | |
|---|---|
| **Epic** | O — Ops, Testing, Store & Meta |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | — |
| **Related** | O02, N03, B01 |

## 1. Summary

The dev test environment (**"parallel space"**) is the **real app**, with the **only** difference being
a **fixed passkey keyset** — so tests run against production code paths, not mocks. It lives under a
`/parallel/*` route prefix with a visible badge, and provides fixture Safes + a local relay / test-dApp.
**All future feature tests run here.**

## 2. Background & context

Testing a passkey/4337 wallet against mocks risks divergence from production. Making the test env the
real app (only the passkey is fixed) means what's tested is what ships. Fixture Safes and a local
relay/test-dApp let dApp-connect and signing flows be exercised end-to-end.

## 3. Users & stories

- As a **founder/tester**, I want to test the real app deterministically, so that tests reflect production behavior.
- As a **contributor**, I want a consistent env with fixture accounts, so that flows are reproducible.

## 4. Functional requirements

- **FR-1** — The parallel env is the real app; **only** the passkey is a fixed keyset (B01).
- **FR-2** — Route under `/parallel/*` with a visible badge distinguishing it from production.
- **FR-3** — Provide fixture Safes and a **local relay / test-dApp** for WalletPair flows (K01).
- **FR-4** — All future feature tests are authored/run in this environment.
- **FR-5** — Combine with the fault harness (N03) to test resilience paths.

## 5. Non-functional requirements

- **NFR-1** — The fixed keyset must never be usable to sign for real user funds (env-isolated).
- **NFR-2** — Parity: no production code path is bypassed for testing.

## 6. UX / flow notes

`src/app/parallel/*` screens with a badge. Documented in `docs/PARALLEL-SPACE.md`.

## 7. Acceptance criteria

- [ ] **AC-1** — Feature flows run end-to-end in the parallel env against real code.
- [ ] **AC-2** — The env is clearly badged and route-isolated (`/parallel/*`).
- [ ] **AC-3** — dApp-connect flows work via the local relay/test-dApp.

## 8. Out of scope / non-goals

- Fault injection — **N03**; engineering rules — **O02**.

## 9. Dependencies, risks & open questions

- **Risk:** the fixed keyset leaking into production builds — must be env-gated.
- **Open question:** None.

## 10. Source anchors

- `src/app/parallel/`, `docs/PARALLEL-SPACE.md`.
- memory `project_parallel_space`.
