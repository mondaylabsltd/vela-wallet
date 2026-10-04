# 099 — Research (Phase 0)

Every decision below follows the owner's rule for this spec: **「跨端同样要使用 crux core 来保持规则逻辑一致性」**
— the rule lives in `vela-core` once; desktop, iOS, Android (and web where it applies) execute it.
Facts are cited from the code as of `main` @ `553bf88db` (2026-10-04).

## R1 — Several webviews in one desktop window

**Decision**: one `wry::WebView` per tab, each `build_as_child` of the main window, shown/hidden with
`set_visible` and moved with `set_bounds`; a `BTreeMap<tab id, Browser>` replaces the
`thread_local! BROWSER: Option<Browser>` (`webview.rs:366-389`).

**Why it works**: wry 0.56 `build_as_child` adds a child NSView (macOS) / WebView2 controller
(Windows) to the window; children are independent and hidden ones keep running. Every callback the
view installs today — IPC (`on_ipc` 1423), load, meta, leave — is a closure built per view, so each
can carry its tab id from the start (today they all report to sinks that assume `"browser"`).

**Per-platform notes**: macOS — each view keeps its own `NavigationGate` (095, held weakly by
WKWebView, so it lives in the `Browser`); WKWebViews share the default website data store, so
cookies/storage are shared across tabs as in any browser. Windows — one `WebContext` (profile
folder) shared by all views (`_context` moves out of `Browser` into a single holder); the menu
"hole" (083 D2) applies to the shown view only. Linux: no in-app browser (unchanged).

**Alternatives rejected**: keep one view and snapshot/restore DOM state — impossible in general
(sockets, JS heap); one window per tab — not the product.

## R2 — Which tabs keep a live engine (all clients)

**Decision**: a pure core function `browser_tabs::plan_engines(..) -> EnginePlan` that every shell
calls, with the inputs owned by the two machines that know them:
- `explore_sites` keeps **recency** — a most-recently-used order of tab ids, updated on
  `TabOpened`/`TabSelected`/`TabClosed`, exposed in `ExploreView.recent_tabs` (today there is no
  MRU, `explore_sites.rs:98-107, 528-534`).
- `dapp_browser` reports **busy** per tab — `DbrTabView.busy`: an open request, a read in flight
  or queued, or a signing job / queued signature / consent from that tab
  (`dapp_browser.rs:368-391, 440-441`).
- The rule: the selected tab is live; busy tabs are live (never suspended); other live tabs stay
  live in MRU order up to `LIVE_TABS_CAP` (6); the rest of the live ones are suspended, least
  recently used first; a dormant (restored or suspended) tab is woken only by being selected.

**Why**: the phones already keep per-tab engines with no cap (iOS `BrowserController.swift:77,
591-604`; Android `BrowserController.kt:769, 827`), and the desktop is about to; one rule keeps
"which tab reloaded and why" identical everywhere.

## R3 — Per-request record and the layer vocabulary

**Decision**: `dapp_browser` keeps, per tab, a ring of the last `REQUEST_RECORD_CAP` (200) settled
requests: id, method, class (local / read / relay-backed read / sign / consent), layer that ended
it, outcome (result or EIP-1193 code), start and end time. A typed `RequestLayer` (browser ·
provider · client · network · relay · signer) with `layer_reason_key(layer, code)` corpus keys —
the `browser_load::reason_key` pattern (`browser_load.rs:241-249`), the closest template.

**Time**: `dapp_browser` has no clock (only `AccountSwitched` and `ConsentApproved` carry
`now_ms`). The events that start or end a request gain `#[serde(default)] now_ms`:
`PageMessage`, `ReadDone`, `UserOpResolved`, `SigningAnswered`, `ConsentRejected`, `TabClosed`,
`RendererGone`, `NavigationStarted`, `LoadFinished`. `0` reads as "unknown" — no shell breaks before
it sends the time.

**Size**: the record reaches a view only for the tab whose inspector is open
(`Event::InspectorOpened{tab}` / `InspectorClosed`); every other render carries counts only
(`open_requests`, `failed_recent`). Views stay small at sixty frames a second.

**Rejected**: logging only in shells — the vocabulary would drift per shell, which is what the
owner ruled out.

## R4 — Every read settles

**Decision**: the deadline is core policy, enforced where reads run: `DbrOperation::Read` gains
`deadline_ms` (`READ_DEADLINE_MS` = 30 000); each shell's read executor stops at the deadline and
answers `ReadAnswered{body: None}` — the existing −32603 path (`dapp_browser.rs:1677-1752`) — and
the record names layer `network`, reason "did not answer in time" when the shell says it timed out
(`ReadAnswered{timed_out: true}`).

**Why not a core timer**: `dapp_browser` has none and adding one means a timer executor in every
shell; the read executor already owns the I/O and its cancellation. The desktop pool bounds an
attempt at 8 s × 3 (`rpc_pool.rs:168-188`) but nothing bounds a read waiting behind 8 slow ones.

