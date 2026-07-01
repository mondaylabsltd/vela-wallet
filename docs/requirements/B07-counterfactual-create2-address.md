# B07 · Counterfactual CREATE2 Address Derivation (Same Address Every Chain)

| | |
|---|---|
| **Epic** | B — Identity: Passkeys & Account Model |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B01 (passkey registration), B06 (Safe account model) |
| **Related** | C02 (create wallet), G01 (counterfactual deploy), H09 (receive), K04 (advertise bytecode) |

## 1. Summary

Every Vela wallet has **one address that is identical on every chain**, computed deterministically
from the passkey public key *before* any contract exists. Users can receive funds at that address on
day zero; the Safe self-deploys on the first outbound transaction. This is the mechanical core of the
"no seed phrase, same address everywhere" promise.

## 2. Background & context

The address must be derivable from nothing but the P-256 public key so that a restored passkey
(B09) reproduces the exact same wallet with no stored state, and so the address is stable across all
12 networks (F01). `chainId` is deliberately **not** an input to the salt — that is what makes the
address chain-independent. The derivation is re-implemented identically in TypeScript, iOS Swift, and
Android Kotlin, locked by a shared golden vector, because a one-bit divergence would strand funds.

## 3. Users & stories

- As a **new user**, I want a receivable address the instant I create a wallet, so that I can be
  funded before I ever pay for a deployment.
- As a **returning user on a new device**, I want my passkey to reproduce the *same* address, so that
  recovery needs no backup file.
- As a **multi-chain user**, I want the same address on Base, Gnosis, Tempo, etc., so that I never
  send to the "wrong-chain" version of myself.

## 4. Functional requirements

- **FR-1** — `saltNonce = keccak256(abi.encode(x, y))` from the COSE P-256 coordinates; the outer
  salt is `keccak256(abi.encode(keccak256(setupData), saltNonce))`. No `chainId` input.
- **FR-2** — Address = CREATE2 over the canonical `SafeProxyFactory`, with
  `initCodeHash = keccak256(proxyCreationCode ++ abi.encode(singleton))` (B06 contract set).
- **FR-3** — The address is computed with **zero RPC calls** and is available offline immediately
  after passkey enrollment.
- **FR-4** — A not-yet-deployed account answers `eth_getCode` with real runtime bytecode
  (`SAFE_PROXY_RUNTIME_CODE`, sliced from creation code) so dApps detect an EIP-1271 smart wallet,
  not an EOA (feeds K04).
- **FR-5** — Derivation output must byte-for-byte match the cross-platform golden vector.

## 5. Non-functional requirements

- **NFR-1** — Pure/deterministic: same public key → same address, forever, on any platform.
- **NFR-2** — Uses BigInt / fixed 32-byte encodings; no floating point in any hashing step.
- **NFR-3** — Keccak-256 is the hand-rolled variant (0x01 padding), verified against vectors (B06).

## 6. UX / flow notes

No direct UI. Surfaces as the address shown in C02/H09 and the QR/share card. Copy must never imply
the account is "created on-chain" until the first tx — it is *counterfactual* until then.

## 7. Acceptance criteria

- [ ] **AC-1** — For a fixed test public key, the derived address equals the golden-vector address in TS, Swift, and Kotlin.
- [ ] **AC-2** — The derived address is identical across all supported chainIds for the same key.
- [ ] **AC-3** — `eth_getCode` on an undeployed account returns the expected non-empty runtime bytecode.
- [ ] **AC-4** — Address derivation completes with the device offline (no network).

## 8. Out of scope / non-goals

- The actual on-chain deployment / initCode assembly — see **G01**.
- Publishing the public key to the index — see **B08**.

## 9. Dependencies, risks & open questions

- **Depends on:** canonical contract addresses (B06); COSE key extraction (B06).
- **Risk:** any change to the contract set, setup data, or salt formula **forks the address** and
  strands existing users — treat as an auto-High-risk change (O02). Keep the golden vector as the gate.
- **Open question:** None.

## 10. Source anchors

- `src/services/safe-address.ts:21-28` — canonical contract set; `:75-208` — salt/initCodeHash; `:220` — address compute; `:34-54` — runtime-bytecode slice.
- `src/services/eth-crypto.ts:110-146,231` — hand-rolled keccak + encoding.
- `src/services/attestation-parser.ts:23-45` — COSE P-256 key extraction.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 6, 36, 38, 47.
