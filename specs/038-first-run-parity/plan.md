# Implementation Plan: First Run, and a stable app in an unstable environment

**Branch**: `038-first-run-parity` | **Date**: 2026-09-11 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/038-first-run-parity/spec.md`

## Summary

Three parts, one branch. **A**: the first thirty seconds on both shells — the
web intro gets the desktop composition its neighbours have and is decided
before first paint; the desktop gains the intro, the 7-day launch gate, and
loses the front-door settings entry; the endpoint sheet becomes dismissible and
honest; the desktop is set in the web's typeface at the web's weights. **B**:
the network path on the desktop stops trusting the shell over the system, tries
candidates in order, and reports the truth; worker panics become a sheet; money
fallbacks become view state; "unreachable" becomes a home state on both shells;
the web survives offline and a mid-session deploy. **C**: four outside issues,
each already pinned to its cause in the spec.

Everything that is a *rule* lands in `vela-core` so both shells inherit it;
everything that is a *surface* is done twice, web and desktop, from the same
contract.

## Technical Context

**Language/Version**: Rust 2024 (vela-core, app-desktop on gpui `c97b7c0`);
TypeScript 5 / Svelte 5 runes / SvelteKit (app-web); wasm via wasm-bindgen.

**Primary Dependencies**: gpui (git), `ureq` 3 (desktop HTTP, one agent factory
in `executor/proxy.rs`), `resvg`/`tiny-skia` (desktop SVG raster), `dotlottie-rs`
(desktop launch animation), `@fontsource/plus-jakarta-sans` (web), Vitest +
Playwright (web), `cargo test` (desktop + core).

**Storage**: desktop — one JSON file, tmp+rename (`executor/storage.rs`,
`read_value`/`write_value`); web — `localStorage` (guarded) + IndexedDB.

**Testing**: `cargo test -p vela-core --features i18n-all,crux`; desktop
`cargo test` (dev-fixtures for the parallel space); web `pnpm test` (Vitest,
incl. browser project) + `pnpm e2e` (Playwright, `welcome-*.e2e.ts` family).

**Target Platform**: macOS / Linux / Windows desktop (gpui); evergreen browsers
at 390–3840 px; the Chrome extension build of app-web shares the code.

**Project Type**: desktop-app + web-app over a shared Rust core.

**Performance Goals**: intro decided before first paint (0 frames of Welcome on
a first run); desktop font load < 50 ms at startup (four TTFs, ~1.4 MB);
network candidate fallback adds at most one extra connect attempt per failure.

**Constraints**: no new corpus keys in Part A (15-locale translation gate);
Part B's "unreachable" sentence reuses existing keys where one exists and is
the ONE new key otherwise (recorded); the web landing page stays prerendered
per locale (SEO); fonts must be TTF/OTF for gpui (cosmic-text/fontdb cannot
read woff2).

**Scale/Scope**: 20 scope items, 29 success criteria, 3 crates/apps touched
(`rust/crates/vela-core`, `app-desktop/vela-wallet`, `app-web/vela-wallet`).

## Constitution Check

`.specify/memory/constitution.md` is the unfilled template; there are no
ratified gates. The house rules that stand in for it, all honoured here:
one implementation of a rule (core), shells add events + view fields only;
no bottom sheets on desktop; `VelaButton` is the only CTA; screenshots on the
running app are the acceptance, not the unit test.

## Research decisions (Phase 0 — see research.md for the evidence)

| # | Decision | Rationale |
| --- | --- | --- |
| R1 | Intro pre-paint = a third inline block in `app.html` setting `data-intro="pending"`, plus `html[data-intro='pending'] [data-intro-page] { opacity: 0 }` in `app.css`, mirroring spec 012's launch block | The exact precedent; a test that reads `app.html` (like `constants.test.ts`) pins the duplicated rule |
| R2 | Web intro desktop composition = `OnboardingRail` with `RailSlot::Tagline` + the flow column, buttons at label width, at `min-width: 1280` | Welcome and `FlowShell` already do this; the intro joins the same three-screen sequence |
| R3 | Desktop intro = new `intro.rs` page state inside `OnboardingPage` (not a new window/route): `launch → intro → welcome`, three slides, drag via `on_mouse_down/move/up` on the viewport (the `window_frame.rs` drag pattern), arrows via `KeyDownEvent`, art rasterised with the existing `resvg` path (`icons.rs::rasterize` generalised to a non-square canvas) from a hand-ported `intro_art.rs` pinned to the contract JSON by a test | Same place in the boot order as the web; the contract already names `desktop resvg` |
| R4 | Intro-seen + launch-played flags on the desktop live in `storage.rs` as two keys (`vela.intro.seen`, `vela.launch.played`), epoch ms, same names as the web | One vocabulary across shells; the 7-day number is shared with `theme.rs` LAUNCH_* consts and asserted |
| R5 | Endpoint sheet: `endpoint_dismissed: bool` on `OnboardingPage`, set by Close, cleared by `Event::Start` (a re-probe) — the automatic open reads it; the sheet is re-parented from a scrim over the whole page to a card under the actions | Records the dismissal the comment promised; the front door stays pressable (SC-420) |
| R6 | Fonts: bundle Plus Jakarta Sans 400/500/600/700 TTFs (copied from `app-ios/.../Fonts/`) into `assets/fonts/`, load with `cx.text_system().add_fonts()` at startup, set the family on the root `div` of every page via `theme::font_ui()`; audit the 144 `FontWeight::` sites against the web token at the same place (button → SEMIBOLD, wordmark → BOLD). CJK fallback stays the OS's (gpui falls back per glyph); mono stays Menlo/DejaVu | No TTF of Noto Sans SC or IBM Plex Mono exists in the repo and fontsource ships woff2 only — recorded deviation, not a silent one |
| R7 | macOS proxy = `scutil --proxy` parsed like the GNOME branch: HTTPS → HTTP → SOCKS (as `Socks5h`), `Enable` + non-zero port required; `ExceptionsList` → `no_proxy` | Same shape as Linux; verified output format on the founder's Mac |
| R8 | Candidate order = **system → environment → direct**; `agent_for(url)` returns the agent for the current candidate; a transport failure (`classify → network`) advances the candidate and re-tries the same request once; `OnceLock` replaced by a `Mutex<CandidateState>` re-derived after any failure | The founder's principle: the stable source first, no answer cached past a failure |
| R9 | "Could not get out" = a new `RegistryError` bit (`transport: Local`) → core `LoginView.endpoint_unreachable` gains a sibling `transport_failed`; the shell renders a different sentence and never the modal | Distinguishes the two facts `classify()` currently merges |
| R10 | Panic boundary = `catch_unwind(AssertUnwindSafe(work))` in `resident.rs::pump` (the single choke point for all machine work) → `resolve` with the machine's own error result; `std::panic::set_hook` logs + raises the failure sheet (`PromptKind::SignInFailed { detail }`-shaped generic) | One place; the sheet already exists |
| R11 | Money fallbacks = `FeeEstimateView.fallback: bool` from the core when an estimate used defaults; both shells render the existing "estimate unavailable" copy on the fee line | Promotes `eprintln!` to view state without a new key |
| R12 | Unreachable home = core `BalanceView.unreachable: bool` (true when `FetchErrored` landed with no tokens and no cache); shells render skeleton + reason; verified with `vela.rpcDown` fault harness | Fixes finding 15 in the core once |
| R13 | Web offline/version = `online`/`offline` listeners in `+layout.svelte` dispatching a pool `Event::ConnectivityChanged` that clears cooldowns; `version.pollInterval` in the inline kit options + `beforeNavigate` reload when `updated.current`; `vite:preloadError` → reload | SvelteKit's own mechanisms |
| R14 | #188 = core: `Model.last_settled_total`, `display_total` returns it while `pending_pulls > 0`; `BalanceNotice::Unpriced` only when `pending_pulls == 0` | One rule, both shells |
| R15 | #189 = delete `$lib/assets/favicon.svg` + `+layout.svelte:91` | — |
| R16 | #190 = SUPERSEDED by R19 (founder ruled: real icons, platform-resolved) | — |
| R18 | Hybrid teardown: `assert_hybrid`/`register_hybrid` wrap the ceremony in a guard that runs `(ceremony.touch)(None)` on every exit; `cable.cancel()` skipped when the port reported CLOSE; `ceremony_failure` maps a peer close after `getAssertion` to `FailureKind::Cancelled` with a detail | One guard, all three exits; the sheet already exists |
| R19 | Passkey icons = `contracts/passkey-icons.json` (six `currentColor` paths, viewBox 16 / 80 normalised to 16); web `PasskeyMethodIcon.svelte` inline `<svg>`; desktop `Icon::Passkey{Apple,Windows,Google,ChromeMac,Fido2,Usb}` through `icons.rs` colour substitution; `platformIcon()` rule: desktop `cfg!(target_os)`, web `navigator.userAgentData?.platform ?? UA` + Chrome detection | Same pattern as the intro art; one contract, two ports |
| R20 | Web sign-in picker = reuse `AddMethodPicker` under the "I already have a wallet" button (expanded in place, like create), dispatching `sign_in { method }`; the picker's row list is one array so the clear-signing spec adds one entry | No new keys (`methodCopy` already has all three) |
| R17 | #191 = reproduce on the running app first (the code path finds the contact and opens the edit sheet); then make naming the primary action on an `auto` row's detail | The spec's own stance |

## Project Structure

### Documentation (this feature)

```text
specs/038-first-run-parity/
├── spec.md
├── plan.md              # this file
├── research.md          # evidence behind R1–R17
├── data-model.md        # the state added to core/shells
├── quickstart.md        # how to see each SC on the running apps
├── contracts/
│   ├── pre-paint-attributes.md   # app.html ↔ app.css ↔ gate.ts contract
│   ├── proxy-candidates.md       # candidate order + failure advance
│   └── passkey-icons.json        # six currentColor icons + platform rule
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (touched)

