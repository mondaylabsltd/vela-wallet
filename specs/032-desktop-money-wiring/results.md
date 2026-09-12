# Results — 032 desktop-money-wiring

## Baselines — recorded 2026-09-05, branch point `049617f5` (tip of `031-desktop-read-wiring`)

Stacked on 031 for the reason 031 stacked on 030: this cut edits
`wallet/page.rs` and `flows/` heavily, and 031's live builders are the ones the
money screens extend. `origin/main` is 21 commits ahead of the branch point and
none of them touch `rust/` or `app-desktop/` (checked), so the stack is not
rebased yet.

### Desktop at the branch point

| | |
|---|---|
| `cargo test` (app-desktop/vela-wallet) | **222 passed · 0 failed · 26 ignored** |
| `src/**/*.rs` | 81 files |
| `wallet/page.rs` | 6,473 lines ⚠ |
| gallery states | 36 |
| `// live in 032` markers | 2, both `network_admin::ClearBundlerCache` |

### vela-core at the branch point

| | |
|---|---|
| `cargo test -p vela-core --features i18n-all,crux` | **1,234 passed · 0 failed** (summed over every test binary) |
| features | `crux`, `identicon-raster`, `bindings`, `i18n-*` — no fixture feature |

### The four machines of group A

| Machine | Core lines | Operations |
|---|---|---|
| `send` | 4,224 | 18 |
| `fee_policy` | 2,322 | 6 |
| `batch_import` | 1,648 | 3 |
| `tx_tracker` | 958 | 6 |
| **total** | **9,152** | **33** |

Group B (`clear_signing` 4,841 · `approval_guard` 2,425 · `sign_request`
2,096 = 9,362) is drawn (DCS1–8, 33 gallery scenarios) and waits on a request
source; it is not in this cut's critical path.

### What the web tier ported, and what this cut must write in Rust

| Web source (`app-web/vela-wallet/src/lib`, at `origin/main`) | Lines |
|---|---|
| `services/safe-transaction.ts` (the Safe user-operation assembly, verbatim from Expo) | 3,068 |
| `services/bundler-service.ts` (the relay client) | 948 |
| `flows/core/send-executor.ts` | 533 |
| `signing/core/sign-executor.ts` | 402 |
| `dev/passkey-fixture.ts` | 350 |
| `wallet/core/tracker-executor.ts` | 278 |
| `flows/core/fee-executor.ts` | 217 |
| `flows/core/batch-executor.ts` | 90 |

**Already in Rust and not to be ported**: the money math (`fee_policy`'s
`same_asset_fee_limit`, `to_base_units`, `max_native_sendable`, the reserves,
`encode_erc20_transfer`), Safe address derivation and the setup calldata
(`safe.rs`), EIP-712 hashing (`eip712.rs`), the WebAuthn digest and DER→raw
conversion (`webauthn.rs`), ABI encode/decode (`abi.rs`, `alloy-dyn-abi`),
and the desktop's own `executor/abi.rs` from 031.

**Genuinely missing in Rust**: the 4337 user operation itself — `initCode`
assembly, `executeUserOp` / MultiSend calldata, the SafeOp EIP-712 hash, the
Safe WebAuthn signature envelope (`abiEncodeWebAuthnSig`, the per-key signer
proxy `r` field), the dummy signature for estimation, the in-band fee leg —
and the relay's wire shapes.

### The blocker the handover named, and why it is phase 1

Every desktop signing path ends at a real authenticator: USB CTAP2, caBLE,
the macOS platform vault, `webauthn.dll`. None can be handed a private key, so
the parallel space's three P-256 scalars cannot sign on the desktop through
any path that exists. Every acceptance test in this cut needs that signer; it
is written first, in `vela-core`, so the bytes it emits come from the same
kernels a real assertion is checked with.

## Phase 1 — the fixed keyset signs in Rust, and the desktop has a parallel space

**What shipped**: the thing the 031 handover said to do first.

- **`vela-core::dev_fixtures`** (feature `dev-fixtures`, default OFF, ~300
  lines + 9 tests): the same three P-256 scalars and credential ids the Expo
  and web clients carry in `passkey-fixture.ts`, derived into the same three
  single-key Safes and the same multi-key Safe; a `build_assertion` that emits
  the exact bytes an authenticator would (`{"type":"webauthn.get",…}` client
  data, `rpIdHash ‖ 0x05 ‖ 0`, low-S DER over `sha256(authData ‖
  sha256(clientDataJSON))`); a `build_registration` whose `fmt: none`
  attestation object the core's own `extract_attestation_public_key` and
  `attested_credential_id` parse back, with the FROZEN `0x45` flags byte; and
  the web's signer-selection rule (`resolve_signer`). No new dependency — the
  signer is the `p256` the registry group proof already uses.
- **The golden locks moved into Rust** and held on the first run:
  `0xD400866e…130b`, `0x031d7D57…772b`, `0x58cd0ce6…1d3d`, and the multi-key
  `0x88cCA0Ee…6894` that every 031 live sweep read from.
- **`app-desktop/src/parallel_space.rs`**: the runtime gate
  (`VELA_PARALLEL_SPACE=1`, behind the cargo feature), the signer seam
  (`register` = first fixture not already founding the wallet, so the
  machine's own exclude list drives a three-key creation through all three;
  `assert` = the named credential, or fixture #1 / `VELA_PARALLEL_SIGNER=n`
  for the discoverable ceremony), and the badge — violet, not a token,
  rendered by `Root` over whichever screen is up.
- **Four seams in `executor/passkey.rs`**: `register`, `assert`,
  `supported`, `platform_supported` consult the space first. Without the
  feature the lines do not exist.
- **CI**: the `rust` job's clippy and test steps now enable
  `vela-core/dev-fixtures`, so the locks are enforced somewhere; the wasm
  canary deliberately does not.

