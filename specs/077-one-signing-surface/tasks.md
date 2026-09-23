# Tasks: 077 — one signing surface

**Spec**: [spec.md](spec.md) · **Plan**: [plan.md](plan.md)

Evidence is a real browser unless it says otherwise: Chrome for Testing 151 with
the packaged extension loaded from `extension/dist`, driven over CDP, signing in
the parallel space against Gnosis. "Measured" means it was watched happening;
"tested" means a test would go red without the change.

## Phase B — the landing (done first: it is what the owner asked for twice)

- [x] **T001** `dapp-receipt.ts` — the landing's states as a pure model, wording
      itself from the SEND receipt's own corpus group so the two surfaces cannot
      drift. *Tested: `dapp-receipt.test.ts`, 11 cases.*
- [x] **T002** `DappReceipt.svelte` — `StatusHero` unchanged, hash-copy,
      explorer link, one Done. *A raw `320px` tripped the token audit; the
      column is `--layout-flowColumn`.*
- [x] **T003** The landing belongs to the SHEET, not to a page: `SigningHost`
      owns it (`receipt`, `onlanding`, `onreceiptdone`) and every surface that
      mounts the sheet gets it. *This is what fixes B-7 — see T012.*
- [x] **T004** It watches `tx_tracker`, not the submit call: `confirmed` +
      `tx_hash` is the tick, `dropped`/`rejected` is a cross, `unreachable`
      keeps waiting (the wallet could not ask, which is not the chain saying no).
- [x] **T005** Read `tracker_handoff` in an `$effect`, not at the answer.
      *Measured: inside `sendResponse` it is `null` every time — the handoff
      lands on a view AFTER the one that answers — and the surface closed on the
      person mid-submit. Found by driving the packaged extension; no test had it.*
- [x] **T006** The ring FILLS. `receiptProgress` was passed a hard-coded `0`,
      so it always returned `undefined` and the ring only circled: a tested
      function that was not wired. Now the chain's own typical time, on a
      ticking clock. *Tested: `ring.test.ts`.*
- [x] **T007** ONE ring curve. The send's easing moved out of
      `SendReceipt.svelte` into `flows/ui/ring.ts`; both receipts call it, so
      "和转账一样" is the same curve and not a similar one. *Tested: a case
      asserts the dApp receipt equals `ringProgress` rather than re-deriving it.*
- [x] **T008** The chain's typical seconds come from the CORE
      (`network_admin::typical_inclusion_s` → `typicalInclusionSeconds`), not a
      second copy of 24 numbers in TypeScript. `send.rs` now uses the same
      lookup it used to inline.

## Phase A — the panel IS the wallet

- [x] **T010** `extension/panel.js` opens `walletPage(locale)` with `?panel`.
      *Measured: the panel is `/en/wallet.html?panel`.*
- [x] **T011** `DappRequestHost.svelte` — the request lifecycle, lifted out of
      `request/+page.svelte` so both surfaces run ONE copy: take the request,
      ask the core, draw the consent card, hand a signature to `sign_request`,
      answer exactly once, settle a teardown with the core's code.
      *Measured on the panel: consent card as a sheet (`role="dialog"`) with the
      wallet behind it — "Parallel One · $5.44 · Receive Send Scan Activity" —
      Connect accepted, the dApp got its address.*
- [x] **T012** Settings and the wallet pass `receipt` to their sheet, so the
      Ethereum backup inherits the landing with no code of its own (B-7).
      *Tested: `one-surface.test.ts` fails if any surface mounts the sheet
      without it. Not measured end to end — the backup writes to Ethereum
      mainnet and this wallet has no mainnet gas; see "Not verified" below.*
- [x] **T013** A request that arrives while the panel is ALREADY open.
      *Measured first as a failure: `sidePanel.open` on a standing panel only
      re-shows it, so `onMount` never ran again and the dApp got "Vela did not
      answer in time". The host now also takes a request when the worker records
      one (`subscribeRequests`, on the worker's own `vela.req.*` bookkeeping —
      nothing was added to the worker to be told).*
- [x] **T014** One request at a time. The panel used to reload per request for a
      fresh fee session (026's one-owner rule) and cannot now — a reload would
      throw away the wallet. `take()` refuses to start while a request is owed
      instead, so the page's one fee session is only ever asked about one
      operation, exactly as when a person signs two sends in a row.
- [x] **T015** The wallet under a pending request is READABLE, not DRIVABLE — an
      invisible guard under the sheet's own scrim. *plan.md's third risk: a
      person who could start a send while a dApp waited would put two operations
      in one fee session.*
- [x] **T016** The panel no longer dismisses itself: `panelDone` and the
      worker's `closePanel` are gone. "Nothing more to show" stopped being a
      thing that happens — there is always the wallet.
- [x] **T017** A SECOND site's consent card has live buttons. The panel outlives
      the request that raised it and `busy` did not: a successful Connect left it
      `true` forever, because the window it was written for closed a moment later
      and nobody noticed. *Measured both ways, two fresh origins in one standing
      panel: before, `Cancel disabled=true, Connect disabled=true` — a card
      nobody can answer; after, both live, and both origins answered.*
- [x] **T018** A request that arrives DURING a landing waits. The receipt
      replaces the sheet (`SigningHost` draws one or the other), so taking it
      then would give it an invisible sheet — a person answering a screen they
      cannot see. Done brings it up.
- [x] **T019** The dedicated-window path re-verified after the lifecycle moved
      out of its page. *Measured: a request fired with no gesture opened
      `request.html?rid=…` in a popup window with no wallet behind it, the sheet
      priced it, the slide answered it, and the worker closed the window at
      t+22s.*

## Phase D — the dedicated window keeps its ending

- [x] **T020** `request/+page.svelte` is now the window alone, and mounts the
      same `DappRequestHost` in `window` mode. *Measured: the worker's
      `settle()` removes the window the moment the answer goes out, so the
      receipt cannot live there — see FR-003, and T019 for the re-verification.*

## The core changes, and the shells that share them

- [x] **T021** `network_admin::typical_inclusion_s` is public and exported to
      wasm; `send.rs` uses it instead of its own inline lookup. *Gates: the Rust
      workspace green, clippy `-D warnings`, `rustfmt` in both trees,
      `build-web --check`, `gen-core-types --check`, the i18n corpus lint, the
      vectors dump, and the Kotlin AND Swift binding smokes — the mobile shells
      read the same table through uniffi, so both were replayed rather than
      assumed.*

## Not verified

- **T012 end to end.** The Ethereum backup signs on chain 1 and this wallet has
  no mainnet gas, so the landing is proven structurally (the sheet owns it; the
  test forbids a surface without it) and not by watching a mainnet transaction
  confirm. Whoever has mainnet gas should watch one.
- **B-5, Arbitrum fails outright.** Deferred by the owner. 42161 is a built-in
  chain (`network_admin.rs`), so it is not a missing network.
