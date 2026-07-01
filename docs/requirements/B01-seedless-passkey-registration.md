# B01 · Seedless Thesis & WebAuthn P-256 Passkey Registration

| | |
|---|---|
| **Epic** | B — Identity: Passkeys & Account Model |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A04 |
| **Related** | B02, B03, B05, B06, B07, B09, C02 |

## 1. Summary

Each Vela wallet is controlled by a **WebAuthn passkey (P-256)** held in the device secure enclave and
unlocked by Face ID / Touch ID / fingerprint. There is **no seed phrase** — "there is no secret you
can type, so there is nothing to phish." Registration creates the passkey, extracts its public key,
and hands it to address derivation (B07). This is the founding thesis of the product.

## 2. Background & context

Seed phrases are the dominant self-custody footgun (lost, phished, screenshotted). WebAuthn moves the
secret into OS-managed hardware the user already trusts to unlock their phone. The private key never
exists in app memory — Vela only ever sees the public key.

## 3. Users & stories

- As a **new user**, I want to create a wallet with my face/fingerprint and no phrase to write down, so that there's nothing to lose or leak.
- As a **security-minded user**, I want the signing key in hardware I control, so that a compromised app can't exfiltrate it.

## 4. Functional requirements

- **FR-1** — Registration calls WebAuthn `create()` bound to the resolved rpId (B03), producing a platform passkey (secure enclave / synced credential).
- **FR-2** — Parse the attestation to extract the **COSE P-256 public key** (`{1:2, 3:-7, -1:1, -2:x, -3:y}`) via the CBOR/attestation parser (B06).
- **FR-3** — Validate the passkey provider is compatible **before** persisting anything (B05); reject incompatible providers non-retryably.
- **FR-4** — The extracted (x, y) feed deterministic address derivation (B07) and public-key upload (B08).
- **FR-5** — The private key is **never** read, stored, or transmitted — only the public key leaves the WebAuthn boundary.

## 5. Non-functional requirements

- **NFR-1** — Works identically across iOS, Android, and web (via rpId resolution B03 + proxy extension B04 on non-canonical domains).
- **NFR-2** — Registration is a one-time, biometric-gated action; failure is recoverable (retry) except for incompatibility (B05).

## 6. UX / flow notes

Entered from Create Wallet (C02). Copy avoids "backup your phrase" language entirely; recovery framing is the OS passkey sync (B09).

## 7. Acceptance criteria

- [ ] **AC-1** — A successful registration yields a valid P-256 public key and a derived address (B07).
- [ ] **AC-2** — The app never holds or logs the private key.
- [ ] **AC-3** — An incompatible provider is rejected before any state is saved (B05).

## 8. Out of scope / non-goals

- Address derivation math — **B07**. Public-key upload — **B08**. Per-tx signing — **B02/G02**.

## 9. Dependencies, risks & open questions

- **Risk:** platform passkey UX varies; non-canonical domains need the proxy extension (B04).
- **Open question:** second/backup passkey per wallet is **not yet supported** (signer-module constraint) — see B09.

## 10. Source anchors

- `src/modules/passkey/index.ts` — WebAuthn registration + rpId.
- `src/services/attestation-parser.ts:23-45` — COSE key extraction; `src/services/webauthn-verify.ts:29-65` — compatibility check.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 1, 3, 46, 48.
