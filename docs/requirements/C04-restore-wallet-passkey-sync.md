# C04 · Restore Existing Wallet via Passkey Sync

| | |
|---|---|
| **Epic** | C — Onboarding & Wallet Lifecycle |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B09 |
| **Related** | B01, B03, B07, A06 |

## 1. Summary

On a new device (or reinstall), a user restores their wallet by **authenticating with the OS-synced
passkey** — no seed phrase, no import file. Because address derivation is deterministic (B07) and the
rpId is consistent (B03), the same passkey reproduces the **identical address**. Local settings that
aren't yet cloud-synced are re-derived or re-entered (a saved address book / full-setup sync is a 🔜
roadmap item).

## 2. Background & context

Restore is the payoff of the seedless model: the "backup" is the platform passkey sync the user
already relies on. The account itself needs nothing but the passkey; app-local state (A06) is
secondary and, for now, device-local.

## 3. Users & stories

- As a **user on a new phone**, I want to sign in with my synced passkey and see my wallet, so that migration is effortless.
- As a **user**, I want the restored address to be exactly my old one, so that my funds are there.

## 4. Functional requirements

- **FR-1** — Offer a "use existing passkey" path that runs WebAuthn `get()` (B01/B03) and derives the address (B07).
- **FR-2** — The restored address must equal the original (deterministic derivation) — no per-device drift.
- **FR-3** — Rebuild balances/tokens/activity from chain data (D02/D08) after restore; local caches (A06) repopulate.
- **FR-4** — If the domain/rpId is unavailable, the proxy extension (B04) is the documented fallback.

## 5. Non-functional requirements

- **NFR-1** — No seed phrase or import file is ever requested.
- **NFR-2** — Restore works across the 15 locales (M05).

## 6. UX / flow notes

Onboarding offers "Create" vs "I already have a Vela wallet." The latter authenticates and lands on home. Cross-device sync of language/currency/address-book is 🔜 (tracked in A06/E06/H05).

## 7. Acceptance criteria

- [ ] **AC-1** — Authenticating with a synced passkey restores the identical wallet address.
- [ ] **AC-2** — Balances/activity repopulate from chain after restore.
- [ ] **AC-3** — No seed-phrase/import-file step exists.

## 8. Out of scope / non-goals

- Full cross-device settings sync — roadmap 🔜 (A06/E06/H05).
- Recovery limits — **B09**.

## 9. Dependencies, risks & open questions

- **Risk:** if the passkey isn't synced (user disabled iCloud/Google sync), restore fails — B09 states this honestly.
- **Open question:** scope of the eventual one-tap full-setup restore.

## 10. Source anchors

- `src/modules/passkey/index.ts`, `src/services/safe-address.ts` — auth + deterministic derivation.
- `getvela.app/src/routes/roadmap/+page.svelte` — "Sync across all your devices" (🔜).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 6, 7, 31.
