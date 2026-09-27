# Tasks: the desktop wallet is the web wallet (078)

`[x]` done · `[ ]` open. IDs refer to research.md. Each task ends with the
suite green, clippy with no new warnings, and — for anything visible — a
screenshot checked against the web.

## Phase 0 — landed before and during the spec
- [x] T000 Windows build, `velawallet://` relay, empty-path callback, `dist/`
      bytes, sidebar/Wallet/Settings layout, clipboard chords, send picker and
      form, share-card identicon, switch-back cache, hero ladder (see spec
      "Done on this branch").
- [x] T001 S-01 / G-02 Close closes the signing column; Escape dismisses it.
- [x] T002 spec.md, research.md (four audits), plan.md, tasks.md.
- [x] T003 (owner call, 2026-09-24) Linux has no Explore, as the web has
      none: the sidebar shows three destinations (`Section::available`), and a
      pinned or restored Explore opens the wallet. A real browser was built —
      WebKitGTK in a window of its own, since a Wayland client cannot host
      another program's page — and parked on the local branch
      `linux-dapp-browser-wip`: a connect or signature still had to be
      answered back in the wallet window.

## Phase 1 — foundations (X)
- [x] T010 X-01 theme: `border_strong`, `accent_soft`; dark `bg_sunken`
      #0F0F0D; check every dark use of the old value.
- [x] T011 X-02 buttons: primary / secondary / danger at the web's 52 / 17
      semibold / 8·24 / r12, `border_strong` outline; replace
      `ghost_button` / `accent_button` call sites.
- [x] T012 X-03 third column: header 16/24, close 36 with a 20 icon, content
      0/24/24, scrolls with the bar, scroll reset on a new subject.
- [x] T013 X-04 dialog primitive (`ui/dialog.rs`): 520 / 24 / r16 /
      hairline / shadow-lg / scrim .35 / Escape + scrim close. On it: the
      switcher, the identicon viewer, settings (add network, fix RPC, erase),
      every confirm, sign-out, remove network, the contact / group / explore
      name forms, the contact QR, the import result. Account removal is
      inline (H-01).
