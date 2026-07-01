# K04 · Capability Advertisement & JSON-RPC Method Map

| | |
|---|---|
| **Epic** | K — dApp Connect (WalletPair) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | K01, B07 |
| **Related** | K05, K07, F06, F01 |

## 1. Summary

On session creation, WalletPair **advertises rich capabilities** to the dApp: signing + **22 read-only
RPC methods**, `accountsChanged` / `chainChanged` / `disconnect` events, **per-chain EIP-5792 `atomic:
supported`** (K07), the **RPC URLs for every configured network** (F01), and the **Safe proxy runtime
bytecode** so the dApp can answer `eth_getCode` for a counterfactual account (B07). A **method map**
translates WalletPair object-params ↔ Ethereum JSON-RPC array-params.

## 2. Background & context

dApps expect an EIP-1193-style provider. Advertising exactly what Vela supports (and translating param
shapes) lets standard dApps work over WalletPair. Advertising the counterfactual bytecode (B07/clue 47)
makes dApps treat an undeployed Vela account as the smart wallet it is (EIP-1271), not an EOA.

## 3. Users & stories

- As a **dApp**, I want to know the wallet's methods/chains/atomic support, so that I can interact correctly.
- As a **user with an undeployed account**, I want dApps to recognize my smart wallet, so that EIP-1271 flows work before deploy.

## 4. Functional requirements

- **FR-1** — Advertise signing + **22 read-only RPC methods** and the events `accountsChanged`/`chainChanged`/`disconnect`.
- **FR-2** — Advertise **per-chain EIP-5792 `atomic: supported`** (K07) and the **RPC URLs** for every configured network (F01).
- **FR-3** — Advertise the **Safe proxy runtime bytecode** so the dApp can answer `eth_getCode` for counterfactual accounts (B07).
- **FR-4** — Provide a **method map** translating WalletPair object-params ↔ JSON-RPC array-params both ways.
- **FR-5** — Read requests flow through the read-only gate (F06); signing bypasses it (F06/B02).

## 5. Non-functional requirements

- **NFR-1** — Advertised capabilities match actual support (no over-claiming).
- **NFR-2** — Param translation is lossless and bidirectional.

## 6. UX / flow notes

No direct UI; determines what the connected dApp can do. Chain/atomic advertisement enables batch calls (K07) on supported chains.

## 7. Acceptance criteria

- [ ] **AC-1** — A dApp reads the advertised methods, chains, and atomic support on connect.
- [ ] **AC-2** — `eth_getCode` for an undeployed account returns the advertised bytecode (B07).
- [ ] **AC-3** — Object-param and array-param requests both resolve via the method map.

## 8. Out of scope / non-goals

- Message signing — **K05**; batch calls — **K07**; read gate internals — **F06**.

## 9. Dependencies, risks & open questions

- **Risk:** capability/method drift vs the SDK — keep the map in sync with `walletpair-sdk`.
- **Open question:** exposing newly added chains' RPCs promptly (F01).

## 10. Source anchors

- `src/services/walletpair-transport.ts:54-202` (capabilities + method map), `:86-130` (bytecode advertise).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 88, 47.
