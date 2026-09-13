# Research — evidence behind the plan's decisions

Every line below was read on 2026-09-11 on branch `038-first-run-parity`.

## R1 — intro before first paint (web)

- `src/app.html` carries spec 012's render-blocking block: reads
  `vela.launch.played`, applies the 7-day rule (`604800000`), sets
  `document.documentElement.dataset.launch = 'playing'`; `app.css:90` hides
  `[data-launch-page]` under it. `lib/launch/constants.test.ts` reads
  `app.html` with `readFileSync` and asserts the numbers agree.
- `routes/[locale]/+page.svelte` decides the intro in `onMount`
  (`if (shouldShowIntro()) intro = true`) — after paint. `lib/intro/gate.ts`
  holds the rule (`vela.intro.seen`, `?intro`, `?skipIntro`, throw → show).
- Decision: duplicate the gate in `app.html` (storage key + both params), set
  `data-intro="pending"`, hide `[data-intro-page]` (the Welcome `<main>`), and
  add `gate.test.ts` cases that read `app.html` and assert the key and params
  match `STORAGE_KEY` / `FORCE_PARAM` / `SKIP_PARAM`.
- The window between first paint and hydration shows the page ground only
  (SC-413). On a true first run the launch animation covers it anyway.

## R2 — intro desktop composition (web)

