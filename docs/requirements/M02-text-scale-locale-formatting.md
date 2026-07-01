# M02 · 6-Level Text Scale & Intl-Free Locale Formatting

| | |
|---|---|
| **Epic** | M — Design System & UI Primitives |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | M01 |
| **Related** | M03, E06, M05 |

## 1. Summary

Two related capabilities: a **6-level text scale (0.82×–1.35×)** for accessibility, and **Intl-free
locale formatting** that works around Hermes' incomplete ICU. Explicit presets cover **4 number formats**
(`comma_dot`, `dot_comma`, `space_comma`, `indian` — real lakh/crore grouping), **5 date** and **2 time**
formats, each with an `auto` device-detection path. Compact suffixes use universal **K/M/B/T** (CJK
myriad avoided on purpose).

## 2. Background & context

React Native's Hermes engine ships incomplete `Intl`, so relying on `Intl.NumberFormat`/`DateTimeFormat`
is unreliable across locales/devices. Explicit presets make formatting deterministic. A generous text
scale makes the wallet usable for low-vision users without breaking layouts (M01 tokens adapt).

## 3. Users & stories

- As a **low-vision user**, I want to scale text up, so that I can read balances and confirmations.
- As an **international user**, I want numbers/dates in my locale's convention, so that values read naturally.

## 4. Functional requirements

- **FR-1** — Provide a **6-level text scale** from **0.82× to 1.35×**; layouts (M01 tokens) adapt without truncation.
- **FR-2** — Number formats: `comma_dot`, `dot_comma`, `space_comma`, `indian` (lakh/crore); plus an `auto` device path.
- **FR-3** — Date (5) and time (2) presets, each with `auto` detection.
- **FR-4** — Compact notation uses universal **K/M/B/T**; do **not** use CJK myriad grouping.
- **FR-5** — Formatting is **Intl-free** (no reliance on Hermes ICU).

## 5. Non-functional requirements

- **NFR-1** — Deterministic across devices/locales (no ICU dependency).
- **NFR-2** — Feeds currency display rules (E06) and atomic display (M03).

## 6. UX / flow notes

Text scale is a settings control; changes apply in place (M01, no remount). Number/date presets pair with the currency selection (E06).

## 7. Acceptance criteria

- [ ] **AC-1** — Setting scale to max (1.35×) enlarges text without breaking layouts.
- [ ] **AC-2** — Indian grouping renders lakh/crore correctly.
- [ ] **AC-3** — Formatting is correct on a device with incomplete ICU (no `Intl` reliance).

## 8. Out of scope / non-goals

- Currency-specific rules — **E06**; atomic display — **M03**; translations — **M05**.

## 9. Dependencies, risks & open questions

- **Risk:** extreme scale + long translations (M05) can crowd layouts — test worst-case locales.
- **Open question:** cross-device sync of scale/format prefs (🔜, A06).

## 10. Source anchors

- `src/constants/text-scale.ts`, `src/services/locale-format.ts`.
- `docs/text-scale-architecture.md`, `docs/localization.md`; `docs/CONTENT-SOURCE-100-CLUES.md` — clues 80, 84.
