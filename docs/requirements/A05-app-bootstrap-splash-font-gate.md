# A05 · App Bootstrap, Splash & Font-Boot Gate

| | |
|---|---|
| **Epic** | A — Product Foundations & Cross-Cutting |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A04 |
| **Related** | M01, M02, C01 |

## 1. Summary

The app must **always boot**, even when a resource fetch hangs. The splash is held while fonts and
theme initialize, but a hung font fetch **releases the splash after 3000 ms** regardless — because "a
wallet must always boot." This guarantees the user is never trapped on a splash screen by a slow
network.

## 2. Background & context

Blocking startup on `Font.loadAsync` is a common way wallets hang on flaky networks. A wallet that
can't open is worse than one with a fallback font. The theme provider sits above a keyed `<Stack>` so
language switching (M05) is restart-free, which makes the boot sequence order-sensitive.

## 3. Users & stories

- As a **user on a bad connection**, I want the wallet to open within a few seconds no matter what, so that I'm never locked out.
- As a **user**, I want correct fonts/theme when they load promptly, so that the UI looks right on a normal launch.

## 4. Functional requirements

- **FR-1** — On launch, load Plus Jakarta Sans (weights 400/500/600/700) and initialize theme tokens (M01) before hiding the splash.
- **FR-2** — A **3000 ms timeout** force-releases the splash even if font loading has not resolved; the app renders with a system fallback font.
- **FR-3** — The i18n provider and theme provider mount above a keyed `<Stack>` so locale/theme changes don't remount app state.
- **FR-4** — First route resolves to onboarding (C01) or wallet home (D01) based on whether a wallet exists (A06).

## 5. Non-functional requirements

- **NFR-1** — Cold-start to interactive ≤ 3 s worst case (font timeout bound).
- **NFR-2** — No boot path can leave the splash visible indefinitely.

## 6. UX / flow notes

Splash → home/onboarding with no flicker (theme rebuilt in place, not remounted — M01). Mono font = Menlo/monospace.

## 7. Acceptance criteria

- [ ] **AC-1** — With font requests blocked, the app becomes interactive within ~3 s using a fallback font.
- [ ] **AC-2** — With fonts available, the app renders Plus Jakarta Sans and hides the splash once ready.
- [ ] **AC-3** — Switching language/theme after boot does not remount or lose screen state.

## 8. Out of scope / non-goals

- Theme token structure — see **M01**; text scaling — see **M02**.

## 9. Dependencies, risks & open questions

- **Risk:** the typeface export is still named `inter` though the font is Plus Jakarta Sans — do not "fix" by loading Inter.
- **Open question:** None.

## 10. Source anchors

- `src/app/_layout.tsx` — boot sequence, splash release, provider order.
- `src/constants/theme.ts` — font family (named `inter`, is Plus Jakarta Sans).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 83.
