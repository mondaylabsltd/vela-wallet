# Tasks: Web Port Completion

**Input**: specs/028-web-port-completion/ (plan, research D44–D52, data-model,
contracts, quickstart). Branch from `main` @ 28d25ae9.

**Format**: `[ID] [P?] [Story] Description` — US1 receive code · US2 preferences
· US3 sweep · US4 custom tokens · US5 contacts I/O · US6 desktop send. Markers:
`[ ]`/`[X]`/`[~]`. Phases = plan's seven commits. Paths repo-root-relative;
`pnpm` runs in app-web/vela-wallet. Every port carries a provenance header
`// Ported from src/services/<file> @ 28d25ae9`.

**Precondition, not a task**: 027's SC-304 (an approve that never completes) is
fixed before this feature ships. It is tracked in 027's ledger.

---

## Phase 1: Setup (one commit)

- [X] T401 Baselines into results.md: artifact bytes/fingerprint, corpus pins,
      green tree (check/lint/unit/e2e counts) @ 28d25ae9
- [X] T402 [P] Port-provenance list with line counts: `qrcode 554`,
      `share-card 330`, `locale-format 331`, `contact-io 341`, `erase-device 160`,
      `image-decode 74`, `avatar-style 54`, `saved-contact 22`, plus
      `QRScanner.tsx`'s web decode engine (693) and, as consumers need them,
      `readonly-rpc-gate 109`, `fiat-convert 231`, `currency-catalog 158`,
      `token-list-filter 98`
- [X] T403 [P] Declare the two decoder deps (`@undecaf/zbar-wasm`, `jsqr`) in
      **app-web's own** package.json — the 026/027 rule — and record that both
      are lazy and bundled from our origin, never a CDN (D45)

## Phase 2: The code is data (one commit) 🎯 the trap

- [X] T410 [US1] Port `qrcode.ts` verbatim with a provenance header; unit
      vectors (byte mode, versions 1–10, EC level M) including the longest
      payment link the card must carry
- [X] T411 [US1] `wallet/qr.ts`: the live code for an address and for a payment
      request. `qr-pattern.ts` stays and keeps its comment — the galleries are
      canon and must not start showing real addresses (D44)
- [X] T412 [US1] `QRCard` takes the modules it draws instead of generating
      them; fixtures pass the placeholder, live passes the real code. Gallery
      pixel-unchanged
- [X] T413 [US1] Port `share-card.ts`: address text + code + the account's
      identicon, composed under the founder's existing rules (D46)
- [X] T414 [US1] e2e `receive-code.e2e.ts` (SC-401): render → **decode** →
      the address comes back; with a payment request, amount/asset/chain survive
- [X] T415 Full gate; results.md Phase 2 entry

## Phase 3: Reading a code (one commit)

- [X] T420 [US1] Port the decode engine from `QRScanner.tsx`: zbar primary at
      [1200, 1000, 800, 600, 400], jsQR fallback with binInvert/invert/binarize,
      the downscale-first preprocessing, all lazy (D45)
- [X] T421 [US1] ~~Port `image-decode.ts`~~ **NOT NEEDED**: it is a pure-JS JPEG
      decoder for native, which has no canvas. A browser decodes images itself —
      the picked-image path is `createImageBitmap`
- [X] T422 [US1] `ScanSurface` gains its live half: camera frames through the
      ladder, the gallery/torch/flip tools, and every refusal SAID — no camera,
      permission denied, insecure origin, nothing found (carried gotchas: the
      file input must be mounted, `<video>` swallows touches). Three corpus
      words added for the refusals native never had (`noCamera`,
      `insecureOrigin`, `cameraUnavailable`)
- [X] T423 [US1] The scan result reaches the send form and the sweep picker —
      through the CORE's own `open_scanner` / `scan_resolved`, so the sweep
      picker's scan button needs nothing of its own
- [X] T424 [P] [US1] Units: the ladder's order, each refusal's reason
- [X] T425 [US1] e2e `scan.e2e.ts` (SC-402): a generated code is picked from the
      file input and read; a refused camera states its reason
- [X] T426 Full gate; results.md Phase 3 entry

## Phase 4: Preferences and erase (one commit)

- [X] T430 [US2] Port `locale-format.ts` — explicit presets, never `Intl` (D47);
      units pinning each preset, including money
- [X] T431 [US2] Port `avatar-style.ts` (folded into `preferences.svelte.ts`);
      theme + language + the three formats + avatar style, persisted under
      `vela.` in the Expo record shapes (D48)
