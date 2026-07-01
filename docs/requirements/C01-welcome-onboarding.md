# C01 · Welcome & Value-Prop Onboarding

| | |
|---|---|
| **Epic** | C — Onboarding & Wallet Lifecycle |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A01 |
| **Related** | C02, B01, B09, A02 |

## 1. Summary

A short, honest onboarding that communicates the value proposition ("Your keys. Your face." — no seed
phrase) and sets accurate expectations about recovery (B09) before the user creates a wallet (C02). It
does **not** contain a seed-phrase step and does **not** show a scary "beta" banner (A02).

## 2. Background & context

First impressions decide trust. Vela's onboarding must land the seedless thesis quickly and be candid
about the recovery trade-off, so users adopt with informed confidence rather than discovering limits
later.

## 3. Users & stories

- As a **first-time user**, I want to understand what makes Vela different in a few screens, so that I know why I'd use it.
- As a **cautious user**, I want the recovery model explained up front, so that I'm not surprised later.

## 4. Functional requirements

- **FR-1** — Present the core value props: no seed phrase, biometric signing, same address everywhere, open source.
- **FR-2** — State the recovery model honestly (OS passkey sync + its limit — B09) before wallet creation.
- **FR-3** — Provide a clear primary CTA into Create Wallet (C02).
- **FR-4** — No seed-phrase, mnemonic, or "write this down" step exists.
- **FR-5** — Route users who already have a wallet straight to home (D01), skipping onboarding.

## 5. Non-functional requirements

- **NFR-1** — Localized across all 15 locales (M05); honesty framing preserved (A02).
- **NFR-2** — Fast, skippable where appropriate; never blocks a returning user.

## 6. UX / flow notes

Welcome → value props → recovery note → Create Wallet. Uses the design system (M01–M04). Copy uses approved taglines verbatim (A02).

## 7. Acceptance criteria

- [ ] **AC-1** — A new user can reach Create Wallet from onboarding in a few taps.
- [ ] **AC-2** — Recovery limits are shown before wallet creation.
- [ ] **AC-3** — No seed-phrase step appears; no fear banner appears.

## 8. Out of scope / non-goals

- Wallet creation mechanics — **C02**; recovery mechanics — **B09**.

## 9. Dependencies, risks & open questions

- **Risk:** over-long onboarding hurts conversion; keep it tight.
- **Open question:** None.

## 10. Source anchors

- `src/screens/onboarding/WelcomeScreen.tsx`, `src/screens/onboarding/OnboardingScreen.tsx`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 1, 2, 7.