- `IntroCarousel.svelte`'s only desktop rule: `max-width: flowColumn + 2×
  screenPaddingX; margin-inline: auto` at 1280. Everything else is the phone
  layout (`min-height: 100dvh`, footer at the bottom).
- `+page.svelte` (Welcome) and `FlowShell.svelte` both render `OnboardingRail`
  at 1280, left-align a column of `--layout-onboardingColumn + 2×gutter`, and
  size buttons to labels (`.actions :global(.button) { width: auto; min-width:
  var(--layout-welcomeCtaMin) }`). `rail.ts` exports `RailSlot` with a
  `tagline` kind — the intro uses it with the same tagline as Welcome.
- e2e conventions: `welcome-layout.e2e.ts` tests 390×844, 1279, 1280, 1440 —
  the intro gets the same four.

## R3 / R4 — intro on the desktop

- No intro module exists (`grep -ri intro src` → comments only).
  `onboarding.rs` owns `launch: Option<LaunchAnimation>`; `render()` overlays
  it and fades the page by `page_opacity()`; `LaunchAnimation::new` is
  constructed unconditionally unless `theme::launch_disabled()` (env var).
- The contract `specs/020-intro-carousel/contracts/intro-illustrations.json`
  (viewBox 160×128, roles line/accent, modes stroke/fill/outline) names the
  consumer "desktop resvg". `icons.rs::rasterize(svg, size)` renders a square
  via `resvg::usvg::Tree::from_str` → `tiny_skia::Pixmap`; `raster.rs`
  converts pixmaps to gpui `RenderImage`. Generalise to (w, h).
- Drag: no horizontal drag exists yet; `window_frame.rs:96-114` shows the
  `on_mouse_down` / `on_mouse_move` / `on_mouse_down_out` pattern with a
  global for "drag pending". Keyboard: `KeyDownEvent` is already imported in
  `onboarding.rs`.
- Storage: `storage.rs` exposes `read_value(key) -> Option<Value>` and
  `write_value(key, Value)` over one JSON file with tmp+rename.
- Corpus: `onboarding.intro.*` keys ship in all locales; `loc.rs` `t()` is
  total.

## R5 — endpoint sheet

- `onboarding.rs:988-990`: `if login_view.endpoint_unreachable &&
  endpoint.is_none() && !creating { open_endpoint(true) }` runs every render;
  Close sets `endpoint = None`. `EndpointSurface::automatic` is read only at
  `:785` for the warning line. `save_endpoint` dispatches `Event::Start`
  which resets the health state — the right moment to clear a dismissal.
- The rail (`ui/rail.rs:47-52`) takes `settings_label` + `on_settings`; the
  one call site is `onboarding.rs:1030`. The warning line at `:610` opens the
  sheet on click; that door stays.

## R6 — typeface

- Web: `routes/+layout.svelte:2-10` imports `@fontsource/plus-jakarta-sans`
  400/500/600/700, Noto Sans SC 400/500/700, IBM Plex Mono 400/500;
  `tokens.css:99` `--font-ui`. Weights: 90× semibold, 72× bold, 19× medium.
- Desktop: `theme::font_mono()` only (Menlo / DejaVu Sans Mono); no
  `add_fonts`, no UI family → gpui default. Weights: 68× SEMIBOLD, 54× BOLD,
  8× MEDIUM, 1× EXTRA_BOLD (`logo.rs:71`, the wordmark; web uses 700),
  buttons `button.rs:115,168` BOLD (web `Button.svelte:91` semibold).
- gpui API (`text_system.rs:102`): `add_fonts(Vec<Cow<'static, [u8]>>)`.
  TTFs available in-repo: `app-ios/VelaWallet/VelaWallet/DesignSystem/Fonts/
  PlusJakartaSans_{400Regular,500Medium,600SemiBold,700Bold}.ttf`. No Noto
  Sans SC / IBM Plex Mono TTF anywhere; fontsource ships woff/woff2 only,
  which fontdb cannot load.

## R7 / R8 / R9 — the network path

- `executor/proxy.rs`: `system_proxy()` = `Proxy::try_from_env()` first
  (`ALL_PROXY` before `HTTPS_PROXY`), else `desktop_proxy()`; cached in a
  `OnceLock` for the process. `desktop_proxy()` is GNOME `gsettings` on Linux,
  WinINET via ureq on Windows, `None` on macOS with the (wrong for non-TUN)
  premise quoted in the spec.
- `scutil --proxy` on the founder's Mac:
  `HTTPEnable 1 / HTTPProxy 127.0.0.1 / HTTPPort 1088 / HTTPSEnable 1 /
  HTTPSProxy 127.0.0.1 / HTTPSPort 1088 / SOCKSEnable 1 / SOCKSProxy 127.0.0.1
  / SOCKSPort 1080 / ExcludeSimpleHostnames 1` (an `ExceptionsList` array
  appears when set). Note the SOCKS entry names the dead 1080 too — which is
  why HTTPS comes first and why a refused candidate must advance.
- Live check: `curl https://p256-index-v2.getvela.app/api/health` → 200 in
  1.1 s via the 1088 HTTP proxy and 1.5 s direct; `socks5h://127.0.0.1:1080`
  → connection refused; `lsof` shows only v2ray on 1088.
- `registry.rs::classify()` maps every non-`StatusCode` ureq error to
  `network: true`; the core's `LoginView.endpoint_unreachable` is the only
  consumer.

## R10 — panics

- `resident.rs::pump` is where every machine's blocking/streaming work is
  handed to `cx.background_executor().spawn` — one choke point. Other
  `thread::spawn`s (`ceremony.rs`, `passkey.rs:503`, `l2cap.rs:209`) already
  `let _ = join()`. No `panic::set_hook`; no `panic = "abort"` profile.
- `outcome.rs::Prompt { kind, details, … }` + `ActionId::ReportError` copies
  details to the clipboard — the sheet to reuse.

## R11 — money fallbacks

- `executor/user_op.rs:206,449,546` log "estimation unavailable / failed,
  using defaults" and continue with defaults; nothing reaches the view.

## R12 / R14 — balance rules (core)

- `balance_dashboard.rs`: `display_total` = live sum when any tokens exist
  (`max(live, cached)` only when `balance_partial`); `chain_assets_arrived`
  merges per chain → the total climbs as chains land. `view()` raises
  `BalanceNotice::Unpriced` whenever `partial && notice_allowed` and no chain
  failed — true mid-stream before prices arrive. `refreshing = pending_pulls >
  0`. `FetchErrored` sets `bootstrapped = true` and keeps last-known; with
  nothing known, `unknown` becomes false and `display_total_usd = Some(0)`;
  `live.ts:211` `zeroLive` then reads it as an empty wallet.

## R13 — web offline / version

- No `navigator.onLine` in `app-web/src`; no `src/hooks.client.ts`; kit
  options live inline in `vite.config.ts` `sveltekit({...})` (no
  `svelte.config.js`), so `version` goes there. Pool cooldowns:
  `COOLDOWN_BASE_MS` 30 s doubling to `COOLDOWN_CAP_MS` 300 s;
  `TEMP_BAN_TTL_MS` 1 h.
- The one bare `fetch()`: `passkey-directory.svelte.ts:60`.

## R15–R17 — the issues

- #189: `$lib/assets/favicon.svg` is the SvelteKit template logo
  (`<title>svelte-logo</title>`); `routes/+layout.svelte:91` links it.
- #190: `AddMethodPicker.svelte` `.icon` is a bordered box; three
  `[data-method]` rules set 24×16 / 16×24 / 24×12. Desktop rows have no icon.
- #191: `ContactsView.contacts` is the merged book (saved ⊕ history-derived);
  `openEdit` finds the row in it and opens the edit sheet with `name ?? ''`.
  The reported "cannot name" is not visible from the code — reproduce first.
