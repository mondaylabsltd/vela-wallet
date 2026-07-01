# B09 · Recovery Model (OS Passkey Sync) & Honest Limits

| | |
|---|---|
| **Epic** | B — Identity: Passkeys & Account Model |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B01 |
| **Related** | B03, B04, C04, A02 |

## 1. Summary

Recovery is the **passkey synced by iCloud Keychain (Apple) or Google Password Manager (Android)** —
**no seed phrase, no social recovery, no guardians**. The honest, repeated caveat: lose **both** your
device **and** your cloud-synced passkey and the account is **unrecoverable by design**. A second
backup passkey is **not yet supported** (a signer-module constraint); the redundancy today is the OS
sync.

## 2. Background & context

Every recovery mechanism is a trade-off. Vela deliberately leans on the platform's mature,
hardware-backed passkey sync rather than inventing guardians or a browser-generated recovery key you
"just have to trust" (a Base Account limitation that motivated Vela — clue 94). The trade-off is
stated plainly, not hidden.

## 3. Users & stories

- As a **user who upgrades phones**, I want my synced passkey to restore my wallet, so that I don't need a written phrase.
- As a **user weighing the risk**, I want the failure mode stated honestly, so that I can decide with open eyes.

## 4. Functional requirements

- **FR-1** — On a new device, the OS-synced passkey (same rpId, B03) reproduces the same wallet (address derivation is deterministic, B07) — see restore flow C04.
- **FR-2** — No seed phrase, social recovery, or guardian mechanism exists or is implied anywhere.
- **FR-3** — Recovery copy states the honest limit: losing device **and** cloud passkey = unrecoverable by design.
- **FR-4** — Only **one passkey per wallet** is currently supported; document the OS sync as the backup.
- **FR-5** — The proxy extension (B04) is the escape hatch if the rpId domain itself is lost.

## 5. Non-functional requirements

- **NFR-1** — No false comfort: never imply Vela can restore an account it architecturally cannot (A02/A03).
- **NFR-2** — Guidance is consistent across all 15 locales (M05).

## 6. UX / flow notes

Onboarding (C01) and About/Settings explain recovery = OS passkey sync + its limit. No "write these 12 words" screen ever appears.

## 7. Acceptance criteria

- [ ] **AC-1** — A synced passkey on a fresh device restores the identical wallet address (C04).
- [ ] **AC-2** — No seed-phrase or guardian UI exists anywhere in the app.
- [ ] **AC-3** — Recovery copy includes the "unrecoverable if both lost" caveat.

## 8. Out of scope / non-goals

- The restore flow UI — **C04**; multi-passkey backup (not yet supported — roadmap dependent on signer module).

## 9. Dependencies, risks & open questions

- **Risk:** users may not understand passkey sync; copy must be concrete (iCloud Keychain / Google Password Manager).
- **Open question:** if/when a second backup passkey becomes possible, this PRD gains an FR.

## 10. Source anchors

- `getvela.app/src/content/docs/recovery.md`, `getvela.app/src/routes/+page.svelte:1191-1198` (one-passkey constraint).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 7, 31, 94.
