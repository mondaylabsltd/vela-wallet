# K04 · EIP-1193 Methods & EIP-5792 Capability Discovery

| | |
|---|---|
| **Epic** | K — dApp Connect (WalletPair) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | K01, B07 |
| **Related** | K05, K07, F06, F01 |

## 1. Summary

WalletPair v1 sends encrypted standard EIP-1193 request, response, and event envelopes. The supported
method allowlist is enforced wallet-side; EIP-5792 `atomic: supported` is discovered with
`wallet_getCapabilities`. The chain context comes from the authenticated CAIP-2 frame suffix, not a
legacy capability negotiation or a WalletPair-specific parameter map.

## 2. Background & context

dApps expect an EIP-1193-style provider. Returning standard Ethereum RPC results directly lets standard
dApps work over WalletPair. `eth_getCode` remains handled by Vela's RPC layer, including the
counterfactual Safe runtime-bytecode behavior (B07/clue 47).

## 3. Users & stories

- As a **dApp**, I want to discover atomic call support, so that I can interact correctly.
- As a **user with an undeployed account**, I want dApps to recognize my smart wallet, so that EIP-1271 flows work before deploy.

## 4. Functional requirements

- **FR-1** — Accept only the documented EIP-1193 method allowlist and emit standard `connect`, `accountsChanged`, and `chainChanged` events.
- **FR-2** — Return per-chain EIP-5792 `atomic: supported` from `wallet_getCapabilities` (K07).
- **FR-3** — Preserve the authenticated CAIP-2 chain context when responding; reject a conflicting explicit request chain ID.
- **FR-4** — Return standard JSON-RPC result values directly; do not translate WalletPair-specific object params or result wrappers.
- **FR-5** — Read requests flow through the read-only gate (F06); signing bypasses it (F06/B02).

## 5. Non-functional requirements

- **NFR-1** — Capability discovery and the method allowlist match actual support (no over-claiming).
- **NFR-2** — Request params and results preserve their EIP-1193 shapes.

## 6. UX / flow notes

No direct UI; the method allowlist and authenticated chain suffix determine what the connected dApp can do.

## 7. Acceptance criteria

- [ ] **AC-1** — `wallet_getCapabilities` returns atomic support for the selected chain.
- [ ] **AC-2** — `eth_getCode` for an undeployed account returns Vela's Safe runtime bytecode (B07).
- [ ] **AC-3** — A response uses the request's authenticated CAIP-2 chain context and standard result shape.

## 8. Out of scope / non-goals

- Message signing — **K05**; batch calls — **K07**; read gate internals — **F06**.

## 9. Dependencies, risks & open questions

- **Risk:** capability/method drift vs the published Ethereum protocol — keep the allowlist in sync with `protocols/ethereum.md`.
- **Open question:** exposing newly added chains' RPCs promptly (F01).

## 10. Source anchors

- `src/services/walletpair-protocol.ts` (envelope/chain authentication), `src/services/walletpair-transport.ts` (method allowlist), `src/hooks/use-dapp-signing.ts` (capability response and bytecode behavior).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 88, 47.
