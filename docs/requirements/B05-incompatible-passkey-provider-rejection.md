# B05 · Incompatible Passkey Provider Rejection

| | |
|---|---|
| **Epic** | B — Identity: Passkeys & Account Model |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B01 |
| **Related** | B06, G02 |

## 1. Summary

Some passkey providers reorder the `clientDataJSON` fields, which breaks the on-chain WebAuthn
verifier's assumptions (G02). Vela **detects these at registration** with a strict prefix check and
rejects them with a non-retryable `PasskeyIncompatibleError` **before anything is saved** — so a user
can never end up with an unusable wallet. The code names **Xiaomi Password Manager** as such a
provider.

## 2. Background & context

The verifier contract templates the `clientDataJSON` and extracts only trailing fields (G02/clue 44).
A provider that reorders those fields produces signatures the contract can't validate. Catching this
up-front avoids the far worse outcome of a wallet that receives funds but can never sign.

## 3. Users & stories

- As a **user with an incompatible provider**, I want to be told clearly at setup, so that I don't create a wallet I can't use.
- As a **security engineer**, I want incompatibility to fail *before* persistence, so that there's no half-created account.

## 4. Functional requirements

- **FR-1** — During registration (B01), verify the `clientDataJSON` structure with a strict prefix check.
- **FR-2** — If fields are reordered (e.g. Xiaomi Password Manager), throw `PasskeyIncompatibleError` — **non-retryable**.
- **FR-3** — No public key, address, or account record is written when this error is thrown.
- **FR-4** — The error surfaces user-facing guidance to use a compatible provider (platform passkey / iCloud Keychain / Google Password Manager).

## 5. Non-functional requirements

- **NFR-1** — Detection is deterministic and offline (pure structural check).
- **NFR-2** — Consistent across platforms.

## 6. UX / flow notes

Registration failure screen explains the provider is incompatible and suggests the OS default passkey. Non-retryable (retrying the same provider won't help).

## 7. Acceptance criteria

- [ ] **AC-1** — A reordered-clientDataJSON provider is rejected with `PasskeyIncompatibleError`.
- [ ] **AC-2** — No account state exists after rejection.
- [ ] **AC-3** — A compatible provider registers normally.

## 8. Out of scope / non-goals

- Signature field extraction — **G02**.

## 9. Dependencies, risks & open questions

- **Risk:** new incompatible providers may appear; keep the detection list/logic current.
- **Open question:** whether to allow a "sign anyway" for advanced users (currently no — safety first).

## 10. Source anchors

- `src/services/public-key-upload.ts:36-59`, `src/services/webauthn-verify.ts:29-65` — compatibility prefix check + rejection.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 48.
