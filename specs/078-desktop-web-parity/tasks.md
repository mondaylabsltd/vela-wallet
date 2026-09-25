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
- [ ] T042 E-01, E-03 explore features; G-05 technical details.
- [ ] T043 M-02, M-03 core Max estimates the real transfer; a fee-coin switch
      re-quotes before Max uses it; M-04 decimal mark.

## Phase 3b — core wiring (W, research.md)
- [x] T044 W-12 Windows Explore: page visible (HWND swap chain, 8 MB stack) and
      built outside gpui's borrow (was: abort on opening a page).
- [ ] T045 W-01 receive watcher per visit; W-02 balance refresh on confirmed
      send, incoming item, deposit.
- [ ] T046 W-03 send simulation via `executor::sim`; W-04 AddNetwork via
      network_admin `AddByChainIdRequested`.
- [ ] T047 W-05 bundler funding pre-check and sponsorship for dApp requests.
- [ ] T048 W-06 contacts search + per-group import/export; W-07 signing for
      another wallet.
- [ ] T049 W-08…W-11.

## Phase 4 — P2 visuals, surface by surface (screenshot each)
- [ ] T050 Wallet home H-08…H-12.
- [ ] T051 Flows F-09…F-11.
- [ ] T052 Contacts C-09.
- [ ] T053 Settings S-06…S-13.
- [ ] T054 Explore E-04, E-05; signing G-06.

## Phase 5 — acceptance
- [ ] T060 Screenshot pairs for every gallery state both apps draw (SC-001).
- [ ] T061 The owner's hour next to the web (SC-003).
