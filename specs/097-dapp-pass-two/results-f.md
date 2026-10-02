# 097 part F — the confirm names the address; the receipt lists every coin (plan, tasks, results)

**Branch:** `097-send-recipient`, on top of `097-token-registry` (097 A + C + B + D). **Spec:** [spec.md](spec.md).

## The findings (real-money sweep through the extension's Send)

Evidence: session scratchpad `dapp096/shots/sw-*.png`.

- **S2 — a public name stood in for the address on the page that signs.**
  - The confirm's To row for the developer wallet `0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c` read only "Wallet" beside an identicon (`sw-022-golden-gnosis-confirm.png`). The address showed only after a tap on the identicon (`sw-024-golden-gnosis-to-identicon.png`).
  - "Wallet" came from the public passkey index, the on-chain wallet registry, where anyone can register any name.
  - The form's line under the field printed the resolver's own label raw: "Wallet · passkey" (`sw-021-golden-gnosis-filled.png`).
  - The sweep's confirm had the same To row (`sw-054-golden-base-confirm.png`).
- **S3 — a two-coin send's success screen named one coin.**
  - It read "Send ETH | Sent 0.000418 ETH | To Wallet · Base" (`sw-054-golden-base-status-12.png`), although 0.034929 USDC moved in the same MultiSend UserOp.
  - The cause: the core's receipt `amount` was the first signed line's figure, and every shell drew it with the selected token's symbol. No shell listed a sweep's coins.
  - Found on the way: a split's headline was its first row, not its total ("Sent 0.5 ETH" for 0.5 + 0.25).

## Plan

**Core** (`rust/crates/vela-core/src/app/send.rs`): two new view facts, decided once.