- [X] T432 [US2] Every figure, timestamp and avatar in the app reads the chosen
      preset; theme choosing writes what `isDarkTheme()` already reads — and is
      applied BEFORE first paint, in `app.html`'s inline script
- [X] T433 [US2] `SettingsHome` **and `SettingsDesktop`** gain the callbacks
      these rows have never had; the route wires them through one
      `SettingsPrefEvent` table. **Corrected the route's stale header** (FR-411)
- [X] T434 [US2] Port `erase-device.ts` as a NAMESPACE sweep over THREE stores,
      with the exception list filled in `contracts/erase-scope.md` (D49); the
      copy says local data, never the wallet
- [X] T435 [P] [US2] Units: format presets, the erase enumeration, the exception
      list as a test
- [X] T436 [US2] e2e `preferences.e2e.ts` (SC-406/407) ×3 engines: each row
      survives a reload; erase leaves nothing under `vela.`; cancel changes nothing
- [X] T437 Full gate; results.md Phase 4 entry

## Phase 5: Sweep and custom tokens (one commit)

- [X] T440 [US3] `live-send.ts` uses the core's `multi_select_mode` /
      `multi_selected_ids` / `multi_chain_id` / `multi_specs`: the picker's
      master tick, the per-asset amounts, the total, the CTA gate — all the
      core's ruling (D51). One shell flag (`sweepPicking`), by the phone's
      precedent, for whether the checkboxes are showing before confirm
- [X] T441 [US3] Sweep confirms into ONE operation through the existing spine
      (`multi_token_specs` in Rust → the 026 MultiSend path; nothing new)
- [X] T442 [US4] `manage_tokens` gets its screen: `go('add-token')` constructs
      the session, the chain answers with the token's identity, the core rules.
      The probe is implicit (the drawn sheet has one CTA); `onsheetclose` added
      to `FlowsMobile` so a dismissed sheet pops its step
- [X] T443 [US4] An added token appears wherever assets are listed and survives
      a reload (`invalidate_token_cache` → `balance.refresh(true)`;
      `vela.customTokens` in the KV)
- [X] T444 [P] Units: sweep builder arms (19), add-token arms (9)
- [X] T445 e2e `sweep.e2e.ts` (SC-404) + `add-token.e2e.ts` (SC-405) — the
      multicall stub now answers PER CALL (`e2e/stub-multicall.ts`), which is
      what two balances and a string-returning probe need
- [X] T446 Full gate; results.md Phase 5 entry

## Phase 6: The book travels, and the desktop sends (one commit)

**Reassigned 2026-09-05**: the founder handed the app-web contacts feature —
import/export included, and every other drawn-but-dead affordance — to a
separate session (`vela-wallet-63`). Done there, on this branch, with the
rules in the core (results.md Phase 6b):

