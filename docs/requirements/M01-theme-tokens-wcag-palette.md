# M01 · Theme Tokens (Mutable, No-Remount) & WCAG Palette

| | |
|---|---|
| **Epic** | M — Design System & UI Primitives |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A04 |
| **Related** | M02, M03, M04, A05 |

## 1. Summary

The design system is built on **mutable design tokens** — `space` (4px grid), `text`, `radius`,
`color`, `shadow`, `motion` — that are **rebuilt in place** (gated by a `_styleVersion`) and **never
key-remounted**, so theme/scale changes don't flicker or lose state. The palette documents exact **WCAG
contrast** remediations (e.g. light `fg.muted #6E6B62` ≥ 4.5:1), with a single accent `#E8572A`. Design
language: **"depth through shadow, not glass"** — no BlurView anywhere.

## 2. Background & context

Remounting the tree on a theme change is the usual cause of flicker and lost state. Mutating tokens in
place (with a version gate) keeps switches instant and safe. WCAG-documented colors ensure legibility is
a designed property, not an accident.

## 3. Users & stories

- As a **user**, I want theme/scale changes to apply instantly without flicker, so that the app feels solid.
- As a **user with low vision**, I want WCAG-compliant contrast, so that text is legible.

## 4. Functional requirements

- **FR-1** — Provide tokens: `space` (4px grid), `text`, `radius`, `color`, `shadow`, `motion`.
- **FR-2** — Rebuild tokens **in place**, gated by `_styleVersion`; **never** key-remount the tree on theme/scale change.
- **FR-3** — Palette meets WCAG (documented remediations, e.g. `fg.muted #6E6B62` ≥ 4.5:1); single accent `#E8572A`.
- **FR-4** — Design language = shadow-based depth; **no BlurView / glass** anywhere.
- **FR-5** — Light/dark schemes supported via the platform-aware color scheme hook (A04).

## 5. Non-functional requirements

- **NFR-1** — Zero flicker / state loss on theme or text-scale change.
- **NFR-2** — Tokens are the single styling source; components don't hardcode colors/spacing.

## 6. UX / flow notes

Consumed by every screen/primitive (M03/M04). The 3s font-boot gate (A05) pairs with token init at startup.

## 7. Acceptance criteria

- [ ] **AC-1** — Switching theme updates colors in place with no remount/flicker.
- [ ] **AC-2** — Key palette pairs meet ≥ 4.5:1 contrast.
- [ ] **AC-3** — No BlurView is used anywhere in the app.

## 8. Out of scope / non-goals

- Text scale — **M02**; primitives — **M04**.

## 9. Dependencies, risks & open questions

- **Risk:** a component hardcoding a color bypasses WCAG review — enforce token usage.
- **Open question:** None.

## 10. Source anchors

- `src/constants/theme.ts`, `src/hooks/use-color-scheme.ts`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 84.
