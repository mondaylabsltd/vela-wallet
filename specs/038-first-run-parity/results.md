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

### Slice 4 — the founder's evening pass, Part E (web)

- #E2 a token's rows open the transaction detail: `feedPositionOf` (the
  inverse of `feedItemAt`) gives each token-detail row the history's own
  index; `TokenDetail` (phone sheet) and `AssetDetailPanel` (desktop third
  column) take `onselect`.
- #E5 the language row ticks the page's locale, not the stored choice.
- #E6 the batch rate is editable in place (`edit_rate` / `reset_rate_to_auto`
  were the core's already); loading/failed states use their own corpus
  sentences (`batchRateLoading` / `batchRateFailed`, now served). The unit
  toggle's wiring is correct on paper (`set_unit` with the same ids the
  toggle compares) — the founder's "无法切换" still needs a reproduction.
- #E7 the split recipient card: grid, two lines when editable, the amount
  field content-sized (`field-sizing: content`, 6–12ch), a hairline under
  each editable field.
- #E8 the balance dialog gains a third section listing every unpriced token
  (symbol · network · balance), titled with the hero's own sentence.
- #E1, #E3, #E4 recorded and pinned in the spec; not fixed in this slice
  (#E1 needs the eleven probes run against Celo from the desktop; #E3 needs
  the surface reproduced; #E4 is a server task).

### Slice 5 — Part D, icons v2, E9

**D1** — core `TrackShellResult::ReceiptWithLogs` (additive; both shells now
send the receipt's authentic logs) and `safe_execution_failed`: a log with
topic `keccak("ExecutionFailure(bytes32,uint256)")` = `0x23428b18…7d23`
(pinned against the core's keccak) turns a `success: true` receipt into
`ReceiptFailed`. A live reproduction of the founder's failed-but-confirmed
send is still owed — the guard covers the one mechanism the code allows.

**D3** — `NetBuiltinChain.typical_inclusion_s` for the 12 builtin chains
(block time × usual depth: Ethereum 24 s, Gnosis 15 s, BSC 9 s, Polygon 6 s,
Tempo 5 s, the OP-stack/Arbitrum/Avalanche/World 4 s, Unichain/Monad 3 s);
`SendReceiptView.submitted_at_ms` + `typical_inclusion_s`. The corpus already
had the three sentences (`send.txTypicalTime`, `txElapsed`, `txSlowConfirm`)
from the Expo app — served now. Web `SendReceipt.svelte` counts once a
second; the desktop's send watcher keeps ticking while a receipt is submitted
and re-renders per second.

**D2** — the split's confirm lists every recipient (name-or-address, amount;
identicon on the web) under a "N recipients · network" subline. The receipt
and the activity detail do not yet list them (T074j partial). Desktop rows
have no identicon (BreakdownRow carries label/value only).

**Icons v2** (founder's revision) — "this device" is the device (lucide
laptop / smartphone by pointer coarseness on the web; laptop on the desktop),
"phone or tablet" is the scanner (lucide scan-line), the USB key keeps the
founder's mark; the vendor marks stay in the contract for the provider line.
Web sign-in opens in `Sheet`, which is now a centred dialog at ≥1280.

**E9** — the network filter lists networks the person added at count 0
until their balance lands (`liveChainRows(…, customChainIds)`).

**E1, measured** — Celo (42220) via forno: all 11 `REQUIRED_CONTRACTS` have
code (69–24 421 bytes) and the RIP-7212 precompile at 0x100 answers `…01` to
the core's own payload. Celo is compatible; the desktop's "not compatible"
was a failed probe (its old proxy path) flattened to that verdict — the
module doc's own admitted trap. The verdict split is still to build.

### Slice 6 — E1 verdict split, the estimate label, the pool on the chain

- #E1: `NetWizardErrorKind::CheckFailed` (additive) — the scan path answers it
  when the probes failed instead of flattening to `NotCompatible` (the
  `add-network.ts:47` port was the trap; its test now pins the split). Web:
  "Unable to verify — RPC request failed" with re-check, no setup tool;
  desktop: the same sentence via `wizard_unable_to_verify`.
- Finding 14 (fee fallbacks): the core already carried `FeeEstimateView.quoted`
  ("the relay's own quote, not a local fallback"); both shells now label such a
  row "Estimated fee" (`send.feeTokenEstimate`) instead of "Network fee".
- T028: `proxy::with_candidates_for(url, …)` — loopback stays direct, every
  other RPC-pool call walks system → env → direct; `agent_for` retired.

### Slice 7 — the share card (#meta), the weight walk

- #meta: `SocialMeta.svelte` on the landing page — `og:site_name/type/title/
  description/url/image(+size)/locale(+14 alternates)`, `twitter:card =
  summary_large_image`, `twitter:title/description/image`. The image is
  `static/og-image.svg` (the canonical mark at 4× on the brand ground,
  text-free so one card serves 15 locales) rendered once to `og-image.png`
  by `scripts/gen-og-image.mjs` (Playwright Chromium) and committed.
  **Deviation**: no new corpus key — `og:description` is `metaDescription`
  clipped at a word boundary to 110 characters, which is what a preview does
  to a 154-character sentence anyway, minus the mid-word cut.
- T044 walked: the desktop's `FontWeight::BOLD` on body-sized text is limited
  to two ✓ glyphs (success disc, ack box) and the Done screen's wallet name,
  which the web also sets bold. No further drift beyond the button and the
  wordmark fixed in slice 2.

### Reproductions on the running web app (dev server, parallel space)

- **#191 — reproduced as WORKING at this branch.** Seeded a confirmed send
  (`saveTransaction`, to `0x600746…f95f4d`) → Contacts lists it under "#" as a
  history-derived row → its detail has **Edit** → the sheet has a NAME field →
  "Bob from history" → Save → the row re-sorts under "B" with the name, and
  the detail shows it. On the desktop layout the Edit button sits at the
  bottom of the third column; the founder's report was on a build before the
  028 contacts completion or missed that affordance — the phone sheet is
  checked below.
- #meta e2e: `welcome-ssr` 43 passed on the isolated build — the card tags on `/en` and `/zh`, `og:description` ≤ 111 chars, 14 alternates, `og-image.png` served as PNG; the first-paint intro case passes too.

### Slice 8 — #E3 on both shells

Reproduced live (dev server, parallel space, a `Sent` record seeded at
2026-06-13 12:00; Settings → Localization → Date format → "13.06.2026").

- **Web**: the detail column read `13.06.2026 12:00 PM`, so `formatDate` and
  the preference were never the fault. The desktop-width feed printed no day
  heading at all — `WalletDesktop.svelte` iterated `activityGroups` without
  the `<p class="day">` `WalletHome.svelte` prints — so a chosen format had
  nowhere to show outside the detail. Fixed: the heading, with the phone's
  rule. After the fix the same page reads `Today` / `13.06.2026` over the
  two records.
- **Desktop**: the number, date and time rows in Settings → Localization
  were the mock's literals with one drawn (inert) menu; `day_label`
  hardcoded `DatePreset::Iso`, every clock `TimePreset::H24`, every figure
  `NumberPreset::CommaDot` (eleven sites across `flows/live.rs`,
  `wallet/live.rs`, `signing/live.rs`, `page.rs`, `settings/live.rs`).
  Fixed: `executor/format_prefs.rs` — one stored object (`vela.formats`,
  the web's words `dmy_dot` / `h12` / `comma_dot`…, unknown words read as
  Automatic), "Automatic · System" resolved from the same locale ladder the
  strings use (`loc::requested_tag`) through a table that answers what
  `Intl` answers the web for the shipped locales (pinned for 18 tags);
  `settings/live.rs::format_menus` builds the three menus (auto row first,
  the web's order, tick on the row in force) and `dropdown_menu_picks` is
  the first menu on this shell whose rows answer a click; `pick_format`
  persists and closes. The gallery and `VELA_PAGE=settings` keep the mock.
  Under test the machine tag is pinned to `en`, so a figure a test pins does
  not depend on the machine running it.
- Not done here: the currency row on the desktop still has no picker (a
  032-era debt, not E3); the desktop's fiat figures still print `USD`/`$`
  regardless of the display currency in several live constructors — noted
  for the desktop parity backlog.
- Desktop 393 passed / 0 failed (+9 tests); web wallet suite 92 passed;
  svelte-check 0 errors; `cargo fmt` clean.

### Slice 9 — #E6, the half that needed a reproduction

Wallet → Send → XDAI → Add recipient → Import list, at desktop width. The
third column opened on the FIXTURE: "In CNY / In USDT", "1 USDT = 7.25 CNY",
three sample rows, "Import 2 recipients". Pressing "In USDT" changed
nothing. The session was real — `openBatch()` had started it and the nav
had routed to `dsd2c` on `batchView` — but `withLiveDesktopBody` had no
`batch-import` arm: the importer is a sheet on the phone (`sd2c`, overlaid
by the sheet arm) and a column body on the desktop, and only the sheet
shape had ever been wired. Every tab press and keystroke reached the core;
the drawn column never reflected it.

Fixed in `flows/live.ts` (the arm) and `flows/live-batch.ts`: the tabs,
the rate label and the hint are worded from `fiat_code` and the token
symbol (the fixture's CNY/USDT was a picture), and the CTA offers what
parsed — `batchApplyEmpty` / `batchApply_one` / `batchApply_other` (the
two extra keys already existed in the corpus; they join the served list).
Tests: `live-batch.test.ts` +2 (the words from the view; the desktop body
overlaid). Flows + i18n suites 173 passed.

Seen while reproducing, not changed: in fiat mode the core converts each
line at the token's full precision — 0.001 USD at 0.9996 became
`0.001000400160064026 XDAI`, sixty times over, on the preview and on the
confirm. `fiat_to_token_amount` (ported verbatim from the phone) rounds at
`decimals`; a payroll line probably wants the token's *display* precision
(the same six places the balance rows use) or the fiat's own cents carried
into the token. That is a money-rounding rule and the founder's call —
raised in the report rather than decided here.

### Slice 10 — #D2, the receipt and the detail

The confirm listed a split's recipients since slice 5; the receipt still
said "To " with nobody after it (the single-recipient line over an empty
`recipient`), and a folded batch row's detail showed no counterparty and no
parts — `FeedItem.batch.transfers` was never read by either shell.

- **Web**: one `flows/ui/Breakdown.svelte` (the confirm's list, extracted,
  with an optional title) is drawn on `SendConfirm`, `SendReceipt` and
  `TxDetail`. `liveSendReceipt` builds the parts from the receipt's frozen
  `transfers` (a split) or the drafts before the core freezes them, titles
  them "N recipients" and puts that title in the confirmed caption;
  `liveTxDetail` builds them from `item.batch` — a split's recipients with
  avatars, a sweep's assets with their marks. Tests +3.
- **Desktop**: `BreakdownRow.seed` (the avatar), `SendReceipt` and
  `TxDetail` gain `breakdown_title` + `breakdown`; `panels.rs::breakdown_list`
  draws the one list on the confirm (now with avatars), the receipt and the
  detail; `flows/live.rs::receipt_parts` / `detail_parts` word them, the
  same rules as the web. Test +1 (a split receipt lists and counts).

### Slice 11 — #E4, the client half

The lookup for an AAGUID the vendored catalog cannot name now goes wherever
the person's service-endpoints object says (`aaguidDirectoryURL`), on both
shells, with the core's catalog still answering first and offline. Until
our node exists the default is the directory the core named; moving every
client to ours is a one-line default change, no build in between.

- Core: `directory_lookup_url_at(origin, aaguid)` / `directory_entry_at`
  (the mark's URL is built on the origin that was asked); the old
  functions delegate with the constant. The wasm exports take an optional
  origin (artifact rebuilt; source fingerprint refreshed). +1 test.
- Web: `DEFAULT_SERVICE_ENDPOINTS.aaguidDirectoryURL` + `getAaguidDirectoryURL()`,
  the `ServiceEndpoints` field, and `passkey-directory.svelte.ts` passes the
  origin to both wasm calls.
- Desktop: `passkey_directory::directory_origin()` reads the same object
  the fiat-rates URL does (+1 test), and the lookup's `fetch` now walks the
  proxy chain (`proxy::agent`) — it was the one request on this shell that
  ignored the person's proxy (a Part B miss, found here).
- Not in this repo: the node itself (biubiu-projects) and a Settings →
  Service Endpoints row for the field (needs two corpus keys; the field is
  reachable through the stored object today).

### Slice 12 — #D4, the send journey's visual pass (web)

Live on the dev server in the parallel space, at 1440 px: `screenshots/`
`web-send-form-1.png`, `web-confirm-1.png`, `web-receipt-confirmed.png`
(one real 0.001 XDAI send on Gnosis — the waiting state read "Submitted to
the network · Waiting for blockchain confirmation… · Gnosis typically
confirms in ~15s · 1s elapsed — almost there" and landed before the
screenshot could; `web-receipt-waiting.png` therefore shows the confirmed
frame too), `web-import-60.png`, `web-send-form-60.png`,
`web-confirm-60.png` + `-top.png`, `web-confirm-12.png`, `web-confirm-3.png`.
Against the design language: open heroes, hairlines, subordinated fiat,
one accent per screen — held at every count. What fell out:

1. **The receipt's hash ran off both edges** — 66 characters on one line,
   label hidden, and the copy button beside it copied nothing (the
   handler only flashed the tick). Now the two ends with the whole hash as
   the title, and the button writes the clipboard.
2. **A refused estimate was invisible on the web.** The first sixty-row
   attempt (addresses `0x…0001`–`0x…003c`) was refused by the relay's
   simulation — "Safe execution failed: the target call in executeUserOp
   reverted" (which of the low addresses reverts was not pinned; a later
   two-row batch including `0x…0009` estimated fine, the retry from
   `0x…1001` upward went through); the core raised
   `ShowAlert { estimate_failed }` and the page had `console.warn` where
   the sentence belonged — total blank, fee "—", Continue doing nothing. The
   alert is now a line on the form and the confirm, worded from the same
   ten corpus keys the desktop uses (`alertWords`, served list +10), kept
   until the person edits a field or moves a stage. Verified live with a
   malformed address (`web-form-alert.png`: "Invalid Address · Please enter
   a valid Ethereum address (0x...)"); the line came down on the next
   keystroke into the field.
3. **The split's "Total" row had no figure** on both shells — it read
   `token_amount`, the single field, empty in a split. Both now read the
   core's sum (`confirm_amount`).
4. **The column kept the previous screen's scroll offset**: a confirm opened
   after a long split form started on its last rows, hero and count above
   the fold. `ThirdPanel` scrolls to its top when the body kind changes.
5. Casing: the fee row prints the relay's `xDAI` beside the token's `XDAI`
   on one screen — the relay and the chain data disagree on the native
   symbol's case. Not changed (the relay's word is the fee's); noted.

Not seen, and worth a founder ruling: the fixture counterparties seeded for
#E3 (`0x600746…`, `0x1111…`) draw a `400` from the p256 index on every
history load (`api/query?walletRef=`) — the index says "unknown ref" with a
client-error status, and the console shows it as an error each time.

**Desktop**: the same three panels (`breakdown_list`, the receipt, the
detail) are built and tested, but this session cannot capture the gpui
window (`screencapture` needs screen-recording consent this shell does
not have; the earlier attempt captured the editor). The founder's own run
is the desktop half of SC-429.

