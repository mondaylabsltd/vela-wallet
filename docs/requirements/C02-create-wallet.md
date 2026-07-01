# C02 · Create Wallet (Passkey Enrollment → Address)

| | |
|---|---|
| **Epic** | C — Onboarding & Wallet Lifecycle |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B01, B07 |
| **Related** | C03, B05, B08, D01 |

## 1. Summary

Creating a wallet is a single biometric action: enroll a WebAuthn passkey (B01), extract its public
key, and **derive the counterfactual address immediately** (B07) — no on-chain transaction, no gas,
no seed phrase. The account is receivable at once and will self-deploy on its first send (G01).

## 2. Background & context

The "create wallet" moment is where the seedless promise becomes real. It must be fast, offline-capable
for derivation, and must fail *before* persistence if the passkey provider is incompatible (B05).

## 3. Users & stories

- As a **new user**, I want to create a wallet with one biometric prompt, so that setup is instant and phrase-free.
- As a **user**, I want a usable receive address right away, so that I can be funded before deploying.

## 4. Functional requirements

- **FR-1** — Trigger WebAuthn registration (B01) gated by biometric; extract the P-256 public key.
- **FR-2** — Reject incompatible providers before saving anything (B05).
- **FR-3** — Derive the counterfactual address (B07) locally, with no RPC call.
- **FR-4** — Persist the account locally (A06) and kick off the background public-key upload (B08).
- **FR-5** — Land the user on home (D01) with a receivable address and $0 balance.

## 5. Non-functional requirements

- **NFR-1** — Derivation works offline; publish (B08) retries silently later if offline.
- **NFR-2** — No private key is ever stored (A03/B01).

## 6. UX / flow notes

One biometric prompt → home. Optional account naming (C03) can follow or be deferred. No gas/deploy prompt at creation (deploy is bundled into the first send, G01).

## 7. Acceptance criteria

- [ ] **AC-1** — Creating a wallet yields a derived address matching B07's golden logic.
- [ ] **AC-2** — An incompatible provider aborts before any account is saved (B05).
- [ ] **AC-3** — The new account can receive funds immediately (counterfactual).

## 8. Out of scope / non-goals

- Address math — **B07**; naming/publish — **C03/B08**; first-tx deploy — **G01**.

## 9. Dependencies, risks & open questions

- **Risk:** partial creation on interruption — creation must be atomic (derive+persist or nothing).
- **Open question:** None.

## 10. Source anchors

- `src/screens/onboarding/CreateWalletScreen.tsx`.
- `src/services/safe-address.ts`, `src/modules/passkey/index.ts`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 1, 6, 48.
