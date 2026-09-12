# Data model — what state 038 adds, and where

Rules live in `vela-core`; shells add storage keys and view fields only.

## Core (`rust/crates/vela-core`)

| Where | Field / variant | Type | Meaning |
| --- | --- | --- | --- |
| `balance_dashboard::Model` | `last_settled_total` | `Option<f64>` | The figure shown while `pending_pulls > 0`. Set at `FetchSettled` (complete or partial), seeded from `cached_total`. |
| `BalanceView` | `unreachable` | `bool` | `FetchErrored` landed with no tokens and no cache — "could not look", never $0. |
| `BalanceNotice::Unpriced` | (rule) | — | Raised only when `pending_pulls == 0`. |
| `login::LoginView` | `transport_failed` | `bool` | The shell could not get OUT of the machine (every candidate refused). Distinct from `endpoint_unreachable` (a route existed; the service was not there). |
| `rpc_pool::Event` | `ConnectivityChanged { online: bool }` | event | `online: true` clears cooldowns and temp bans so a reconnect is not served by a stale ban map. |
| `FeeEstimateView` | `fallback` | `bool` | The estimate used defaults; shells label the fee line. |

State transitions (balance):

```
refresh start ── pending_pulls += 1 ──► display = last_settled_total ?? cached
chain arrives ── tokens merged ────────► display unchanged
settle ────────── pending_pulls -= 1 ──► last_settled_total = live; display = live; notice may raise
errored ───────── bootstrapped = true ─► unreachable = tokens.empty && cached.none
```

## Desktop (`app-desktop/vela-wallet`)

| Where | Key / field | Type | Meaning |
| --- | --- | --- | --- |
| `storage.rs` | `vela.intro.seen` | epoch ms | Written when the intro is left (skip, create, sign in). Same name as the web. |
| `storage.rs` | `vela.launch.played` | epoch ms | Written when the launch animation finishes or is skipped; replays after `LAUNCH_REPLAY_AFTER_MS = 604_800_000`. Same name and number as the web. |
| `OnboardingPage` | `intro: Option<IntroState>` | `{ index: usize, drag_px: f32, dragging: bool }` | The three-slide state; `None` once left. |
| `OnboardingPage` | `endpoint_dismissed` | `bool` | Set by Close; cleared by a re-probe (`Event::Start`). The automatic open requires it false. |
| `proxy.rs` | `CandidateState` | `{ order: [System, Env, Direct], current: usize, last_failure: Option<Instant> }` | Re-derived after any transport failure; never process-lifetime. |
| `theme.rs` | `font_ui()` | `&'static str` | `"Plus Jakarta Sans"`, applied at each page root after `add_fonts`. |

## Web (`app-web/vela-wallet`)

| Where | Key / attribute | Meaning |
| --- | --- | --- |
| `app.html` → `<html data-intro="pending">` | set before paint when `vela.intro.seen` is absent (or `?intro`), unless `?skipIntro` | Hides `[data-intro-page]` until Svelte mounts the intro. Contract: [contracts/pre-paint-attributes.md](contracts/pre-paint-attributes.md). |
| `+layout.svelte` | `online` / `offline` listeners | Dispatch `ConnectivityChanged`; render the offline line. |
| `vite.config.ts` kit options | `version: { pollInterval: 60_000 }` | With `beforeNavigate` + `updated.current` → hard reload; `vite:preloadError` → reload. |

## Removed

- `ui/rail.rs`: `settings_label`, `on_settings` parameters and the
  `rail-settings` element.
- `$lib/assets/favicon.svg` and its `<link>` in `+layout.svelte`.
