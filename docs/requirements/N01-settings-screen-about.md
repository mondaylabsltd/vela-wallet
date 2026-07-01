# N01 · Settings Screen & About

| | |
|---|---|
| **Epic** | N — Settings, Self-Host & Diagnostics |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A01 |
| **Related** | N02, N04, E06, M02, M05, O04 |

## 1. Summary

A single Settings surface aggregates user-facing configuration — display currency (E06), language
(M05), text scale (M02), RPC/service endpoints (N02), a quiet **Feedback** row (N04) — plus an **About**
screen carrying the honest project facts (alpha status, audit posture, open-source, founder/entity —
A02/O04). No scary banners; trust is conveyed through candor.

## 2. Background & context

Settings is where the product's honesty stance becomes tangible: advanced self-host options are
available but unobtrusive, and About states the real story (solo founder, MONDAY LABS LTD, MIT license)
rather than marketing gloss.

## 3. Users & stories

- As a **user**, I want one place to set currency, language, and scale, so that I can tailor the app.
- As a **cautious user**, I want an honest About with the real status and audit posture, so that I can trust the team.

## 4. Functional requirements

- **FR-1** — Expose currency (E06), language (M05), and text scale (M02) controls.
- **FR-2** — Provide an **Advanced → Service Endpoints** entry (N02) for self-hosting.
- **FR-3** — Provide a quiet **Feedback** row → prefilled bug report (N04), not a nag.
- **FR-4** — **About** shows alpha status, audit posture (A02), open-source/MIT + repo, founder & legal entity (O04).
- **FR-5** — No "beta / tolerate bugs" banners anywhere (A02).

## 5. Non-functional requirements

- **NFR-1** — All copy localized (M05) and honesty-preserving (A02).
- **NFR-2** — Advanced options are discoverable but not intrusive.

## 6. UX / flow notes

`SettingsScreen` + `AboutScreen`. Feedback row is understated (feedback stance). Build info surfaces the version/commit (`build-info.ts`).

## 7. Acceptance criteria

- [ ] **AC-1** — Currency/language/scale changes apply and persist (A06).
- [ ] **AC-2** — About shows the mandated alpha/audit framing (A02).
- [ ] **AC-3** — The Feedback row opens the prefilled report path (N04).

## 8. Out of scope / non-goals

- Endpoint validation — **N02**; bug-report internals — **N04**; roadmap/status page — **O04**.

## 9. Dependencies, risks & open questions

- **Risk:** About copy drifting from mandated phrasing — review against A02.
- **Open question:** None.

## 10. Source anchors

- `src/screens/settings/SettingsScreen.tsx`, `src/screens/settings/AboutScreen.tsx`, `src/constants/build-info.ts`.
- memory `feedback_beta_disclaimer_stance`; `docs/CONTENT-SOURCE-100-CLUES.md` — clues 9, 93.
