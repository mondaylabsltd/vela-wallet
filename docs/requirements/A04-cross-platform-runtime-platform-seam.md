# A04 · Cross-Platform Runtime & Platform-Abstraction Seam

| | |
|---|---|
| **Epic** | A — Product Foundations & Cross-Cutting |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | — |
| **Related** | A05, A06, M01–M05, O03 |

## 1. Summary

Vela runs from **one React Native + Expo Router codebase** on iOS, Android, and the web. All
platform branching is funneled through **a single seam** (`platform.ts`) so screens never scatter
`Platform.OS` checks, and the web build degrades gracefully (in-app modal alerts, `navigator.clipboard`,
`window.open`). Web is the first-class, no-download surface; native apps are in device testing.

## 2. Background & context

Three platforms from one codebase only stays maintainable if platform differences live in exactly one
place. Scattering platform checks across screens is how cross-platform apps rot. The web target also
lacks native APIs (alerts, clipboard, haptics), so the seam must provide graceful fallbacks rather
than crashing or no-op-ing invisibly.

## 3. Users & stories

- As a **web user**, I want the full wallet in a browser with nothing to install, so that I can start immediately.
- As a **maintainer**, I want platform differences isolated, so that a change doesn't silently break one platform.

## 4. Functional requirements

- **FR-1** — `platform.ts` is the **one** module that branches on `Platform.OS`, exposing: alert, clipboard, openURL, in-app browser, app-active/foreground state.
- **FR-2** — Web fallbacks: alert → in-app modal (A05/M04 `AppAlert`); clipboard → `navigator.clipboard`; openURL → `window.open`.
- **FR-3** — Screens and services consume the seam, not raw platform APIs; new platform-specific behavior is added to the seam, not inline.
- **FR-4** — Platform-variant files (`*.web.tsx`) are limited to genuinely divergent UI (e.g. color scheme, animated icon) and stay in sync with their native counterparts.

## 5. Non-functional requirements

- **NFR-1** — No un-fallback-ed native API call reaches the web build (would throw at runtime).
- **NFR-2** — One codebase: iOS/Android/web share business logic; only presentation/seam differs.

## 6. UX / flow notes

Web haptics are no-ops (M04); web modals slide up as CSS portals (M04 `AppModal`). Keyboard avoidance differs per OS (ScreenContainer) while the Modal uses `behavior="padding"` on both.

## 7. Acceptance criteria

- [ ] **AC-1** — A grep for `Platform.OS` outside the seam (and sanctioned `.web` files) returns effectively nothing.
- [ ] **AC-2** — Clipboard/alert/openURL all work on web via fallbacks.
- [ ] **AC-3** — The same feature build runs on iOS, Android, and web without platform-specific business-logic forks.

## 8. Out of scope / non-goals

- The design tokens/primitives themselves — see **M01–M04**.

## 9. Dependencies, risks & open questions

- **Risk:** a new dependency assumes a native API; must be wrapped in the seam or guarded for web.
- **Open question:** None.

## 10. Source anchors

- `src/services/platform.ts` — the single platform seam.
- `src/hooks/use-color-scheme.web.ts`, `src/components/animated-icon.web.tsx` — sanctioned web variants.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 15, 82.
