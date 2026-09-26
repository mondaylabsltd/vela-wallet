# Research: every gap between the web and the desktop (078)

Method: four read-only audits on 2026-09-24, one per area, each comparing the
web's Svelte components (the reference) with the desktop's gpui code, citing
both, and resolving the web's values from
`app-web/vela-wallet/src/lib/tokens/tokens.css`. Paths: **W** =
`app-web/vela-wallet/src/`, **D** = `app-desktop/vela-wallet/src/`,
`page.rs` = `D wallet/page.rs`. Line numbers are as of branch point
`9986d77d` and drift as fixes land.

IDs: `X-nn` shared foundations, `H-nn` wallet home, `F-nn` money flows,
`C-nn` contacts, `S-nn` settings, `E-nn` explore, `G-nn` signing,
`M-nn` money logic. Priority P0 (wrong/unsafe) · P1 (missing) · P2 (looks
different).

## Shared foundations — each fixes many surfaces at once

- **X-01 P1 · Colours.** Light matches the web token for token. Missing on
  both themes: `--color-border-strong` (#D8D6CE / dark #3E3E38) — outline
  buttons, fields, the scanner's tools all use it; the desktop reaches for
  `border_card`, which in dark is the raised colour itself, so outlines
  vanish. Missing `--color-accent-soft` (#FFF0EB / #2C1A12, the slide's
  fill). Dark `bg_sunken` is #262622 on the desktop, #0F0F0D on the web —
  sunken surfaces read lighter than raised ones in dark.
- **X-02 P1 · Buttons.** Web `Button`: min-height 52 (`--size-control-lg`),
  17px semibold, padding 8/24; `secondary` = 1px `border-strong`, fg-muted
  text; `primary` = accent, white text, radius 12 when rounded; `danger` =
  filled `error-base`. Desktop `ghost_button` / `accent_button`
  (`D flows/components.rs:984-1014`) ≈ 37px, 13px regular; no danger.
- **X-03 P1 · Third-column chrome.** Web `ThirdPanel.svelte:71-121`: header
  padding 16/24, close 36px (`--size-control-sm`) with a 20px icon, content
  padding 0/24/24, content scrolls (`overflow-y: auto`). Desktop
  (`page.rs` `panel_scaffold_with` ≈4258-4326): 36/8/20 header, 32px close,
  18px icon, content `overflow_hidden` — the asset detail, the signing
  column and every flow cut off their bottoms.
- **X-04 P1 · Dialogs.** Web `settings/ui/Dialog.svelte:51-115`: 520 wide,
  padding 24, radius 16, `bg-raised`, hairline `border-base`, `shadow-lg`
  (0 4px 16px rgba(26,26,24,.08)), scrim rgba(0,0,0,.35), header gap 16,
  title 20 bold, close 32 round on sunken; Escape and a scrim click close.
  Desktop dialogs: padding 28, scrim `bg_base` at .55, no shadow, Escape and
  scrim ignored for the settings dropdown, contact form, group form, QR
  dialog, import result; contact dialogs radius 20.
- **X-05 P1 · Search fields are pictures.** `flow_search`
  (`D flows/components.rs:272-289`) and contacts `search_field`
  (`D contacts/components.rs:530-553`) draw a placeholder and take no input.
  Web filters as you type: token pick, contact pick, receive list, contacts
  (with a clear ✕), and shows an empty-result line (13 fg-subtle, padding
  24). Field: height 52 (flows) / 36 (contacts), `bg-raised`/`bg-sunken`,
  radius 12, 18px icon, 13px text.
- **X-06 P1 · Copy buttons.** Web copy buttons work everywhere and show a
  success tick (150 ms, or "Copied" 1.2–1.5 s). Desktop: fact-row copy icons
  have no listener (`D flows/components.rs:218-220`), receive network rows'
  copy/QR icons are inert, key-row "Copied" never resets
  (`page.rs:7707-7713`), contact copy is silent.

## Wallet home (H)

- **H-01 P1 · Account switcher.** Header click does nothing
  (`D wallet/components.rs:116-165` has no id). Web: the name/chevron/address
  button opens `AccountSwitcher` — a centred 520 dialog (X-04): summary (13
  subtle), one row per account (30 identicon → viewer, name 15 semibold,
  accent when active, mono 11 address, amount 15, 18 accent check, × with an
  inline confirm; padding 12, hairline), create / sign in side by side
  (auto width, padding-inline 32, gap 12). The desktop's Settings → Account
  already draws this content (`page.rs:6877-7067`, `sync_switcher`).
- **H-02 P1 · Identicon viewer.** Web `IdenticonViewer.svelte:57-168`: every
  identicon with an address is a button; card max 440, padding 24, radius
  16, 160 artwork, title 17 bold, caption 13 muted lh 1.6, full address mono
  11 in a sunken 12-radius box, primary "Copy address" (→ "Copied" 1.5 s) and
  secondary "Close", Escape closes. Desktop: none.
- **H-03 P1 · Balance status line.** Web: a button (`BalanceDisplay.svelte:57-68`)
  — unreachable chain → that chain's RPC-fix dialog; otherwise the balance
  breakdown dialog (`settings/ui/BalanceDetailBody.svelte`: pending chains
  with Retry, updated chains with totals, unpriced tokens). Text order: RPC
  unavailable → refreshing (grey) → unpriced (amber). Desktop: plain div
  (`D wallet/components.rs:381-397`), no RPC-unavailable text, still-updating
  drawn amber, shown while hidden.
- **H-04 P1 · Activity day headers** (Today / Yesterday / date; 11 subtle,
  padding-block 4). Desktop drops them (`D wallet/live.rs:1481-1490`).
- **H-05 P1 · "Listening for deposits"** line with a pulsing 8px green dot on
  a zero-balance live wallet; desktop always `live: None`.
- **H-06 P1 · Asset detail**: contract copy button; Price row always (No
  price); explorer drawn disabled when absent; Send/Receive 44 high; hairline
  facts. Owner: "从某个代币点进去，怎么没展示代币 logo 名字".
- **H-07 P0 · Hero decimals** split on `.` regardless of the person's
  decimal mark (`D wallet/live.rs:189`).
- **H-08 P2 · Section headers**: only the trailing action is a button, hover
  → fg-base; padding 12; chevron 14.
- **H-09 P2 · Rows**: padding 12 (64px); no asset-row hover/radius/bleed;
  badge 12 (16 with a logo), border 1.5; token circle 1px border-base; glyph
  10.
- **H-10 P2 · Hero**: decimals 26 (`--text-3xl`); skeleton 55% × 32 r8 pulse.
- **H-11 P2 · Sidebar**: header gap 12, name wraps to 2 lines, chevron
  fg-base, title→list 8, nav radius 12 and hover = text colour only, dark
  sidebar on `bg-sunken`.
- **H-12 P2 · Action pills**: icon 20, hover opacity .92.

## Money flows (F)

- **F-01 P1 · Scan from the send form.** Web: QR button (36 round, 18 icon)
  after the contacts button, and the contact picker's scan row; both open the
  scanner; `scan_resolved` fills recipient/chain/token/amount. Desktop: no
  button; `open_scan = None` (`page.rs:5562`).
- **F-02 P1 · Scanner feedback**: camera refused / none / unavailable, no QR
  in file, invalid QR (keeps the viewfinder, re-arms after 2 s), Flip dimmed
  when no camera. Desktop: silent grey box; a non-payment code closes it.
- **F-03 P1 · Scanner look** (owner: "scan 你的问题是按钮很丑"): modal
  `min(90vw,440)`, radius 20, shadow, padding 0/20/20; header 16 padding, 17
  bold title, 36 borderless close with a 20 icon; viewfinder full width 3:2
  radius 8 with four 30px corner brackets (1.5 fg-base); hint 13 muted left,
  padding 12; tools `flex:1` equal buttons, gap 8, padding 8, 1px
  border-strong, radius 12, 11 fg-base, no icons. Desktop: 560 wide, 336
  fixed well, uneven label-width ghost pills.
- **F-04 P1 · Receipt stages** (`StatusHero`): 88 disc — submitting
  (spinner), submitted (clock + 2.5px ring), confirmed (success), failed
  (error); ETA countdown; short hash with copy; explorer secondary; CTA
  primary once confirmed. Desktop: always a grey refresh disc, full hash.
- **F-05 P1 · Sweep form** (several tokens): summary, raised rows each with a
  Max chip, no amount field. Desktop shows the single-token form.
- **F-06 P1 · Split rows**: address field + contacts button per row, errors
  under the row. Desktop: amount only.
- **F-07 P1 · Add token**: working ERC-20/Native tabs; searching / not-found
  line; "added" chip; field errors; CTA disabled when it cannot act.
- **F-08 P1 · Recipient label** stays "Recipient"; the trust note goes under
  the field. Desktop replaces the label with the note.
- **F-09 P2 · Form bottom**: in a split the total/alert/Continue stick to the
  bottom; refusals are one line (11 error, 14 icon); Continue spins when busy.
- **F-10 P2 · Token pick**: CTA a real pill button; hairline dividers;
  selected row raised r12 bleed 8; dimmed .45; select-all 11; chain notice
  raised 8/12 with the chain logo; chips raised, padding 4/12, 11 medium.
- **F-11 P2 · Contact pick / fee token / confirm / tx detail / history /
  receive QR / network row / status chip / breakdown** — sizes listed in the
  flows audit: 30px avatars, 15 semibold names, 11 mono addresses, raised
  surfaces, 32px amounts on confirm and detail (not 40), fixed 20px tick
  column, danger Delete, network rows with the chain logo, receive address
  card without a well.

## Contacts (C)

- **C-01 P0 · Deletes ask first** (contact, group). Desktop deletes at once.
- **C-02 P1 · Search works** (X-05) with a clear ✕.
- **C-03 P1 · Empty book** shows the empty-state card, its Add / Import wired.
- **C-04 P1 · "View all activity"** works (or "no activity").
- **C-05 P1 · Add/edit contact** in the third column, name AND address
  required.
- **C-06 P1 · Group membership**: multi-select dialog with Save; "+ group"
  chip; "name this contact" for unnamed ones.
- **C-07 P1 · Import into a group / export a group**, CSV/JSON choice.
- **C-08 P1 · Copy feedback** (toast; QR dialog copy button).
- **C-09 P2 · Chrome**: header 16/24 gap 8; Add 36 pill 13 semibold hairline;
  ⋯ 36 round; body padding 24; rail "Groups" hairline 16/12/4; detail
  actions 44/13.

## Settings (S)

- **S-01 P0 · Close** — fixed in `9986d77d` (merge leftover reopened the
  column).
- **S-02 P1 · Backup row states**: could-not-check clickable (refresh icon),
  not-backed-up in warning colour with a chevron, backed-up green check,
  title accent on hover; `backup_for` reset when the column closes.
- **S-03 P1 · Feedback page** missing from the nav.
- **S-04 P1 · Transaction speed / Sign with**: one label row with a dropdown
  whose options carry a description line.
- **S-05 P1 · Add network**: two states (search · chosen chain with verdict).
- **S-06 P2 · URL fields** 44 high, `bg-sunken`, r12, mono 13, success
  border. **S-07** dropdowns r12, right-aligned, ≤560 wide, 52 rows, 18
  check. **S-08** panel ≤560, padding 32/48/48. **S-09** keys block sizes.
  **S-10** account rows (15 semibold, accent active, × remove). **S-11**
  endpoints/providers layout. **S-12** About. **S-13** nav rows r12 18 icon.

## Explore (E)

- **E-01 P1 · Empty start page** (brand mark 56, title 20 bold, caption 13,
  52-high pill) when no favourites and no history.
- **E-02 P0 · Custom-group rows open their site** (desktop switches to
  whatever page was last loaded; ids collide across groups).
- **E-03 P1 · Site menu**: Refresh, Share, Copy link, Add to favorites, Open
  in system browser, Disconnect, Close page — all wired.
- **E-04 P2 · Toolbar/tabs**: back/forward enabled state + hover; filled star;
  start-tab mark; tab radius 8, 20px close box with hover.
- **E-05 P2 · Start page**: ≤800, 8-column grid (20/8 gaps), two-column rows;
  tile label 11; row padding 12; connection panel sizes; chain logo on the
  network dot (currently always Ethereum's colour).

## Signing (G)

- **G-01 P0 · Slide to confirm** is a single click on desktop while it says
  "Slide". Web: drag, commit past 88%, spring back, Enter/Space, accent-soft
  fill, label fades.
- **G-02 P0 · Escape** — fixed in `9986d77d`.
- **G-03 P0 · Refused request**: one full-width Close; raw data hidden.
- **G-04 P1 · dApp receipt** after submit (stages, hash copy, explorer, Done).
- **G-05 P1 · Technical details**: summary line, "Interacting with" row with
  copy/explorer, raw params in a sunken card, mono 13.
- **G-06 P2 · Footer/blocks**: blocks gap 16 padding 16, footer hairline gap
  12; warning border colour, padding 12/16, 16 icon; fee selector 12/16.

## Money logic (M) — the core, every shell

- **M-01 P0 · Max during Continue's fee check** leaves `estimating_gas` true
  forever (`send.rs` `tap_max` overwrites `Pipeline::PreCheck`); Continue
  stays "Estimating…". Max during `ContinueCredential` cancels the Continue.
- **M-02 P1 · Max without a quote** estimates a placeholder, not the real
  transfer; native Max has no buffer (1×) — a re-quote at Continue can bounce
  the person back to the amount.
- **M-03 P1 · Max after a fee-coin switch** uses the previous coin's reserve
  (`ChooseFeeToken` keeps `fee_estimate`).
- **M-04 P2 · Max figure** in the person's decimal mark (web
  `amountToInput`).
- Verified correct: 1× native / 1.5× ERC-20 fee coin / 0 when another coin
  pays (`send.rs:2981-2997`, tests `tests/app_send.rs:1357-1622`).

## Core wiring (W) — is every desktop feature driven by vela-core?

Two further audits (2026-09-24) listed, per core machine, every `Event` the
web dispatches and the desktop does not, and every operation the desktop
answers with a stub. Session, login, create_wallet, clear_signing,
approval_guard, sign_pref, fee_speed, fee_tier_pref, display_currency,
batch_import, rpc_pool, token_trust, tx_tracker, browser_history and
dapp_browser are fully wired. The gaps:

- **W-01 P1 · Deposit watching dies after 5 min.** `receive_watch` runs once
  by design; the web starts a session per receive visit, the desktop keeps
  one resident and never sends `Start` again. `SignalDeposit` refreshes
  nothing on the desktop (web: balance + feed nudge).
- **W-02 P1 · The balance does not refresh when money moves** — the web
  forces one on a confirmed send (`tracker-resident.ts:179`), a new incoming
  feed item and a deposit; the desktop waits for its 10-minute tick or focus.
- **W-03 P1 · send `SimulateCalls` is a stub** (`executor/send.rs:577`, "no
  simulation engine yet") though `executor::sim::simulate` exists and signs
  dApp requests with it: own sends show no balance-change preview.
  *Resolved as not a gap (T046):* the web's send executor does run the
  simulation, but no web screen reads the result (`sim_json` is never
  rendered) — the web's own send shows no balance-change preview either. A
  desktop preview would be a feature the reference lacks, not parity.
- **W-04 P1 · send `AddNetwork` always errors** (`executor/send.rs:445`);
  network_admin's `AddByChainIdRequested` is never dispatched: a payment
  link on an unknown chain can never add it.
  *Fixed (T046):* the executor dispatches `AddByChainIdRequested` and maps
  the wizard's end to the send outcome (saved → added, unknown → not found,
  else not compatible). Two more defects on the same path, both found live:
  the refusal was only built for the send form, while `LockError` shows the
  token list — the "Add this network" button was never on screen; and the
  core's retry after adding did not mark the lock resolving, so the form
  showed empty ("—", 0) until the token reload landed. The web never opens a
  LOCKED send from a scan (its home scan fills the recipient), so it has no
  such screen to compare; the desktop's is the core's refusal drawn on the
  list, with "Checking compatibility..." while the add runs. Noted, not
  fixed: once resolved on Linea the fee row showed "—" with the chain's mark.
- **W-05 P1 · sign_request `CheckBundlerFunding` / `AttemptSponsorship` are
  stubs** (`executor/sign_request.rs:215-243`): no funding pre-check, no
  sponsorship for dApp transactions.
  *Fixed (T047):* `relay::check_funding` and `relay::attempt_sponsorship`
  port `checkBundlerFunding` / `attemptSilentSponsorship` (built-in relay
  only, 25 s denial memory, idempotency key, timeout = maybe-granted).
  Verified against a local relay that forwards to the real one but answers
  `/v1/account` short and `/v1/sponsor` as told: denied → the top-up card;
  granted with a lagging balance → "continues automatically". The card's
  amount subtracted the balance from a figure that already nets it off
  (asking for too little), and printed all eighteen decimals; both fixed.
  `VELA_FORCE_FUNDING=1` is the debug seam for `vela.forceFunding()`. The
  web computes this state but its signing screen never draws it.
- **W-06 P1 · Contacts search** never dispatches `query`; **per-group import
  and export** exist in the core (`ImportFile { into_group }`,
  `ContactExportScope::Group`) but the desktop leaves them `None`.
  *Resolved (T048):* search was built under X-05/C-02 — the web's rule (name,
  resolved name or address; All view only; "No matches for …") — and per-group
  import/export under C-07; `query` is web UI state, not a core event. Checked
  live: "ca" → Carol, "5555" → dave, "zzz" → the empty state.
