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