- [x] T014 X-05 search field as a real input that filters: token pick,
      contact pick, receive list, contacts (with ✕), empty-result line.
      (Assets too. Contacts filter in the shell, as the web's
      `letterSections` does — W-06's "never dispatches `query`" is the
      web's own design, not a gap; per-group import/export still open.)
- [x] T015 X-06 copy button with tick feedback; wire fact rows, receive
      network rows, key rows (reset after 1.2 s), contacts.

## Phase 2 — P0 behaviour
- [x] T020 G-01 (keyboard path still open) slide to confirm: drag, commit ≥ 88 %, spring back,
      Enter/Space, `accent_soft` fill, label fades.
- [x] T021 G-03 refused request: one full-width Close, raw data hidden.
- [x] T022 C-01 delete contact / group asks first (danger button).
- [x] T023 E-02 custom-group rows open their own site; unique ids.
- [x] T024 H-07 hero decimals split on the person's decimal mark.
- [x] T025 M-01 core: Max during the fee check or the credential load does not
      strand Continue; test in `tests/app_send.rs`.

## Phase 3 — P1 features
- [x] T030 H-01 account switcher dialog from the sidebar header.
- [ ] T031 H-02 (header and switcher rows done; contact rows, recipient
      cards, the signer line open) identicon viewer from every addressed
      identicon.
- [x] T032 H-03 balance status line: RPC-fix dialog / balance breakdown; the
      web's text order and colours.
- [x] T033 F-01 scan from the send form (recipient card button, contact
      picker row) → scanner → `scan_resolved`.
- [x] T034 F-02 scanner notices; F-03 scanner look after `ScanSurface`.
- [x] T035 F-04 send receipt stages; G-04 dApp receipt.
- [x] T036 H-04 activity day headers; H-05 listening line.
- [x] T037 H-06 asset detail parity (logo + name, facts, copy, explorer).
- [x] T038 F-05 sweep form; F-06 split rows editable; F-08 recipient label.
- [x] T039 F-07 add token parity.
- [x] T040 C-02…C-08 contacts features.
- [x] T041 S-02…S-05 settings features.
- [x] T042 E-01, E-03 explore features; G-05 technical details.
- [x] T043 M-02, M-03 core Max estimates the real transfer; a fee-coin switch
      re-quotes before Max uses it; M-04 decimal mark.

## Phase 3b — core wiring (W, research.md)
- [x] T044 W-12 Windows Explore: page visible (HWND swap chain, 8 MB stack) and
      built outside gpui's borrow (was: abort on opening a page).
- [x] T045 W-01 receive watcher per visit; W-02 balance refresh on confirmed
      send, incoming item, deposit.
- [x] T046 W-03 not a gap (the web renders no send simulation either); W-04
      AddNetwork via network_admin `AddByChainIdRequested`, the refusal on
      the token list, resolving again after the add.
- [x] T047 W-05 bundler funding pre-check and sponsorship for dApp requests.
- [x] T048 W-06 contacts search + per-group import/export; W-07 signing for
      another wallet.
- [x] T049 W-08…W-11.

## Phase 4 — P2 visuals, surface by surface (screenshot each)
- [x] T050 Wallet home H-08…H-12.
- [x] T051 Flows F-09…F-11.
- [x] T052 Contacts C-09.
- [x] T053 Settings S-06…S-13.
- [x] T054 Explore E-04, E-05; signing G-06.

## Phase 5 — acceptance
- [x] T060 Screenshot pairs for every gallery state both apps draw (SC-001).
  *Done 2026-09-27:* 38 pairs (D1–D3, DC1–DC6, DST1–DST8 + DST4b + DSR1,
  and the 19 flow states), taken on this branch at 1282×801 @175% against
  wallet.getvela.app at the same size and scale, and published for review at
  https://claude.ai/artifact/GAN4Ss8BR3kxW7xHDcT6np. 14 match; 24 still show a gap,
  each named under its pair. The largest are DSD2c (the desktop still draws the
  older import screen: no names, no duplicate row, no over-balance refusal),
  DSR1 (the RPC banner behind the dialog draws empty), the QR centre badge
  (DR2, DR3), the tx-detail row height (DA2, DA3), the send amount's unit
  (DSD2) and the batch recipient rows (DSD2b). Explore and signing have no
  shared fixture state and were compared live in T054.
- [ ] T061 The owner's hour next to the web (SC-003).

## Phase 5b — the gaps T060's pairs showed (screenshot each against the web)
- [x] T062 DSD2c batch import on the web's screen: the unit question, file and
      template as icon pills over the formats line, the rate as one equation,
      rows with identicon, name over address and amount over the sheet's
      figure, the duplicate dimmed with its reason, the refused line with a
      cross, the skipped-rows warning, and the total over the balance with the
      refusal — pinned with the button at the foot. Checked on the fixture and
      live (a pasted sheet: named, unnamed, duplicate and refused lines). A
      disabled accent CTA now fades its fill, not its label.
- [x] T063 DSR1 and DST4b dialogs on the web's measures: the identity name 17
      and meta 11, callouts with a hairline in their tone on a 1.4 line, link
      chips with the external-link glyph 24 under their hint, the report link
      underlined, every settings-dialog button 52 with a 17 label (Add
      Network, the chain setup tool, Save & Retry, the erase pair), and the
      DST4b mock shows the verdict alone, as the live dialog and the web do.
      The "empty banner" was the gallery's chip row pushing the page under
      the dialog, not a defect. The DST4b "4 more contracts" stays: it is what
      the live check counts (seen live on Scroll); the web fixture's 8 is off.
- [ ] T064 DR2, DR3: the QR centre badge (chain / token art, not a grey disc).
- [ ] T065 DA2, DA3: tx-detail row height and the space under the amount.
- [ ] T066 Send form: DSD2 unit and ⇕ switch; DSD2b recipient rows, icon
      pills, stacked total; DSD3 unit size; the live disabled Continue label.
- [ ] T067 Labels and small gaps: DSD1 title, DST1 nav label, DST2 theme
      label, DST4b search field, DST5 counts and key link, DR1 hairlines,
      DT1/DT4 filter row, D2 network chevron, DC1 rail networks, DC2 groups
      and action widths, DC6 menu anchor, DSD4 hash.