- **W-07 P1 · Signing for another of the person's wallets**: the desktop
  mirrors only the active account into `sign_request`; `SwitchActiveAccount`
  never tells the session.
  *Resolved (T048):* the desktop opens each request AS the granted account
  (spec 070, pinned by construction) and grants follow the active account, so
  the one-account mirror signs with the same account the web's switch would
  pick. Run live against a local page, it found the real defect: the sheet's
  "Signing account" row was the design mock's wallet, so every request read
  "大表哥" whichever wallet signed. It now shows the core's `signer_address`
  — 666 · Key 2 after switching, 大表哥 after switching back.
- **W-08 P2 · Rate-limited chains reported as broken** —
  `rate_limited_chain_ids` always empty (`executor/balance_dashboard.rs`).
  *Fixed (T049):* the pool thread answers its core's `rate_limited_chains`;
  a failed chain the pool saw only 429s on is "still updating", not broken.
  Live, with Zora's only RPC a local 429 stub: the grey "Some balances are
  still updating" and "Zora · Rate-limited · retrying automatically".
- **W-09 P2 · Feed polls every 30 s** (web 10 s while visible).
  *Fixed (T049):* a 10 s `LiveTick` while the wallet section shows in a window
  that is shown and not minimized (asked of the system each beat); a 30 s
  `FocusTick` otherwise. Logged live: 10 s shown, 30 s minimized or on
  Settings, 10 s again on restore.
- **W-10 P2 · manage_tokens** resident keeps the last Add Token state across
  visits (web starts a session per open); no native-network tab.
  *Fixed (T049):* opening Add Token drops the ERC-20 session (the native tab
  came with F-07). Live: "0x1234" and its error no longer greet the reopen.
- **W-11 P2 · Explore**: `explore_sites` group rename/delete/hide/member
  removal never dispatched; site menu Share / Add to favourites / Open in
  new tab `None`; favourites "Edit" inert; fixture tabs and host shown until
  the first live tab exists.
  *Fixed (T049):* favourites "Edit" and each group's "⋯" open "Manage groups"
  (the web's `GroupManageSheet`): every group, an eye to hide/show, a trash
  for the person's own, "New group". The start page honours the hidden
  flags; a hidden Favorites keeps its heading so Edit can bring it back.
  Checked live: create, hide, delete, Recent hidden and shown, Favorites
  hidden and shown. Group rename and member removal stay unreached — the
  web draws no control for either. The site menu items were done in E-03;
  the fixture tabs are T054's visual work.
- **W-12 · Linux** has no in-app browser and no signing host at all (by
  design since spec 032; out of scope here). **Windows** had one that could
  not be seen or opened — fixed in `f4334ffd`.