```text
rust/crates/vela-core/src/app/
├── balance_dashboard.rs      # R12, R14
├── login.rs                  # R9 (transport_failed)
├── rpc_pool.rs               # R13 (ConnectivityChanged)
└── send.rs / fee types       # R11 (fallback flag)

app-desktop/vela-wallet/src/
├── intro.rs (new), intro_art.rs (new)     # R3
├── onboarding.rs             # intro routing, endpoint_dismissed, rail without settings, launch gate
├── ui/rail.rs                # settings affordance removed
├── ui/launch_animation.rs    # gate consult
├── executor/proxy.rs         # R7, R8
├── executor/registry.rs      # R9 classification
├── executor/storage.rs       # intro/launch keys
├── resident.rs, main.rs      # R10
├── theme.rs                  # font_ui(), weight audit
└── icons.rs / raster.rs      # non-square rasterize

app-web/vela-wallet/src/
├── app.html, app.css         # R1
├── lib/intro/gate.ts (+test) # pre-paint agreement test
├── lib/ui/intro/IntroCarousel.svelte   # R2
├── routes/+layout.svelte     # R13, R15
├── vite.config.ts            # version polling
├── lib/ui/onboarding/v2/AddMethodPicker.svelte   # R16
├── lib/onboarding/core/passkey-directory.svelte.ts  # timeout
└── lib/contacts/…            # R17

assets/fonts/                 # PlusJakartaSans_{400,500,600,700}.ttf (R6)
```

## Phase 1 design → see data-model.md, contracts/, quickstart.md

## Constitution re-check (post-design)

No gate to fail. The one deviation worth naming: R6 leaves CJK and mono on
OS fonts on the desktop, so SC-424 is met for Latin UI text and the corpus's
CJK locales inherit the platform's CJK face — recorded in results.md when the
work lands, with the TTF sourcing as the follow-up.
