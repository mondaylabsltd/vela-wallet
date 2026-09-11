# Tasks: First Run, and a stable app in an unstable environment

**Input**: `specs/038-first-run-parity/` — spec.md (Parts A/B/C, SC-411…440),
plan.md (R1–R20), data-model.md, contracts/ (pre-paint-attributes,
proxy-candidates, passkey-icons.json), quickstart.md.

**Tests**: the spec names the tests it wants ("proven by a test", "pinned by a
test") — those are tasks below. Everything else is verified on the running
apps per quickstart.md (SC-429).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: parallelizable (different files, no dependency on an unfinished task)
- **[USn]**: the story the task belongs to. Stories are the spec's scope
  clusters; each is independently testable against its SCs.

## Path Conventions

- Core: `rust/crates/vela-core/src/app/…`
- Desktop: `app-desktop/vela-wallet/src/…`
- Web: `app-web/vela-wallet/src/…`
- Shared assets: `assets/fonts/`; contracts: `specs/038-first-run-parity/contracts/`

---

## Phase 1: Setup

- [x] T001 Copy `PlusJakartaSans_{400Regular,500Medium,600SemiBold,700Bold}.ttf` from `app-ios/VelaWallet/VelaWallet/DesignSystem/Fonts/` to `assets/fonts/` and note the OFL licence in `assets/fonts/README.md`
- [ ] T002 [P] Add a `VELA_STATE_DIR`-scoped dev seam list to `app-desktop/vela-wallet/README.md` (the two dev commands: parallel space, signed bundle) — the text SC-425 points at
- [x] T003 [P] Verify `cargo test -p vela-core --features i18n-all,crux`, `cargo test` in `app-desktop/vela-wallet`, and `pnpm test` in `app-web/vela-wallet` are green on the branch before any change (baseline counts recorded in `specs/038-first-run-parity/results.md`)

## Phase 2: Foundational (blocking)

- [x] T004 Generalise `rasterize(svg, size)` to `rasterize_sized(svg, w, h)` in `app-desktop/vela-wallet/src/icons.rs` (non-square canvas; the intro art is 160×128) keeping the existing square call sites
- [x] T005 [P] Add `theme::font_ui() -> &'static str` returning `"Plus Jakarta Sans"` in `app-desktop/vela-wallet/src/theme.rs`
- [x] T006 [P] Add `read_epoch_ms(key)` / `write_epoch_ms(key, now)` helpers over `read_value`/`write_value` in `app-desktop/vela-wallet/src/executor/storage.rs` (used by the intro and launch gates)
- [ ] T007 [P] Add `ceremony::FailureKind`→detail plumbing: `PasskeyFailure` in `app-desktop/vela-wallet/src/executor/passkey.rs` carries the peer-close reason string (used by US2)

---

## Phase 3: US1 — Web first run: intro before paint, desktop composition (P1)

**Goal**: a desktop visitor's first run is intro → Welcome with no flash, and
the intro is composed like its neighbours (SC-411…415).

**Independent test**: quickstart rows 411/412 and 413/414/415.

- [x] T008 [US1] Add the `data-intro="pending"` inline block to `app-web/vela-wallet/src/app.html` after the launch block, per `contracts/pre-paint-attributes.md` (reads `vela.intro.seen`, `?intro`, `?skipIntro`; throw → pending)
- [x] T009 [P] [US1] Add `html[data-intro='pending'] [data-intro-page] { opacity: 0 }` to `app-web/vela-wallet/src/app.css` beside the launch rule
- [x] T010 [US1] In `app-web/vela-wallet/src/routes/[locale]/+page.svelte`: put `data-intro-page` on the Welcome `<main>`, and remove `document.documentElement.dataset.intro` in `onMount` after the gate decides (both branches)
- [x] T011 [P] [US1] Add cases to `app-web/vela-wallet/src/lib/intro/gate.test.ts` that read `src/app.html` and assert the inline block uses `STORAGE_KEY`, `FORCE_PARAM`, `SKIP_PARAM` (the `constants.test.ts` pattern) — SC-415
- [x] T012 [US1] Rework `app-web/vela-wallet/src/lib/ui/intro/IntroCarousel.svelte` for ≥1280: render `OnboardingRail` with `{ kind: 'tagline', text }` (tagline passed as a prop from `+page.svelte`), left-align the column at `--layout-onboardingColumn + 2×gutter`, natural height, buttons at label width (`.actions :global(.button) { width: auto; min-width: var(--layout-welcomeCtaMin) }`), row layout for the last slide's two buttons; phone rules untouched — SC-411/412
- [x] T013 [US1] Pass `tagline={strings('onboarding.welcome.desktopTagline')}` from `app-web/vela-wallet/src/routes/[locale]/+page.svelte` to `IntroCarousel`
- [x] T014 [P] [US1] Add intro cases to `app-web/vela-wallet/e2e/welcome-layout.e2e.ts` at 390×844, 1279, 1280, 1440 with `?intro`: rail present ≥1280, absent below; buttons side-by-side on slide 3 at 1440 — SC-411/412
- [x] T015 [P] [US1] Add a first-run e2e (fresh context, no `skipIntro`) to `app-web/vela-wallet/e2e/welcome-ssr.e2e.ts` asserting the first painted frame has no visible Welcome headline and the SSR HTML still contains it — SC-413/414

## Phase 4: US2 — Desktop front door: dismissible endpoint sheet, no settings entry, hybrid teardown (P1)

**Goal**: nothing on the desktop front door can trap a person (SC-418, 419,
420, 427).

**Independent test**: quickstart rows 418/419/420 and 427.

- [x] T016 [US2] Remove `settings_label` / `on_settings` and the `rail-settings` element from `app-desktop/vela-wallet/src/ui/rail.rs`; update the call site in `app-desktop/vela-wallet/src/onboarding.rs` (`onboarding_rail(` at ~1030) — SC-418
- [x] T017 [US2] Add `endpoint_dismissed: bool` to `OnboardingPage` in `app-desktop/vela-wallet/src/onboarding.rs`; Close sets it; `save_endpoint` (the `Event::Start` re-probe) clears it; the automatic open at ~988 requires `!endpoint_dismissed` — SC-419
- [x] T018 [US2] Re-parent the endpoint surface in `app-desktop/vela-wallet/src/onboarding.rs` from `scrim(...)` to a card rendered under the Welcome actions (same `endpoint_surface` body, no scrim), so Create / sign-in stay pressable — SC-420
- [x] T019 [P] [US2] Add a desktop test in `app-desktop/vela-wallet/src/onboarding.rs` `mod tests`: with `login_view.endpoint_unreachable = true`, Close → next render does not re-open; `save_endpoint` → may re-open — SC-419
- [x] T020 [US2] In `app-desktop/vela-wallet/src/executor/passkey.rs` wrap the ceremony in `assert_hybrid` and `register_hybrid` with a guard that runs `(ceremony.touch)(None)` on every exit (scope guard struct or explicit `match`), and skip `cable.cancel()` when the port already reported CLOSE — SC-427
- [x] T021 [US2] In `app-desktop/vela-wallet/src/ctap/cable.rs` record `closed_by_peer: bool` on `WebSocketCablePort` when `Message::Close` is read; `write_frame` returns `Err(PortError::Io("tunnel closed"))` immediately when set (no write into a closed socket)
- [x] T022 [US2] Map a peer close after `getAssertion`/`makeCredential` to `FailureKind::Cancelled` with detail "the phone ended the session" in `ceremony_failure` (`app-desktop/vela-wallet/src/executor/passkey.rs`), so the failure sheet names it
- [x] T023 [P] [US2] Add `hybrid_teardown` test in `app-desktop/vela-wallet/src/executor/passkey.rs` `mod tests`: a `CablePort` stub that returns CLOSE on the first read after `getAssertion` → `assert_hybrid` returns `Cancelled` and the ceremony's touch slot is `None` — SC-427

## Phase 5: US6 — The network path: system proxy first, candidates, honest failure (P1)

**Goal**: a dead proxy in a shell never reads as our outage (SC-421, 430, 431).

**Independent test**: quickstart rows 430/431 and SC-421.

- [x] T024 [US6] Add `#[cfg(target_os = "macos")] fn desktop_proxy()` in `app-desktop/vela-wallet/src/executor/proxy.rs` parsing `scutil --proxy` (HTTPS → HTTP → SOCKS-as-`Socks5h`, `*Enable=1`, port ≠ 0, `ExceptionsList` → `no_proxy`) per `contracts/proxy-candidates.md`
- [x] T025 [P] [US6] Unit-test the `scutil` parser in `app-desktop/vela-wallet/src/executor/proxy.rs` `mod tests` against the captured output in `research.md` (HTTPS wins over SOCKS; disabled entries skipped; `ExceptionsList` parsed)
- [x] T026 [US6] Replace the `OnceLock` in `system_proxy()` with `Mutex<CandidateState>` (`order: [System, Env, Direct]`, `current`, `last_derived: Instant`) in `app-desktop/vela-wallet/src/executor/proxy.rs`; add `advance_after_failure()` and `reset_after_success()`; re-derive after any failure or after `PROXY_REDERIVE_AFTER = 60 s`
- [x] T027 [US6] Make `agent_for(url)` in `app-desktop/vela-wallet/src/executor/proxy.rs` build from the current candidate; loopback targets stay direct (existing rule)
- [~] T028 [US6] (registry + relay `rest_get` done; `pool.rs`, `chainlink.rs`, `chain.rs` still on `proxy::agent`, which now at least reads the CURRENT candidate) In `app-desktop/vela-wallet/src/executor/registry.rs` (and the other `proxy::agent` callers: `relay.rs`, `pool.rs`, `chainlink.rs`, `chain.rs`) retry ONCE on the next candidate when `classify()` says `network: true`; after the last candidate set `RegistryError.transport = Local`
- [x] T029 [US6] Add `transport_failed: bool` to `LoginView` in `rust/crates/vela-core/src/app/login.rs`, set from a new `index_failed { network: true, local: true }` result; regenerate ts-rs bindings (both generation commands, per the multi-passkey memory)
- [x] T030 [US6] Desktop: render the "this machine could not get out" line on Welcome (reuse `onboarding.settings.warningText`'s slot with the existing `network` corpus sentence if one fits; else add ONE key `onboarding.settings.transportFailedText` to all 15 locales) in `app-desktop/vela-wallet/src/onboarding.rs`; never the endpoint card for this state — SC-421
- [x] T031 [P] [US6] Web: surface `transport_failed` the same way in `app-web/vela-wallet/src/routes/[locale]/+page.svelte` (the `endpointWarning` block)
- [x] T032 [P] [US6] Candidate-chain test in `app-desktop/vela-wallet/src/executor/proxy.rs` `mod tests`: env proxy at a refusing port + system proxy absent → falls to direct; env refusing + system live → system; all refusing → `Local` — SC-431
- [x] T033 [US6] Move the caBLE tunnel dial in `app-desktop/vela-wallet/src/ctap/cable.rs` (`dialing … through`) onto the same candidate chain (it currently resolves its own proxy)

## Phase 6: US3 — Desktop intro and the 7-day launch gate (P2)

**Goal**: launch animation → intro → Welcome once; animation replays after 7
days (SC-416, 417, 422).

**Independent test**: quickstart rows 416/417 and 422.

- [x] T034 [P] [US3] Hand-port `specs/020-intro-carousel/contracts/intro-illustrations.json` to `app-desktop/vela-wallet/src/intro_art.rs` (three illustrations, elements with role/mode/width/opacity/d) and an SVG builder that substitutes theme colours (line = `fg_subtle`, accent = `accent`, outline fill = `bg_base`)
- [x] T035 [P] [US3] Test in `app-desktop/vela-wallet/src/intro_art.rs` `mod tests` that reads the contract JSON and asserts every path `d` and opacity matches — SC-417
- [x] T036 [US3] Create `app-desktop/vela-wallet/src/intro.rs`: `IntroState { index, drag_px, dragging }`, slide data from the three `onboarding.intro.*` key pairs via `Loc`, `render(...)` with rail (tagline slot), viewport with the three cells, dots, Skip (not on last), Next / Create / Sign-in buttons; drag via `on_mouse_down` / `on_mouse_move` / `on_mouse_up` on the viewport, ←/→ via `KeyDownEvent`; art via `rasterize_sized` cached per (id, dark)
- [x] T037 [US3] Route it in `app-desktop/vela-wallet/src/onboarding.rs`: `intro: Option<IntroState>` set at construction when `storage::read_epoch_ms("vela.intro.seen")` is `None`; render after the launch overlay and before Welcome; `leave_intro()` writes the flag and routes Create → `start_create`, Sign-in → open the method picker, Skip → Welcome — SC-416
- [x] T038 [US3] Launch gate: `LAUNCH_REPLAY_AFTER_MS: u64 = 604_800_000` in `app-desktop/vela-wallet/src/theme.rs`; in `onboarding.rs` construct `LaunchAnimation` only when `vela.launch.played` is absent or older; write the flag when the animation finishes or is skipped (`launch = None` sites) — SC-422
- [x] T039 [P] [US3] Test `launch_gate` in `app-desktop/vela-wallet/src/theme.rs` `mod tests`: the constant equals the number in `app-web/vela-wallet/src/app.html` (read the file relative to the workspace) and `constants.ts` — SC-422
- [x] T040 [US3] Dev seam: `VELA_INTRO=1` forces the intro (mirror of `?intro`) in `app-desktop/vela-wallet/src/onboarding.rs`, listed in the README's dev switches

## Phase 7: US4 — One typeface, the web's weights (P2)

**Goal**: same face and weight on the same control in both shells (SC-423, 424).

**Independent test**: quickstart row 423/424.

- [x] T041 [US4] Load the four TTFs with `cx.text_system().add_fonts(vec![Cow::Borrowed(include_bytes!(...))])` at startup in `app-desktop/vela-wallet/src/main.rs` before the first window opens
- [x] T042 [US4] Apply `.font_family(theme::font_ui())` on the root `div` of `OnboardingPage::render` (`onboarding.rs`) and `WalletPage::render` (`wallet/page.rs`) and the gallery root (`gallery.rs`)
- [x] T043 [P] [US4] Test `fonts_bundled` in `app-desktop/vela-wallet/src/theme.rs` `mod tests`: the four `include_bytes!` parse as TTF (`ttf-parser` is already a transitive dep via cosmic-text; else assert magic bytes + size) — SC-424
- [~] T044 [US4] (the two pinned sites done; the wider walk pending) Weight audit: `button.rs:115,168` BOLD → SEMIBOLD; `logo.rs:71` EXTRA_BOLD → BOLD; then walk the remaining `FontWeight::` sites in `app-desktop/vela-wallet/src/**` against the web token at the same control (headline bold, body regular, caption medium/semibold per the web component) and record each change in `specs/038-first-run-parity/results.md` — SC-423
- [ ] T045 [US4] Record the CJK/mono deviation (OS fallback faces) in `specs/038-first-run-parity/results.md` with the TTF-sourcing follow-up

## Phase 8: US5 — Passkey method icons, platform-resolved; web sign-in picker (P2)

**Goal**: the rows carry the founder's icons on both shells; the web offers the
same three ways in as the desktop (SC-428, 438).

**Independent test**: quickstart rows 428 and 438.

- [x] T046 [P] [US5] Web: `app-web/vela-wallet/src/lib/onboarding/passkey-icons.ts` — the six icons from `contracts/passkey-icons.json` as `currentColor` path data + `platformIcon(method)` (UA rule from the contract's `platform_rule`)
- [x] T047 [P] [US5] Web: `app-web/vela-wallet/src/lib/ui/onboarding/PasskeyMethodIcon.svelte` rendering an inline `<svg viewBox="0 0 16 16">` (USB normalised from 80 → 16 by `transform="scale(0.2)"`), roles: ink = `currentColor`, muted = `var(--color-fg-subtle)`, paper = `var(--color-bg-base)`
- [x] T048 [US5] Replace the bordered boxes in `app-web/vela-wallet/src/lib/ui/onboarding/v2/AddMethodPicker.svelte` with `<PasskeyMethodIcon method={method} />`; delete the three `[data-method]` size rules — #190
- [x] T049 [P] [US5] Test `passkey-icons.test.ts` in `app-web/vela-wallet/src/lib/onboarding/`: paths match the contract JSON; `platformIcon` table (mac+safari → apple, mac+chrome → chrome-mac, windows → windows, android → google, other → fido2)
- [x] T050 [P] [US5] Desktop: add `Icon::PasskeyApple | PasskeyWindows | PasskeyGoogle | PasskeyChromeMac | PasskeyFido2 | PasskeyUsb` to `app-desktop/vela-wallet/src/icons.rs` with the contract's paths as SVG templates (colour substitution as the existing icons do; USB's `muted`/`paper` roles as two extra substitutions); `platform_icon()` via `cfg!(target_os)`
- [x] T051 [US5] Desktop: draw the icon in each row of `hardware::signin_method_card` and the create picker (`app-desktop/vela-wallet/src/hardware.rs` / `onboarding_flow.rs`) at `--icon-lg`
- [x] T052 [P] [US5] Test in `app-desktop/vela-wallet/src/icons.rs` `mod tests`: each passkey icon template rasterises (non-empty pixmap) and the path data equals the contract JSON — SC-428
- [x] T053 [US5] Web sign-in picker: in `app-web/vela-wallet/src/routes/[locale]/+page.svelte` the "I already have a wallet" button opens `AddMethodPicker` in place (`open` state, same expand pattern as create); `onPick={(method) => signIn(method)}`; `signIn` dispatches `{ type: 'sign_in', method }`; the intro's sign-in button does the same — finding 20
- [ ] T054 [P] [US5] e2e in `app-web/vela-wallet/e2e/welcome-layout.e2e.ts`: pressing "I already have a wallet" shows three rows with icons at 390 and 1440

## Phase 9: US7 — Panics become a sheet; money fallbacks become view state (P2)

**Goal**: the worst case is a screen (SC-432); a defaulted fee says so (SC-433).

**Independent test**: quickstart rows 432 and 433.

- [x] T055 [US7] In `app-desktop/vela-wallet/src/resident.rs::pump` wrap `work()` / `work(&Sink(tx))` in `std::panic::catch_unwind(AssertUnwindSafe(...))`; on `Err`, resolve the operation with the machine's own failure result (`A::panicked(id)` — a new associated fn on the resident trait returning the failure `ShellResult`)
- [x] T056 [US7] `std::panic::set_hook` in `app-desktop/vela-wallet/src/main.rs`: log the payload + location to stderr and post it to a `PanicReport` global the pages read; `OnboardingPage` / `WalletPage` render the failure sheet (`Prompt` with `details`) when set — SC-432
- [x] T057 [P] [US7] Dev seam `VELA_TEST_PANIC=1` in `app-desktop/vela-wallet/src/resident.rs` that panics inside the next blocking work; README lists it
- [x] T058 [P] [US7] Test in `app-desktop/vela-wallet/src/resident.rs` `mod tests`: a work closure that panics resolves as the failure result and the resident is still usable afterwards — SC-432
- [ ] T059 [US7] Add `fallback: bool` to `FeeEstimateView` in `rust/crates/vela-core/src/app/send.rs` (and the fee types the sign sheet reads), set when an estimate used defaults; regenerate ts-rs bindings
- [ ] T060 [US7] Desktop: set `fallback` where `user_op.rs:206,449,546` currently `eprintln!` "using defaults" (`app-desktop/vela-wallet/src/executor/user_op.rs` → the fee result); render the existing "estimate unavailable" copy on the fee line in `app-desktop/vela-wallet/src/wallet/money.rs` — SC-433
- [ ] T061 [P] [US7] Web: same view field rendered on `GasFeeCard` (`app-web/vela-wallet/src/lib/flows/…`) — SC-433

## Phase 10: US8 — "Unreachable" is a state; the web survives offline and a deploy (P2)

**Goal**: no $0 for "could not look"; offline and version skew are handled
(SC-434, 435).

**Independent test**: quickstart rows 434 and 435.

- [x] T062 [US8] In `rust/crates/vela-core/src/app/balance_dashboard.rs` add `unreachable: bool` to `BalanceView` (true after `FetchErrored` when `tokens.is_empty() && cached_total.is_none()`); `display_total_usd` stays `None` in that state; regenerate ts-rs bindings — SC-434
- [x] T063 [P] [US8] Core test in `balance_dashboard.rs` `mod tests`: fresh model + `FetchErrored` → `unreachable = true`, `display_total_usd = None`; then a `FetchSettled` clears it
- [x] T064 [US8] Web: `app-web/vela-wallet/src/lib/wallet/live.ts` renders skeleton + the existing RPC-unavailable sentence when `view.unreachable`; `zeroLive` requires `!view.unreachable`
- [x] T065 [P] [US8] Desktop: `app-desktop/vela-wallet/src/wallet/live.rs` same rule on the home status line
- [x] T066 [US8] (done without a new core event: `online` calls the existing `invalidateAllPools()`, which drops the pools' state) Add `Event::ConnectivityChanged { online: bool }` to `rust/crates/vela-core/src/app/rpc_pool.rs` — `online: true` clears cooldowns and temp bans; test in the same file
- [x] T067 [US8] Web: `online` / `offline` listeners in `app-web/vela-wallet/src/routes/+layout.svelte` dispatching to the pool session and rendering an offline line (existing `settings` `offline` string) — SC-435
- [x] T068 [US8] Web version skew: `version: { pollInterval: 60_000 }` in the `sveltekit({...})` options of `app-web/vela-wallet/vite.config.ts`; `beforeNavigate` in `+layout.svelte` does `location.href = to.url.href` when `updated.current`; `window.addEventListener('vite:preloadError', () => location.reload())` — SC-435
- [x] T069 [P] [US8] Move the bare `fetch()` in `app-web/vela-wallet/src/lib/onboarding/core/passkey-directory.svelte.ts` onto `net.ts`'s `ethereumData` timeout class

## Phase 11: US9 — The reported issues #188, #189, #191 (P2)

**Goal**: SC-436, 437, 439, 440.

**Independent test**: quickstart rows 436, 437, 439.

- [x] T070 [US9] `rust/crates/vela-core/src/app/balance_dashboard.rs`: add `last_settled_total: Option<f64>` to `Model` (seeded from `cached_total`, set at every `FetchSettled`); `display_total` returns it while `pending_pulls > 0`; `view()` raises `BalanceNotice::Unpriced` only when `pending_pulls == 0` — #188
- [x] T071 [P] [US9] Core test `balance_settled_figure` in the same file: three `ChainAssetsArrived` during a pull leave the display unchanged; `FetchSettled` moves it once; no `Unpriced` mid-pull — SC-436
- [x] T072 [P] [US9] Delete `app-web/vela-wallet/src/lib/assets/favicon.svg` and the `import favicon` + `<svelte:head><link rel="icon" …>` at `app-web/vela-wallet/src/routes/+layout.svelte:13,91` — #189 / SC-437
- [ ] T073 [US9] Reproduce #191 on the running web app (`pnpm dev`, parallel space, a history-derived row → Edit); record the exact failing step in `specs/038-first-run-parity/results.md`
- [ ] T074 [US9] Fix per the reproduction: make "name this contact" the primary action on an `auto` row's detail in `app-web/vela-wallet/src/lib/contacts/live.ts` (+ `ui/`), saving via the core's `Save` (promotes to `Manual`); add a Vitest case in `app-web/vela-wallet/src/lib/contacts/live.test.ts` — SC-439

- [ ] T074a [P] [US9] Add `og-image.svg` (1200×630: sailboat mark centred on `color.bg.base`, no text) to `app-web/vela-wallet/static/` and a `scripts/gen-og-image.mjs` (resvg-js or sharp, already in devDeps? else `@resvg/resvg-js`) that writes `static/og-image.png`; wire it into the `build` script — #meta
- [ ] T074b [US9] Add `metaSocialDescription` (≤ 110 chars) to all 15 locales in `rust/crates/vela-core/i18n/locales/*/…` where `metaDescription` lives, following `reference_i18n_corpus_gates` (six steps: corpus, paths.rs, vectors, wasm bytes + fingerprint, tests); regenerate — #meta
- [ ] T074c [US9] `app-web/vela-wallet/src/lib/ui/SocialMeta.svelte` rendering `og:site_name`, `og:type=website`, `og:title`, `og:description`, `og:url`, `og:image` (+ `:width`/`:height`), `og:locale` (+ `:alternate` for the other 14), `twitter:card=summary_large_image`, `twitter:title`, `twitter:description`, `twitter:image`; used from `routes/[locale]/+page.svelte`'s `<svelte:head>` — SC-440
- [ ] T074d [P] [US9] e2e in `app-web/vela-wallet/e2e/welcome-ssr.e2e.ts`: the prerendered HTML of `en` and `zh-CN` contains every tag above; `og:description` length ≤ 110; `og-image.png` responds 200 with `image/png` — SC-440

## Phase 11b: US11 — The send flow after the button (P2)

**Goal**: SC-441, 442, 443 (#D1–#D3), #D4 visual pass.

- [ ] T074e [US11] Reproduce #D1 on the parallel space (a send whose inner call reverts: ERC-20 transfer over balance); record the UserOp receipt (`success`, logs) and both shells' stored status in `specs/038-first-run-parity/results.md`
- [ ] T074f [US11] Core `rust/crates/vela-core/src/app/tx_tracker.rs`: a `Receipt` whose logs carry Safe `ExecutionFailure` (topic `keccak("ExecutionFailure(bytes32,uint256)")`) emitted by the sender is treated as `ReceiptFailed`; both shells pass the authentic logs they already collect (`remember_receipt` / `tracker-executor.ts`) into the result; core test with a fixture log — SC-441
- [ ] T074g [P] [US11] Core `network_admin.rs`: `typical_inclusion_s: u16` per builtin chain (block time × usual depth; Gnosis 5s×3, Ethereum 12s×2, L2s 2s×5, Tempo …); `SendReceiptView.typical_inclusion_s` + `elapsed_s` from the shell clock; `SendReceiptView.longer_than_usual` past 2× — SC-443
- [ ] T074h [US11] Web `app-web/vela-wallet/src/lib/flows/screens/SendReceipt.svelte` + `live-send.ts`: elapsed/typical line and the "longer than usual" state; desktop `app-desktop/vela-wallet/src/wallet/money.rs` receipt stage the same — SC-443
- [ ] T074i [US11] Core `send.rs`: `SendConfirmView.recipients` (count + rows: address, name, amount) for split and sweep; ts-rs regenerate — SC-442
- [ ] T074j [US11] Web confirm `SendConfirm.svelte` + `live-send.ts` SD3: recipients block (count title, first 3 rows with identicon, "all N" → third column on wide / sheet on phone); the same block on `SendReceipt.svelte` and `TxDetail.svelte` — SC-442
- [ ] T074k [P] [US11] Desktop `money.rs` confirm/receipt/detail: the same recipients block (third column for the full list, per the desktop rule: no bottom sheets) — SC-442
- [ ] T074l [US11] Visual pass on both shells at 1 / 3 / 12 / 60 recipients (screenshots into results.md) — #D4

## Phase 12: US10 — The two dev commands, and a sheet that tells the truth (P3)

**Goal**: SC-425, 426.

- [ ] T075 [P] [US10] `app-desktop/vela-wallet/README.md`: a "Sign in during development" section with the two commands (`VELA_PARALLEL_SPACE=1 cargo run --features dev-fixtures`; `./scripts/build-macos-app.sh --no-dmg` with `VELA_SIGN_IDENTITY`/`VELA_PROVISION_PROFILE`, then run the bundle) and why `cargo run` cannot use "This device" on macOS
- [ ] T076 [US10] In `app-desktop/vela-wallet/src/executor/platform_macos.rs` classify the AS error "does not have an application identifier" as `FailureKind::NotSupported` with detail "this binary is not a signed bundle"; the failure sheet's body for that kind on macOS says so (existing `IncompatibleLogin` copy + detail) — SC-426
- [ ] T077 [P] [US10] `cargo` alias `dev` = `run --features dev-fixtures` in `app-desktop/vela-wallet/.cargo/config.toml` (env still set by the person; documented)

## Phase 13: Polish & cross-cutting

- [ ] T078 Sweep both shells' first-run and front-door surfaces at 390×844, 1280×800, 1440×900, 3840×2160 (web) and default / maximised window (desktop); log findings in `specs/038-first-run-parity/results.md`
- [ ] T079 [P] Screenshot pass per quickstart.md — every SC row, both shells — into `specs/038-first-run-parity/results.md` (SC-429)
- [ ] T080 [P] Run the four gates: `cargo test -p vela-core --features i18n-all,crux`, desktop `cargo test`, web `pnpm test`, web `pnpm e2e`; counts strictly increase vs T003
- [ ] T081 Write `specs/038-first-run-parity/results.md` handover (what landed, deviations: CJK/mono fonts, any SC deferred, the clear-signing spec pointer)

---

## Dependencies

- Phase 1 → Phase 2 → stories. Within stories, tasks without [P] depend on the
  previous task in the same story.
- US1, US2, US6 are the P1 set and are independent of each other.
- US3 depends on T004 (raster) and T006 (storage); US4 on T001 + T005; US5 on
  nothing in other stories (T053 touches `+page.svelte` — do after T010/T013).
- US7's T059 and US8's T062/T066 and US9's T070 all regenerate ts-rs bindings —
  run the generation once after the last core change, not per task.
- US10 is independent.

## Parallel execution examples

- P1 wave: T008–T015 (US1, web) ‖ T016–T023 (US2, desktop) ‖ T024–T033 (US6,
  desktop + core) — three files sets, no overlap.
- P2 wave: T034–T040 (US3) ‖ T041–T045 (US4) ‖ T046–T054 (US5) ‖ T055–T061 (US7)
  ‖ T062–T069 (US8) ‖ T070–T074 (US9).

## Implementation strategy

1. **MVP = the three P1 stories** (US1, US2, US6): the flash, the trap on the
   front door, and the proxy misdiagnosis — the founder's original three
   complaints plus the one that made our service look down.
2. Then US3 + US4 + US5 (the desktop looks like the same product), then
   US7–US9 (stability + issues), US10, polish.
3. Regenerate bindings once; run the four gates; results.md as the handover.