| Fact | Rule |
|---|---|
| `SendView.payees: Vec<SendPayee>` | Who the money goes to, as the form's line and the confirm name them. The address is always there, in full; a name appears only beside it, with `name_source` (a `#[serde(tag = "type")]` enum).<ul><li>`own`: the person's own account (resolver source `self`), or a split row's name, which is the person's contact name or their own list's name column.</li><li>`registry`: resolver source `passkey`, the public wallet registry.</li><li>`service { label }`: a forward-verified name service (`ENS`, `.bnb`, `Basename`…).</li></ul>A name with no or blank source, or one that fails `valid_display_name`, is dropped. One payee for a single send or a sweep (none until the address is whole); one per row, in `recipients` order, for a split. |
| `SendReceiptView.coins: Vec<SendReceiptCoin>` | Every coin the operation sent, summed over its recipients, in signing order: `{amount, symbol, logo_urls, token_address, usd_value}`.<ul><li>A coin with one line keeps that line's figure exactly.</li><li>A coin paid to several recipients is their exact base-unit sum.</li><li>`SendLine` gains `token_address` to tell coins apart.</li></ul> |
| `SendReceiptView.amount` / `usd_value` | `amount` is `coins[0].amount` when exactly one coin moved (so a split's total), and `""` for a sweep of several. `usd_value` is all coins together. |

**Shells** draw only. The words are `send.velaUser` ("Vela User"), which was in all 15 locales and used by no shell; a name service's own label, as before; and `componentsTx.detail.sent`, `componentsTx.receipt.assetsCount` and `send.multiSendTitle`. There is no corpus change.

| Surface | Draws |
|---|---|
| Form recipient line | Token-contract warning first, then "{name} · {tag}" ("Wallet · Vela User", "bob.eth · ENS", "Savings"), then the first-time tag. The raw resolver label is never printed. |
| Picker To line, confirm To row (single and sweep), split confirm rows | Line 1: the name alone; it may be cut. Line 2: the tag and the short address, "Vela User · 0x14fB…eA5c", in mono and never cut. Unnamed: the short address alone, in mono. The identicon is seeded with the full address, and a tap shows it whole. A split confirm has no single To row. |
| Receipt | The header is "Send tokens" for a sweep. The confirmed title is "Sent {amount} {symbol}" for one coin (a split's total) and "Sent" for several. A sweep lists every coin under "N assets" (Android: as caption lines, since its receipt is captions only), while submitted and when confirmed. A sweep's caption keeps "To {name} · {chain}"; a split's keeps "N recipients · {chain}". A sweep that sent one coin (its native line dropped for gas) is titled by it and not listed. |
| Group → split seed | The contact's own `name` only. A resolved or registry name would otherwise be drawn untagged, as if it were the person's own word. Changed on web, desktop and iOS; Android already did this. |

Why the tag sits on the address line: the core allows names up to 64 UTF-16 units. With "Wallet · Vela User" on line 1, a long registered name pushed the tag out of the row (seen on desktop: "A very long registry name that s…"). Line 2 is never cut.

## Tasks

- [x] T1 Core: `SendPayee`, `SendNameSource`, `SendView.payees`; `SendReceiptCoin`, `SendReceiptView.coins`, corrected `amount`/`usd_value`; `SendLine.token_address`.
- [x] T2 Core tests (`tests/app_send.rs`, section "Spec 097 F"), plus an old-core probe.
- [x] T3 Regenerate: fmt, `gen:i18n` (no change), wasm + sync, TS mirrors (`SendPayee.ts`, `SendNameSource.ts`, `SendReceiptCoin.ts`), Swift bindings (unchanged: the send view crosses as JSON); `build-web --check`, `gen-onboarding-types --check`, `gen-core-types --check`.
- [x] T4 Web + extension: `flows/live-send.ts`, `FactRow` `detail` line, `messages.ts` keys, group seed; tests; sweep e2e.
- [x] T5 Desktop: `flows/live.rs`, `FactRow` / `BreakdownRow` `detail`, receipt panel title, group seed; tests.
- [x] T6 iOS: `SendWire.swift` (tolerant name source), `SendLive.swift`, `FactRowModel` / `BreakdownRowModel` `detail`, group seed; tests.
- [x] T7 Android: `SendWire.kt` (tolerant name source), `SendLive.kt` (also the sweep confirm, see below), `FactRowModel` / `BreakdownRowModel` `detail`; tests.
- [x] T8 Suites, CI scripts, screenshots, this file.

## Results

### Tests that fail on the old code

**Core**, `tests/app_send.rs`, 10 new tests:
- `a_registry_name_never_stands_for_the_address_on_the_confirm`: form and confirm; wire shape `name_source.type == "registry"`.
- `an_own_name_carries_the_address_too`: source `self`.
- `a_name_service_name_says_which_service`
- `a_name_of_unknown_source_or_unprintable_is_not_drawn`
- `a_half_typed_address_names_nobody`
- `a_split_rows_own_name_carries_its_address`
- `a_sweep_names_its_one_payee_with_the_address`
- `a_two_coin_sweep_receipt_lists_both_coins`: ETH 1.5 net of the gas reserve, then USDC 5; `amount == ""`; `usd_value` is the total.
- `a_split_receipt_heads_with_its_total_not_its_first_row`: 0.5 + 0.25 gives `coins == [0.75 ETH]` and `amount == "0.75"`.
- `a_single_send_receipt_is_its_one_coin`

These tests read the new fields, so they cannot compile against the old core. Their claims were re-asserted, on fields the old core has, against `send.rs` from `032524b1d` (temporary probe, not committed). All three probes fail there and pass on the new core:

| Probe | Old core |
|---|---|
| Sweep receipt `amount` | `"1.5"` (the first coin), expected `""` |
| Split receipt `amount` | `"0.5"` (the first row), expected `"0.75"` |
| Named recipient | the view has no `payees`: no field says who is paid with the address |

**Web** (`src/lib/flows/live-send-payee.test.ts`, 10 tests; the first drives the real wasm core):
- open → token → recipient → `identity_resolved {Wallet, passkey}`
- the form line "Wallet · Vela User" (never "passkey")
- the confirm To row "Wallet" over "Vela User · 0x14fB1f…D1eA5c" with the full-address identicon
- an own name untagged
- unnamed in mono
- a 64-character name keeps the tag
- the sweep To row
- split rows from payees, with no To row
- a two-coin sweep receipt, confirmed and submitted: header "Send tokens", title "Sent", "2 assets", both coins, caption "To Wallet · Base"
- a one-coin sweep
- a split's total

Against the old `live-send.ts`, 6 of the first 8 failed. The split rows and the split total passed, because the old web already drew split names over addresses and read `receipt.amount`; the total's fix is in the core.

**Desktop** (`flows/live.rs` `payee_tests`, 5 tests; the first three drive the real core): registry name, own name and service name; split names and no stale To row; the two-coin sweep receipt; the one-coin receipt and one-coin sweep.

**iOS** (`VelaWalletTests/SendPayeeTests.swift`, 13 tests; one drives the real core): the same cases plus an unknown name source. Against the old `SendLive.swift`, 12 of 23 tests in `SendPayeeTests` and `SendAssetsParityTests` failed.

**Android** (`SendLiveTest`, `SendSplitVerdictsTest` with the real core over UniFFI, `CoreWireDriftTest`): the same cases plus the sweep confirm; an unknown name-source tag reads as null. Against the old `SendLive.kt`, all 8 new or updated tests failed.

### Updated expectations

| Test | Was | Now | Why |
|---|---|---|---|
| web `live-send.test.ts`: picker "says whom the money is for" | value `alice.eth` | `alice.eth` over `ENS · 0xabab…` | S2 |
| web: form "says who the recipient is" | identity `{alice.eth, ENS}` | the core's payee | the line reads `payees` |
| web: confirm "names a resolved recipient instead of their address" | value `alice.eth` alone | renamed "…over their address, never instead of it" | S2 |
| web: split "never shows a name without the address" | names from drafts | adds the core's `payees` | names come from `payees` |
| web fixtures (`live-send`, `live-send-sweep`, `live-receive` tests) | — | `payees: []`, receipt `coins` | new fields |
| web e2e `sweep.e2e.ts` | ended at "Submitted" | also asserts "Send tokens", "2 assets", "100 USDC" on the receipt | S3 |
| desktop `the_picker_says_whom_the_money_is_for` | `recipient_identity.name` alone | `alice.eth` over `ENS · …` | S2 |
| desktop `a_group_pick_carries_each_members_name` | `[Ana, bo.eth, None]` | `[Ana, None, None]` | a resolved name must not pass as the person's own |
| desktop `a_split_receipt_lists_its_recipients_and_counts_them` | — | asserts the title is the total | S3 |
| iOS `SendAssetsParityTests.aSplitConfirmListsTheTypedPayeesAndNeverTheDrawing` | "Alice · 0x…" label | `Alice` + `detail` | two lines |
| iOS `SendAssetsParityTests.thePickersGroupsAreTheBooksOwn` | seeds `alice.eth` (ENS) | no name; an own "Bob" is kept | own word only |
| iOS `CoreWireDriftTests.anUnknownVariantDoesNotFailTheView` | — | adds an unknown name source and an ENS source | tolerant wire |
| Android `SendLiveTest` "recipient note names the source of the name" | raw `identity.source` | renamed "…says whose word the name is, never the resolver's label" | S2 |
| Android `SendLiveTest` pick / split confirm | one line, "Alice · 0x…" | name + `detail` | two lines |

### Suites

| Suite | Command | Result |
|---|---|---|
| core | `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,500 passed, 0 failed, 2 ignored (097 D: 2,490) |
| core lint | `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings`; `cargo fmt --all --check` | clean; clean |
| i18n | `i18n_residency` | ja+en resident 137,675 B (budget 141,800): **0 bytes added**, no corpus change |
| artefacts | `build-web --check`, `gen-onboarding-types --check`, `gen-core-types --check` | current. Wasm 4,442,118 → 4,447,267 B. Swift bindings unchanged. |
| web | `pnpm build:extension`; `npx vitest run`; `pnpm check` | 176 files, 2,601 passed, 5 skipped; `svelte-check` 0 errors, 0 warnings |
| web e2e | `playwright test e2e/sweep.e2e.ts e2e/send-lands.e2e.ts --project=chromium` | 2 passed |
| desktop | `cargo fmt --check`; `clippy --all-targets`; `cargo test` | clean; no warning inside a 097 F hunk (the toolchain reports 64 warnings, all in older code); 919 passed, 49 ignored |
| Android | `:app:testDebugUnitTest -PvelaSkipRustBuild` | 974 tests in 109 classes, 0 failures |
| iOS | `build-ios-xcframework.sh`; targeted (`SendPayeeTests`, `SendAssetsParityTests`, `ParityTests`, `CoreWireDriftTests`, `SendReceiptVerdictTests`, `SplitVerdictTests`); full `VelaWalletTests` (own clone of iPhone 16 Pro) | targeted 70 tests in 6 suites passed; full `VelaWalletTests` 1,177 tests in 148 suites passed |
| CI scripts | reachability / event payloads / dead controls; `app-ios/scripts/check-ios-dropped-judgement.mjs` | reachable; 0 mismatches (539 sites); 0 dead controls; dropped-judgement: `payees` and `coins` on the wire and read (its one Send flag, `SendReceiptView.usdValue`, predates this) |

### Found and fixed on the way

- **Android sweep confirm.** A live sweep confirm drew the single-send hero over an empty figure: the first coin's symbol ("XDAI"), "≈ $0.00", and no coin rows. It now reads "N assets", "Total ≈ $X · network", and one row per coin from the core's reserved `multi_specs`, as on the other shells.
- **Desktop split confirm.** It kept a stale single To row from `send.recipient` (the core keeps the field when a split is entered). That row is gone.
- **Android sweep receipt.** It was captioned "2 recipients"; a sweep is one recipient and N coins.

### Screenshots

Session scratchpad `097f/`.
- **Web**, gallery over core-shaped views. A temporary, uncommitted state (`web-gallery-patch.diff` + `x097f-gallery.scratch.ts` + `shoot.mjs`) runs the live `liveSendConfirm` / `liveSendReceipt`.
  - `web-confirm-{en,zh}.png`, 390 px phone: To "Wallet" over "Vela User · 0x14fB1f…D1eA5c".
  - `web-d-confirm-{en,zh}.png`: desktop-width panel.
  - `web-receipt-sweep-{en,zh}.png` and `web-d-receipt-sweep-{en,zh}.png`: "Send tokens / Sent / To Wallet · Base / 2 assets / ETH 0.000418 ETH / USDC 0.034929 USDC".
  - `web-e2e-sweep-receipt.png`: the real app (parallel space, stubbed chain and relay) after a two-coin sweep: "Send tokens / Submitted to the network / 2 assets / ETH 1.4969 ETH / USDC 100 USDC".
- **iOS simulator**, the live builders over the core's view in a temporary gallery patch (`ios-gallery-patch.diff`, reverted):
  - `ios-confirm-{en,zh}.png`: To "Wallet" over "Vela User · 0x14fB…eA5c". The figures are the gallery's.
  - `ios-receipt-{en,zh}.png`: "Send tokens / Sent / Wallet · Base / 2 assets / ETH, USDC".
- **Desktop:** `097f-desktop/dsd3-desktop.png`, the DSD3 mock with a named To row and split rows. It was taken BEFORE the tag moved to the address line, and shows the cut tag on a long name that motivated the move.
- **Android:** no JVM screenshot tooling exists (no Robolectric/Roborazzi), so there are no Android screenshots.

## Shells without a surface

None: all four shells and the extension (it is the web build) have Send's form, confirm and receipt. The extension's MV3 worker draws no Send screen.

## Open questions

1. **The receipt's "To {name}" caption** still shows the untagged name ("To Wallet · Base") after sending, on all shells. The rule was scoped to the page that signs. Say if the receipt and Activity rows ("Sent · To Wallet") should carry "Vela User" too.
2. **Own contact names on a single send.** A contact picked from the book reaches the core as an address only (`PickedAddress`), so its own name never shows; the registry name does, tagged. Carrying the contact's own name (`own`, ahead of the registry, as issue 191's rule does for lists) needs a name on that event.
3. **Mono second line.** "Vela User · 0x…" is set wholly in the mono face on every shell. Setting the tag in the text face and only the address in mono would mean two styled runs per shell.
4. **Seen in the evidence, not changed:** the Activity row for the sweep read "Sent · To Wallet · 2", a bare count with no unit (`sw-054-golden-base-status-12.png`). That is Activity's merge of the sweep's sibling records, not Send.