- [X] T450 [US5] ~~Port `contact-io.ts` + `saved-contact.ts`~~ → the file
      FORMAT went into the core instead (`app/contacts_io.rs`, lifted from the
      desktop shell's copy) behind `import_file` / `export_requested`; existing
      entry wins, groups preserved, a malformed file refused before any write
      (D50) — and "导入到本组" seats every valid row. `saved-contact.ts` needs
      no port: the core's `recipient.saved` already answers it
- [X] T451 [US5] The contacts route gains export/import through 026's file
      seams (`pickTextFile` / `saveTextFile`) — plus the rest of the book:
      members, 移入分组, the QR, 最近往来 from the feed, the menus, and the
      hand-off to /wallet (`?to=`, `?group=`, `?flow=receive`)
- [X] T452 [P] [US5] Units: the collision matrix, a malformed file, a round
      trip — in Rust (`app_contacts` 44 → 54) and vitest (live builders,
      pickers, report, hand-off, history mapping)
- [X] T453 [US6] The wallet route hands `FlowsPanel` the same send actions
      `FlowsMobile` has — and reads the core's stage for the third column
      (`desktopSendState`), as the phone host has since 026
- [X] T454 e2e `contacts-io.e2e.ts` (SC-408, `vela-wallet-63`) +
      `desktop-send.e2e.ts` (SC-409)
- [X] T455 Full gate; results.md Phase 6 entry (isolated worktree, port 4174)

## Phase 7: Budgets and closeout (one commit)

- [X] T460 [P] Budget re-assertions: the decoder's wasm absent from Welcome and
      from every startup chunk (the 026 SheetJS treatment); the core artifact
      ~~byte-identical~~ (+129 B for three corpus words, Phase 3) and still the
      only one a wallet route loads. **Finding**: every `chunksCarrying` budget
      had passed vacuously since 027 (`output/client` is the extension build's
      `app/` dir); `chunkSource` now reads what the preview serves
- [ ] T461 [~] The device pass: scan a real code with a real camera, and a photo
      of one — the ladder was measured on hardware and the port must keep it
- [X] T462 Close results.md: SC-401…410 verdicts, deviations, 029 handoff
- [X] T463 Final sanity: all gates + `gen-core-types --check` (isolated worktree: 181/1/0)

## Phase 8: Replacing the Expo web — the six gaps the audit named (2026-09-05)

The founder's ask, after the closeout: "都完成掉，然后我要替换掉 expo web".
An audit of app-web against the Expo web (receive, send, add network, every
setting, displayed-equals-signed, account switching, activity list/detail)
found six gaps. Each is one task, each has its own proof, and the phase is
commits 8a–8c on this branch.

- [X] T470 [SC-304] The extension signing chain: `approve_tapped` reached the
      machine and nothing signed. Root cause: the web resident never sent
      `accounts_changed`, so `approve_with` found no signer and returned
      `Command::done()` (Expo's `setSignAccounts` had no web twin). Fix:
      `syncAccounts` on boot, on every session change, and inside the
      account-switch ack; the `test.fixme` is a `test` again and passes.
- [X] T471 [US-accounts] The account switcher is live on both widths: the
      session's rows, the balance core's switcher totals (`switcher_opened`),
      a tap is `SwitchAccount` in the session's domain, and the two buttons
      leave for create / sign-in. `accounts.e2e.ts`.
- [X] T472 [displayed = signed] A second implementation checks the shell's
      assembly before every passkey prompt: 032's `user_op.rs` is on this
      branch, the wasm exports `attestSafeOpHash` / `attestSafeMessageHash`,
      and `sign-attest.ts` refuses a calldata that is not the shown calls, a
      fee leg that is not the shown fee, a hash the core does not reproduce,
      and an assertion over any other challenge. Wired at the three SafeOp
      sign sites and the three message sites. `sign-attest.test.ts` +
      21 Rust vectors.
- [X] T473 [add network] The e2e 024's SC-001 named and did not have:
      search → verdict (11 contracts + P-256, stubbed) → add → listed as custom
      → survives a reload → removed. `add-network.e2e.ts`.
- [X] T474 [storage] "Clear all caches" clears (`device-storage.ts`: every key
      classified into the three drawn groups; the cache group is exactly the
      sweep), each user-data row's Clear clears its own keys, and the storage
      page's headline, bar and meta lines are measured, not fixture figures.
      `device-storage.test.ts`.
- [X] T475 [rescue] The three drawn rescues open where 023 placed them: the
      balance status line opens SR2 (RPC fix — save, probe, restored, Done →
      `fix_chain_resolved`) for an unreachable chain and SR3 (balance by
      network) otherwise; the send core's `treasury_bootstrap` opens SR4
      (relayer treasury, real code, copy, retry). Phone sheet / desktop dialog.
      `home-truth.e2e.ts` (RPC fix).
- [X] T476 [activity] The detail's "Delete record" (a new corpus word,
      `history.deleteRecord`, 15 locales): `activity_feed`'s `DeleteRequested`
      tombstones the record, the row leaves at once, a reload does not bring
      it back. `activity-delete.e2e.ts`.
- [X] T477 Gates: `pnpm check` · `pnpm lint` · unit · corpus six steps · cargo
      `user_op` · wasm rebuilt (+48 KB, new fingerprint) · e2e on the isolated
      4174 preview, three engines.

## Dependencies

Phase 2 blocks 3 (the decode round trip needs something real to decode) and is
what the feature exists for. Phases 4, 5 and 6 are independent of each other and
of 2–3; they are ordered by how much a person notices their absence. Phase 7
last.

## MVP strategy

Phases 1–3 close the only item on this list that is a trap rather than a gap: a
receive code that scans, and a scanner that reads. Everything after is a
capability the wallet claims and does not yet perform.

## Phase 9: The founder's second pass (2026-09-05) — plan, for ruling

The founder walked the web app after Phase 8 and came back with two lists.
The first (header account switcher, the identicon viewer from EVERY artwork,
no ⌘K bar, one quiet focus rule for text entry, the prompt cards' missing
width token) is done — batch 9a, on this branch. The second is below, verified against the
code, grouped by what a person hits first. Every root cause here was read off
the source, not guessed; three items need a design ruling and are marked
**RULING**.

### What was found (root causes)

| # | Symptom the founder saw | Root cause in the code |
|---|---|---|
| a | 收款 list: "同一地址，通用于全部 8 个网络" while 12 networks are listed | `liveReceiveList` replaces the rows and keeps the fixture `subtitle` (`NETWORK_COUNT = 8`) |
| b | 收款 list ignores the sidebar network filter | `liveReceiveList` never reads `inputs.chainFilter` |
| c | QR screen: title always "接收 Ethereum 上的资产", centre always ETH, whichever row's QR was tapped | `go('receive-qr', i)` drops `i` at the page (`onnavigate` only keeps an index for `tx-detail`); `liveReceiveQr` uses the fixture title and `centre` — its own comment records the gap |
| d | Network logos in 收款 rows / QR centre / send token card / fee row / send picker / token detail are letter glyphs, while the sidebar and asset rows draw real logos | `TokenMarkModel` and `NetworkRowModel` carry `ticker + badgeColor` only; `logoUrls` / `badgeLogoUrl` exist on `AssetRowModel` alone (`balanceTokenLogoURLs`, `chainLogoURL`) |
| e | 保存图片 / 在区块浏览器查看 on the QR screen do nothing | `ReceiveQr` takes `onsave` / `onexplorer`; neither host (`FlowsMobile`, `FlowsPanel`) passes them; no rasteriser exists in app-web (`share-card.ts` composes the model, nothing draws it to pixels) |
| f | Copy shows the tick but copies nothing (收款 rows, QR address, QR contract, token facts) | `copy()` in `ReceiveList` / `ReceiveQr` / `TokenDetail` only flips the icon; the five surfaces that really write are `IdenticonViewer`, `DoneScreen`, `AddressStrip`, the contacts page and the wallet page |
| g | Token detail (phone): 在浏览器中查看 dead; contract not copyable; (desktop) same button has no `onclick` | `TokenDetail` takes `onexplorer`, no host passes it; `AssetDetailPanel`'s explorer button has no handler; `explorerAddressURL` exists in `services/networks.ts` and is unused here |
| h | Token detail (phone) shows the wrong token | `withLiveFlow` has no `token-detail` case: the phone's T2 sheet is the drawn fixture, and `onnavigate('token-detail', i)` drops `i`. The desktop is live (`liveAssetDetail`) |
| i | Token detail → 转账 asks to pick a token again | `AssetDetailPanel.onsend` → `onflow('send')` → `openSend()` with no prefill; the core already accepts `preselected_symbol` + `preselected_network` (`send.rs:1819`) |
| j | Token detail → 收款 asks to pick a network; QR shows no token mark | Same entry path → `r1`/`dr1`; R3 (asset QR: contract line, token centre, `qrTitleAsset`) exists as a drawn state and is never entered live |
| k | Send form title "发送 USDT" over an ETH card (desktop) | `withLiveDesktopFlow` re-derives `body` and keeps the fixture `title`; the phone's `FlowScreen` reads the live `header.title`, the desktop `ThirdPanel` reads `model.title` |
| l | 最大 does nothing | `SendForm.onmax` is passed by no host; the core's `tap_max` (→ `MaxEstimate` pipeline) is never dispatched |
| m | 网络费 shows "—" and the fee-coin picker cannot choose | The core asks `estimate_fee` on Continue (and on a sweep's warm-up), not while the form is being filled; `feeRow` shows `send.fee ?? fee.fee` = nothing, and `liveFeeTokenPick` has no `fee.options` to list. Expo estimated on the form (debounced) |
| n | Hero total "$1.575.55" under the `1.234.567,89` preset | `moneyParts` groups by the preset but `BalanceDisplay` joins integer and decimals with a literal `.` (lines 38/43); `moneyText` (rows) uses `numberSeparators().decimal` and is right |
| o | 货币 cannot be switched (desktop) | `SettingsDesktop`'s localization dropdown emits `{ kind: 'currency' }` which `onPrefEvent` has no case for; `withLiveCurrency` (value + selected row) is applied to the phone model only |
| p | Is the fiat rate wired? | Yes: `resolveRate` = Chainlink fiat feeds (ENS-addressed, mainnet multicall) → the configured endpoint (`vela-currency` Frankfurter, `fiat-fx`) → `null` (formats in USD, refuses conversion). What is NOT wired: the selectable list is the fixture `CURRENCIES` (8 codes), not the endpoint's coverage |
| q | Is device storage real? | Headline, bar, user-data rows and cache rows are measured (`measureDeviceStorage`, on entry and after every clear). The dApp row on the DESKTOP is the drawn "4 个站点": `withLiveConnections` is applied to the phone model only. 浏览记录 is a phone-only concept (the web has no in-app browser) and reads 0 honestly. Latency figures and the RPC-provider health are still canon data |
| r | Sidebar network click on 通讯录 / 设置 jumps to the wallet | By construction: `pickChain` on both routes selects, then `goto(walletHref)` ("the filter is the wallet's to show: choose a network here, land there"). The founder questions it — **RULING** |
| s | Desktop prompt cards spanned the window | `--layout-promptCard` was never declared (spec 019 recorded declaring it) — FIXED in 9a, 440px |

### RULING 1 — the receive list and the sidebar filter (b, a)

Recommendation: **the filter answers the list's only question, so skip the
list.** With a network selected in the sidebar, 收款 opens the QR for that
network directly (R2/DR2L: "接收 BNB Chain 上的资产", that chain's logo in the
centre), with one link back to "全部网络" (R1). With 所有网络 selected, R1 lists
every network the wallet knows — 12 today, the count live in the subtitle —
never a fixture 8. Not recommended: R1 filtered to one row (a list of one), or
R1 with the filtered row pinned on top (two ways of choosing the same thing on
one screen). The address is the same on every network; R1 exists only to pick
a network, and the filter already did.

### RULING 2 — the network list on 通讯录 / 设置 (r)

Recommendation: **the network list is a wallet filter, so it is drawn only on
the wallet.** On 通讯录 and 设置 the sidebar carries the header and the three
nav rows and stops. Reasons: (1) a click that leaves the page you are on is a
surprise, and every other row in that sidebar stays; (2) a filter that changes
nothing on the current page is noise wearing a checkmark; (3) the wallet's
filter is one tap away through 钱包 anyway. If a context cue is wanted, one
line under the nav — "网络 · BNB Chain" — linking to the wallet is enough.
Alternative kept for the record: keep the list, make it a pure filter (no
navigation), accept the checkmark-that-does-nothing.

### RULING 3 — token detail's two doors (i, j)

Recommendation, as the founder framed it: 转账 from a token opens the form with
that token chosen (the core's `preselected_symbol` + `preselected_network`;
back goes to the picker, not out); 收款 from a token opens R3 for that token's
chain — contract line, the token's logo in the centre, `qrTitleAsset` — and the
saved image carries the same mark and network note. No network picker on
either door: the token already names its chain.

### Tasks (order = what a person hits first)

**9b — Truth first: figures and titles that lie**

- [X] T480 [n] `BalanceDisplay` joins integer and decimals with the preset's
      decimal mark (carry `decimalSeparator` on `BalanceModel`, from
      `numberSeparators()`); unit test under all five presets; the hidden
      state untouched
- [X] T481 [a,b] `liveReceiveList`: subtitle from `receive.networksLine` with
      the LIVE count; rows follow RULING 1 (filter → straight to R2, `chainId`
      rides the flow entry); e2e: 12 rows with 所有网络, one QR with a filter
- [X] T482 [c] The tapped network rides `onnavigate('receive-qr', i)` like
      `tx-detail` does (`selectedReceiveChainId`); `liveReceiveQr` fills the
      title (`qrTitleNetwork` / `qrTitleAsset`), the centre mark and the
      explorer target from it; e2e decodes the code and reads the title
- [X] T483 [h] The phone's token detail goes live: `liveTokenDetail` (the
      desktop's `liveAssetDetail` facts, the T2 shape) + the tapped index
      rides `onnavigate('token-detail', i)`; unit test on the mapping
- [X] T484 [k] `withLiveDesktopFlow` re-derives `title` for the send stages
      (`send.sendTitle` with the symbol; the picker / multi titles likewise);
      unit test
- [X] T485 [q] `withLiveConnections` on the desktop model too; hide 浏览记录
      on the web (no browser to have a history) — one `webOnly` filter in
      `withLiveStorage`, not a fixture edit; unit test

**9c — Dead controls**

- [X] T486 [f] One `services/clipboard.ts` (`copyText`, refused-clipboard
      returns false; the on-screen address is the fallback) used by
      `ReceiveList`, `ReceiveQr` (address + contract), `TokenDetail` facts,
      `AssetDetailPanel` (contract fact gains the copy affordance the phone
      has), `TxDetail`; browser test on the tick + a `navigator.clipboard`
      stub
- [X] T487 [g,e] Explorer: `onexplorer` wired in both hosts —
      `explorerAddressURL(chainId, account)` for the QR, the token page
      (`/token/<contract>?a=<account>`) for a token, `explorerTxURL` already
      live for a tx; opens a new tab; `AssetDetailPanel` gets the handler
- [X] T488 [e] 保存图片: rasterise the R4 share card (`share-card.ts` model →
      inline SVG → canvas → PNG blob; `file-io.saveBlob` beside
      `saveTextFile`; the extension's download is the same call). Carries
      the token mark and network note per RULING 3; e2e downloads and
      decodes the PNG's code
- [X] T489 [l] 最大: hosts pass `onmax`, `sendActions.max` dispatches
      `tap_max`; the core's `MaxEstimate` fills the amount fee-aware; e2e on
      the stubbed chain (native max = balance − fee, ERC-20 max = balance)
- [X] T490 [m] The fee on the form: the core requests `estimate_fee` once the
      form is complete (recipient valid + amount > 0), throttled and
      chain-guarded (`selectedFeeEstimate`), so the row shows a figure and
      the fee-coin picker has options before Continue — a `send.rs` change
      with vectors (the rule is the core's), wasm rebuilt; e2e: the row is
      never "—" with a stub bundler answering
- [X] T491 [o,p] 货币 on the desktop: `SettingsPrefEvent` gains
      `{ kind: 'currency'; id }` → `currency.choose`; `withLiveCurrency`'s
      desktop twin; the list from the endpoint's coverage (`fetchFxRates`
      keys ∪ Chainlink feeds, cached, the fixture 8 as the offline floor)
      on both widths, per the 024 intent ("provider-driven currency list");
      e2e switches to EUR on the desktop and reads the hero glyph

**9d — Logos, once**

- [X] T492 [d] `TokenMarkModel` gains optional `logoUrls` / `badgeLogoUrl` /
      `badgeHidden` (the `AssetRowModel` triple); `TokenIcon` already draws
      them. Filled by the live builders in one pass: send picker rows, the
      send token card, the fee row and fee options, receive rows (chain
      logo), the QR centre (token or chain), token detail head, the share
      card. Browser test: a mark with a logo draws `<img>`, without one the
      glyph — the TokenIcon contract already proven

**9e — The two doors and the filter (RULINGS 1–3)**

- [X] T493 [i] Token → 转账: `onflow('send', { symbol, chainId })` →
      `openSend({ preselected_symbol, preselected_network })`; verify the
      core lands on `enter_details` (send.rs:1819) and that back returns to
      the picker; e2e from the asset detail on both widths
- [X] T494 [j] Token → 收款: `onflow('receive', { symbol, chainId })` enters
      R3/DR3L directly (contract line, token centre, `qrTitleAsset`); the
      save image carries the mark; e2e
- [X] T495 [b] Filter → 收款 enters R2 for the filtered chain (RULING 1); the
      "全部网络" link back to R1; e2e
- [X] T496 [r] RULING 2: `Sidebar` takes `networks?`; the contacts and
      settings routes stop building `liveChainRows` and pass none; the
      `pickChain` navigation goes with it; gallery boards for DC1/DST1
      re-exported without the list; e2e: no network rows on /contacts,
      /settings

**9f — The careful pass the founder asked for**

- [~] T497 A walk of every drawn control on both widths against
      `docs/MANUAL-TEST-100-CLUES.md`: each control either does what it says
      or is removed — "有交互效果但没成功" is the class of bug this phase
      exists to end. Findings go in results.md Phase 9 as a table (control ·
      what it did · what it does now)
- [X] T498 Gates: `pnpm check` · `pnpm lint` · unit · corpus (if T490 adds
      words: none planned) · cargo `send` vectors · wasm rebuilt · e2e on the
      isolated 4174 preview, three engines; results.md Phase 9 entry

### Dependencies

T492 (logos) before T482/T483/T494 land their marks; T486 (clipboard) before
T487's surfaces; T490 (fee on the form) is a core change and can run beside
everything else; the three RULINGS gate T481, T493–T496 and nothing else.
