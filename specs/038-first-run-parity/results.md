# 038 — results

## Baseline (T003, 2026-09-11, branch off `main` at 047ca320)

| Suite | Result |
| --- | --- |
| `cargo test -p vela-core --features i18n-all,crux` | core baseline: passed=1295 failed=0 ignored=0 |
| `cargo test --features dev-fixtures` (app-desktop) | 405 tests: 367 passed, 0 failed, 38 ignored (real network / hardware) |
| `pnpm test` (app-web, server + browser projects) | 70 files: 68 passed, 2 failed — 925 tests: 923 passed, 2 failed |

The two web failures are PRE-EXISTING on `main` and not this feature's:
`explore/fixtures.test.ts` expects 7 phone explore states and finds 8;
`signing/fixtures.test.ts` expects 33 scenarios and finds 35. Both are
inventory pins that a later cut added to without moving the pin. Left alone
here; noted for whoever owns explore/signing.

## Landed

### P1 slice — US1, US2, US6 (2026-09-11)

**US1 web first run** — `app.html` decides the intro before paint
(`data-intro="pending"`, contract `contracts/pre-paint-attributes.md`);
`app.css` hides `[data-intro-page]`; `+page.svelte` clears the attribute in
`onMount`; `IntroCarousel.svelte` renders `OnboardingRail` and the flow column
at ≥1280 (buttons at label width, row on the last slide), phone rules
untouched; `gate.test.ts` reads `app.html` and pins the key and both params
(4 new cases); e2e cases added to `welcome-layout` (rail at 1280/1440, phone
below, side-by-side CTAs) and `welcome-ssr` (no Welcome frame on a first run;
prerendered HTML still the landing page) — e2e NOT yet run (needs the isolated
build per `reference_concurrent_sessions`).

**US2 desktop front door** — `ui/rail.rs` has no settings affordance
(signature `onboarding_rail(theme, slot)`); `OnboardingPage.endpoint_dismissed`
recorded by Close, cleared by a re-probe, read by the pure
`endpoint_should_auto_open` (3 tests); the endpoint card is rendered UNDER the
two ways in, no scrim; `WebSocketCablePort.closed_by_peer` refuses writes after
a CLOSE; `assert_over` / `register_hybrid` take the touch card down on EVERY
exit and only say goodbye after a success; a peer close is reported as
`Cancelled` ("your phone ended the session before signing") — `hybrid_teardown`
test drives it over a fake cable.

**US6 network path** — `proxy.rs` holds a candidate chain (system → env →
direct, duplicates collapsed) re-derived after any failure and every 60 s;
macOS reads `scutil --proxy` (HTTPS → HTTP → SOCKS-as-socks5h, exceptions →
no_proxy; parser tested on the captured dictionary); `with_candidates` retries
a transport failure on the next route and reports `Transport { local }`;
`registry.rs` and `relay.rs::rest_get` use it; `RegistryError.local`;
`probe_health() -> Probe::{Reachable, Down, Local}`; core gains
`ShellResult::IndexTransportFailed` (additive) and `LoginView.transport_failed`
(`app_login` test); desktop Welcome shows `onboarding.common.networkBody` and
withholds the endpoint card in that state; web serves and renders the same key.
Generated TS types regenerated (`gen-core-types.mjs onboarding`).

Test counts: desktop 367 → 376 passed; core +1 (`app_login` 25); web i18n +
intro 73 passed.

Deviations / left open in this slice: `pool.rs`, `chainlink.rs`, `chain.rs`
still call `proxy::agent` (which now reads the current candidate but does not
advance on failure) — T028 partial; the caBLE tunnel dial (T033) not moved;
`failed_inside_this_machine` recognises resolver failures by libc's sentence
(three desktops' words listed in `resolver_said_no`) because `std` surfaces
`getaddrinfo` as an `Uncategorized` io error, not `HostNotFound`.

### Slice 2 — typeface, marks, launch gate, desktop intro (commits 1ee5256e, 44a069bf)

See the two commit messages; desktop 376 → 387.

### Slice 3 — stability (core #188 / unreachable, panic sheet, web offline & version)

**Core** — `balance_dashboard.rs`: `fetch_in_flight` + `last_settled_total`;
the FIGURE holds at the last settle while chains stream and moves once, at
settle; `BalanceNotice::Unpriced` is raised only at rest; `BalanceView.unreachable`
when the first fetch errored with no tokens and no cache. The existing
`slow_chains_keep_their_last_value_mid_refresh` test asserted the old climbing
total and was rewritten to the new rule (the LIST still streams); two new
tests. Bindings regenerated (`gen-core-types.mjs wallet-state`).

**Desktop** — `panic_report.rs`: process hook + mailbox; `resident.rs::pump`
runs every blocking/streaming work inside `catch_unwind` (a panic leaves the
operation unresolved and the screen in its last state); both pages raise the
report as the ordinary failure sheet ("Something went wrong", details,
Report). `VELA_TEST_PANIC=1` panics once inside the next background work.
Home: `unreachable` → the network sentence over the skeleton.

**Web** — `+layout.svelte`: `online`/`offline` listeners (offline line;
reconnect calls `invalidateAllPools()`), `beforeNavigate` + `updated.current`
→ full load, `vite:preloadError` → reload; `vite.config.ts`
`version.pollInterval`; `passkey-directory.svelte.ts` on `fetchWithTimeout`;
home `unreachable` line; #189 favicon deleted.

Not in this slice: fee fallback flag (T059–T061), the `pool.rs`/`chainlink.rs`
callers' candidate retry (T028 partial), the remaining `FontWeight::` walk
(T044), #191 reproduction, #meta, Part D.

Screenshots: the desktop app was launched with `VELA_INTRO=1` on a fresh
state (launch animation → intro confirmed from the log), but a window-only
capture needs the no-TCC harness — the visual pass (SC-429) is still owed.
