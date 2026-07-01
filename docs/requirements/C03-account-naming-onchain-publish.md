# C03 · Account Naming & On-Chain Name Publish

| | |
|---|---|
| **Epic** | C — Onboarding & Wallet Lifecycle |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B08 |
| **Related** | C02, C05, H02 |

## 1. Summary

A user can give an account a **human-readable name**, which is published on-chain alongside the public
key via the Passkey Index (B08) on Gnosis. That name becomes resolvable by others (feeding recipient
identity resolution, H02) and labels the account in the switcher (C05).

## 2. Background & context

Addresses are unmemorable; a chosen name makes accounts legible to the owner and resolvable to
counterparties. Because the name lives in the same on-chain index as the public key, it is public by
design — consistent with A03's "only the public key + name are stored server-side."

## 3. Users & stories

- As a **user**, I want to name my account, so that I recognize it and others can resolve it to a name.
- As a **user with several accounts**, I want distinct names, so that the switcher (C05) is legible.

## 4. Functional requirements

- **FR-1** — Let the user set/edit an account name during or after creation (C02).
- **FR-2** — Publish the name via the Passkey Index upload (B08), idempotently.
- **FR-3** — The name appears in the account switcher (C05) and can resolve via the Vela Passkey Index in H02 (first-priority name source).
- **FR-4** — Name changes re-publish (idempotent verify-after-write) without a biometric prompt (B08).

## 5. Non-functional requirements

- **NFR-1** — Name is public by design; UI states this clearly (A03).
- **NFR-2** — Reasonable length/charset limits to keep on-chain storage bounded.

## 6. UX / flow notes

Naming is optional and low-friction; skippable at creation. Copy notes the name is published on-chain (not private).

## 7. Acceptance criteria

- [ ] **AC-1** — A set name is published to the index and resolvable via H02.
- [ ] **AC-2** — Editing the name re-publishes without a biometric prompt.
- [ ] **AC-3** — The name labels the account in the switcher.

## 8. Out of scope / non-goals

- Upload mechanics — **B08**; name resolution on send — **H02**.

## 9. Dependencies, risks & open questions

- **Risk:** name squatting/impersonation in a public index — H02 uses priority ordering and does not treat a name as proof of identity.
- **Open question:** name uniqueness policy (currently not enforced as unique).

## 10. Source anchors

- `src/services/public-key-upload.ts`, `src/services/public-key-index.ts`.
- `src/services/recipient-identity.ts` — Vela Passkey Index as first-priority resolver.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 26, 30, 49.