**The artifact rule, met**: `rust/` changed, so `rust/pkg-web` was rebuilt
and committed. The wasm is **3,630,664 bytes — the same count 026 recorded**;
only the source fingerprint (and with it the asset's name) moved, because
the fingerprint hashes every source file and a feature-gated module is a
source file. `verify-web.mjs` 46,408 green; `gen-onboarding-types --check`
current.

**Recorded, not done**:
- The fixture assertion carries no user handle (`user_id_hex: None`), as the
  web's does. A parallel-space login therefore names the wallet from the
  registry, not from the credential. Same behaviour as web; stated so the
  next reader does not go looking for a bug.
- SC-302 (sign in to the golden Safe through the real onboarding machines)
  is not yet exercised end to end — that needs the registry round trip and
  is the natural first live check once a send exists to justify it.
- `platform_supported()` answers `true` in the space so the "this device"
  row is offered; the fixture reports `platform` attachment, so that is the
  row it is.

**Gates**: vela-core **1,234 → 1,243** (with `dev-fixtures`; 1,234 without —
the feature adds, never changes) · desktop **222 → 225** with the feature,
222 without · fmt clean both crates · `cargo clippy --workspace --all-targets
--features vela-core/dev-fixtures -- -D warnings` clean · desktop clippy adds
no warning in the touched files.

## Phase 2 — the user operation, in Rust, cross-checked

**What shipped**: `vela-core::user_op` (~640 lines, 21 tests), the pure half
of `safe-transaction.ts` — the part every native tier would otherwise write by
hand. Unconditional (no feature): it is money code the uniffi tiers will need.

- **Calldata**: `executeUserOp` (CALL), the MultiSend batch (DELEGATECALL
  into `MULTI_SEND`, packed `op ‖ to ‖ value ‖ len ‖ data`), the
  lone-call-stays-single rule, `transfer(address,uint256)`, the in-band fee
  leg in both shapes, `is_plain_transfer_call` by shape not size.
- **InitCode**: `factory ‖ createProxyWithNonce(singleton, setupData, saltNonce)`
  for one key (byte-identical to the historical single-owner setup, because
  it takes `compute_safe_address`'s own `setup_data`/`salt_nonce`) and for a
  founding set (`compute_safe_address_multi`'s).
- **The signer rule**: `signer_address_for` — shared `WEBAUTHN_SIGNER` for
  keys[0] or a one-key wallet, the key's own counterfactual proxy for a later
  key, an error for a foreign credential (never mis-encoded).
- **Hashes**: the SafeOp EIP-712 digest under the Safe4337Module domain; the
  SafeMessage digest under the Safe's own domain (EIP-1271, group B's).
- **The signature envelope**: `validity(12) ‖ r=verifier ‖ s=65 ‖ v=0 ‖
  len ‖ abi.encode(bytes authData, string clientDataFields, uint r, uint s)`;
  the EIP-1271 form without the window; the estimation dummy built by the
  same encoder (37-byte authData, `r = s = 1`).
- **Wire**: the v0.7 dictionary (`factory`/`factoryData` split, paymaster
  quartet, Vela extension fields such as Tempo's `feeToken`).
- **Padding and parsers**: `pad_gas_estimate` (×1.5, floors, +10,000);
  `parse_existing_user_op_hash`; `parse_hex_quantity`.

**Cross-checked, not just ported.** Three of the tests hold the hand-laid
bytes against an INDEPENDENT implementation already in this crate:
- `calculate_safe_op_hash` == `eip712::hash_typed_data` over the SafeOp
  typed data (13 fields, two of them `uint48`), and moves with the chain id;
- `compute_safe_message_hash` == the same hasher over `SafeMessage(bytes)`;
- the WebAuthn payload == `alloy_dyn_abi`'s `abi_encode_params` of
  `(bytes, string, uint256, uint256)` with a 37-byte and a 51-byte tail.
All three agreed on the first run. The web vector suite's assertions
(selectors, word layout, DELEGATECALL bit, factory prefix, signer rule,
parsers) are ported alongside.

**Two divergences, both stricter**, recorded in the module note: `r`/`s`
must be exactly 32 bytes; a non-hex quantity is a typed error rather than a
thrown `SyntaxError`.

**Not ported here, on purpose**: everything that reads the chain or the
relay — `isDeployed`, the nonce and its 10 s cache, the gas signals, the
quote, the estimate, the submit and its 3× retry, the receipt wait. Those are
the desktop executor's (phase 3), in `sendUserOpInBand`'s order. Tempo's
variant (`feeToken` extension + splitter) is a later phase. `encode_erc20_transfer`
now exists twice — `fee_policy`'s hex-string form behind `crux`, this
byte form without — because FR-308 forbids touching the machine file; the
next machine change should make `fee_policy` call this one.

**Gates**: vela-core **1,243 → 1,264** · fmt clean · `cargo clippy
--workspace --all-targets --features vela-core/dev-fixtures -- -D warnings`
clean · `rust/pkg-web` rebuilt — the wasm is **3,630,664 bytes again**: the module is dead code to the wasm crate, so only the source fingerprint moved.

## Phase 3 — the relay, the reads, and the submit spine

**What shipped**: the desktop side of the money path below the screens —
five new executor modules, one pool extension, one marker flipped.

- **`executor/relay.rs`** (the bundler over HTTP): the REST base asked of the
  pool (`bundler_base`, invariant ③ — the same relay the op goes to) with the
  configured/built-in host as fallback; `/v1/account` (30 s cache, Tempo's
  pathUSD branch, a corrupted `settlementRecipient` degrading to the deposit
  address) and `/v1/treasury` (404 = uncovered, anything else transient =
  `Unknown`, never `Uncovered`); `vela_getInBandGasQuote` rows parsed with
  the web's admission rules (8 s cache, the no-native-price stablecoin
  filter); the raw `pimlico_getUserOperationGasPrice` tier, UNJUDGED;
  `eth_estimateUserOperationGas`; `eth_sendUserOperation` with the 3×/3 s
  busy retry; the receipt and status polls collapsed to the tracker's typed
  axis; and the two wording parsers the machines leave to the shell.
- **`executor/chain.rs`**: `is_deployed` (only `true` is cached; an
  indeterminate read is an error, never a guess), the EntryPoint nonce (10 s,
  bumped after a submit), the raw gas signals, `chain_gas_price` with the
  5-gwei default, `verify_chain_ready`.
- **`executor/user_op.rs`**: `simulate_gas` (the quote's estimate, byte-
  identical to the submit's MultiSend) and `submit` — `sendUserOpInBand` and
  `sendUserOpTempo` in their order: chain ready → deployment + nonce (the two
  refusals) → placeholder-leg estimate → the DISPLAYED fee baked in (or the
  web's send-time fallback quote through `fee_policy`'s amount rule) → SafeOp
  hash → the passkey seam → the contract-signature envelope → the AA20 guard
  → submit with `[existingHash]` recovery → nonce bump. The failure
  vocabulary maps 1:1 onto `SendSubmitFailure`.
- **`executor/fee.rs`**: `impl Machine for FeePolicy` — six operations, six
  reads, the keys of an undeployed account read from the stored record.
- **`executor/tracker.rs`**: `impl Machine for TxTracker` as the app-resident
  it must be — the pending-record sweep over `vela.transactionHistory`, the
  in-place patch, the receipt's authentic logs held per hash and handed to
  `token_trust::receipt_confirmed` (the single auto-add entry point) — plus
  `start` (boot + a 3 s tick loop that re-fetches the resident so a sign-out
  cannot strand it) and `submitted`.
- **`executor/pool.rs`** learned the two questions that are not routed calls:
  `bundler_base` and `best_rpc_url`, answered from the core's own
  `BundlerBase` / `BestRpcUrl` verdicts through a query table beside the
  in-flight map.
- **`network_admin::ClearBundlerCache` is live** — the last two `// live in
  032` markers are gone.

**Live, against Gnosis** (per module, `--test-threads=1`, proxy unset):

| Read | Answer |
|---|---|
| relay base | `https://vela-relay.getvela.app` — no chain suffix; treasury **Covered** |
| golden Safe quotes | XDAI native, recipient `0xee2c…f0dd`, balance 0.75897; USDC and USDT rows at 0 |
| fast bundler quote | `maxFeePerGas 17`, no network/relayer fields (the core's fallback applies) |
| account info | `settlementRecipient 0xee2c…f0dd`, status ACTIVE, **`activeDepositAddress` absent** |
| golden nonce / signals | `0x…04`; `eth_gasPrice 11 · baseFee 10 · tip 2 → 12` |
| a dust transfer's estimate | `100000 / 112472 / 101600` |
| an unknown hash | receipt: reached, nothing; status: `None` (the relay has no status method, or answers nothing) |

Two of those are worth a line. The relay publishes no `activeDepositAddress`
for the golden Safe on Gnosis — so `fee_recipient()` is the settlement
recipient alone, and a funding sheet that showed the deposit address would
show nothing; the in-band path does not need it. And `eth_getUserOperationStatus`
answered nothing for an unknown hash, which the tracker reads as
`StatusUnavailable` — an honest unknown, exactly the case the core's window
logic exists for.

**Not exercised**: a real submit. `submit` is compiled and its envelope is
proven on the fixture keyset (the second key's own proxy in `r`, a foreign
credential refused), but no dust has moved — that is SC-303 and it needs the
send host of the next phase to carry a quote the core displayed.

**Gates**: desktop **225 → 244** with the feature (240 without) · fmt clean
· warnings 1 (pre-existing) · `rust/` untouched this phase.

## Phase 4 — the send host, and the screens

**What shipped**: the desktop sends. Every drawn send panel (DSD1L–DSD4L,
DSD2eL, DSD2fL) now reads the `send` and `fee_policy` machines when a person
is signed in, and every affordance on them is an EVENT to the core.

- **`executor/send.rs`** — the nineteen operations as the web executor has
  them: the token fetch through `balances::fetch_all` (chains read AFTER the
  fetch), the credential lookup, the record persistence in one write, the
  identity waterfall, the recipient risk (`eth_getCode` with the EIP-7702
  delegated-EOA exemption + the prior-interaction read), and `SubmitUserOp`
  as the sign closure over `passkey::assert` — routed on the first key's
  transports (a phone over caBLE, a platform vault, or a security key asked
  DISCOVERABLY so any founding key on it can answer). Four arms are the
  screen's and say so.
- **`wallet/money.rs`** — `SendHost`: one `send` + one `fee_policy` per
  journey, born with the flow and dropped with it; `EstimateFee` answered by
  the live fee session (deployment read → `QuoteRequested` → settle on the
  same view the card renders); the card's re-quotes mirrored back as
  `FeeBusyChanged` / `FeeUpdated`; `TrackSubmitted` handed to the resident
  tracker whose view the host observes and forwards as the three
  `ReceiptUpdate` verdicts; the ceremony channel's poll for PIN / pick /
  touch / QR; `SigningStarted` raised by the sign closure and dispatched
  once.
- **`flows/live.rs`** — `send_pick`, `send_form`, `send_confirm`,
  `send_receipt`, `fee_token`, `contact_pick`, and `send_panel` (the core's
  stage → the panel; the two pickers are the core's flags, the fee sheet is
  the page's). The receipt reads `receipt.status`, never `tx_status`.
- **`flows/panels.rs`** — the drawn gaps closed as props: per-row listeners
  on the token, contact and fee-coin rows; editable amount and recipient
  fields (the value is the core's; the field holds no copy); the Max chip;
  an address-book pill beside the typed recipient. Fixtures untouched; the
  gallery renders every state exactly as before (sweep: 36, every state
  rendered).
- **`wallet/page.rs`** — the flow stack is REBUILT from the core's stage on
  every frame of a live send; the chevron asks the core to step back; the
  column's close and the core's `Close` both drop the machines; the cable's
  three dialogs and the core's alert render over the column; the tracker
  starts on sign-in. Eight new strings resolve from keys the corpus already
  had (`send.txConfirmedTitle`, `send.txSubmitting`, …).

**Live, headless, for the golden Safe** (`wallet::money` — a synchronous
pump of both machines, the same performs as the host minus the thread):

| step | the core said |
|---|---|
| `Open` | 1 holding: **xDAI on Gnosis, 0.75897** — the number 031's hero showed |
| `SelectToken` → `SetRecipient` (fixture #2) → `SetAmount "0.001"` | `can_continue: true`, no warning |
| `Continue` | stage **Confirm**; fee **0.010 xDAI** native, quoted (not a local fallback), recipient `0xee2c…f0dd`; treasury none; no alert; **`can_confirm: true`** |

That fee is the very figure 026's web sweep signed on the same Safe
(0.001 sent + 0.010 in-band). The slide was behind `VELA_LIVE_SEND=1` — it
spends dust, and that was the founder's call. **They made it on 2026-09-07;
the slide is pulled, and SC-303 is proven.** See below.

**Recorded, not done**:
- `AddNetwork` from a locked request answers the ported `catch` (`Error`);
  the settings wizard is the desktop's add-network journey and the send flow
  has no `/pay` link to arrive from yet.
- `SimulateCalls` answers no simulation — no engine on the desktop.
- The batch importer (DSD2cL) still draws the fixture; its machine is phase 5.
- The scan row (DS1) stays a picture; no camera pipeline.
- A real USB / caBLE / vault signature for a send has not been run on a
  device this session; the parallel space's signer went through the same
  `passkey::assert` seam login uses.

**Gates**: desktop **244 → 251** with the feature (247 without) · fmt clean
· 1 pre-existing warning · gallery sweep every state rendered ·
`check-windows.sh` green · `rust/` untouched.

## Phase 5 — paying many at once

**What shipped**: the batch importer runs on its machine, and DSD2cL is
the last send panel to stop being a picture.

- **`executor/batch.rs`** — the three capabilities the core cannot have:
  the USD→fiat rate through `display_currency::resolve_rate` (never a
  display helper that ends in `?? 1` — a CNY payroll split at 1:1 is ~7× the
  intended payout behind a green button); a picked table as text for
  CSV/TSV/TXT (the core parses it) or as a cell matrix for a workbook
  (`calamine` reads the first sheet, integral floats print as the person
  typed them, short rows are padded so column positions hold); the file's
  name for the sheet.
- **`wallet/money.rs`** — the `batch_import` core is born when the send
  machine raises `show_batch_import` and dropped when it falls, so a stale
  paste or rate can never survive a re-open (the core is new). The two
  dialogs are gpui's and awaited in the host with the file work off-thread;
  `applied` seeds the send machine's split editor with exactly the core's
  drafts and closes the sheet. The desktop's paste: the drawn box is not a
  text editor and the custom field ignores ⌘V, so clicking the box reads the
  clipboard.
- **`flows/live.rs::batch_import`** — the rate line in its three states
  (loading / a number / "enter one manually"), the no-price hint, the
  preview rows (a converted row says the token amount, an unconverted one
  its raw fiat), the rejected count (one / other plural keys), the over-cap
  and over-balance notices side by side, the Apply CTA counting only the
  rows that will be sent and drawn shut when the core shut it.
- **`flows/panels.rs`** — the unit toggle's halves, the paste box, the
  file and template affordances, the rate as a field with its Auto pill:
  bindable props, fixtures untouched, the gallery pixel-unchanged.
- **One new dependency**: `calamine` (pure Rust, no system library), read
  only. The template the sheet saves is CSV text the core composes.

**Headless, hermetic**: a pasted two-row USD table previews two rows, counts
the address-less line as rejected, converts at the self-priced rate, and
applies to exactly two drafts (`wallet::money::a_pasted_table_applies_to_its_rows`).

**Recorded**: no workbook fixture is committed, so the xlsx path is covered
by the cell codec and the not-a-zip refusal rather than a real sheet; the
sweep mode (N tokens → one address) stays fixture, as on web.

**Gates**: desktop **251 → 256** with the feature (252 without) · fmt clean
· 1 pre-existing warning · gallery sweep every state rendered · `rust/`
untouched.

## Phase 6 — the screen says what the core refuses

**The finding, from a sweep of my own two phases.** `SendView` carries
sixteen fields that are judgements — what is wrong, what is waiting, what may
not proceed — and the live builders I wrote in phase 4 read **none** of them.
Type more than you hold and the Continue button simply does not respond:
the core computed the sentence (`send.warnNotEnoughToken`, in every locale)
and the desktop threw it away. Fifteen more sat beside it.

| the core says | phase 4 | now |
|---|---|---|
| `amount_warning` (4 sentences) | dropped | amber notice under the fee |
| `same_asset_fee_issue` | dropped | title + "sending X + fee Y needs Z, you have B" + "you can send up to M" + **Edit amount** |
| `split_over_balance` | dropped | red notice |
| `confirm_amount_issue` | dropped | red notice + Edit amount, confirm page only |
| `denom_toggle_reason` | dropped (no ⇄ control drawn) | said as a warning, since no control exists to explain itself |
| `treasury_bootstrap` | **silent dead end** | the funding sheet's own words + the top-up address + the shortfall + **Check now** |
| `lock_error` / `add_network_msg` | dropped | the lock's title/body + the last attempt's outcome + **Add this network** |
| `can_continue` / `can_confirm` | CTA always armed | drawn shut, answering to nothing |
| `estimating_gas` / `sending` / `tx_status` | nothing | the button says "Estimating…" and keeps its accent — busy is not disabled |

Every word came from the corpus (`send.warn*`, `send.sameFeeToken*`,
`send.lock.*`, `componentsUi.funding.*`, `componentsUi.gas.estimating`);
**zero new keys**. One drawn model gained `notice` and `cta_state`; the
fixtures are untouched and the gallery renders exactly as before.

**One traversal, two readers.** `build_notice` returns the sentence AND the
way out (`NoticeWayOut::{RetryAfterBootstrap, AddNetwork, EditAmount}`), so
the button and the words can never disagree about what is being offered. The
page maps the way-out to the core's own recovery event.

**Two more drawn-but-dead affordances closed**: a group in the contact picker
now seeds a split with everybody in it (the hand-off web calls 群发转账), and
the notice's action is the only new button — it is the core's, not one this
file invented.

**A wrong assumption the test caught.** I asserted that an over-balance amount
disables Continue. It does not: the ported gate arms it and the refusal
arrives as an ALERT when it is pressed (`SendAlertKind::InsufficientBalance`,
`can_continue` stays true for an unlocked send). So between typing and
pressing, the warning sentence is the **only** thing on screen — which is
precisely why dropping it was worse than it looked. The test now drives the
real machine through both steps and asserts the sentence, the armed button,
the alert and that the flow stays on the form.

**Two more of the same class, found by carrying the sweep into phase 5's
work**: the fee sheet drew a coin that cannot cover the fee as selectable
(`FeeOptionView.insufficient` dropped — the core's invariant ⑧ says such a
row is shown for context and is NOT selectable, because paying gas in it only
produces a doomed operation), and a picked file the shell could not read
raised `BatchView.file_error` that nothing showed — a picker that silently
does nothing is indistinguishable from a broken one. Both fixed; the fee gate
lives in ONE place (the panel dims the row and drops its listener from the
core's own flag — a second gate in the page would be a second opinion about
one fact), and the over-balance refusal now carries the figure it is about.

**The sweep, carried across every machine the desktop reads.** The same
comparison — a view's fields against what a live builder reads — run over
`balance_dashboard`, `activity_feed`, `manage_tokens`, `receive_watch`,
`display_currency`, `contacts`, `network_admin`, `token_trust`,
`payment_request`. Triaged:

| unread | verdict |
|---|---|
| `BalanceView.{failed_chain_ids, rate_limited_chain_ids}` | **correct** — the core hands over `banner_chain_ids` (failed MINUS rate-limited, invariant ⑦) and 031 reads that. A rate limit lifts on its own and must never raise the "fix your RPC" banner |
| `BalanceView.{balance_partial, cached_total_usd, last_refreshed_at_ms}` | covered by `notice: StillUpdating` and `holdings_loading`, both read; no "as of" line is drawn |
| `MtokView.save_error` | **a real gap, fixed here** — adding a token could fail and the button just did nothing, twice |
| `NetWizardView.{phase, error}`, `NetView.last_added_chain_id` | the settings add-network wizard: a debt, and the same class. Not this cut's surface |
| `FeedView.{toast, new_item_id}` | the "money arrived" celebration — no drawing on the desktop |
| `ContactRecipientView.{saved, verified, is_contract, first_interaction}` | the contacts detail's trust line. The SEND path's own risk is live (phase 4); this one is 028's territory |
| `PaymentRequestView` (10/14) | the pay-link surface — `/pay` has no desktop entry yet (debt #4's neighbour) |
| `TrustView`, `TrustSimView`, `TrustIncomingView` | consumed by the executor and the feed writer, not by a display model |
| `BalanceSwitcherView` | the home's own account switcher — undrawn on the desktop (settings has one, 031) |

**Still not drawn** (recorded, not hidden): the ⇄ fiat/token control (its
refusal is now spoken, but the control itself is a drawing the desktop does
not have), the multi-token sweep picker (`multi_select_mode` and its
checkbox column), and per-row editing of a split (a seeded group's amounts
are typed in the batch importer or not at all).

**Gates**: desktop **256 → 258** with the feature (254 without) · fmt clean ·
1 pre-existing warning · gallery sweep every state rendered · the live spine
still reaches Confirm with the relay's real 0.010 xDAI quote.

## SC-303 — the dust moved

创始人 2026-09-07 点头,滑块拉了。

```
after continue: stage=Confirm fee=("10000000000000000", Native, "0xee2cca98…f0dd")
                fee_busy=false treasury=None alerts=[] can_confirm=true
relay: submitting sender=0x88cCA0…6894 nonce=…04 initCode=no callData=452B signature=429B
after slide: user_op_hash=Some("0x4d1cf38350afc661b18caa4c9862ffcef2d2a579ac1e4f6e3cec24d4e529849c")
test result: ok. 1 passed … finished in 88.86s
```

**链上核对(不看 `tx_status`——见第 3 条教训,那个字段签完名就翻)**:

| 查什么 | 屏幕说 | 链上 |
|---|---|---|
| 金标 Safe 余额差 | 0.001 + 0.010 = **0.011** | 0.75897 → **0.74797**,差 **0.011000**,逐位对上 |
| 收款方 fixture #1 `0x031d…772b` | +0.001 | 1.01599999… xDAI(收到) |
| 中继收费地址 `0xee2c…f0dd` | 0.010 | 0.078896… xDAI(收到) |
| **`receipt.status`** | — | **`0x1`**,`eth_getUserOperationReceipt` 的 `success: true` |
| 上链交易 | — | `0x98c8f65c6a9fa77906113022974ef2af2f74049c61ef718aa00fad0cfe9adfc9`,区块 48120671 |

**SC-303:达标。** 桌面从真持仓、真报价、固定密钥集签名、真中继提交,到链上收据状态,
整条链路走通,数字和屏幕一致。

## Phase 7 — the same sweep, on somebody else's screen

交接表里的第 9 条:加网络向导。**不是本刀画的界面**,但是本刀 phase 6 那个毛病的同一株
——核心把判断算好了,屏幕不说。这一刀把 phase 6 的规矩搬过去。

`NetWizardView` 有 `phase` 和 `error`,`NetView` 有 `last_added_chain_id`;桌面
`settings/live.rs` 一个都没读(第二条 grep 的差集)。后果不是难看,是**对话框看着坏了**:

| 核心说什么 | 之前 | 现在 |
|---|---|---|
| `phase: Searching` | 无 | 转圈 + "Searching…" |
| `phase: Resolving` / `Checking` | 无——点完一条建议,索引解析加一轮 RPC 竞速,几秒里对话框一动不动 | 转圈 + "Checking compatibility…" |
| `error: AlreadyAdded` | 无——最常撞上的那条:挑一条钱包已有的链,向导原地停死,CTA 也不画,对话框像是没反应 | "This network is already added" |
| `error: NotFound` | 无 | "Chain info not found" |
| `error: NoRpcEndpoint` | 无 | "{链名} RPC unavailable"——链已经解析出来了,句子就该点名它;底下那个自定义 RPC 框就是出路 |
| `error: NotCompatible`(扫码路径) | 无 | "Incompatible" |
| `last_added_chain_id` | 没读:**按下就关**对话框 | 关不关由核心说了算 |

最后一条是本刀改动里唯一动了行为的。`add_confirmed` 的每一道门(未加载、非 `Checked`、
不兼容)都是**静默** `return done()`,而壳按下就把对话框关掉——一旦哪道门拦住,人按了一下,
屏幕消失,什么都没加。现在壳在派发前后各读一次 `last_added_chain_id`,变了才关;没变就把
对话框留在原地,上面那行拒绝理由自己会说话。

### 顺着同一把尺子往下查:网络卡片上的那句"已保存"

第二条 grep 对 `network_admin` 的 38 个视图字段跑完,还剩四个没读:`rpc_save_deferred`、
`explorer_health`、`bundler_url`、`native_symbol`。后两个是资料不是判断,`explorer_health`
记进欠账;`rpc_save_deferred` 当场修了,因为它**不是沉默,是小谎**:

改完 RPC 一失焦,卡片下面那句提示写着"Saved as soon as you leave the field"。可核心这时
把 `rpc_save_deferred` 置了真——覆盖值**还没写**,要等 RPC 自报的 chain id 对上才写。
那几秒里屏幕替一件没发生的事打了包票。现在三态按核心的分量排:拒绝(说清它到底服务哪条链)
> 待判(`componentsUi.funding.checking` = "Checking…")> 那句常驻提示。抽成
`settings::live::override_hint`,页面只负责画——+2 个测试。

**新增语料键 0 个**,和 phase 6 一样。六句话本来就都在语料里:`addToken.errorAlreadyAdded`
/ `errorChainNotFound` 是因为 `NetWizardErrorKind` 一份服务两个调用方(核心的不变量①),
扫码路径和加代币页早就在说同样的话;`assets.rpcUnavailableSingle` 带 `{{name}}`,正好点名。

新增 9 个测试(`settings::live::wizard_tests`)。要点:`select_chain` 在 `loaded` 之前
**fail closed**,所以测试必须先把 `ReadStore` 答掉——不答的话向导测试是空跑,一条都验不到。

### 编译器早就在报的第 11 条

`cargo test` 有一条 `field \`notice\` is never read`(`flows/fixtures.rs` 的 `AddToken`)。
那个字段是 **phase 6 自己加的**:`live.rs` 从 `MtokView.save_error` 填了它,而
`panels.rs::add_token` 从来没画。**代币存不进的那句话,phase 6 送到显示模型就死在那儿了**
——和文件读不出被吃掉是同一个缺陷,只是往外挪了一层,而且唯一注意到的是一条没人看的警告。
现在画在 CTA 上方(复用 `notice_card`)。

顺带:交接里写的基线"1 个既有 warning"不实。`--tests` 下有 3 个,多出来的两个是
`AddToken.notice`(真缺陷,已修)和 `user_op.rs` 测试里的 `to_hex`(feature 关掉时没人用,
已按 feature 门住)。现在两种 feature 配置下都确实只剩 `BLE_CHANNEL_SUPPORTED` 一个。

## Phase 8 — 028 并进来,桌面那份分叉删掉

创始人 2026-09-07 点批:**就在这棵树上并**。`origin/main` = `61568f22`(PR #186)。

**冲突五处,四处是生成物**:`.specify/feature.json`(取本侧)、`rust/pkg-web/*` 与
`public/vela_core_bg.*.wasm`(两边都重建过 wasm)。按交接第 5 步重建入库:
`node rust/scripts/build-web.mjs` → 新指纹 `1b6c8ce4be03`,**3,725,860 字节**
(本分支原 3,630,664——028 的新事件与拼音表在里面);`verify-web.mjs` 46,513 例全绿,
`gen-onboarding-types.mjs --check` 25 个类型现行。唯一手并的是
`vela-core/src/lib.rs` 的一句文档注释:取 main 的措辞,因为它把两个壳都点了名。

**一个意外的好消息**:`user_op.rs` 在 main 里已经和本分支**逐字节相同**——028 Phase 8
把它当作 web 那份 TypeScript 装配的第二实现来对照。所以本 stack 在 `rust/` 下真正独有的
只剩 `dev_fixtures.rs` 和那个 feature。

**六步的结果**:

| # | 事 | 结果 |
|---|---|---|
| 1 | `contacts/live.rs` 测试字面量 | 补了三个字段——但**不是填 `Vec::new()`**:那样每行都会归到 `#`,测试照过、什么也没证。改成调核心的 `section_contacts` 现算 |
| 2 | 导入/导出改派核心事件 | 已改。`ImportFile`/`ImportAcknowledged`/`ExportRequested`/`ExportTaken`,**`executor/contact_io.rs` 565 行连测试一起删** |
| 3 | `add_group_members` 等三个新事件 | 桌面根本没有成员选择器,无处可核对;记为将来的能力 |
| 4 | `send.rs` 两条 | 都不用改:Dsd2e 的"选中 + 关闭"双发本来就是按"哪个核心都画同一个屏"写的,新核心下第二发是空操作;`prefilled_recipient` 进 `recipient` 现在是核心自己做 |
| 5 | `pkg-web` 冲突 | 见上,重建入库 |
| 6 | 分组字母归核心 | 已改,见下 |

**第 2 条是这次并树真正的理由。** 坏文件(非法 JSON、没有地址列的 CSV、空表)在 web 上被
**拒绝**,在桌面上原来是"成功导入 0 条"——从外面看和一本空通讯录一模一样。现在壳只负责
读字节和它自己的失败(文件读不出),**关于这些字节的一切判断都归核心**;拒绝优先于报告,
因为被拒的文件什么也没写,"新增 0、跳过 0"会把它描述成一次成功的空导入。

**第 6 条改了行为,不只是搬家。** 桌面原来的 `section_of` 只认 ASCII:阿豪归 `#`。核心的
`contacts_initials.rs` 逐码点拼音首字母:阿豪 → A。**同一个人在两端归到不同字母下**,
这种事没人会报但人人会注意到。顺带修掉桌面独有的一个 bug:原来按**连续段**分组,书序里
不相邻的两个 A 会变成两个 A 段;核心的目录不会。

### 并完之后的两个数字,和一条藏了很久的假绿

**desktop 263(feature on)/ 259(off)· vela-core 1,282**(并树前 267/263 · 1,264)。
桌面**少了 4**。原因说清楚:
删掉的 `executor/contact_io.rs` 带走了它自己的 **6 个测试**,本刀新增 2 个,净 −4。
那 6 个测的规则没有消失,是搬到了核心:`rust/crates/vela-core/tests/app_contacts.rs`
有 **57 个测试**,四种拒绝(`MalformedJson` / `NoAddressColumn` / `Empty` / `UnknownGroup`)
都在里面。SC-306 写的是"两个 crate 的测试数严格增加":核心侧 +18(1,264 → 1,282)是增的,
桌面侧是减的,**因为删的是一份重复实现**——如实记在这里,而不是让它看起来像退步。

闸门全绿:两端 fmt clean、clippy `-D warnings` 无话、gallery 36 态全渲染、
Windows 通过类型检查、`verify-web` 46,513 例。

**028 的暖报价撞上了 phase 6 的测试驱动。** `every_refusal_the_core_makes_reaches_the_screen`
的 pump 有个 `unreachable!` 兜底,新核心从**表单**就发一次 `EstimateFee`(`tx: None,
batch: None`——028 phase 10 的"选中就暖一次报价"),于是当场炸。处理同 15 秒竞速:
挂着不答。这条和 028 web 会话记的"FIFO 驱动遇上新计时器操作"是同一个坑的两端。

**闸门命令本身会瞒报。** 交接里(以及本文件上面)那条
`cargo test 2>&1 | tail -3 && …`,`&&` 接的是 **`tail` 的退出码**,永远是 0——
上面那次真实的测试失败,后台任务报的是 **exit 0**,我是靠读输出才发现的。
以后跑闸门要么加 `set -o pipefail`,要么别把 `cargo test` 接进管道再用 `&&` 串。

老测试 `sectioning_groups_without_reordering` 断言的正是那条桌面规则(字母按书序)。
它被**改写而不是删掉**:字母是目录(A–Z 然后 `#`),字母**之内**仍是书序(收藏优先、
最近其次)——后半句一直是对的。新增 2 个测试(拼音首字母、一个字母一段)。

## Phase 9 — 并完之后再跑一次第二条 grep

核心在脚下换过了(028 给 `send.rs` 加了 229 行、`contacts.rs` 加了 349 行),而桌面的
live 构造器是在那之前写的。所以并完树立刻重跑普查:**核心所有 View 的判断字段 vs
桌面读了什么**,7 个未读,逐条判:

| 未读字段 | 判定 |
|---|---|
| `SendReceiptView.hold_reason` | **真缺陷,关钱。已修** |
| `BalanceView.failed_chain_ids` | 有意不读——核心给了 `banner_chain_ids`(失败减限流,不变量⑦),`wallet/live.rs:608` 已注明 |
| `RpcPoolView.failed_chains` | 同上,它是余额横幅的来源,余额那台机器已经在读 |
| `FeeView.stale` | 有意不读。**核心自己写着**:"Staleness is advisory — it does not disable confirm, because today's UI does not either",真正的门在提交侧(`tempo_quote_is_stale`、中继的 in-band gate)。要做刷新控件得先有图 |
| `SignFundingView.denial_reason` | B 组,桌面还没有请求来源 |
| `PaymentRequestView.can_copy` / `can_save` | **要创始人定**,见下 |

### 修的那个:收据不说它为什么停着

核心把收据的"停"分成两种(`SendHoldReason`),桌面一种都没读:

- **`FeeHold`** — 手续费涨过了你批准的数,**交易排着队,费用回落会自动发出去**。
  状态仍是 `Submitted`,所以屏幕说的是普通的"等待确认"。那不是同一件事:
  人盯着一笔可能很久不动的转账,屏幕上没有一个字解释。
- **`FeeRejected`** — 费用一直没回落,**什么都没发出去**,出路是按当前费用重发,
  不是重试同一笔。屏幕原来只给一句通用错误。

**新增语料键 0 个**——`send.txHeldFees` 和 `send.txRejectedFees` 这两句话一直在语料里,
一字不差,**而且 web 也没读**(`hold_reason` 在 app-web 只出现在一个测试夹具里)。
这条是跨端的:核心算了,两个壳都没说。+2 测试,基线 **265 / 261**。

> **给 web 那边的人**(不是我的范围,但漏在同一处):
> `app-web/vela-wallet/src/lib/flows/live-send.ts:572` 的 `submitted` 分支写死
> `captions: [m['send.txWaitingConfirm']]`,`failed` 分支同理没有 hold 的位置。
> `hold_reason` 在 app-web 只出现在 `live-send.test.ts:270` 的夹具里(`null`)。
> 两句语料键:`send.txHeldFees`、`send.txRejectedFees`。
> 注意两个标志不互斥,别把两句合成一句(下面那段)。

**改完自己又抓到一个边**:两个模型标志**不互斥**——先 `FeeHold` 后失败的收据,
`fee_held` 还留着。第一版我把 `hold_reason` 当成一句话往两个分支里塞,
于是"排着队、费用回落会自动发出去"有可能印在一张**失败**的收据下面。
现在每个分支只认自己那个原因,并且有一条测试专门盯着这个组合。
**核心把两件事分开了,壳就不能把它们合成一句。**

### 要创始人定的:收款页的确认门

`PaymentRequestView` 的 `can_copy` / `can_save` 都等于 `acknowledged`——人得先确认过
一个提示,才允许复制/保存收款请求。桌面的收款页是**活的**(它已经在读 `payment_request`
决定二维码内容),但这两个字段和 `acknowledged`、`gate_loading` 都没读,等于门是开的。

**没有自作主张给它加门**:记忆里 Receive 的链上门(issue #14)是 2026-07-03 被判为
过时关掉的(passkey 才是信任根)。这个门是不是同一件事、还该不该有,是产品判断,
不是接线判断。

## Phase 10 — 那张从来没被打开过的表

欠账第 1 条的后半句:"没提交 xlsx 样张"。查下来比缺个样张更糟——**workbook 那条路
一次都没在测试里跑过**。`executor/batch.rs` 的四个测试测的是:扩展名判断、单元格格式化、
一个"不是 zip 的假 xlsx"、以及 CSV。`calamine::open_workbook_auto` → `worksheet_range_at(0)`
这一句,也就是真正读表的那句,从来没执行过。

"从 Excel 把工资表拿进来"是**花钱的路**,以前每一个测它的测试用的都是测试里自己敲的文本。

现在提交了一张真表 `app-desktop/vela-wallet/tests/fixtures/payroll-sample.xlsx`
(1,737 字节,手工装配的 OOXML:五个 part,inlineStr + 数字单元格),故意做了三件事:

| 表里有什么 | 想钉住什么 |
|---|---|
| `5000`(整数) | 表格眼里是数字,人眼里是 `5000`——不能变成 `5000.0` |
| `173.88` | 上面那条去 `.0` 不能变成无差别截断 |
| 只有一个单元格的**短行** | 读的时候要补齐到最宽;不补,下一行的值会滑进别人的金额列 |

两个测试:一个在 `read_table` 旁边,证明 calamine 把文件读成了那个矩阵;一个在
`wallet/money.rs`,把**同一个文件**经 `PickFile` 喂进 `BatchImport` 一路走到付款行——
表头不算行、短行进 `rejected` 计数(不是悄悄丢掉)、合计 `5173.88`、
Apply 之后两行金额 `5000` 和 `173.88` 一位不差。

**"真表格实机点一次"仍然欠着**——那要人去点文件对话框。但现在它不是唯一的证据了。

### 顺手:浏览器端点的探针结果(欠账 10b)

网络卡片的浏览器地址栏和 RPC 地址栏用的是**同一个组件、同一个徽章位**,RPC 传了
`rpc_badge`,浏览器传的是 `None`。核心两个都探了,`explorer_health` 就这么扔了——
量到延迟的浏览器和根本没探过的长得一模一样。改成传 `explorer_badge`,一行,
用的是已经有的 `probe_badge`,没有新画面。

## Phase 11 — Windows 的日界线,以及怎么验一段编译不了的代码

031 留的第 5 件:`GetTimeZoneInformation` 没接,`local_utc_offset_seconds()` 在
非 unix 上直接返回 0。后果不小:**Windows 上活动列表按 UTC 分日**,晚上的转账归到明天,
"每天有一段时间,谁的列表都在说错话"。

**这段代码在这台机器上编译不了。** 桌面 app 的依赖树要编 C(ThorVG、resvg、hidapi),
`--target x86_64-pc-windows-gnu` 会死在 build script 里,连 Rust 都到不了——
这正是 `check-windows.sh` 只检一个独立小 crate 的原因。所以验证分三层:

1. **算术单独拆出来、不带 `unsafe`、每个平台都编都测**。`windows_offset_seconds`
   是纯函数,4 个测试在本机跑:
   - **符号**:Win32 定义 `UTC = local + bias`,所以偏移是 bias 取反。柏林冬天 bias −60 → +3600。
     搞反了柏林就成 UTC−1、纽约成 UTC+5。
   - **季节**:DAYLIGHT 要用 `DaylightBias`。夏天误用 `StandardBias` 就差一小时,
     而且差得"看着很合理"。
   - **半小时区**:印度 UNKNOWN + bias −330 → +19800,整点假设会丢掉它。
   - **调用失败**:`TIME_ZONE_ID_INVALID` 时结构体根本没填,必须**退回 UTC 而不是拿垃圾算**
     ——错得没规律比错得有规律更糟。
2. **FFI 那几行,原样抬进一个隔离 crate 交叉编译**(`x86_64-pc-windows-gnu`,
   `clippy -D warnings` 也过)。这一步当场抓到:**windows-sys 0.59 只导出
   `TIME_ZONE_ID_INVALID`**,另外三个 id 不存在,所以按值匹配、并对导出的那个下
   `const _: () = assert!(… == u32::MAX)`——将来哪个版本改了编号会编译失败,
   而不是悄悄挪掉所有人的日界线。
3. **依赖连线**:`cargo tree --target x86_64-pc-windows-gnu -i windows-sys@0.59.0`
   确实显示 `vela-wallet` 这条边。Cargo.toml 第 150 行那段警告是认真的
   ——这个 crate 就曾经被写进 macOS 的 target 段里、Windows 路径整个没链上而闸门全绿。

顺手把 cfg 从 `not(unix)` 收紧成 `windows`:函数体现在依赖 `windows-sys`,而它只在
`cfg(windows)` 下存在;既不是 unix 又不是 windows 的目标现在会**找不到这个函数**,
比再默默按 UTC 分一次日要好。

**没验的那一层写在这里**:没有在 Windows 上跑过。编译、clippy、算术都过了,
行为没有。和 `check-windows.sh` 自己的说明是同一句话。

### 把那次验证做成闸门,而不是一句记录

上面第 2 层本来是我在临时目录里手工做的一次性动作。现在写进 `scripts/check-windows.sh`:
它把 `src/executor/mod.rs` 里那两个函数**原样抬**进一个临时 crate(没有 C),
交叉编译 + `clippy -D warnings`。抬取是**故意死板的文本匹配**,函数被改名就大声失败,
而不是悄悄什么都没检。

**并且验过它会失败**:把 `info.StandardBias` 改成 `info.StandrdBias`,脚本以 1 退出、
指着那个字段报错;改回来就绿。一条不会失败的闸门比没有闸门更糟,这是本刀
"管道吃掉退出码"那条的同一个教训。

> **闸门的第二个坑**(第一个是管道吃退出码):Bash 工具的**工作目录会留在上一条命令**。
> 我在隔离 crate 里 `cd` 过一次,下一条闸门就在**那个目录**跑了 `cargo fmt --all --check`,
> 报的是隔离 crate 的格式问题。闸门命令自己带上 `cd`,别指望继承。

## Phase 12 — 余额一条一条地到

031 留的第 4 件,也是欠账表里最后一件**纯工时**的事。核心一直支持流式:

```text
AccountChanged ─► reset ─► ReadBalanceCache ∥ FetchTokens ──► stream:
    ChainAssetsArrived (merge per chain, slow chains keep last value)
```

桌面一直没用。`FetchTokens` 是一次 `Answer::Blocking`:十二条链十二个线程一起跑,
然后 **join 完再一次性回答**。后果是——**一条 RPC 不通,整个英雄区就按它的超时僵着**,
另外十一条早就答完了,屏幕上却还是骨架(或者昨天的缓存总额)。

### 壳缺一条缝

`Answer` 只有 `Now` / `Blocking` / `After` 三种,都是**答一次**。流式要的是"边做边说",
所以加了第四种:

```rust
Streaming(Box<dyn FnOnce(&Sink<E>) -> T + Send>)
```

`Sink<E>` 可 Clone、可跨线程(十二个线程共用一个),每次 `send` 变成一个**事件**,
在主线程按顺序 dispatch 进这台机器,**全部在它自己的结果之前**。这条顺序就是全部契约:
`balance_dashboard` 把每次到达并进 token 列表、只结算一次,**结算之后再来的快照会把
已经算过的链复活**。

实现上顺序是自然保证的,不是靠小心:排空循环在 sink 被丢弃时结束,而 sink 是在
`work` 返回时丢的。写了一个不带 gpui 的测试盯着这条(12 个事件按序、然后才是结果),
外加一条"接收端没了 send 不能炸"——窗口在取余额途中被关掉,十一条链手里还攥着 sink。

`Answer` 从 `Answer<T>` 变成 `Answer<T, E>`,十个 `Machine::perform` 签名跟着改成
`Answer<XShellResult, Self::Event>`(编译器一个个指出来的)。`wallet/money.rs` 那个
**屏幕自己拥有的** fee 泵也补了同一条臂——`fee_policy` 今天不流式,但写成 `unreachable!`
的话,它哪天开始流式就是确认页上的一次 panic,而这不过是资深泵里同样的八行。

### 取数那半边

`fetch_all` 拆成 `fetch_all_streaming(address, &Arc<ChainSink>)`,每条链的线程算完
**自己那条链的 token 就立刻报**;`fetch_all` 就是传一个什么都不做的 sink,所以账户切换器
和现有测试一行没动。快照必须是"那条链的",因为核心的合并规则是**按 chain_id 替换、
其余保留**;空快照(那条链什么都没有)正确地什么也不改。

`FetchAccountAssets` **故意不流式**:它读的是切换器里别人的账户,快照会被并进当前账户。
核心的文档也是这么写的("never streams")。

结算仍然只有一次:核心要完整图景才能决定写不写缓存、哪些链算失败。

### 证据

**五个**同步测试驱动器(它们自己在测试里跑泵)也补了这条臂,共用一个
`resident::run_streaming` 而不是各写一遍。第五个只在 `--features dev-fixtures` 下编译,
所以第一遍闸门才发现它——**闸门要跑两种 feature 配置,不是一种**。真网那条 `#[ignore]` 测试:金标 Safe 上
**报告不止一份**、**其中一份在结算之前就已经把钱和一个能画的总额交到核心手里**、
结算后的持仓不少于那一份。

**第一版这条测试写错了,而且是真网跑出来打脸的**:我断言"**第一份**报告就带着钱"。
金标 Safe 只在 Gnosis 上有 xDAI,**十二条链里十一条正确地报了空快照**,
先答完的几乎必然是空的那些。断言改成"**存在**一份结算前的报告带着钱"——
这才是这个功能的主张。多亏跑了真网,不然这条测试会一直是个假的规格。

顺带清了三条自己带进来的警告(`#[must_use]` 落在类型别名上、`try_next` 已弃用、
测试模块里多余的 `StreamExt`)——第 6 条教训的现场复习:两种配置各只剩
`BLE_CHANNEL_SUPPORTED` 一个。

**这条测试证明什么、不证明什么**(先写清楚,免得名字比内容大):它证明报告存在、
每条链报自己那份、第一份在结算之前就有可画的总额。它**不**证明墙钟意义上的"更早"
——同步驱动器 `run_streaming` 保的是**顺序**,不是并发(它自己的文档就这么写)。
真正的时间性归 async 泵,而这个仓库没有 gpui 测试夹具能驱动它。**这一层没测。**

## Phase 13 — 桌面的 web 引擎:选型,和一次跑通的探针

创始人问"接哪家、你会不会接"。没有列表格,直接接了一个跑起来。

### 决定性的事实(都是查出来的,不是记得的)

| 事实 | 出处 |
|---|---|
| `gpui::Window` **实现 `HasWindowHandle`** | `crates/gpui/src/window.rs:6390` |
| macOS 交出来的是 **`AppKitWindowHandle`(NSView)** | `gpui_macos/src/window.rs:1918-1921` |
| Windows 交出来的是 **`Win32WindowHandle`(HWND)** | `gpui_windows/src/window.rs:581` |
| gpui 钉 `raw-window-handle = "0.6"` | Zed 根 `Cargo.toml:758` |
| wry 0.56.1 也钉 `raw-window-handle = "0.6"` | wry `Cargo.toml:152` |
| `build_as_child<W: HasWindowHandle>` | wry `src/lib.rs:1571` |

**两边在同一个版本的 `raw-window-handle` 上碰头**,所以句柄类型是同一个类型
——这正是本仓库 Cargo.toml 里那条"两个 rwh 版本会变成两个类型"的警告说的事,
这次是它成立的一面。

### 探针:`VELA_WEBVIEW=1`

`main.rs` 在开窗时把一个真 webview 挂成子视图,注入一段脚本,脚本回调 IPC。跑出来两行:

```
[vela-wallet] webview: attached as a child of the gpui window
[vela-wallet] webview ipc: vela:probe
```

第二行才是重点:**注入的脚本在页面里执行了,并且通过 IPC 通道说回来了**——
`window.ethereum` 要的那条缝是通的,不只是"画出了像素"。截图确认页面可见。

### 三个必须先说清楚的代价

1. **合成在 gpui 之上,不在里面。** 原生子视图就是这样。
   **我第一版把这条写成"签名面板要盖在浏览器上"——写错了**,创始人当场纠正:
   桌面的签名面板是**第三列**(`PanelId::Signing` → `panel_scaffold`,和收款、
   资产详情同一个脚手架),它挨着浏览器、把浏览器挤窄,不盖在上面。手机版
   clearsigning 那些图看起来像盖上去,是因为手机只有一列。
   这条约束真正落在两个地方:**离开浏览器时**(原生子视图不会因为 gpui 换了路由
   就消失,必须显式藏)、和**居中弹窗**(扫码、设置对话框会被画在 webview 底下)。
2. **Linux 是另一件事。** wry 在 Linux 上 `os-webview` 拉 gtk + webkit2gtk + soup3,
   而 CI 的 `desktop` job 和 `desktop-linux-packages.yml` 都没装这些;而且
   `build_as_child` 在 Linux **只支持 X11、不支持 Wayland**,还要 `gtk::init` +
   在 gpui 的循环旁边推 GTK 的循环,而 gpui 有 Wayland 就走 Wayland。
   所以本刀把 wry 放在 `[target.'cfg(not(target_os = "linux"))'.dependencies]`——
   **不是忘了 Linux,是把它记成一个单独的决定**:桌面浏览器先只上 macOS + Windows,
   Linux 明说"暂不支持",还是为 Linux 单开一个顶层窗口(要 GTK 双循环,脆)。
3. **objc2 会有两份。** wry 要 0.6.4,本 crate 为了跟 gpui 一致钉 0.5。两份能共存,
   因为跨过去的只有一个 rwh 裸指针,没有 objc2 类型。编译时间会长一点。

clippy 警告数 **42 → 42**(基线也是 42,用 stash 量过),没有新增。

### 没做的

探针就是探针:固定 bounds、一段内联 HTML。真正的 C 组还要按列的布局跟随 bounds、
导航/前进后退、per-site 权限、把 027 的 `inpage.js`/`protocol.js` 接到
`with_initialization_script` + `with_ipc_handler` 上,再驱动 `dapp_session`
`dapp_permissions` `browser_history` 三台机器(3,771 行)。

## Phase 14 — 浏览器成了一列(C 组的壳)

创始人点批"先 A 后 B":先把浏览器做实,B 组的签名请求才有来源。

**真 Uniswap 现在跑在 explore 那一列里**(截图为证:app.uniswap.org 的
"Swap anytime, anywhere." + 真兑换组件,上面是 Vela 自己的标签条和工具栏,
左边是侧边栏)。

### 位置跟着列走

webview 的 bounds 由**拥有那块矩形的元素在 paint 阶段**给出(`gpui::canvas`),
所以窗口缩放、第三列(签名面板)打开挤窄浏览器,它都跟着走。只有 bounds 真的变了
才跨平台边界调 `set_bounds`——不然一秒六十次。

### 那条约束的正确形状

我 phase 13 把它写成"签名面板要盖在浏览器上",**是错的**,创始人纠正了:
桌面签名面板是第三列,和收款/资产详情同一个 `panel_scaffold`,挨着而不是压着。
真正要管的是两处:

1. **离开浏览器**。原生子视图不会因为 gpui 换路由就消失。所以每一帧只要不是在画
   浏览器列,就 `webview::hide()`——漏了这一句,webview 会浮在钱包上面。
2. **居中弹窗**(扫码、设置对话框)会被画在 webview 底下。这两个还没处理,记账。

### provider:027 的脚本原样注入

`inpage.js` **一个字没改**地 `include_str!` 进来(434 行,扩展里那份)。扩展是
MAIN world + isolated world 用 `window.postMessage` 对话,wry 没有 isolated world,
所以补了**十一行 bridge**:把同样的信封转给 `window.ipc`,答案再用 `window.postMessage`
送回去。provider 分辨不出区别,这正是重点——第二份 EIP-1193 实现就是第二套 bug。

**origin 由宿主读,不信页面。** 扩展的 content script 存在的理由就是"带两个页面伪造不了的
事实:哪个标签页、哪个 origin"。这里同样:origin 从 `webview.url()` 读,
页面在信封里自称的 origin 一律忽略。

**请求现在被拒绝而不是被吊着**:`dapp_session` 三台机器还没接,所以答 4900
(不是 4001——027 D37:永不结算的 promise 是这条通路最坏的产出,而"干净的拒绝"
和"提交了但卡住"必须能分辨)。

### 顺手修的真 bug

收藏格子原来**每一个都只是把 `browsing` 置真**,页面画同一张 mock——点 Aave 出 Uniswap。
现在每个格子带着自己的 host 去 `navigate`。这个 bug 在 mock 时代看不出来,页面一真就是错的。

### 还欠

地址栏是画的静态 host(图就是这么画的,没自作主张改成可编辑);标签条还是 mock;
逐站点权限、历史、`dapp_session`/`dapp_permissions`/`browser_history` 三台机器(3,771 行)
都还没接。Linux 依然在 `cfg(not(target_os = "linux"))` 外面——**创始人已定:Linux 要支持,
但先上 mac + win**。

## Phase 15 — B 组开工:第一台机器(approval_guard)

签名面板三台机器,9,362 行核心。**这是多刀的活,不是一刀**。本 phase 落第一台,
把模式立住。

`approval_guard` 只问三个 `eth_call`,但三个都是关于同一件事:**人到底同意让合约动多少**。

| 操作 | 为什么它关钱 |
|---|---|
| `ReadTokenMetadata` | 没有 decimals,授权额度就渲染在错误的数量级上。`1000000` 是一千个还是一个,只由这一个调用决定 |
| `ReadErc20Allowance` | `increaseAllowance` 是**加**不是**换**。结果总额 = 已有 + 增量;只显示增量就低估了人正在同意的东西 |
| `ReadErc20Balance` | 「永不无限额」要给人一个能填的数,自己的余额是他能推理的那个(issue #86) |

核心把失败分了级,壳不能抹平:`None` 元数据是"整批读失败",而某个代币**不在**列表里
是"这个解析不出来"——两者的兜底不同。批次回来长度不对时我返回 `None` 而不是空列表,
就是这条。

**新增 `abi::enc_allowance`**(`allowance(address,address)`,选择器 `dd62ed3e`)。

### 一个只有真网能抓到的错

第一版 `eth_call` 写成 `pool::call(...).ok()?.as_str()`——**`pool::call` 答的是整个
JSON-RPC 信封,不是 result**。把信封当字符串读,一个完全正常的调用会静静地答 `None`,
于是三个好读变成"整批失败"。真网测试当场炸出来;离线测试永远看不到,因为它根本不发请求。
房子里现成的写法(`manage_tokens::eth_call`)是 `.get("result")`,照抄就对——
**这就是"先看隔壁怎么写"比"自己想当然"便宜的地方**。

两个测试:一条离线(**没有代币要读 ≠ 读失败**,必须答 `Some(vec![])` 且不发请求),
一条 `#[ignore]` 真网(Gnosis 上的 USDC.e:符号有、6 位小数、余额读得到)。

## Phase 16 — B 组第二台:clear_signing 的读

三台里最大的一台(4,841 行核心),五个操作。这一层的分工比别处更要紧,
因为**降级阶梯的每一级都是"知道多少"的判断**,壳一旦替它判断,级就悄悄塌了:

- 404 的描述符 ≠ 解析失败的描述符
- **revert 的 `eth_call` ≠ 够不着的 `eth_call`**。前者是真答案("这不是 ERC-721"),
  后者是"我们没能问到"。掉不掉一级,取决于分得清
- 没人认识的选择器 ≠ 我们忘了去查的选择器

所以答案原样回去:body-或-`None`、result-或-`None` **外加一个独立的 `rpc_error` 标志**、
查不到就 `[]`。级由核心挑。

| 操作 | 做法 |
|---|---|
| `HttpGet` | 5s 预算(`NET_TIMEOUTS.descriptor`),200 才给 body。404/超时/断网都答 `None`——对核心是同一件事 |
| `RpcEthCall` | 走 pool,**信封里有 `error` ⇒ `rpc_error: true`**(这次没再踩 phase 15 那个信封坑) |
| `SelectorDbLookup` | 三个库**并发问再合并**(不是竞速):Sourcify 4byte、OpenChain、4byte.directory。openchain 形状的两个过滤过垃圾所以排前,4byte.directory 补缺口并按 id 升序(最小 id 是规范签名)。进程内缓存 |
| `Timer` | `Answer::After` 用 gpui 的计时器,不是停一个线程——一次解析要问三个问题,那就是三个白等的线程 |
| `Now` | `Answer::Now` |

`chain_tokens::data_base()` 改成 `pub(crate)` 共用:描述符和代币索引必须从**同一个**
配置端点拿,不然设置页改了地址只有一个调用方跟着变。

两个测试:一条离线(**什么算选择器**:大小写、可选 `0x`、整段 calldata 取前四字节、
太短/非十六进制都不问),一条 `#[ignore]` 真网(`transfer(address,uint256)` 必须在候选里
——它要是找不到了,通用解码那一级就没了,每一笔不认识的转账都掉到盲签;
外加第二次调用必须走缓存,不然签名面板每敲一个键就锤三个公共服务)。

## Phase 17 — B 组第三台:sign_request(会花钱的那条)

七个操作。**这是这个 app 里第二条会把钱送出去的路径**,所以先说清楚它复用了什么、
拒绝发明什么。

### 复用,不是重写

`SignAndSubmit` 就是 032 给发送流写的那条 `user_op::submit`(passkey → 装配 → 提交),
**故意是同一条**:dApp 的交易和人自己的转账要由一份实现装配、定价、签名,
否则"显示会发生什么的那张单"和"真正让它发生的代码"就是两个意见。
`SignContext` 也不是抄一份 `SendContext`,而是**调用它**再取字段——
同一个账户该用哪种仪式,两个答案就是一个钱包在这屏要手机、那屏要安全钥匙。

### 报两次,差别要紧

`SignAndSubmit` 用的是 phase 12 那条 `Answer::Streaming` 缝:

- **中途**把中继接受的 `user_op_hash` 通过 `Event::OpSubmitted` 送回核心。
  **在等收据之前**——等待期间窗口被关掉,重开时也得知道这笔提交过,
  否则一笔已提交的交易看起来像从没发生过。
- **一次**最终结果。交易给的是**真 tx hash**(等到收据),因为
  `eth_sendTransaction` 就该 resolve 成 tx hash;**userOpHash 不是 tx hash**,
  dApp 拿它去查会永远查不到。

收据等 90 秒封顶。超时给 userOpHash 而不是错误:**dApp 的 promise 必须结算**,
而"卡住但已提交"和"干净的失败"必须分得开(027 D37 的双花风险)。

### 拒绝发明的两个数

- **`funding` 给 `None`**。中继的 `account_info` 有存款地址和余额,但**没有
  threshold / recommended**——那是**策略数字**,而一个凭空编数字的充值屏会让人打错金额。
  核心对"凑不出 funding"的既定兜底就是普通失败,那是诚实的。
- **赞助给 `Denied { reason: None }`**,不是 `Funded`:桌面没有赞助通路,
  `Funded` 会是"别人替你付了"的谎。

`SubmitFailure` 本来就是**类型化**的枚举,所以这里一处字符串匹配都没有——
核心文档警告的那层正则(`parseBundlerUnderfunded`、`PasskeyErrorCode.CANCELLED`)
032 的发送路径已经付过一次,不付第二次。

### 四个测试,和我自己漏的那个洞

params → calls 是纯函数,离线测得到,而它正是"数字错了就是金额错了"的地方:
十六进制上线、**十进制进核心**;缺 value 是 0、缺 data 是 `0x`(大多数合约调用不带钱);
批量保序(重排过的批量是另一笔交易)。

第四条测"读不出来就拒绝,而不是提交一个空批量"——**写完才发现我自己留了这个洞**:
`wallet_sendCalls` 的 `calls: []` 原本会答 `Some(vec![])`,那会装配出一笔什么都不做、
却照样收费的 user operation。检查放在映射**之后**,因为"每一条都读不出来"和
"本来就是空的"产出同一个空向量。

### 还没接

模块整体挂着 `#[allow(dead_code, reason = "wired by the signing panel's host, phase 18")]`
——和 030→031 之间的 `pool::call` 同一个状态,标出来而不是让警告数失去意义。
**host 落地时必须摘掉**:allow 会连带把被调用者标活(第 1 条教训),挂着的时候
这个模块内部的任何死代码都看不见。

`persist_record` / `update_record` 目前是空实现:dApp 历史记录要落到发送流写的同一个
交易存储里(这样一笔 dApp 签名和一笔转账在历史里长得一样),那要先确定键与形状。

## Phase 18 — 宿主:四台机器跑成一列

web 的签名单读**四个视图**(`sign` / `clear` / `guard` / `fee`),所以桌面的宿主也持四台。
形状照 `SendHost` 抄——**发送列已经证明了"跨机器的一段旅程要一个主人、一台机器一个泵"**,
而它踩过四次的相关性规则(第 2 条教训:fee 会话必须只有一个)不值得重新发现一遍。

**三台机器分别被告知,谁也不等谁**:解码是一次网络往返,授权编辑器不是。
等最慢的那个才开单,就是每次取描述符时人盯着一张白单。

**关闭由核心说了算**:`SignSurface::Hidden` 才是"这个请求结束了",不是这个文件
对某次点击的解释。

### 谁回答 dApp

`SendResponse` 在执行器里答 `Screen`,因为**只有这一层知道请求是从哪条通路来的**。
今天只有一条(浏览器列),所以答案走 `webview::respond`;将来有两条时,
核心一直带着的 `transport_id` 就是用来选的——那个 id 存在的理由正是
**一个响应绝不能发给另一个站点**。

`webview::respond` 把核心的判决翻成 provider 在等的信封。**错误码是核心的**
——4001 是拒绝、4900 是卡住但已提交——壳自己挑码就可能把"拒绝"报成"失败",
而 dApp 对这两件事的处理不一样。

### 两个翻译,两条测试

宿主要把请求翻给解码器,这两处翻错都不会崩,只会**悄悄降级**:

- **批量从第一条腿解码**(和手机单一样)。`value` 缺就是缺,不能变成 0。
- **`eth_signTypedData_v4` 的文档是第二个参数**。读第一个就是把地址喂给解码器,
  于是每一个 typed 请求都掉到盲签那一级——没人会当成 bug 看,只会觉得
  "清晰签名在这儿从来没生效过"。顺带处理了有些站点把文档当对象而不是字符串传。

### 差最后一跳

宿主还没有人构造:**浏览器的 ipc handler 要够到 page 才能开一个**,
而那需要在 wry 的回调里拿到 gpui 的句柄。这一跳我没在长会话的尾巴上赶——
它是"哪个页面、哪个窗口、什么时候通知"的三岔口,赶出来的版本会是下一次
"看着能跑但少通知一次"的来源。

模块挂 `#[allow(dead_code, reason = "opened by the browser's request hop, phase 19")]`,
和 phase 17 同一个规矩:**接上时必须摘**,因为 allow 会连带把被调用者标活。

## Phase 19 — 最后一跳,和一个"什么都不报"的失败

浏览器的 ipc handler 现在够得到 page,dApp 的请求真的走进机器了。全程实测:

```
browser rpc: eth_chainId from http://127.0.0.1:8137/
browser answer: {"dir":"res","id":"…:1","error":{"code":4900,…}}
browser rpc: eth_accounts …
browser answer: {"dir":"res","id":"…:2",…}
```

页面 → provider → bridge → ipc → sink → page → 判决 → deliver → provider → promise 结算,
id 逐个对上。

### 这一跳为什么要延后一拍

wry 从平台回调里调 ipc handler,而 `AsyncApp::update` 会 **borrow 那个 app cell**;
在另一次 borrow 里面同步这么干,在钱包里就是一次 panic。所以 sink 走
`AsyncApp::spawn` 落到前台执行器,活儿在下一个 runloop 轮次里做。

### 谁决定开不开那一列

**核心。** 我第一版写成"来请求就 `panel = Signing`"——那会让页面只是问一句
"现在是哪条链"就给人推一张签名单。改成:开完机器看 `SignSurface`,
`Hidden` 就什么都不显示。dApp 发的大多数东西人根本不该看见。

### 那个什么都不报的失败

接完第一版,**一个请求都没到**。原因:`inpage.js` 第 29 行是
`import { CHANNEL, … } from './lib/protocol.js'` —— 它是 **ES 模块**,
而 initialization script 是 classic。原样注入就是**语法错误**:
文件根本没跑、`window.ethereum` 从来没出现、于是每一个请求都"静静地从未发生"。
**没有任何东西报告这件事**:宿主收不到错误,页面只是没有钱包。

修法是把 `protocol.js` 去掉 `export` 前缀、`inpage.js` 去掉那一行 `import`,
拼进一个 IIFE。**磁盘上两个文件一个字节都没动**,两个模块关键字是在 Rust 里去掉的。
没选"自定义协议 + 动态 `import()`"是因为**严格 CSP 的 dApp 可以拒绝它**,
而"在某些站点能用"的 provider 比哪儿都不能用更糟。

两条测试盯着这里:注入的脚本里**不许再有 import/export**(哪个文件再长出一个就大声失败),
以及**它还得是真的那个 provider**(channel 常量、EIP-6963 公告、两个文件的长度)
——一个悄悄拼出空字符串的实现能完美通过前一条。

### 读方法暂时被拒绝,不是被回答

`sign_request` 只管**签名方法**;`eth_chainId` / `eth_accounts` 是读和权限,
归 `dapp_session` / `dapp_permissions`(C 组,没接)。所以 page 按方法分流:
签名的进机器,其余**答 4900**。

**不是 4001**:人没有拒绝,而一个把"拒绝"读出来的 dApp 会告诉他们"你拒绝了某件
你从没看见的事"。也**没有让壳自己回答**读方法——那是壳替核心做决定。

### allow 摘掉了

phase 17/18 挂的两个 `allow(dead_code)` 都拿掉了,警告数仍是 **42**——
这就是接线是真的的证明:allow 一摘,编译器在这三个执行器和宿主里找不出一个死项
(第 1 条教训说的正是 allow 会连带把被调用者标活)。

### 还欠

签名列现在开得起来,但**画的还是 fixture**:核心视图 → 已画好的 block 渲染器
那个 live 构造器还没写。C 组三台机器(读、权限、历史)没接,读方法因此被拒。

## Phase 20 — 签名单读核心,不再读手写场景

`signing/live.rs` 是 `fixtures.rs` 的**兄弟,不是替代**:两边都产出 `SigningModel`,
面板挑谁喂它。这正是让 33 个画好的场景在真请求到来后**仍然可评审**的东西,
也让"画廊没变"成为 diff 能证明的事。

### 滑块要三台机器一起点头

`SignView.confirm_gate_open` 的文档自己写着:"the shell must AND it with
`GuardView.confirm_allowed` and `FeeView.confirm_fee_ready`"。
少任何一台,都是**一次没人同意过的签名**:
少了 fee 的,是在没人拿到过的价格上开滑块;少了 guard 的,是在没人设过上限的
无限额授权上开滑块。测试逐个把三台按掉,每次都必须关。

### 警告按"最坏先读"排

单子是从上往下读的,所以顺序不是风格:
`to_own_token`(把代币转给它自己的合约 = 不可逆销毁)排第一,
然后 `best_effort`(4byte 恢复的,是"解析通过的猜测",不是谁发布的描述符)、
`partial`(描述符声明的字段比解析出来的多)、`unverified`(小数没人验证 = 数量级没人验证)、
`expired`。**每一条都是核心已经算好的旗标**,壳只是把它说出来。

`detail` 字段**不进摘要**——它们是 Advanced 的。提上来就是把"这笔在干什么"
埋进"它用什么参数干的"底下。

### 没解码出来就不画

`result` 是 `None` 时返回空,而不是一个空的 intent——空 intent 读起来像
"这笔什么也不做"。

### 费用只有一个格式化器

复用 `flows::live::fee_text`(改成 `pub(crate)`)。**两个格式化器就是两个关于
"这笔要花多少"的答案**,而且是在两块给同一个操作定价的屏幕上。
未定价时渲染成它的 `—` 而不是消失:**没有那一行读起来像"免费"**。

### 第四台机器还没跑,而这件事是明说的

宿主持 `fee_view`,但那是一台**pristine 的** `fee_policy`——它答
`confirm_fee_ready: false`,所以这条缝没接上之前**滑块一直是关的**。
这是对的失败(三方 AND 存在的理由就是不让人在没有价格时确认),
而它是**held 的一个视图**、不是每帧新建一个:等 fee 会话落地时,
只有一个地方要接上,也只能有一个(第 2 条教训:fee 会话必须只有一个)。

四条测试:三方 AND(逐个按掉)、Advanced 字段不进摘要、每个旗标都变成一条警告
且最坏的排第一、没解码就不画。

## Phase 21 — fee 会话:滑块能开了

B 组最后一块。宿主现在持第四台机器 `fee_policy`,**一个请求一个会话**。

### 报价从哪来

`sign_request` **没有** `EstimateFee` 操作——签名单自己驱动 `fee_policy`:
请求打开时读一次部署状态(后台),然后 `QuoteRequested`。
只有交易报价:`personal_sign` 不花钱,给它挂一条网络费,是在一个从不碰链的签名上
写一个费用。

**部署状态读不出来就不报价**。猜"已部署"会发出一个没有 initCode 的操作,
猜"未部署"会给一个活账户挂上 initCode——两种猜法算出的费用都是**另一笔操作的费用**。
所以宁可没有报价:滑块保持关闭,这正是 `confirm_fee_ready: false` 的意思。

### 签下去的价 = 屏幕上的价

`approve()` 的报价是从 **`fee_view` 读的**——**渲染确认卡的那同一个视图**——
而不是重新问一次。重新问就是第二个数字,于是**人同意的那个数字和被签下去的那个数字
不是一个**。这就是第 2 条教训在桌面上的样子(web 记了四次因为拆成两个对象而失败的集成)。

两条测试:被签的报价必须逐字段等于被显示的那个;没定价的单子**带 `None` 而不是 0**
——0 是一个费用主张,而提交会把它签下去。(后者理论上到不了,因为滑块是关的。)

### 滑块现在真的会响

`slide_to_confirm` 多了一个 `action`。**只在三台机器都点头时才传**:
一个关着却仍然挂着动作的滑块,是一个核心说了不、却等着一次点击说是的控件。
mock 传 `None`——画着但不答应任何事,画廊因此一个像素没变。

**这套词汇里没有拒绝按钮**:关掉这一列就是拒绝。所以这个控件唯一能做的事就是确认。

## Phase 22 — 让一个真请求走到面板上,三个只有跑起来才看得见的错

不是点真 dApp(那要点 Connect,而合成点击要 TCC 授权),而是让一个本地页面直接发一笔
**真的 `eth_sendTransaction`**:Gnosis 上一笔 ERC-20 转账,`transfer(address,uint256)`
的真 calldata。通路都通了,问题是**解码出来的东西对不对**。

### 第一次:`rejected: 4902`

页面拿到的是 CHAIN_NOT_ADDED。原因:**宿主开出来的机器对这个钱包一无所知**——
我从没告诉过 `sign_request` 有哪些网络、有哪些账户。它默认一条链都没有,
于是**每一笔交易都被拒**。

修法:`begin()` 里先发 `NetworksChanged`(内置链 + 用户加的,和设置页同一份名单
——设置说在、签名说不在,是钱包在跟自己吵架)和 `AccountsChanged`,再发 `RequestArrived`。

**没有测试能抓到这个**,因为要点在于"机器没被告知什么"。

### 第二次:面板开了,但它说的是 mock 的话

单子画出来了,内容是对的(Send、数量 1、收款方 `0x031d7d…772b`、真费用 0.01 xDAI),
但**抬头写着 "Uniswap / app.uniswap.org"**——请求其实来自 `127.0.0.1:8137`。
我的 live 构造器只换了 blocks / fee / confirm,**dapp 身份和网络徽章还是 fixture 的**。

**签名屏上认错请求方,是它能犯的最严重的错**:那正是人被要求判断的那一件事。
徽章还写着 Ethereum,而费用是 xDAI——两处互相矛盾,谁也没说破。

修法:宿主保留 `origin` 和 `chain_id`,抬头和徽章从请求画。
**名字就用 host 本身**:从域名猜一个好看的名字是猜,而这个位置上的猜正是
仿冒域名冒充真站点的路子;图里那些漂亮名字要靠请求带 dApp 身份,现在还没有。

### 第三次:滑块上写着"确认兑换",而这是一笔转账

confirm 文案也还是 fixture 的。改成读核心的 `ClearConfirm`——
核心自己注明 **`Confirm` 永远不是 "Approve"**,那个动词只属于真正的代币授权
(那是 `approval_guard` 的地界)。intent 以英文规范键传过来,壳只翻它有词的那几个,
其余显示中性动词——**给读中文的人看一个英文键,比显示"确认"更糟**。

### 还差的那一个:代币符号 —— 查下来是**四端共有的核心缺口**

现在仍然显示 `1 0x2a22…`——**数量对(小数解析出来了),符号是原始地址**。
追下去不是接线问题:

`clear_signing::format_token_amount` 的符号来自 `known_token_symbol`,而它查的是
**一张写死 19 条的 `KNOWN_TOKENS` 表**(`services/tokens.ts` 移植过来的),
**而且只按地址查、不带 chain id**——表里第 17–19 条是 Polygon USDC、Polygon USDC.e、
Arbitrum USDC。**Gnosis 一条都没有。**

`clear_signing` 一共四个事件(`ResolveTransaction`/`ResolveTypedData`/
`MessagePresented`/`Cleared`),**没有一个能让壳把代币元数据递进去**;
它的探针只有 ERC-165 和 decimals,**没有 symbol**。所以核心没有任何途径知道
它表外的符号——**这 19 个地址之外的每一个代币,在四个端上都显示成 `0x…`**,
因为四个端跑的是同一份核心。桌面只是第一个把它照出来的。

**这需要一个决定,我没有替你做**:

| 选项 | 代价 |
|---|---|
| A. 给 `clear_signing` 加一条"壳提供代币元数据"的缝(事件或操作) | 改的是四端共用的核心机器,要重生成 ts-rs 两套、重建 wasm;而且 `app/` 下这些文件的演进现在是 web 会话在管 |
| B. 壳侧兜底:字段带着 `token_address`(核心注明是给 logo 用的),当核心退回到地址缩写时,用壳自己认识的符号替换 | 壳在改核心产出的字符串。要严格守住"只在等于核心那个兜底串时才替换",否则就是盲改 |
| C. 只往 `KNOWN_TOKENS` 里加几条 | 治不了本:每条链都有自己的代币,而这张表连 chain id 都不带 |

我倾向 **A**——符号是"人在读自己转什么",它应该和 decimals 一样是核心问得到的事实,
而不是每个壳各自打的补丁。但它动的是别人在管的文件,所以等你点。

## Phase 23 — 代币符号:核心多问一句 `symbol()`

按上一段那三个选项里的 **A** 做:给核心一条问符号的路,而不是每个壳各打各的补丁。
创始人点的。

### 为什么是探针,不是新事件

我原来把 A 写成"加一个事件让壳递元数据"——那要动 `Event`、要每个壳配合。
实际最小的形状是**照着 decimals 再来一个**:核心早就在用 `RpcEthCall` 问
`decimals()`,只是从来没问过 `symbol()`。所以只加 `ClearProbe::Symbol`
和一个选择器常量。

**四个端一行都不用改**:每个壳的 `RpcEthCall` 都是把 `probe` 原样回显的
(web 的 `clear-executor.ts` 是,我这刀写的桌面执行器也是),
所以新探针天然被回答。**先查了再动**,不是改完祈祷。

### 顺序:表 → 链 → 地址

`KNOWN_TOKENS` 那 19 条仍然优先(它是 TS 那边的权威),然后是链自己答的,
最后才是地址缩写。**只有最后一档会让人对着一个合约地址读金额。**

**只有真词才教缓存**:revert、空答案、解不出 UTF-8 的,都留着原来的兜底——
一个乱码符号挂在金额旁边,读起来像一个没听过的真符号,比地址更糟。

### 探针不设门

符号和 decimals 一起发,但 **warm 那一步仍然只等 decimals**。
慢的符号不该把单子卡住;它没赶上,代价只是回到这条探针出现之前的样子。

### ABI string 两种形状

`symbol()` 的返回有两种布局,因为 ERC-20 早于字符串约定:寻常的
`[offset][length][data]`,和 **bytes32**(MKR 那一代返回一个定长字)。
长度对不上载荷时按第二种读——**越界的长度正是 bytes32 答案在偏移读法眼里的样子**。
三条测试钉住:两种布局、多字节符号 `USD₮0`、以及 revert/空/非 UTF-8 都答 `None`。

### 四个测试挂了,是驱动不是产品

核心测试的 `Sut::resolve` 是 **FIFO**(`pending.pop_front()`),
每个地址多一个操作就把位置全错开了——**和 028 记的"FIFO 驱动遇上新计时器操作"
是同一个坑**。改法是让那些测试也回答符号探针(按发出顺序),
或者把 `drop_oldest` 补成两次。产品行为没错:问符号正是这一刀的目的。

vela-core **1,285 passed**、clippy `-D warnings` 干净;wasm 重建
`6b0ea32cc2ed`(3,727,829 字节),`gen-core-types` 两套镜像都跑了
(`ClearProbe.ts` 现在是四个变体),`verify-web` 46,513 例全绿。

## Phase 24 — 池子不再是一条单人队,和"1 USDC 显示成 0"

phase 23 提交时只跑了测试,没把面板真的打开看一眼。**打开一看,数量是 `0`**——
一笔 1 USDC.e 的转账,签名屏上写着 0,旁边挂着"金额无法链上核验"。
比 phase 22 那次(显示 `1 0x2a22…`)更糟:那次错的是符号,这次错的是**钱数**。

### 追下去:不是解码,是排队

在执行器里给每次 `eth_call` 打上耗时,第一次跑:

```
[probe] Symbol   … done in 4.116s
[probe] Decimals … done in 4.752s
```

而 `decimals` 的 warm 窗口是 **4 秒**。超时那一档的兜底是"18 位小数 + unverified",
于是 `1000000 / 1e18` 被格式化成 **`0`**(`format_token_value` 保留 4 位小数)。
curl 直连同一个 Gnosis 端点是 0.8–1.9 秒。慢的不是链,是我们自己。

在池子里打上每个 POST 的开始/结束,真相很干净——**每一次 START 都正好是上一次 DONE**:

```
[pool]  4.617 post eth_getBalance https://mainnet.optimism.io    START
[pool] 12.619 post eth_getBalance https://mainnet.optimism.io    DONE 8.001s
[pool] 13.209 post eth_getBalance https://bsc-dataseed.binance.org START
[pool] 21.210 post eth_getBalance https://bsc-dataseed.binance.org DONE 8.001s
```

`executor/pool.rs` 的 `drain()` 在**池子线程上原地做**每一个操作,包括那次 HTTP POST
和 `StartBackoff` 的 `thread::sleep`。所以:**进程里任何一次 RPC 都排在其他所有 RPC 后面**。
Optimism 一个 8 秒超时,期间整个钱包一次链都读不了——签名单那条 4 秒预算的
`decimals()` 探针,输给的是一条它根本不关心的链上的余额读。

这不是"慢",是**架构上的单点串行**:池子既是路由权威,又是唯一的执行者。

### 改法:路由留在一根线上,等待搬出去

池子线程仍然是唯一的路由权威(封禁表、EMA、竞速赢家——那正是"一个会话"的意义),
但**会阻塞的三件事交给工作线程**:`JsonRpcPost`、`ProbeChainId`、`StartBackoff`。
一条 channel 同时收调用者的 `Ask` 和工作线程的 `Finished`(mpsc 没有 select,
所以是一条 channel 两种消息);回来的 body 由**池子线程**归档进 `inflight`,
工作线程不碰任何共享可变状态。

- **上限 32 个工作线程**,到顶就退回原地执行——降级成从前的样子,而不是一千个线程。
- 迟到的答案是安全的:`CoreHost::resolve` 的规则 2 就是"没人再问的问题,答案丢掉"。
- **核心的端点竞速这才第一次真的发生**。`rpc_pool` 一直能一次给出多个 POST,
  而壳把它们一个接一个地做——于是"最快的端点"其实是"第一个端点,加上排在它前面的人"。

同一次启动,改完之后:

```
[pool] 3.466–3.472 十二条链的 eth_getBalance 一起 START
[probe] Decimals … done in 622ms   ← 从 2.0–4.7 秒
```

面板上数量变成 **`1`**,"无法核验"的警告消失。整个开机 RPC 从 25 秒压到 5 秒。

### 顺手一个:池子在倒着执行核心的清单

`drain()` 用的是 `Vec::pop`——**从尾巴取**。核心给出多个端点时是**按分数排好序的**,
壳却先去够最差的那一个。结果不算错(接受哪个答案是核心的判决,它点名 URL),
但"先打哪个端点"是路由决定,而这个文件自己的头一句就是它不做路由决定。
改成 `VecDeque` 按序取。

### 测试

一条**不联网**的测试:两条本地链、两个 loopback 上的假 RPC,一个故意慢 1.5 秒。
慢的先发,快的后发,断言快的**在慢的还没回来之前**就答完了。
把 `offload` 临时改成永远拒绝,它会失败(`1.527s`,正好是慢的那条的延迟)——
**证明这条测试真的在测这件事**,而不是两种实现都能通过。

真网四组(`executor::pool` / `relay` / `chain` / `user_op`)与 `wallet::money`
全部照跑:金标 Safe 0.74797 xDAI 读到、封禁仍然跨机器共享、确认页仍然拿到真报价。

desktop **291 / 287**(+1),fmt clean,画廊 36 态全渲染,Windows 类型检查通过。

### 还欠(这一刀照出来的,都要人点)

1. **代币符号仍然显示成地址,phase 23 那条探针实际上等于没生效。**
   日志里符号是**答出来了**的(`USD C.e` 的 UTF-8 就在 payload 里,622ms/642ms
   两条几乎同时回来),但 phase 23 明写了"探针不设门":warm 只等 decimals,
   decimals 一到就**立刻格式化**——晚 20 毫秒的符号只进了缓存,这张单子再也不看它。
   两条探针是**同一次往返一起发出去的**,谁先回来是掷骰子,所以第一次看到某个代币时
   **几乎总是**显示地址。三条路:
   (A) 让符号和 decimals 一起当门(同一次往返,4 秒上限照旧兜底);
   (B) 符号迟到时**重排字段**(但签名屏上的金额行在人读的过程中变化,本身是一类风险);
   (C) 维持现状(只有第二次遇到同一个代币才有符号)。
   我倾向 **A**,但它要推翻 phase 23 自己写下的"探针不设门",所以等你点。
2. **决定不了小数时,`0` 是一句谎话。** 核心在 warm 超时后按 18 位格式化并标
   `unverified`,于是 1 USDC 变成 `0`。四个端共用这条规则(web 移植过来的)。
   一个签名屏宁可说"数量未知",也不该说一个**确凿的错数字**。同样是核心的事,同样等你点。
3. **每次 POST 都新建一个 ureq Agent**(`proxy::agent`),所以每次调用都付一次 TLS 握手,
   而那个函数的注释自己写着"连接复用很重要"。并发之后这件事更值钱了。没动,记账。

## Phase 25 — 创始人点的两件事:符号当门,和"不知道就说不知道"

phase 24 末尾报的两个决定,创始人都点了:**A(符号也当门)** 和 **说「数量未知」**。

### 符号当门 —— 推翻 phase 23 自己写的"探针不设门"

phase 23 写着"慢的符号不该把单子卡住",听起来对,跑起来是空的:两条探针
**是同一次往返一起发出去的**,实测 622ms / 642ms 落地,相差 20 毫秒。
`decimals` 一到就格式化,于是符号只进了缓存——**第一次见到一个代币,几乎必然显示地址**。
"不设门"实际不是"偶尔晚",是"基本没有"。

现在 `Step::AwaitWarm` 拿两个集合,两边都空了才收工。**代价有上限**:一个答得出小数、
却在符号上挂住的代币,最多等到那条本来就有的 4 秒 timer,然后照旧用已知的东西格式化。
下限没动,动的是常见情况。

### `0` 是一句谎话 —— 核心不再算它算不出来的数

`1000000` 用 18 位兜底格式化,`format_token_value` 保留 4 位小数,结果就是 **`0`**:
**一笔 1 USDC 的转账,在签名屏上写着 0。** 小数不可验证 = 数量级不可验证,
核心因此**什么都不说**:`UNKNOWN_AMOUNT`(一个 em dash,就是这个钱包在费用卡上
"没有价格"时用的那个符号),`unverified` 旗标照旧。

壳再把它翻成人话。桌面读 `unverified` + 值以那个破折号开头,渲染 **`amountUnknown`**
——**这一刀唯一新增的语料键**(15 个语言各一句;`gen-i18n.mjs` 里那份"为什么加这个键"
的清单也照规矩续了一条)。**破折号本身是给还没接这个字段的另外三个端的**:
它们照现在的样子渲染 `value`,得到的是"没有数字",而不是一个错数字。

### 测试

核心两条:符号**后到**仍然上单(`500 USDC.e`);小数查不出来时那一行是破折号而**不是** `0.5`
(原来的测试断言的正是 `0.5`,它被改了——**这条 diff 就是这次修的东西本身**)。
桌面一条:未核验的那一行渲染成语料里的词、色调是 caution,而**已核验的那一行原样不动**
(不许改一个核心真算出来的数字)。

vela-core **1,286**(feature `i18n-all,crux,dev-fixtures`),clippy `-D warnings` 干净;
desktop **292 / 288**;wasm 重建 `5d01841e0bb3`(3,728,061 字节),`verify-web` 46,513 例全绿,
`gen-core-types` 两套镜像跑了(**类型没变**,只有 wasm 指纹动),画廊 36 态、Windows 照旧。

### 真机眼见为实

同一个本地页面、同一笔真 `transfer`:**`Amount  1 USDC.e`**。
(phase 22 是 `1 0x2a22…`,phase 24 打开时是 `0 0x2a22…` 挂着"无法核验"。)

### 这一跑又照出一个:**解析途中,面板画的是 mock 的内容**

截图时抓到了一个中间态:抬头是真的(`127.0.0.1:8137` · Gnosis),
**下面的正文却是画稿里那笔 "Swap 0.5 ETH → 1,278.11 USDC · Uniswap V3 Router"**。

原因在 `page.rs::signing_body`:`if !blocks.is_empty()` 才用核心的块。
核心还没解析完(`resolved=false`)、或者解析完但**没有结果**(盲签那一档),
`blocks` 就是空的,于是**画稿的正文留在屏幕上**——挂在一个真请求的抬头下面。

phase 22 修过反过来的那一半(真请求配 mock 的抬头);这是同一个错误的另一半,
而且更糟:**抬头是真的,会让人以为正文也是真的**。归 phase 26。

## Phase 26 — 面板的正文归核心,而核心早就说了该画哪一屏

phase 25 那一跑抓到的:**真请求的抬头下面画着画稿里的那笔 swap**。

### 洞在哪

```rust
let blocks = signing_live::blocks(&host.clear_view, &self.signing);
if !blocks.is_empty() { model.blocks = blocks; }   // ← 空就留着 mock
```

而 live 构造器的第一行是 `let Some(result) = clear.result else { return Vec::new() }`。
**核心六个面它只认一个**,另外五个一律空:解析途中、personal_sign、eth_sign、
盲签 typed data、盲签交易——**每一种都会把画稿的正文留在屏幕上**。

phase 22 修的是反过来的一半(真请求配 mock 的抬头)。这一半更糟:
**抬头是真的、是可核对的,它会让人以为下面那段也是真的。**

### 修法:读核心自己的 `ClearSurface`

核心早就有这个分派,而且文档里写着不变量⑦:**"盲签的视图绝不能在清晰的之前闪一下"**——
`Loading` 就是为这件事存在的。壳之前用 `result.is_some()` 自己造了一个二分法,
把六个面压成"解出来/没解出来",于是四个面没东西可画。

现在 `blocks()` 按 `clear.surface` 分派:

| 面 | 画什么 |
|---|---|
| `None` | 什么都不画(核心说它没有请求要展示,壳再画就是壳在编请求) |
| `Loading` | 一行 `loading` 语料。**空正文读起来像"这笔什么也不做"** |
| `ClearSign` | 原来那条路 |
| `MessageSign` / `EthSign` | 核心分好的 danger class:`eth_sign` 是硬警告面(先说这是一个不可读的摘要,再给摘要,最后红警告),普通消息给 `decoded_text`;SIWE 三行 + **只有 `Ok` 才敢说"匹配"**(`Unknown` 什么都不说——权威解不出来不是钓鱼的证据,更不是安全的证据) |
| `BlindTypedData` | primary type + domain + 载荷前五个字段 |
| `BlindTransaction` | 两个仍然为真的事实:**没人读得懂的字节数**、**收款合约是谁** |

**SIWE 那一行显示的是 `domain_host`——核心比对时用的那个串**,不是更好看的那个:
核心自己的字段注释写着,显示一个比对之外的串正是仿冒域名混过去的路子。

### 盲签那一屏故意缺一样东西

**金额没画。** 把 wei 缩放成人读的数,核心在别的每一档都做了(`format_wei_amount`),
壳自己算一个就是"这笔多少钱"的第二个权威。所以这一档暂时不显示原生金额——
**记成缺口**,而不是悄悄补一个自己算的数。(要补就该核心出一个 blind-tx 视图。)

### 测试

- **每一个核心能展示的面都得画出东西**:panel 已经没有"空就退回 mock"的兜底了,
  所以从今往后空正文就是白屏,而白屏读起来像"这笔什么也不做"。哪个面哪天不画了,
  先在这条测试上响。
- 盲签只说真话:字节数在警告里、收款方在 Party 里、**没有 `Block::Amount`**。
- `eth_sign` 必须是硬警告面;`Unknown` 的 SIWE 绑定既不给"已验证",也不给红警告。
- 老测试 `nothing_decoded_draws_nothing` 改名了:**"没解出来"现在不等于"不画"**,
  不画的是 `ClearSurface::None`。

desktop **295 / 291**,fmt clean,画廊 36 态全渲染,Windows 通过。

### 真机眼见为实

同一个本地页面,连拍 40 帧:**第一帧就是 "Loading…"**——真抬头、诚实的正文、滑块关着;
后面的帧是 `Amount 1 USDC.e`。原来这一帧是画稿里的
"Swap 0.5 ETH → 1,278.11 USDC · Uniswap V3 Router"。

## Phase 27 — C 组开工:一个真 dApp 现在能连上了

spec 里写着 C 组不做,理由是「桌面没有 web 引擎」——那个理由在 phase 13/14 就没了。
而没有 C 组,浏览器那一列其实是废的:**除了签名方法,所有请求一律答 4900**,
而任何一个真 dApp 在要签名之前都会先问 `eth_chainId`、`eth_requestAccounts`。
本地那个探针页之所以能走通,只因为它**不问自答**直接发 `eth_sendTransaction`。

### 三件事,各归各的

| 谁 | 管什么 |
|---|---|
| `dapp_permissions`(核心) | 是不是这一帧、是不是安全源、这个源有没有授权、要不要弹同意、`eth_accounts` 答什么 |
| `browser_host`(新,壳) | 执行核心的操作:读写授权、答页面、发 EIP-1193 事件、写"已连接"活动行、把签名类交给签名列 |
| `executor::dapp_rpc`(新,壳) | 核心转出来的那一堆里,**谁答**:签名 / 钱包状态 / 切链 / 只读代理 |

**为什么 `browser_host` 是宿主而不是 resident**:`ForwardToSigning` 要开签名列、
`SettleForwarded` 要结掉签名列还欠的答复,两件都要 `Context`——和 `SigningHost` 同一个形状。

### 只读代理是白名单,而且白名单在两个地方——所以要测

`dapp_rpc` 的表是从 `extension/lib/protocol.js` 的 `classifyMethod` / `READ_PROXY_METHODS`
移植的。**那个文件本来就 `include_str!` 进了这个二进制**(页面里的 provider 就是它拼出来的),
所以有一条测试**把 JS 解析出来逐个比对**:两边不一致就红。

黑名单式路由是**失败朝开**的:`eth_signTransaction` 不被任何"是不是签名方法"抓住,
一个 catch-all 的 read 桶会把它转给公共节点——那时钱包就是一个挂着钱包名字的开放 RPC 中继。

### 跑起来抓到的两个,都是"字符串是谁"的错

**① 授权是按整条 URL 存的。** 第一次连成功后去看盘:
`vela.perm.http://127.0.0.1:8137/?v=3`——**带 path 带 query**。
`webview.rs` 把 `view.url()` 原样当 origin 递了进去。授权覆盖的是**一个站点**,
按 URL 存意味着同一个站点翻一页就再问一次,连接徽章也永远对不上号。
修法不是我自己写个 trim:**核心已经导出了 `dapp_permissions::origin_of`**(它每次导航都用它),
壳改成调它——不然写进去的 key 和查出来的 key 会是两种拼法,还少了默认端口归一化那条规则。

**② 机器出生时,文档已经开着了。** 这台机器是**第一个请求**才出生的,而那时页面早就 load 完了。
核心的 `current_origin` / `connected_addr` 只有 `NavigationStarted` 会设——
于是连接面板画出来是一个没有名字的「?」、没有"已连接"、Disconnect 无从谈起。
修法:宿主一建好就把**当前文档的 URL** 补告诉它一次。

### 还有第三个,是 phase 26 那个错的第三次露头

连接面板本身画的还是画稿:**"app.uniswap.org · Connected" + "Ethereum"**,
而真连着的是 `127.0.0.1:8137` 上的 Gnosis。同一类错误第三次出现
(phase 22 抬头、phase 26 正文、这次是连接面板),现在这三处都读核心了。
"Connected"只在核心说有授权地址时才写——它是一句断言。

### 实机全程

同一个本地页面,连拍与点击:

```
ethereum: present
eth_chainId -> "0x64"          ← 壳答的钱包状态,EIP-1193 最简十六进制
eth_accounts -> []             ← 核心答的:没授权就是空,而且不弹窗(不变量⑧)
[同意面板弹出 → 点 Connect]
evt accountsChanged ["0x88cca0…266894"]
eth_requestAccounts -> ["0x88cCA0EeDbF2C4426110bbFc998F048689266894"]
eth_accounts -> ["0x88cCA0…"]
```

盘上:`vela.perm.http://127.0.0.1:8137` = 金标地址 + chain 100;
活动里一行 `type: "connect"`。**再开一次同一个站点:不再问**,accountsChanged 直接来,
第三列根本不出现(该问的问完了就没有可问的)。

desktop **305 / 301**,fmt clean,画廊 36 态,Windows 通过。

### 坑:合成点击只在窗口是 key 的时候算数

gpui 的窗口不是 key 时,鼠标**移动**照收(按钮会变色),**点击却不派发**——
第一次点只是激活窗口。所以自动化要么先激活再点,要么像我最后那样**新开一个窗口就点**。
(`osascript` 设 frontmost 不够;窗口变亮才算。)另外这台机器要在
系统设置 → 隐私与安全性 → 辅助功能 里勾上 Terminal,否则 CGEvent 一个都不落地。

### 还欠(C 组只做了一台机器)

- `dapp_session`(WalletPair / 远程注入)和 `browser_history` 没接:收藏、历史、
  多标签仍是画稿;`wallet_switchEthereumChain` 改的是**这一列的链**,不是全局设置。
- `wallet_addEthereumChain` / `wallet_watchAsset` 按扩展的做法**答应但什么都不改**——
  网络和代币只在设置里由人添加。
- 连接面板的 Disconnect 已接 `RevokeRequested`,但**没实机点过**。

## Phase 28 — 浏览器记得去过哪里

C 组第二台:`browser_history`(471 行,三个操作、四个事件)。
探索页那条 **Recent dApps** 从今天起是真的。

### 标题和图标是页面自己说的,所以有两条规矩

wry 没有 title/favicon 回调(手机壳是从原生 WebView 拿的),所以**桥自己报**:
`DOMContentLoaded` 和 `load` 各报一次——标题通常在前一刻就有,图标链接常常还没有,
而核心本来就写着"没带标题的那次上报不许覆盖已经抓到的标题"。

1. **空标题当作"没有",不是当作空标题**:壳把 `""` 翻成 `None` 再递进去,
   否则第二次上报会把第一次抓到的标题擦掉——这正是核心那条规则要防的事。
2. **favicon 存但绝不去取**:桌面画的是首字母。去取一个页面递过来的 URL,
   等于每次有人打开历史,那个页面就白得一次信标。

行上**标题在上、host 在下,而且 host 永远在**:页面可以把自己叫成任何名字,
只显示标题的一行会让一个站点冒充另一个。(测试直接钉这条:
`evil.example` 把自己 title 成 `app.uniswap.org`,行上仍然写着 evil.example。)

### 只有 Recent 是活的,其余仍是画稿——而且是明说的

`vela-core` 里没有机器管收藏和自定义分组,所以 Favorites 和「交易 / 预测市场」还是画的。
**画稿那条 Recent 在登录后被丢掉**,不是画在活的旁边:两个 "Recent" 标题、
其中一个是编的,正是 phase 22 / 26 / 27 各修过一次的那个漏法。

清空按钮只挂在活的那条上;画稿分组的按钮保持无效——**长得一样却什么都不删,
是两种谎话里更糟的那种**。

`ClearAll` 走核心的语义:**删键,不是写 `[]`**(移植时就是这样)。
差别对下一个读的人是可见的:没有这个键 = 这个浏览器没人用过,空数组 = 有人清过。

### 实机

开着本地页面跑一次,盘上出现
`{"origin":"http://127.0.0.1:8137","url":"http://127.0.0.1:8137/","host":"127.0.0.1:8137","title":"swap probe",...}`;
再启动一次(不进浏览),探索页的 **Recent dApps** 就是这一条,
首字母头像的颜色是从 host 派生的(同一个站点每次都同色)。点 Clear:整段消失,
`vela.browserHistory` 这个键从盘上没了。

desktop **312 / 308**,fmt clean,画廊 36 态,Windows 通过。

### 还欠

收藏、自定义分组、多标签仍无核心;`DeleteOrigin`(删单条)核心有、桌面没有入口
(右键菜单是画稿);`dapp_session`(WalletPair / 远程注入,1,959 行)没接。

## Phase 29 — 第二条 grep,跑在这一轮新接的机器上

本文件末尾那条规矩:**每接完一台机器就跑第二条 grep**——视图给了什么判断字段,
壳读了什么。这一轮接了四台(pool 不算机器),所以跑一遍。差集如下:

| 机器 | 视图字段 | 壳读了 | 差集 |
|---|---|---|---|
| `dapp_permissions` | 4 | 3 | `popup`(扩展弹窗专用,桌面永不设) |
| `browser_history` | 1 | 1 | — |
| `clear_signing` | 8 | 4 | phase 26 已补齐(surface 分派) |
| `approval_guard` | 10 | **1** | 九个,见下 |
| `sign_request` | 13 | **2** | 十一个,三个是钱的事 |

### 最重的三个,都在 `sign_request`

**① `tracker_handoff` 没人读 → dApp 发出去的交易,发完就被忘了。**
核心自己的注释写着"壳一看到它就喂给 `tx_tracker::Event::Submitted`(幂等,按哈希合并)"。
没人喂 ⇒ 没有待确认、没有确认、下次开机也不会有人去追。
**发送列 phase 4 就做了这件事**,dApp 这条从来没做过。

**② `persist_record` / `update_record` 是两个空壳。**
`executor/sign_request.rs` 里这两个函数的函数体是 `let _ = record;`——
**这个钱包发出去的每一笔 dApp 交易,盘上什么都没留**:活动列表里没有,
下次启动也没有待settle的行。现在按 `buildSigningRecord`(`dapp-history.ts:162-228`)
逐字段写,并且**同一个 id 只有一行**:关闭是原地打补丁,不是追加第二行。

签名(personal_sign / typed data)那两种行**不带金额也不带符号**——
一行声称有 value 的签名记录,会在活动里读成一笔钱。

载荷**存但要剪**:长度是页面定的,存多少是钱包定的。8 KB,按字符边界剪,
带一个 `requestTruncated`。(一个被拦腰砍断的多字节字符既不是 JSON 也不是文本。)

**③ `error` / `is_signing` / `is_submitting` / `pending_op_hash` / `funding` 都没人读。**
最难看的是:`enforce_no_unlimited` 在提交口**失败朝关**——所以无限额授权确实签不出去,
但桌面**一个字都不说**,人看到的是一张忽然不动了的单子。
现在:无限额说"为了安全,无限额授权已停用"(`signingApprove.unlimitedDisabled`),
链不支持说"找不到那个网络",其余共用发送流那句"没发出去,你的钱是安全的"
(`send.txErrorGeneric`)——**中继自己的话一个字不上屏**(SC-305)。
**新增语料键 0 个。**

已提交(有哈希)压过"签名中":签完还挂着"Signing…",读起来像第二次签名。

### 判定为"不读是对的"

- `notice`(Expired / AlreadySettled):核心设它时**根本没开单**(surface 仍是 Hidden),
  桌面没有可画的地方;它是扩展弹窗那条路的。
- `popup`:同上,`dapp_permissions` 那台机器给扩展用的。
- `request` / `global_chain_id`:抬头和链徽章画的是宿主保留的那两个请求事实,同一个来源。

### 还欠(明说,不是忘了)

- **`approval_guard` 九个字段没接**:授权编辑器(改额度的那几个 chip)、
  `rewritten_params_json`(改完额度之后**真正该被签的那份参数**)、
  `increase_total`、`decimals_unverified`、`expired`、`batch`。
  今天的后果是:**无限额授权在桌面上无法封顶,只能被拒**——安全,但残废。
  这是下一刀,而且是创始人"绝不无限额"那条军令的正主。
- `funding` 只说了一句话,**没有充值流程**(发送列有一整套;dApp 这条没接)。
- `swipe_action` / `reconcile_pending` 没读:前者是滑块该做什么(桌面只有一种),
  后者是账户切换还没 ack——都记着。

desktop **317 / 313**,fmt clean,画廊 36 态,Windows 通过。

## Phase 30 — 额度编辑器:无限额授权终于可以封顶,而不只是被拒

phase 29 的普查结果:`approval_guard` 十个字段,壳只读了一个。后果是
**桌面上的无限额授权只能被拒,不能封顶**——`enforce_no_unlimited` 在提交口失败朝关,
安全,但人没有任何办法把它改成一个有限的数。这一刀补的是那条路。

### 三处接线

1. **编辑器画出来**:`GuardView.surface == ApprovalEditor` 时,从核心画一张额度卡——
   额度值(核心自己的 `format_token_amount`)、chip 状态、注解、`increase_total`
   那一行("增加 100"绝不能读成"上限 100")、小数未核验、已过期。
2. **chip 能点**:`block_with_actions` 给每个 chip 挂一个 `PresetSelected { mode }`;
   mock 传空,**画廊一个像素没动**(和 phase 21 滑块同一个规矩)。
   **禁用的 chip 永远不挂动作**,哪怕调用方传了:"Requested" 灰着,
   是这个钱包在拒绝那个数额,一次能点的拒绝就不是拒绝。
3. **签的是改过的那份参数**:`approve()` 现在传 `guard_view.rewritten_params_json`
   ——**不变量⑨**。原来这里是 `None`:人选了上限、屏幕上显示了上限,
   而真正被签的还是站点最初那份无限额的请求。

### 桌面上不给的那个 chip

**"Grant all anyway" 不画。** 核心有这个事件(某些代币确实只能布尔授权),
桌面不给它入口——创始人的军令,而且画稿里本来就没有这个 chip。

**"Custom"(自定义金额)也没画**:任何一张桌面画稿里都没有那个输入框,
而**一个点了不打开任何东西的 chip,比没有这个 chip 更糟**。记为缺图。

### 核心动了一行(加了个 re-export,没加规则)

`format_token_amount` 收 `U256`,而桌面**故意不依赖 alloy**
(`executor/abi.rs` 自己写着:ABI 的活在核心,不在这里)。所以核心把它需要的那个类型
`pub use ... as GuardAmount` 挂在那个函数旁边——**一个 re-export,比每个壳里再写一个
格式化器便宜**,而第二个格式化器就是"我到底在授权多少"的第二个答案。

### 实机全程

本地页面发一笔真的 `approve(spender, 2^256-1)`(Gnosis USDC.e):

- 单子:**Approve**(红)· Amount **Unlimited** · Spender `0x031d7d…84772b`
- 额度卡:**Unlimited**(红)· Requested **灰掉** · Revoke 可点 ·
  注解"为了安全,无限额授权已停用。请设一个有限的数额。"· **滑块关着**
- 点 **Revoke** → chip 亮起 · **Spending cap 变成 `0 USDC.e`**(符号是 phase 23/25
  那条 `symbol()` 探针查到的)· 费用 **0.01 xDAI** · **滑块武装**

没有滑下去:那会真花钱、真发一笔授权,和 SC-303 一样是创始人的决定。

### 测试

无限额那张单子:Requested 必须是 Disabled、注解必须说明为什么、
**`modes` 里不许出现 `Grant`**;选了 Balance 之后值是 `1,240 USDC`(核心的格式化器,
六位小数);permit 和 `Surface::None` 一律不画编辑器(**链下 permit 封不了顶**,
给它一个封顶控件是钱包假装自己能管一件它管不了的事)。
还有一条钉不变量⑨:`approve_opts` 在 guard 改写过参数时必须把那份参数带上。

desktop **321 / 317**,vela-core **1,286**,clippy `-D warnings` 干净,
wasm 重建 `b51aefd99292`,`verify-web` 46,513 例全绿,画廊 36 态,Windows 通过。

### 还欠

自定义金额输入(缺图)、批量授权的逐腿编辑器(`GuardView.batch`,同样缺图)、
`funding` 的充值流程(phase 29 只说了一句话)。

## Phase 31 — 加不起油的那一屏,关单子的那一下,和一个代理把本机挡在外面的真缺陷

phase 29 记下的三件里的后两件,加上一个跑测试时自己撞上来的。

### 充值这一屏:换,不是叠

核心给这个 surface 写的注释是:"the in-sheet funding swap(BUG-1: never a
stacked second modal)"——就是手机上那个叠出来、结果整块看不见的老 bug
(记忆里那次修了一周)。桌面的做法是**把这一列的正文换掉**:
请求的抬头、费用卡都收起来,只剩一件事要人做。

**要多少,只问差额**:推荐额减掉已经有的。已经放进去一半的人,不该被再要一次全款
(饱和减:余额在两帧之间超过推荐值不是一个负数)。地址按等宽画。
**中继拒绝赞助时说的话不上屏**(SC-305);人能动手的是地址和金额。

滑块在这一屏**自己武装**:它不是签名,是"我打过去了,再看一眼"。
三机 AND 管的是签名,套到这里就等于把充值唯一的出口锁死。

### 关掉单子 = 一个答复,而且由核心决定是哪一个

关闭按钮以前只是 `panel = PanelId::None`——**dApp 那个 promise 就那么挂着**,
要等下一次导航或者关浏览器才被 4900 收尾。现在发 `SwipeDismissed`,
**核心按阶段自己分派**:还没提交=拒绝(4001)、已提交/已失败=只是关掉、
在充值屏=取消充值。壳不挑。

### 撞上来的那个:**代理把 127.0.0.1 也代理了**

跑测试时四个 pool 测试忽然全红,而它们的代码一行没动。真因:
`ureq` 按环境变量走代理,而这台机器的 socks5 正好在抽风——
于是**连本机 loopback 的假 RPC 都被送去代理**,连不上,pool 当成端点失败。

这不只是测试的事:**有人用本地节点(`http://127.0.0.1:8545`)配着系统代理,
在这一版之前根本连不上**,而且会被 pool 当成坏端点封掉。
`NO_PROXY` 靠不住——只有 `ureq` 自己从环境挑代理时才读它,而 `proxy.rs` 这一层
故意用改写过的代理替换了那个挑选(socks5→远端解析那条)。

修法:`proxy::agent_for(url, timeout)`——目标是 loopback / `localhost` /
`*.localhost` / `[::1]` 就用一个**明确不带代理**的 agent。测试钉了两侧:
`192.168.1.10` 不算本机(局域网还是听系统设置的),
`localhost.attacker.example` 更不算(只是名字里有那个词)。

现在**带着代理环境变量**跑 pool 测试:5 passed。

desktop **323 / 319**,fmt clean,画廊 36 态,Windows 通过。

### 还欠

`approval_guard` 的自定义金额输入(桌面没图)、批量逐腿编辑器(没图)、
`swipe_action` 仍未用于**标注**(核心自己分派了,所以它只剩"给按钮起名字"这一个用途)。

## Phase 32 — 封顶编辑器,在一笔真的无限额授权上

phase 30 建的东西一直没在真请求上见过。这一刀只做一件事:让它发生,然后看。

### 全程(临时文件被清了,所以是从零开始的一遍)

1. 空状态开 app → **走真的 onboarding 登录**(不是塞盘):
   "I already have a wallet" → "This device" → 固定密钥集答签名 → 金标 Safe
   `0x88cCA0…266894`(SC-302 又验了一次)。
2. 探索页 → 本地页面 → 同意面板 → Connect(授权按 origin 落盘)。
3. 页面发一笔**真的 `approve(spender, 2^256-1)`** —— USDC.e on Gnosis。

### 屏幕上

抬头 `127.0.0.1:8137` · Gnosis,intent **Approve**(红),
**Amount: Unlimited**(红),Spender `0x031d7d…84772b`;
**Spending cap 卡**:值 `Unlimited`(红),三个 chip——
**Requested 灰(这一笔要的额度,这个钱包不签)**、Balance 灰、**Revoke 可点**;
下面一句"Unlimited approvals are disabled for your safety Set a finite amount to continue."
费用 0.01 xDAI 已报价,**滑块是关的**——因为还没有人选过额度。

点 **Revoke**:chip 亮起,**值从 `Unlimited` 变成 `0 USDC.e`**(核心的格式化器 + 探针查到的符号),
**滑块武装**。三机 AND 的最后一票就是这个选择。

**没有滑下去**:那会真花 xDAI 并把一条授权写上链,和 SC-303 一样是创始人的决定。
被签的是不是改写后的参数,由 phase 30 那条单测钉着(`a_chosen_cap_is_what_gets_signed`)。

### Balance 为什么是灰的 —— 查了,不是缺陷

金标 Safe 在 Gnosis 上的 USDC.e 余额**就是 0**(直接问链:`balanceOf` 返回 0)。
余额上限等于 0,而 0 就是 Revoke,核心因此不提供这个 chip。读也通、规则也对。

### 这一屏把那个缺口照得很清楚

今天可选的只有 **Revoke** 和(有余额时的)**Balance**。
一个人想授权"就 100 USDC",**桌面上做不到**——自定义金额输入没有图。
这不是接线欠账,是**缺一张图**;这张截图就是要这张图的理由。

### 顺手记一个排版毛病

那句提示是 `unlimitedDisabled` + `choosePrompt` 用空格拼的,
而语料里前一句**没有句号**,于是屏幕上读作"…for your safety Set a finite amount…"。
**画稿里也是这么拼的**(`fixtures.rs` 同一行),所以我没有单方面改:
要么语料补标点,要么两边一起换拼法——是创始人的字。

desktop **323 / 319**(无新增测试:这一刀是跑,不是写),fmt clean。

## Phase 33 — 创始人裁决:WalletPair 不接;顺手把站点菜单接活

### 裁决(2026-09-08):`dapp_session` 桌面不做

原话:"wallet pair 不用接呀 直接用 dapp browser inject 就行呀"。

于是 **C 组在桌面上就齐了**:`dapp_permissions`(phase 27)+ `browser_history`
(phase 28),`dapp_session`(1,959 行,X25519 + 中继传输)**记为判定不做,不是欠账**。
理由成立:桌面自己有内置浏览器并注入 provider,远程配对是给"钱包在手机、dApp 在另一台
机器的浏览器里"准备的,桌面没有这个形状。

### 那条注释已经过时了,而它挡着三个能接的东西

`menu_actions` 里写着"the site and tile menus belong to a browser this client
does not have"——**这个客户端现在有浏览器了**(phase 13/14/27)。
工具栏那个 ⋯ 菜单画了六项,一项都不能点。

现在六项里**三项接活**:

| 项 | 谁答 |
|---|---|
| Refresh | webview |
| Add to favorites / Open in new tab / 分享 | 没有核心,**保持画着但不响应** |
| Disconnect | `dapp_permissions::RevokeRequested { origin: None }` —— `None` 是"眼前这个源",核心据此还欠页面一个 disconnect 事件;指名的源是静默撤销 |
| Close | 离开浏览器(核心从"这一帧不再画这一列"那里照旧听到 `BrowserClosed`) |

**`None` 让一项画着但不答应任何事**——和滑块、和授权 chip 同一条规矩:
一个长得能点、点了什么都不做的菜单项,比一个明显不可用的更糟。

### 一条按位置对齐的契约,所以钉住

页面是**按下标**给这六项配动作的(0、4、5)。画稿里插一项,下面所有动作就整体挪位——
"添加到收藏"会变成撤销一个站点的授权。**不崩,只是悄悄做错事**,
所以 `the_site_menu_keeps_the_order_the_page_arms` 把顺序钉死了。

desktop **324 / 320**,fmt clean,画廊 36 态,Windows 通过。

### 还欠(更新)

- 自定义额度输入(**缺图**)、批量逐腿编辑器(**缺图**)
- `browser_history::DeleteOrigin`:核心有,桌面**没有入口**——Recent 行没有右键菜单
  (收藏磁贴有,但那是收藏的菜单)。**要一张图**:历史行的上下文菜单。
- 收藏 / 自定义分组 / 多标签:**没有核心**,不是接线能解决的
- 实体认证器签一笔(要人插钥匙)、xlsx 真表点一次(要人点文件对话框)

## Phase 34 — 去 web 版找答案:一条能移植,两条查清了不是我以为的那样

创始人:"web 版基本功能都有了,收藏/分组/多标签 web 都做了,你可以看看"。看了。

### 能移植的那一条:**两句话,两行**

`app-web/.../signing/ui/AllowanceEditor.svelte` 的 `.note` 用 `white-space: pre-line`,
拼接用换行,注释写着理由:

> Two sentences, two lines: joining them with a space produces a run-on in CJK,
> where a space is not a sentence break.

正是 phase 32 我记下的那个毛病(桌面上读作"…for your safety Set a finite amount…")。
桌面**两边一起改成换行**——live 构造器和画稿 `fixtures.rs` 同一行,
不然画廊和真单子会在同一句话上分家。实测 gpui 认这个换行:那句现在是两行。

### 查清楚的第一条:**收藏 / 分组 / 多标签,web 也只是画稿**

`src/routes/[locale]/wallet/+page.server.ts` 自己写着:

> No explore data here (spec 022 founder call): 探索 is the in-app dApp browser,
> and this client IS a browser tab — it cannot host one. The vocabulary still
> ships for the gallery, which is the design source the three native clients
> are reviewed against.

web 上探索**只存在于 gallery 路由**,`ExploreHome` / `ExploreDesktop` 只被
`/gallery/[state]` 引用;`model.ts` 顶上也写着"当真的浏览器引擎和 dApp 注册表到来时,
它们替换掉建这些的 fixture 层"。所以 web 有的是**和桌面同一套图**,不是活的收藏。

**结论没变,措辞要更准**:桌面缺的不是图(图两边都有),是**没有任何一端有规则**——
收藏存在哪、分组是什么、标签页由谁拥有,`vela-core` 里没有机器管。
要做就是**新写一台核心机器 + 一个共享存储键**,那是产品决定,不是接线。

### 查清楚的第二条:**自定义额度输入,web 也没接**

web 的 chip 里**有** `custom`,`SigningHost.svelte` 也把它派发成
`preset_selected { mode: 'custom' }`——但整个 web 仓库里
**没有任何地方派发 `custom_amount_changed`**,`AllowanceEditor.svelte` 里也没有输入框。
也就是说:选了 Custom 之后没有地方输数字,**和桌面一样卡在同一步**。

所以这条不是"桌面欠 web 一块",是**两端都欠核心已经准备好的那个事件一个输入框**。
桌面这边我照 web 的形状把 `custom` chip 也画上(和它一样可点、派发同一个事件)
在**输入框有图之前是没有意义的**,所以没画——一个点了没反应的 chip,
正是这一刀反复在删的东西。

### 顺带确认:两端的"被签的是改写后的参数"是一致的

web `SigningHost.approveOpts()` 里 `params_override_json: guard.rewritten_params_json`,
和桌面 phase 30 的修法逐字一致。两端同一条不变量⑨。

desktop **324 / 320**,fmt clean,画廊 36 态。

## Phase 35–36 — 收藏/分组/标签有机器了,桌面的收藏和分组接上了

创始人点批(2026-09-08):"好的 干吧"。

### 35:核心多了一台机器 `explore_sites`

spec 022 画了收藏格、自定义分组和标签条,四个端各画各的 mock,**没有任何一端有规则**。
这台机器就是那些规则:一份文档 `vela.explore`,**十八条规则,一条一个测试**
(规则是**写**出来的不是移植的,所以每条都得能用一句话被反驳)。挑几条:

- **收藏是"站点"**:同一个站点收两次仍是一块砖;它会跟着人走到更深的那一页
  (人从哪一页收的就开哪一页),但**不重排格子**——那是人自己摆的,不是最近列表。
- **人取的名字盖过页面**:页面的 `<title>` 随时可以变,取过名的砖再也不被页面改名;
  空白不是名字(两个方向都是)。
- **取消收藏会把它从每个分组里带走**;水化时也会丢掉指向不存在站点的成员——
  存下来的文档是数据,不是承诺。
- **删分组保留站点**——联系人那条规矩,一个人在这个钱包里只该遇到一种行为。
  分组是**书架**,不是箱子。
- **同一毫秒建的两个分组是两个分组**(壳的时钟是这台机器唯一的 id 来源,
  撞 id 意味着重命名一个会改另一个)。
- **系统分组只能隐藏**:根本没有删除它的事件——规则写在协议里,不是写在判断里。
- 标签:**开一个就选中它**(开就是这个意思);关掉选中的那个,选中权**右移,
  没有右边就左移**;关掉最后一个,**什么都不选**——那就是浏览器在显示起始页;
  失效的选中读作第一个。
- 两个上限,而且**视图会说满了**,壳因此可以**不画**那个"+",而不是画一个会拒绝的。

vela-core **1,304**(原 1,286),clippy 干净,ts-rs 两套镜像各 **326** 个文件(原 316)。

### 36:桌面接上收藏和分组

- 工具栏那颗**星**收藏当前页:url 从 **webview** 读(不是地址栏的文本——收的必须是
  真正加载着的那份文档),名字用**桥报上来的页面标题**,没有才退回 host。
- 砖块**点开它自己那一页**(收的时候那一页,可能比 origin 更深);
  右键菜单的**移除**接核心的 `FavoriteRemoved`。
- 菜单里的"重命名/移到分组/新标签页打开"**保持画着但不响应**——前两个要输入框和分组选择器
  (没图),第三个要标签条(还没接)。
- **格子满了就不画那个"+"**,核心说满没满。
- 画稿那些分组(交易 / 预测市场)在登录后**整体让位**给人自己的分组,
  和 Recent 一样——mock 是内容不是外壳。

实机:点星 → 盘上 `vela.explore` 出现一条(origin/url/host/name/renamed);
重启进起始页,**收藏格里就是那一块砖**(名字是页面标题 "approval probe"),
Recent 一条,画稿分组全部消失。

desktop **327 / 323**,fmt clean,画廊 36 态,Windows 通过。

### 还欠

标签条仍是画稿(接它要让 webview 跟着选中的标签走,是行为改动不只是画);
重命名/分组选择器缺图;`DeleteOrigin`(删单条历史)缺一张历史行的右键菜单图。

## Phase 37 — 标签条:一个 webview 跟着选中的那一个走

C 组的最后一块画稿。

### 一个视图,N 个标签 —— 明说的代价

桌面只有一个原生 webview,所以**切标签是一次导航**,不是换一个活页面:
页面里的状态(滚动位置、表单)不保留。这是单视图的诚实代价,
也是为什么标签存的是一个 **url** 而不是一个会话。

### 谁来决定

- 点标签 → 核心 `TabSelected`,然后壳导航过去;url 为 `None` 就是**起始页**
  (钱包自己那一屏,不是一张空白文档)。
- **关标签**:壳只发 `TabClosed`,**下一个选谁是机器的事**(右、没有右就左),
  壳读它的答案再导航——壳自己挑下一个标签,等于对"人刚才走到哪"发表第二个意见。
- 关闭那个叉是**它自己的控件**:点叉必须是关掉,不能变成"选中"——两者一个套在另一个里面。
- **新标签开的是起始页**:浏览器不替人决定下一步去哪。

### 一处钩子让标签条永远说真话

页面 settle 的那一个回调(它本来就在报标题给历史)现在也告诉核心:
**有选中标签就 `TabNavigated`,没有就 `TabOpened`**。
所以第一次打开一个站点时,第一个标签是**自己出现**的——
一个"页面已经在屏幕上、标签条却空着"的浏览器,是在骗人关于自己在哪。

### 实机

开着本地页面启动:标签条上就是**一个「approval probe」标签**(页面自己的标题+首字母),
画稿那两个(Uniswap / Polymarket)不见了。点 **+**:出现第二个「Start page」标签并选中,
中间那一列换成起始页(人自己的收藏 + Recent)。点回第一个:选中权回去,webview 重新导航。
盘上 `vela.explore.tabs` 两条、`selected_tab` 逐次对上。

desktop **327 / 323**,fmt clean,画廊 36 态(画稿标签条一个像素没动:
`tab_strip` 仍是那一个不带动作的函数),Windows 通过。

### C 组到此为止

`dapp_permissions`(27)、`browser_history`(28)、`explore_sites`(35/36/37)——
**桌面浏览器一列的每一块都由核心说了算**;`dapp_session` 是判定不做。
探索页上还剩两个不响应的东西,都因为缺图:砖块的重命名/移到分组,
和历史行的右键菜单(`DeleteOrigin` 在核心里等着)。

## Phase 38 — 收款那道门:地址在被读之前不交出去

创始人点批(2026-09-08,回答"设置/转账/收款好了吗"时确认要做)。

`PaymentRequestView` 只有两个判断字段,而**桌面一个都没读**:`can_copy` / `can_save`,
两个都等于 `acknowledged`。它们存在的理由是一道门:**一个账户第一次进收款页,
先读一句"哪些网络能收",读过之后地址才交出去**——门的状态按账户存,
`vela.payAck.<address>`。

### 门是替换,不是遮罩

覆盖在二维码上的警告是**可以绕着读**的,那就等于没有。所以门**站在码的位置上**:
读完点"I Understand"之前,地址卡和二维码根本不画。

`gate_loading` 那一档也照做:标记还在读的时候**画封面但不画按钮**——
晚一帧才出现的按钮,是一个会被点两次的按钮,而第二下会落在顶替它的东西上。

### 复制跟的是核心,不是"有没有载荷"

原来的判断是 `qr_payload.is_some()`(有地址就能复制)。改成 `can_copy`:
**地址早就准备好了,不等于已经有人被告知它在哪些网络上是安全的。**

`can_save` 天然被满足:门一竖起来,整块正文都不画,那个"保存图片"按钮也就够不着。
(那个按钮**本身仍然什么都不做**——桌面没有存图这件事,它要的是创始人定过的
分享卡构图,那是一张图不是一根线。记在欠账里。)

### 语料新增 0

`receive.warningTitle` / `warningBody` / `warningCounterfactual` / `warningConfirm`
四条早就在语料里躺着(手机壳一直在用),桌面只解析过 `warningReminder` 一条。

### 实机

清掉 ack 键开收款页:**"Before you receive" + 两句话 + I Understand**,没有码也没有地址。
点下去:地址卡(复制图标可用)和二维码出现,提醒行在下面,
盘上 `vela.payAck.0x88cca0…` = "1",下次开就直接是码。

desktop **328 / 324**,fmt clean,画廊 36 态(画稿的 `gate: None`,一个像素没动),Windows 通过。

## Phase 39 — 缺的那张图,我自己画:上限输入,两端都画了也都接了

创始人:"如果画廊里也没有,就按好的 UI/UX、遵循当前风格和功能设计,
在 web 和 desktop 上画出来,然后集成。"

先核实"确实没有":两端画廊都只画了 `Custom` 那颗 chip,
**没有任何一态画"点下去之后"**——web 的 `AllowanceEditor.svelte` 里也没有输入框,
整个 web 仓库从来没有派发过 `custom_amount_changed`。所以不是桌面欠 web 一块,
是**两端都欠核心那个早就准备好的事件一个输入框**。

### 设计:跟发送页的金额栏是同一个东西

- 字段**画在 chips 下面、note 上面**,阅读顺序因此是"上限是多少 · 怎么改 · 它哪里不对"。
- 用**发送页那同一个 `ui::text_field` 原语**(web 上是同一套 token):
  一个人输上限和输金额,输进去的应该是长得一样的东西。
- 币种当作字段的标签/后缀(`USDC` 贴右),数字用等宽/数值字体。
- **上面那个大数字继续跟着走**——边打边看着上限长出来,这个反馈才是"打一个上限"敢用的原因。
- 值是**核心的 `custom_text`**,不是本地回显:被机器拒掉的那一下,不该在屏幕上像是被接受了。
- 错误(`invalid_amount` / `unlimited_disabled`)画在字段下面,danger 墨色;
  这时上面的大数字**退回它仍然为真的那个值**——`Unlimited`,红的,
  因为"解析不出来的上限"不是上限。

### 两个新画态,两端同号

- **cs34**:打了 `500`,`Custom` 亮着,大数字 `500 USDC`,滑块**武装**。
- **cs35**:打了 `12.3.4`,字段下面 `Enter a valid amount`,大数字回到红色 `Unlimited`,
  两行提示还在,滑块**关着**。

桌面 `ALL_STATES` 33→35、`DESKTOP_STATES` 9→10;web 的 `SigningStateId` 与 `ALL_STATES` 同步,
两个画态都在 `/[locale]/gallery/cs34|cs35` 预渲染出来了(HTML 里逐字核对过)。

### 接线

- 桌面:`Block::Allowance` 多一个 `custom` 数据字段;`block_with_actions` 现在也收一个
  `AddressField`——**有绑定就是真输入框,没有就是画稿那一版**,和滑块、chip 同一条规矩,
  所以 33 个老画态一个像素没动。每一次击键 `CustomAmountChanged` 回核心。
- web:`AllowanceEditor` 收 `custom` + `oncustom`,`SigningHost` 把击键派发成
  `custom_amount_changed`,`live.ts` 从 `editor.mode === 'custom'` 建这个字段。
- **新增语料键 0**:`invalidAmount` 早就在 `componentsUi.signingApprove` 里。

### 闸门

desktop **328 / 324**,fmt clean,画廊全渲染,Windows 通过。
web:`pnpm check` **0 errors**、`pnpm lint` 干净、`pnpm build` 成功。
**`pnpm test:unit` 在这棵工作树里起不来**(vitest 项目初始化阶段
`Could not resolve 'node:module' in rolldown/runtime.js`,在任何测试文件被加载之前就失败),
和这次改动无关——但也就意味着 web 的单测没跑过,记在这里而不是含糊过去。

## Phase 40 — 第二张缺的图:历史行的菜单(`DeleteOrigin` 终于有入口了)

`browser_history::DeleteOrigin`(忘掉**一个**站点,而不是清空整张单子)在核心里从
016 就有,**四个端一个入口都没画**——手机、web、桌面的画廊里都只有"磁贴菜单"和
"站点菜单",历史行上什么都没有。

### 设计:照磁贴菜单的形状,三项

| 项 | 归谁 |
|---|---|
| 在新标签页打开 | `explore_sites::TabOpened`(这一刀刚接的标签条) |
| 添加到收藏 | `explore_sites::FavoriteAdded` |
| ── 分隔线 ── | 破坏性的那项永远在线下面(和磁贴菜单同一规矩) |
| 删除 | `browser_history::DeleteOrigin` |

**三项全都有核心**,所以这个菜单画出来的当天就能接活——不像磁贴菜单里那两项
(重命名/移到分组)还要输入框和分组选择器。

用词全是现成的:`openInNewTab` / `addToFavorites` / `delete`。**新增语料 0**。

### 一条只有做的时候才会想到的规矩

行在屏幕上按 **host** 认,核心里所有规则按 **origin** 写,所以两者相遇只放在一个函数里
(`explore_live::live_origin`)。而且菜单**只在活的那条 Recent 上武装**:
画稿那些行右键什么都不弹——一个"删除"没有东西可删的菜单,比没有菜单糟。

另外,菜单点下去时**重新按 origin 查当前列表**,不是用打开菜单那一刻的副本:
两者之间可能落进一次访问,而人指的是光标下面那一行。

### 两端

- 桌面:`explore_fixtures::recent_menu` + `ContactsMenu::Recent`,右键 Recent 行打开。
- web:`recentMenuItems` + `recentMenuSheet`,新画态 **E8**(手机画廊),
  `ExploreStateId` 与 `MOBILE_STATES` 同步。web 的探索页只存在于画廊里
  (spec 022 创始人裁决),所以那边只画不接——这是设计源,不是漏接。

### 实机

右键那条 Recent 行:菜单如图弹出(两项 + 分隔线 + 红色 Delete)。
点"在新标签页打开":真的开了第三个标签并导航过去(顺带又验了一次标签条)。
点 Delete:`vela.browserHistory` 从 1 行变 **0 行**,收藏没被动。

desktop **328 / 324**,fmt clean,画廊全渲染,Windows 通过;
web `pnpm lint` 干净、`pnpm build` 成功(单测仍起不来,见 phase 39)。

## Phase 41 — 第三张图:收藏砖的重命名和"移到分组",分组从此真的能用

磁贴菜单画了四项,只有"移除"能点。这一刀把另外三项接活,并补上它们需要的两个东西:
一个名字对话框,和一个分组选择器。

### 都不是新发明的形状

- **名字对话框**照抄联系人那一个(同一张卡、同一个字段原语、同样的取消/保存):
  **两个都在问名字的对话框,不该长成两个不同的问题**。创建和重命名共用一个,
  和联系人那边一样——它们问的是同一件事,只是答案交给不同的事件。
- **分组选择器**用**菜单卡**,不是新组件:"这些里面选哪个"就是菜单本身。
  `新建分组` 排在**第一项**,因为第一次用的时候列表必然是空的,
  一个唯一入口不可达的菜单是死路。

### 一条规矩:走"移到分组 → 新建"的人,意思是两件事都要

新分组建好后,**那块砖立刻进去**(`then_add`)。让分组空着、再逼人做第二遍,
是把一次操作拆成两次。分组 id 是**从核心读回来的**,不是猜的——机器保证它对已有的唯一。

### 实机全程

右键收藏砖 → 菜单四项(新标签页打开 / 重命名 / 移到分组… / 移除,红色在分隔线下)→
"移到分组…" → 选择器只有 `New group`(还没有分组)→ 名字对话框(Save 在有字时才亮)→
保存 → **起始页上出现那个分组,砖就在里面**;盘上
`groups: [(名字, ['http://127.0.0.1:8137'])]` 逐字对上。

("重命名"和"在新标签页打开"走的是同两条已验证的路:同一个对话框、同一个
`TabOpened`;截图里没再单独走一遍。)

### web

web 的探索只活在画廊里(spec 022 裁决),而它的画廊**已经有分组管理那一页**(E3),
名字对话框在 web 上也是联系人那一套。所以这一刀两端**不需要新画**:
桌面用的是两个都已存在的形状。

desktop **328 / 324**,fmt clean,画廊全渲染,Windows 通过。

### 自动化留下的一条注记

合成键盘事件(`CGEventKeyboardSetUnicodeString` + keycode 0)在这个 app 上**全打成 'a'**,
所以截图里的分组叫 "aaaaaaa"。字段本身是好的(Save 由有没有字决定,存下来的就是打进去的),
但**下次要验中文/长名字,得让人真敲一下**。

## Phase 42 — 收款分享卡:照 web 那份移植,桌面自己出图

创始人:"收款分享卡参考 web 呀"。web 有一份完整的(spec 028 phase 9 建、phase 10 按
创始人参考图重画),所以这不是缺图,是**缺一次移植**。

### 同一份构图,换一种出图方式

480×700、344 的码卡、中心 identicon(圆形裁切)、两行等宽地址、带链徽的网络药丸、
带弧线上沿的白脚 + 应用标记和字标。**几何常量逐个照抄**;
web 是"合成 SVG → 画进 canvas → PNG",桌面是"合成同一份 SVG → `resvg` 2× 光栅 → PNG"。

三样东西同时在卡上是有理由的(web 自己写的,值得重复):**地址的文字**让人不用扫码就能核对、
**码**让相机能读、**中心的 identicon 是从地址派生的**——有人把卡上的地址 P 掉换成自己的,
那张图的头像就对不上了。

### 两处**故意不同**,都记着

- **不嵌网络字体**:浏览器不渲染没加载的字体所以 web 必须嵌 woff2;resvg 是"给它一个字体库",
  于是用本机的 sans/mono。
- **链徽用字母圆盘,不去取 logo**:在"保存"这个动作里塞一次网络请求不划算;
  web 取不到时退回的也正是这个圆盘。

### 一处**必须改依赖**的地方

桌面的 `resvg` 一直是 `default-features = false`(它只用来光栅化图标和 identicon,
那些是纯形状)。**这样 usvg 会静默丢掉所有 `<text>`**——卡上会只剩码和色块,
没有一个字。所以为这一个界面打开 `text` + `system-fonts`,并在 Cargo.toml 里写清楚为什么:
**这种失败没人会当场发现,直到有人已经把卡发出去了。**

### 门:没读过警告就没有卡

`can_save` 现在真的有东西可管了:门没过 ⇒ `receive_share_card` 返回 `None` ⇒ 按钮不出图。
一张保存下来的图片,是"把地址交出去"最可复制的一种形式。

### 测试与实机

四条测试:卡上**同时**有文字地址和码;码解得回它被给的载荷(**图上写一个地址、码里是另一个,
是这功能能犯的最糟的错,而流水线上没有别的地方会发现**);外来文字被转义
(`</text><script>` 进不去);光栅出来的确实是 PNG 且不是空图。
外加一条 `#[ignore]` 的"写一张出来给人看"。

实图见 `card.png`(本次会话产物):橙底、白卡、码中间是那只 identicon、
`MultiTest` + 两行地址、`XDA Gnosis 支付` 药丸、白脚上的帆标和 `Vela Wallet`。

desktop **332 / 328**(+4 测试),fmt clean,画廊全渲染,Windows 通过。

## Phase 43 — 到账庆祝:最后一个没被读的判断字段,和它根本没有的入口

交接清单上写的是"缺图 + 没入口"。图确实缺,而**"没入口"比字面严重**:
桌面壳从来没有给 `activity_feed` 打过任何一次 tick。

### 这台机器在桌面上从来没有跑过第二趟

核心把节奏明确划给壳:"`FocusTick`/`LiveTick` 的节奏留在壳里——哪个页签可见是核心看不见的
渲染域状态。"手机打 30 秒,web 每 10 秒打一次 `liveTick`,**桌面一次也没打过**。
于是机器只跑过 boot 那一趟,而"第一趟永不庆祝"是核心的不变量③(backlog gate)——
也就是说,**到账庆祝在桌面上原理上不可能发生**,不是没画所以看不见。

同一次缺失还有第二个受害者:tracker 把 pending 改成 confirmed 是写在 store 里的,
而**发现那次改写的重读,就是这同一次重读**。没有 tick,一笔刚发出去的钱在屏幕上会一直
"待确认"到下次启动。30 秒的 `FocusTick` 一并把它兜住了。

### 没人告诉过它余额被遮住了

核心按不变量④在余额隐藏时**整个扣掉** toast("a toast would leak the masked number"),
但它只能靠壳告诉它——`hidden` 住在 `balance_dashboard`,机器之间不通话。
桌面从来没 dispatch 过 `PrivacyChanged`。也就是说,这张图要是照直接上,
**遮住余额的那一屏会被庆祝原样念出被遮的数字**。现在由要泄漏的那一处
(toast 覆盖层自己)在变化时说一次,一次切换一次 dispatch,不是一帧一次。

### 图:手机那条实心绿条不能照抄

手机是绿底白字的通栏。暗色调色板的 success 是 `#3da872`,配白字约 3:1 ——
28 像素的字形过得去,一整句话过不去。所以**颜色收进圆盘**(小面积、是形状不是字),
句子落在这只壳里每个浮层都用的 raised 面上,浅深两套都是满对比。

三条各有出处的决定:

- **不 occlude**。它在窗口中间待 2.8 秒,伸手去点底下的东西必须点得到——
  `menu_card` 需要的正是相反的东西。
- **浮在窗口上,不是浮在钱包列上**。钱到了不是"你现在这一屏"的消息,
  在通讯录里和在首页一样该被告知。它自己让开标题条和画廊 chip 条。
- **进场是手机那一份的数字**:320ms、透明度起、下滑 12 px。之后的事归核心:
  toast 什么时候走由 `FeedView::toast` 说,那是机器自己的 2.8 秒。

### 光晕:同一场庆祝的另一半

`new_item_id` 也没人读过,而核心**从不用计时器清它**——toast 走了,行还亮着。
所以 D1b 画的是两样东西:药丸,和它说的那一行。行底一层浅绿,不动布局(不是描边不是徽标),
免得庆祝落下时列表还抖一下。

### 30 秒,和一条并发扫描的护栏

一次 tick 不是一次本地读,是**在这个人有钱的每条链上做一次收据发现**;
所以取两个节奏里慢的那个,并且一直打。顺带补上一条护栏:网络差的时候一次扫描能活过 30 秒,
两次叠在一起会读到同一份 store、都发现同一笔收据不在里面、都写回去——
**一笔钱两行,庆祝两次**,而流水线上没有别的地方会发现:核心数的是壳报的数,壳报的两次都是真话。
`sync_received` 现在同时只跑一个,后来的那次答 0(这函数对"跑不起来"本来就是这么答的)。

### 一个新的 env 钉,和一条别信合成点击的教训

`VELA_SETTINGS_STATE` 只能开设置区的 chip,而 D1b 在设置区外——无头截图够不着。
于是有了 `VELA_GALLERY_TAB=D1b`(按 chip 标签,大小写不敏感,`VELA_FLOW` 那一套)。
**教训**:同一坐标的合成点击今天在这台机器上点出了三个不同的 chip(D1b、DST2、DC3),
连点三次三个结果;env 钉是可靠的那条路,截图前别再靠点。

### 测试与实机

四条测试:庆祝的句子来自**真机器跑出来的序列**(第一趟花掉 backlog gate、第二趟的扫描找到一笔,
才是核心允许庆祝的那一次——手写一个带 toast 的 `FeedView` 只能证明我会填结构体);
遮住余额就没有句子、但光晕还在;读不出的金额**不庆祝**(核心自己的话:fail closed,
宁可没有也不要错的);并发扫描答 0。

实机两张:浅色 en(`120 USDT received` + 那一行亮着)、深色 zh(`已收到 120 USDT`)。

**没验的一件事**:真网到账。扫描窗口是 100 个块,所以要看见真的庆祝,
必须现在真收一笔钱进金标 Safe——那要花钱,留给创始人点头。

desktop **336 / 332**(+4 测试),fmt clean,画廊全渲染,Windows 通过。

## Phase 44 — 拿 web 当清单:第一条对照发现"首页的数字停在启动那一刻"

创始人:"web 版本也有桌面网页版呀,现在 native 和 web 相比还差什么。"
于是做了一次**逐事件对照**:把每台核心机器的 `Event` 变体列出来,两个壳各 grep 一遍,
差集就是"web 会做而 native 不会"的动作。**42 个**。这一 phase 先修其中最要命的一组。

### 桌面这辈子只给余额机器发过一个事件

`AccountChanged`,开机那一次。**屏幕上的总额就是启动那一刻的总额** ——
钱到了、发的钱结算了,数字都不动,重启是唯一的办法。核心把两条节奏点名划给壳
(`AUTO_REFRESH_MS` 十分钟、窗口回到前台时的 `AppFocused`),两条都没接。

顺带发现两件同科的事:

- **`vela.balanceHidden` 从 spec 030 起一直在写,从来没人读回来。** 隐藏余额只活到进程结束。
  现在开机 hydrate 一次(先写先赢是核心的不变量⑧,所以是事件不是字段)。
- **`ReconcileCompleted` 也没接**:tracker 把 pending 改成 confirmed 是写在同一份 store 里的,
  而屏幕上那一行要等下一次重读才会变。现在 tracker 自己的 3 秒心跳把"刚补过几条"交给 feed,
  ≤3 秒,而不是 30 秒或者下次启动。

### 四个"我知道答案变了"的时刻,照 web 那份接

web 在四个地方强制读一次,桌面一个都没有:加了代币、加了网络、修好了 RPC、窗口回前台。

- **加代币**:核心在写落盘之后才要 `InvalidateTokenCache`,所以标记打在那个操作里
  (`perform` 没有 `cx`,够不着另一台机器),由页面下一帧取走 —— 而下一帧正是这次改动引起的那一帧。
- **加网络**:`AddConfirmed` 真的记上了才刷(核心拒绝时 `added` 为假,不刷)。
- **修 RPC**:`FixChainResolved` + 强制读。**被拒绝的端点不算修好** ——
  `rpc_chain_mismatch` 是核心对"这个节点自称是另一条链"的判决,拿它当门。

### 一条测试之外的事实

聚焦刷新在屏幕上**是静默的**,这不是缺陷:`refreshing` 只在 `pending_pulls > 0` 时为真,
也就是只有下拉刷新才转圈,web 同理。所以这一条是靠日志和真机验的,不是靠截图。

### 实机(parallel space,金标 Safe)

固定密钥集登录 → `$0.75`(Gnosis 上的 xDAI,真网真读)。
临时打了两行日志,看到:开窗一次 focus、切到 Finder 一次 blur、点回来又一次 focus,
三次都对上;验完把日志删了。

desktop **338 / 334**(+2 测试),fmt clean,画廊全渲染,Windows 通过。

## Phase 45 — 隐藏余额:桌面一直没有那个手势

对照清单第二条:`PrivacyToggled` 从来没接过。桌面**画着**隐藏态(圆点 + 划掉的眼睛),
`wallet_live::balance` 也一直在读 `view.hidden`,只是**没有任何地方能把它变成 true** ——
一个只能从别处进入、进不去也出不来的状态。

### 图形本身就是控件

手机和 web 都是"点金额即隐藏"(spec 025)。这里照做:`balance_display` 多收一个可选的
按下动作,`None` 就是原来那张画(画廊、没登录的窗口)。两个状态用**同一个** helper 包起来,
因为隐藏是个开关——只朝一个方向生效的手势是陷阱;隐藏态里那只眼睛就是回来的路,
光有一排圆点的屏幕会被读成"钱没了",而不是"我把它藏起来了"。

不加 hover 底色、不加涟漪:这是一个 40 像素的数字,给钱画个框说"这里能点",
正是这套设计语言花力气去掉的那种容器。

### 顺带把 phase 44 的另一半合上了

44 接了开机 hydrate,45 接了写入的那一下,两半这才见面:**藏起来 → 关掉 → 再打开,还是藏着**。
一条测试就是这条路(只执行 `WritePrivacy` 那一个操作——`AccountChanged` 会捎带一次
十二链拉取,单元测试没有理由去连网)。

### 实机

parallel space 金标 Safe:点金额 → 圆点 + 眼睛,**下面 xDAI 那行的金额也一起遮了**
(每个钱的面一起遮,是核心的不变量⑧);`wallet.json` 里 `vela.balanceHidden = 1`;
杀进程重开,还是遮着;再点一下 `$0.75` 回来,文件变 `0`。

desktop **339 / 335**(+1 测试),fmt clean。

## Phase 46 — 侧栏那排网络:看着能点,五个 spec 没人接过

`chain_row` 从 spec 015 起就带着指针光标、hover 底色和选中打勾 —— **一个监听器都没有**。
`ChainFilterChanged` 从来没被 dispatch 过。也就是说筛选是一张画:点下去什么都不会发生,
而它长得像会发生。

### 谁跟着变、谁不变

持仓和流水**跟着窄**,英雄区总额**不变** —— 手机 `selectedChainId` 的语义,web 原样移植过。
这是故意的不对称:筛选是"翻一份短一点的清单",不是"假装别的链上的钱没了"。

**流水那一半归核心**(`ChainFilterChanged`)。核心在过滤后的循环里才发日期头,
所以窄下来不会留下一个空日子的标题;在壳里过滤 `rows` 就会 —— web 恰好就是在壳里做的
(`narrowedFeed`),那是它自己的偏差,这边不学。而且核心一改,首页预览和 DA1 全屏
**同时**窄,不用接两遍。

### 一个会变成钱的 bug,提前挡掉

资产面板是**按索引**寻址的(索引进核心自己那份 `tokens`)。窄过之后
"Gnosis 的第 0 行"**不是**持仓 0。所以映射是**带着走**的,不是猜的:
`visible_token_indices` 返回的是核心列表里的下标,页面拿它把点中的那一行翻译回去。
丢了这个映射,就是从 USDC 那行打开 ETH 的面板,而那个面板下一个按钮是**转账**。
切筛选时顺手把开着的资产面板关掉 —— 它是用旧列表的索引开的。

顺带三处小的:资产全屏也窄(药丸显示**那条链的名字和点**,不再是"全部");
窄到一条没有持仓的链时**不出**引导性空态(那是在回答没人问的问题);
筛着某条链时按 收款,收款流直接停在那条链上(web 同理,否则筛的是 Gnosis 开出来是以太坊)。

### 实机 + 测试

金标 Safe 只有一条链有钱,所以真机能证明的是**打勾会动**:点 Gnosis,勾从"所有网络"移过去,
总额还是 `$0.75`。**窄和映射由测试守**:四个持仓、三条链,窄到 Gnosis 剩两行,
`visible_token_indices` 给出 `[1, 2]` 而不是 `[0, 1]`;窄到没持仓的链是空,不是回退到全部。

desktop **340 / 336**(+1 测试),fmt clean,画廊全渲染,Windows 通过。

## Phase 47 — 先复核清单,再动手:三个"删除"里只有一个是真欠账

本来要做"删除三处:历史行 / 自定义代币 / 自定义网络"。动手前按方法回查了一遍
**web 那边够不够得着**,结果两条不成立:

- **删除历史行**:`TxDetailModel.deleteLabel` 在 web 里**没有任何地方赋值**,
  而按钮是 `{#if model.deleteLabel !== undefined}` 才画 —— 所以 web 自己也点不到。
- **删除自定义代币**:web 只 dispatch 过 `address_input` / `save_requested` /
  `detect_requested`,`delete_requested` 一次也没发过。

**方法要补一句**:差集里的每一条,还要看 web 那边**是不是真能点到** ——
一个没人调用的函数里 dispatch 的事件,不是能力。顺带 `payment_request::AmountChanged`
同样不成立(`/request` 那条路由是扩展的 dApp 请求页,不是"收款要个金额"),
所以"收款指定金额"从清单里撤下:**两端都没有**,要做就是新功能,不是对齐。

于是这一刀改做**网络设置这一簇**,五件都验过 web 真能点到:

### 删除自定义网络

垃圾桶图标从 spec 023 起就画在自定义网络行上 —— 而它在**行自己的点击区里面**,
所以按下去干的是行的事:展开卡片。**这屏上唯一画成"销毁"的控件,干的是另一件事。**
现在它是自己的目标(`stop_propagation`)、是红的,并且**先问一句**。

web 是从垃圾桶直接 dispatch 的,不问。这里问,理由和这只壳里其他destructive 一样:
那一行背后是某人手填过端点的一条链,而它离"展开"只有一个像素。
话是语料里现成的(`settingsModals.network.remove*`,手机早画过这个对话框,十五种语言齐全),
**零新键**;后面补一句网络名,因为对话框盖住了它说的那一行。删完强制读一次余额 ——
刚才还在数那条链。

### 另外四件

- **供应商"测试"按钮**(`ProviderTestRequested`):键 blur 本来就会测,这个是给
  "我什么都没改,就想知道现在还行不行"的人 —— 而这是这一页唯一会被打开来问的问题。
  放在标题行上,挨着它要改写的那句结论。
- **"恢复默认"**(`ResetEndpointsToDefaults`):原来是一个刷新图标 + 一行灰字,没有监听器。
  **这是最坏的一种可供性**:一个把端点填坏了的人,读到的是一条不存在的出路。
- **打开面板即探测**(`EndpointsOpened` / `ProvidersOpened`):web 在同一个手势上发这两个事件,
  桌面一次也没发过。一次访问只发一次(面板每帧都画,探测不能每帧都跑),
  离开面板清标记,回来重测 —— 实机截图里四个端点各有各的真延迟(546ms / 1.1s / 1.3s / 1.8s)。
- **关掉添加网络对话框时 `WizardReset`**:核心会一直留着搜索词、选中的链和检查结果,
  所以再打开是别人没做完的一半。这个事件还会把在飞的探测作废,所以它是核心的事件,
  不是这里清个字段。

### 实机

parallel space 里手写一条自定义网络进 state,重开:X Layer 行带 `Custom` 标签和红垃圾桶;
点它 —— **行没有展开**,弹出 `Remove Network / Remove this custom network? · X Layer`;
按 Remove,行没了,`wallet.json` 里 `vela.customNetworks` 变成 `[]`。

desktop **341 / 337**(+1 测试),fmt clean,画廊全渲染,Windows 通过。

# 交接:下一个会话从这里开始

**范围:只做 desktop**(安卓/iOS/web 是别人的)。分支 `032-desktop-money-wiring`
(叠在 031 → 030 → 029 上,均未合并;028 已并进来)。工作区
`/Volumes/data/production/vela-wallet-native`,**47 个 phase,104 个提交**(`049617f5..`)。

## 一句话状态

**桌面已经没有纯接线的活了。** 钱能发(SC-303 真网达标)、签名面板读四台核心机器、
真 dApp 能连能读能签、浏览器有历史/收藏/分组/标签、收款有门也能出分享卡、
到账会庆祝(phase 43 起,`activity_feed` 也终于有人给它打 tick 了)。
剩下的每一件要么**要人动手**,要么**要产品决定**,要么**缺图**——清单在最后。

## 立刻可跑的闸门

> **别把 `cargo test` 接进管道再用 `&&` 串**(`| tail -3 &&` 接的是 tail 的退出码,
> 永远 0,测试挂了照样报绿)。要么原样跑,要么先 `set -o pipefail`。

```bash
cd /Volumes/data/production/vela-wallet-native/app-desktop/vela-wallet
cargo fmt --all --check && cargo test --features dev-fixtures && cargo test \
  && scripts/sweep-gallery.sh && scripts/check-windows.sh
cd ../../rust && cargo fmt --all --check \
  && cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings \
  && cargo test -p vela-core --features i18n-all,crux,dev-fixtures
cd ../app-web/vela-wallet && pnpm check && pnpm lint && pnpm build
```

**当前基线**:desktop **341 passed(feature on)/ 337(off)· 36 ignored**;
vela-core **1,304**;web `pnpm check` 0 errors、lint 干净、build 成功。
**web 的 `pnpm test:unit` 在这棵树里起不来**(vitest 项目初始化阶段
`Could not resolve 'node:module' in rolldown/runtime.js`,任何测试文件加载之前就失败)
——环境问题,不是代码,别当成回归。

**动过 `rust/` 就要**:`node rust/scripts/build-web.mjs` → `verify-web.mjs` →
`gen-core-types.mjs`;web 那边还要 `node scripts/sync-wasm.mjs`。
当前 wasm:`aaefc6530923`(3,728,061 字节)。

## 这一刀新增的核心机器

`explore_sites`(`vela.explore` 一份文档:收藏/分组/标签),18 条规则各一测试。
**四端共用**,ts-rs 两套镜像已生成(326 个文件),web 随时可接。

## 还欠的(全部,按"卡在什么上"分)

| # | 事 | 卡在 |
|---|---|---|
| 1 | **发送**:多币归集 sweep(五个事件)、报价过期 `Requote`、中继金库空的那张单 | 接线 + 一张图 |
| 2 | **签名**:交易模拟余额变化(桌面连 `sim/` 模块都没有)、收款人风险 `InspectRecipient` | 中等新功能 |
| 3 | **账户切换器里各账户的总额**(`SwitcherOpened/Closed`) | 纯接线(web 有) |
| 4 | 联系人 `SetGroupMembers` / `SetContactGroups`(批量设置分组) | 纯接线(web 有) |
| 5 | **删除历史行 / 删除自定义代币 / 收款指定金额** | ⚠️ **两端都没有**,做就是新功能不是对齐(phase 47 复核) |
| 6 | 批量授权的**逐腿编辑器**(`GuardView.batch`) | 缺图(两端都没有) |
| 7 | 设置里**新建/登录另一个账户** | 要导航决策(产品) |
| 8 | **扫码 DS1** | 桌面没有相机管线(新功能,不是接线) |
| 9 | 插**实体钥匙**签一笔 / 点一次 **xlsx 真表格** | 要人动手 |
| 10 | **Tempo** 链上跑一次提交 / **真网看一次到账庆祝** | 要人决定(真链真钱) |
| 11 | dApp 充值(funding)**完整流程** | 只说了一句话,没有"去充值"的那条路;发送列有一整套可抄 |

**phase 44 起,欠账清单换了来源**:不再靠"我觉得还缺什么",而是**拿 web 当清单** ——
逐台机器列 `Event` 变体,两个壳各 grep 一遍,差集就是 web 会做而 native 不会的动作。
第一次跑出 **42 条**,phase 44 修掉了刷新/聚焦/隐私 hydrate/reconcile 这一组。
剩下的按屏幕分组,见下表。

**判定不做**:`dapp_session`(WalletPair/远程注入)——创始人 2026-09-08 裁决,
桌面用内置浏览器注入,不做远程配对。别再把它当欠账捡起来。

## 接手前必读的三条方法(这一刀反复用到)

1. **第一条 grep**:`grep -n 'fixtures::' src/wallet/page.rs` —— 还有哪块界面在画 mock。
   现在只剩"登录前的占位"和送/扫/加币三处(每处都在代码里点名了)。
2. **第二条 grep**(每接完一台机器就跑):视图的判断字段 vs 壳读了什么。
   差集里每一个 `warning`/`can_*`/`error`/`stale`/`notice` 都是核心替人算好、
   屏幕却不说的一句话。phase 29 一次抓了十一个,其中三个关钱。
3. **画稿与实景的分叉规矩**:有绑定=真控件,没绑定=画稿那一版
   (滑块、授权 chip、菜单项、标签条、上限输入全走这条),
   所以老画态永远"一个像素没动",而 `sweep-gallery.sh` 是它的证据。

## GUI 自动化(这台机器上已验证)

- 终端已获**辅助功能**权限,`CGEventPost` 可用;**但窗口不是 key 时只收 hover 不收 click**
  ——先激活或对新开的窗口点。
- **合成键盘事件全打成 'a'**(`CGEventKeyboardSetUnicodeString` + keycode 0),
  中文/长名字要人真敲。
- **合成鼠标点击也别信**(phase 43):同一坐标连点三次,点中了三个不同的 chip。
  要开某个状态就用 env 钉——`VELA_GALLERY_TAB=<chip 标签>`(钱包画廊,大小写不敏感)、
  `VELA_SETTINGS_STATE`(设置)、`VELA_FLOW`(流程面板)、`VELA_SECTION`(区)。
- 截图:`screencapture -x -o -l <windowId>`;窗口 id 用 Quartz 列窗口按
  `kCGWindowName == "Vela Wallet"` 找(启动时还有若干 30px 高的辅助窗口,别抓错)。
- 本地 dApp 探针页在 `<scratchpad>/dapp/index.html` + `python3 -m http.server 8137`;
  跑 app 用 `VELA_PARALLEL_SPACE=1 VELA_STATE_DIR=<scratchpad>/state`,
  且**清掉代理变量**(`env -u all_proxy -u http_proxy -u https_proxy`)。
  真网 pool 现在自己会绕开 loopback 代理(phase 31),但外网调用仍受代理影响。
- **临时目录会被系统清掉**:state 没了就重新走一遍登录
  ("I already have a wallet" → "This device",固定密钥集自动答签名)。
