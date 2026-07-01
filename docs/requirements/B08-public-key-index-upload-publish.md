# B08 · Public-Key Index: Upload + On-Chain Publish (Gnosis)

| | |
|---|---|
| **Epic** | B — Identity: Passkeys & Account Model |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B07 |
| **Related** | B01, C03, A03, N02 |

## 1. Summary

After a wallet is created, its **passkey public key** (and chosen account name) are uploaded to the
**Passkey Index** service, which stores them **on Gnosis Chain** and **signs the storing transaction
itself** (no client challenge/signature needed). Upload is **idempotent, verify-after-write**, uses an
`Idempotency-Key`, and silently retries on launch with **no biometric prompt**. This powers the
"wallets created" counter and address→name resolution (H02).

## 2. Background & context

Publishing the public key on-chain lets anyone verify the wallet exists and resolve its name, aligning
with "don't trust — verify" (clue 16). It's the *only* server-side data about a user (A03), and it's
public by design. The server signs the tx because the client has nothing to prove beyond owning the
key it just generated.

## 3. Users & stories

- As a **user**, I want my account discoverable/verifiable on-chain, so that others can resolve my name and the network can prove the wallet count.
- As a **user**, I want the upload to just work and retry quietly, so that I'm never nagged for a signature to publish a public key.

## 4. Functional requirements

- **FR-1** — Upload the public key + account name to the Passkey Index with an `Idempotency-Key`.
- **FR-2** — **Verify-after-write**: the server record is source of truth and is checked **byte-for-byte** against what was sent.
- **FR-3** — On app launch, **silently retry** any pending upload with **no biometric prompt**.
- **FR-4** — The Passkey Index stores keys on **Gnosis Chain** and **signs the on-chain tx itself** (commit-reveal via a CF Worker + D1 + Durable Object queue — backend lives in a separate repo).
- **FR-5** — Uploads are idempotent: repeating is a no-op, not a duplicate.

## 5. Non-functional requirements

- **NFR-1** — Only the public key + name are ever sent — no private key, no PII (A03).
- **NFR-2** — Endpoint is self-hostable / configurable (N02); default `p256-index.getvela.app`.

## 6. UX / flow notes

No blocking UI — publish happens in the background; account naming (C03) feeds the name field. The homepage's live on-chain "wallets created" counter reads this index on Gnosis.

## 7. Acceptance criteria

- [ ] **AC-1** — A newly created wallet's public key + name appear in the index (verify-after-write passes).
- [ ] **AC-2** — Re-running upload does not create a duplicate (idempotent).
- [ ] **AC-3** — A pending upload retries on next launch without a biometric prompt.

## 8. Out of scope / non-goals

- Address derivation — **B07**; name-resolution on send — **H02**; the backend implementation (separate repo).

## 9. Dependencies, risks & open questions

- **Risk:** cross-repo coupling with the Passkey Index backend; endpoint/health contract must stay in sync (N02).
- **Open question:** None.

## 10. Source anchors

- `src/services/public-key-upload.ts:69-147` — idempotent verify-after-write + silent retry.
- `src/services/public-key-index.ts:1-6` — Gnosis index, server-signed.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 16, 49; memory `reference_pubkey_index_backend`.
