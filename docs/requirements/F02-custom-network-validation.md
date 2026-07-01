# F02 · Custom Network Add — Contract-Suite + Precompile Validation

| | |
|---|---|
| **Epic** | F — Networks & RPC Infrastructure |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F01, B06 |
| **Related** | F03, G01, N02 |

## 1. Summary

Before persisting a user-added network, Vela checks (via the fastest reachable HTTPS RPC) that **all 11
required contracts** exist **and** the **RIP-7212 P-256 precompile** is present — because without them a
Vela account can't be derived, deployed, or verified on that chain. A companion "Chain Setup" tool can
deploy the missing contracts on chains that lack them.

## 2. Background & context

The account model (B06) assumes a specific contract set at canonical addresses and on-chain P-256
verification. A chain missing any piece would silently produce a wallet that can receive but never
sign — the same failure B05 guards at the passkey layer. Validation up-front prevents that.

## 3. Users & stories

- As a **power user**, I want to add a custom chain safely, so that I don't create an unusable wallet there.
- As an **operator**, I want to deploy the required contracts on a bare chain, so that Vela can support it.

## 4. Functional requirements

- **FR-1** — Validate presence of the full suite: Deterministic Deployment Proxy, Safe Singleton Factory, **Multicall3** (`0xcA11bde0…CA11`), EntryPoint v0.7, Safe L2, Proxy Factory, 4337 Module, Module Setup, WebAuthn Signer, Fallback Handler, MultiSend.
- **FR-2** — Validate the **RIP-7212 precompile** exists (checked two ways, per B06/network-checker).
- **FR-3** — Perform checks over the fastest reachable **HTTPS** RPC; reject non-HTTPS.
- **FR-4** — Only persist the network (A06) if all checks pass; otherwise explain what's missing.
- **FR-5** — Offer/point to a "Chain Setup" companion to deploy missing contracts.

## 5. Non-functional requirements

- **NFR-1** — Validation is bounded (timeouts) and doesn't hang the UI.
- **NFR-2** — Never persist a partially-valid chain.

## 6. UX / flow notes

Add-network form: RPC URL + chain metadata → validation progress → success or a specific "missing X" error. Custom/test networks never get sponsored gas (G06).

## 7. Acceptance criteria

- [ ] **AC-1** — A chain missing any of the 11 contracts is rejected with the specific gap named.
- [ ] **AC-2** — A chain without the RIP-7212 precompile is rejected.
- [ ] **AC-3** — A fully-equipped chain validates and persists.

## 8. Out of scope / non-goals

- The account model itself — **B06**; RPC scoring — **F03**; the Chain Setup deployer (companion tool).

## 9. Dependencies, risks & open questions

- **Risk:** a chain with contracts but no precompile can't sign — 🧭 roadmap explores a non-precompile signing path.
- **Open question:** UX for guiding users to the Chain Setup tool.

## 10. Source anchors

- `src/services/network-checker.ts:20-32,191-212`, `src/services/add-network.ts:42-53`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 58; clue 45 (precompile).