## R5 — The provider, offered or not

**Decision**: `DbrTabView.provider: ProviderState` — `Offered` (the document's bridge said `hello`),
`NotOffered{reason}` (the load finished with no `hello`: insecure context, sub-frame-only, or the
script did not run), `Pending` (loading). The core already retires a document whose load finished
without `hello` (`dapp_browser.rs:593-602`); this names it.

## R6 — The landing says what the relay is doing

Facts: the first status ask is ≥ 12 s after submit (`STATUS_POLL_INTERVAL_MS`,
`tx_tracker.rs:107, 1562-1585`); the fee hold is classified only when the 120 s window closes
(1529); an acknowledged op ignores `not_found` forever (`in_doubt`, 828-830; 1372-1399), and gets no
status polls past the window.

**Decisions** (all in `tx_tracker`):
- `FIRST_STATUS_POLL_MS` = 3 000: the first status ask 3 s after acceptance, then every 12 s.
- `TrackEntryView.relay_sent_at_ms` — when the tracker first learned the bundle is on the network
  (`submitted`/`included`, a named bundle tx, or a receipt). `send` and `sign_request` pass it on.
- The countdown ladder itself (`elapsed < typical` → "~N s left", `< 2×` → "N s so far", else
  "taking longer than usual", and the ring curve) is written four times today (desktop
  `flows/live.rs:3776-3870`, web `etaLines`/`ring.ts`, iOS, Android). It moves to one pure
  `tx_tracker::landing_pace(sent_at_ms, typical_s, now_ms)`, counted from the relay's send.
- **Forgotten**: an acknowledged op that the relay answers `not_found` for — its record expired —
  keeps being asked (status at the slow receipt pace) and its find-event scan runs. It ends
  `NotSent` after `FORGOTTEN_NOT_SENT_MS` (10 min) from the first `not_found`, two `not_found`
  answers, and a caught-up scan with no event; an event or receipt confirms it instead.

## R7 — Why the confirm is shut

Facts: staleness does not shut confirm in the core (`fee_policy.rs:158-163, 1964-1965`); each
shell ANDs `confirm_gate_open`, `GuardView.confirm_allowed` and `FeeView.confirm_fee_ready` itself
with no reason (desktop `signing/live.rs:57-70`, Android `SigningLive.kt:440`, iOS
`SigningController.swift:697`) — the desktop lock on a stale fee (2026-10-04) is shell-local.

Beyond the three gates, each shell adds rules of its own: a message has no fee to wait for
(`off_chain`), another speed's figure is not this speed's (`fee_of_another_tier`), a request not
yet read cannot be signed (096 F7) — desktop `signing/live.rs:57-70`, Android `SigningLive.kt:436-444`,
iOS and web their own copies.

**Decision**: `sign_request::confirm_state(ConfirmInput{sign, guard, clear, fee, speed_tier}) ->
ConfirmState{enabled, block}` holds all of it, with `ConfirmBlock` naming the first gate that is
shut (in flight, reading, refused, account switching, funding, answered/held, approval choice,
batch unsettled, fee measuring, fee failed, fee short, fee missing) and a corpus line each; every
shell draws the line under the disabled slide with the matching action. A stale fee stays
advisory (`fee_policy`'s rule; the sheet's refresh affordance is unchanged).

**Correction (2026-10-04 device pass)**: the explanation given then — "the fee went stale and the
slide stays locked until it is refreshed" — does not match the code: no client gates on
`stale`. What shut the slide is not established; the candidates the code allows after a failed
passkey are the held failure (`responded || held`, 096 F8, until Try again) and a fee re-quote in
flight. `confirm_state` makes whichever it was name itself, which is the point of R7.

## R8 — The signer says how it failed

Facts: the core knows `FailureKind{Cancelled, NotSupported, NotDiscoverable, Other}` (`mod.rs:617-626`)
but `sign_request`'s `SignSubmitOutcome::Failed` carries only free text; the desktop maps "not a
signed app bundle" to `NotSupported` (`platform_macos.rs:394-420`) and then loses it
(`user_op.rs:371-377`).

**Decision**: reuse `FailureKind` — every shell already classifies its platform's passkey errors
into it for create and login. `SignSubmitOutcome::Failed` gains `signer: Option<FailureKind>`;
`SignErrorKind::SignerFailed` answers the page (−32603) and the notice carries the kind, so the
sheet and the request record name the signer and how it failed.

## R9 — Phones and web

iOS/Android already keep per-tab engines; they adopt R2 (eviction), R3/R5 (status and record),
R6–R8 (same words). The web wallet's dApp path (extension) shares R6–R8 through the same core;
its tabs are the browser's own and out of scope for R1/R2.
