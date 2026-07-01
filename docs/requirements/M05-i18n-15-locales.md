# M05 · i18n: 15 Locales, Typed Keys, Restart-Free Switch

| | |
|---|---|
| **Epic** | M — Design System & UI Primitives |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A04 |
| **Related** | M02, E06, A02 |

## 1. Summary

The UI ships in **15 locales** (~857 keys, near-100% parity, type-checked): en, zh, zh-TW, zh-HK, ja,
ko, vi, id, tr, ru, es-MX, pt-BR, fr, it, de. **zh / zh-TW / zh-HK are maintained as genuinely
distinct** (zh-HK is spoken Cantonese, not just Traditional glyphs). Keys are **TypeScript-typed against
English** (`typeof en`), so a bad `t()` key is a **compile error**. Language switching is **restart-free**
(the provider sits above a keyed `<Stack>`).

## 2. Background & context

A wallet used globally must feel native in each language, and honesty framing (A02) must survive
translation. Typing keys against English prevents drift and missing-key bugs at build time. Distinct
Chinese variants respect real linguistic differences rather than faking them with glyph conversion.

## 3. Users & stories

- As a **non-English user**, I want the whole app in my language, so that I can use it confidently.
- As a **Cantonese speaker**, I want zh-HK to read naturally, so that it's not just Traditional Mandarin.

## 4. Functional requirements

- **FR-1** — Provide 15 locales with ~857 keys at near-100% parity; each namespace wired in `resources.ts`.
- **FR-2** — Type keys against English (`typeof en`) so an invalid `t()` key fails compilation.
- **FR-3** — Maintain zh / zh-TW / zh-HK as **distinct** (zh-HK = spoken Cantonese).
- **FR-4** — Language switching is **restart-free** (provider above a keyed `<Stack>`, A05).
- **FR-5** — No ICU/plural syntax reliance (Hermes limitation, M02); avoid translator-note leakage; machine-translated strings flagged for human review.

## 5. Non-functional requirements

- **NFR-1** — i18n key depth ≤ 3 segments (a signing-sheet gotcha, I01).
- **NFR-2** — Honesty/audit framing (A02) preserved, not softened, across locales.

## 6. UX / flow notes

Language picker in settings; switching applies instantly. Long translations must fit worst-case text scale (M02).

## 7. Acceptance criteria

- [ ] **AC-1** — All 15 locales render the app with near-complete coverage.
- [ ] **AC-2** — A misspelled `t()` key fails the type-check/build.
- [ ] **AC-3** — Switching language updates the UI without a restart.

## 8. Out of scope / non-goals

- Number/date formatting — **M02**; currency — **E06**; cross-device language sync (🔜, A06).

## 9. Dependencies, risks & open questions

- **Risk:** machine-translated strings need human eyeball; keep a review pass.
- **Open question:** adding further locales.

## 10. Source anchors

- `src/i18n/resources.ts`, `src/i18n/` locales, `i18next.d.ts`.
- memory `project_i18n_localization`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 81.
