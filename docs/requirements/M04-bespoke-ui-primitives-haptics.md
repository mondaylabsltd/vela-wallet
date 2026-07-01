# M04 · Bespoke UI Primitives & Semantic Haptics

| | |
|---|---|
| **Epic** | M — Design System & UI Primitives |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | M01 |
| **Related** | M03, A04, D01, H09 |

## 1. Summary

Vela ships **bespoke "big-tech" UI primitives** built to work identically on Expo web: `SlideToConfirmButton`,
`VelaRefresh` (branded pull-to-refresh that "draws" an accent arc with the pull), `WaveDock` (home
action bar with a concave wave cradle + emerging Scan FAB), and `AppModal` (three implementations:
native pageSheet on iOS, custom drag-to-dismiss on Android, CSS slide-up portal on web). **Five semantic
haptics** fire on key moments (no-op on web).

## 2. Background & context

Native default components feel generic; the founder wants premium, branded feedback (gesture + haptic +
animation) that reads as a top-tier app. Building these as cross-platform primitives (A04) keeps the
experience consistent everywhere, including web where haptics degrade to no-ops.

## 3. Users & stories

- As a **user**, I want polished, responsive controls, so that the wallet feels premium and trustworthy.
- As a **user**, I want tactile confirmation on important actions, so that signing/refresh feel deliberate.

## 4. Functional requirements

- **FR-1** — `SlideToConfirmButton` for high-consequence confirmations (precedes biometric, B02).
- **FR-2** — `VelaRefresh` branded pull-to-refresh drawing an accent arc with the pull gesture.
- **FR-3** — `WaveDock` home action bar with a concave wave cradle and an emerging Scan FAB (H04).
- **FR-4** — `AppModal` with **three** platform implementations: iOS pageSheet, Android drag-to-dismiss, web CSS slide-up portal (A04).
- **FR-5** — **Five semantic haptics** on key moments; **no-op on web**.

## 5. Non-functional requirements

- **NFR-1** — Identical behavior across iOS/Android/web via PanResponder/Reanimated (A04).
- **NFR-2** — Haptics degrade gracefully (silent no-op) where unavailable.

## 6. UX / flow notes

Modal keyboard avoidance uses `behavior="padding"` on both platforms; ScreenContainer differs per OS. Copy-feedback + haptics power the receive/copy flows (H09).

## 7. Acceptance criteria

- [ ] **AC-1** — Slide-to-confirm, VelaRefresh, WaveDock, and AppModal all work on iOS, Android, and web.
- [ ] **AC-2** — Haptics fire on native and no-op on web without errors.
- [ ] **AC-3** — AppModal uses the correct per-platform implementation.

## 8. Out of scope / non-goals

- Tokens — **M01**; amount display — **M03**; platform seam — **A04**.

## 9. Dependencies, risks & open questions

- **Risk:** gesture parity across platforms is delicate — keep the shared primitives in sync.
- **Open question:** None.

## 10. Source anchors

- `src/components/ui/` — `SlideToConfirmButton.tsx`, `VelaRefresh.tsx`, `WaveDock.tsx`, `AppModal.tsx`; `src/hooks/use-copy-feedback.ts`.
- memory `feedback_premium_custom_ux`, `feedback_keyboard_avoidance`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 85.
