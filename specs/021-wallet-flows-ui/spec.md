# Feature Specification: Receive / Send / Activity / Assets UI

**Feature Branch**: `021-wallet-flows-ui`

**Created**: 2026-09-01

**Input**: "Implement the `design/wallet-2/` UI on all four clients
(`app-android/vela-wallet`, `app-ios/VelaWallet`, `app-web/vela-wallet`,
`app-desktop/vela-wallet`). The ~50 mocks are compositions of a small set of
reusable components — build those once per platform and assemble the screens
from them. Wire them into the real app, not only the gallery. Business state
stays on fixtures for now."

## Why

Spec 015 built the wallet home's component vocabulary; spec 018 proved it
extends (contacts reused nine of its components without forking one). This
feature is the rest of the wallet: the four journeys a person actually takes
once they are inside — take money in, send money out, look at what moved, look
at what they hold.

It is deliberately one feature and not four. Read the mocks as pictures and
there are fifty screens; read them as compositions and there are ~27
components, most of which appear in three or four of the journeys. `AssetRow`
alone carries the assets list, the send token picker, the multi-send picker,
and the token-detail sheet. Building Receive alone would mean discovering that
vocabulary and then rediscovering it three more times.

The wiring requirement is the other half. Spec 015 shipped the home screen with
`收款 / 转账 / 扫码` buttons that did nothing, and the entry points in
`SPEC 动效` are all from that screen. Components that exist only in a gallery
are not a wallet — so every screen here hangs off a real entry point, and
"is this reachable by tapping?" is a gate, not a follow-up.

## Design Authority

`design/wallet-2/` is the visual authority. The three `SPEC 动效` sheets are the
behavioural authority — they name entries, transitions, degradations, and in
two places the exact i18n key to add.

## Scope

**In**: the ~27 components below, the 19 mobile states and 14 desktop states
they compose into, on four platforms; entry-point wiring from the wallet home;
preview-gallery coverage of every state; the i18n keys the mocks need.

**Out**: real balances, real RPC, real signing, camera decode, QR encode of a
real address, file parsing for the batch importer. Every screen is driven by
the fixture layer, exactly as spec 015 and 018 left it. Buttons that would
spend money are inert.

## Component Inventory

The point of the feature. `[015]` / `[018]` = already exists, extend in place —
never fork. `[new]` = built here.

### Chrome

| # | Component | Used by | Notes |
|---|-----------|---------|-------|
| 1 | `FlowScreen` `[new]` | R1 A1 T1 SD1 SD2 SD3 SD4 | Back chevron + large title + optional trailing text action + optional network pill. The mobile page frame for every non-sheet screen here. |
| 2 | `ThirdPanel` `[015]` | all `D*` | Gains a leading back chevron beside the title (DSD2L, DT3L, DA2L) — the panel now stacks, so it needs a way back one level. |
| 3 | `BottomSheet` `[015]` | R2 A2 T2 T3 SD2c SD2e SD2f | Gains the grabber, an `x` in the title row, and `fit` vs `tall` heights. |

### Inputs

| # | Component | Used by | Notes |
|---|-----------|---------|-------|
| 4 | `SearchField` `[new]` | R1 T1 SD1 SD2e | Magnifier + placeholder, filled rounded field. |
| 5 | `SegmentedToggle` `[new]` | T3 SD2c | Two segments. The *only* segmented control (design-review theme). |
| 6 | `FilterChipRow` `[new]` | SD1 | 全部 / 稳定币 / Gas 币 / 其他 — selected chip inverts. |
| 7 | `MonoField` `[new]` | T3 T3b SD2c | Monospace input; error variant draws the accent-danger border + hint. |
| 8 | `AmountInput` `[new]` | SD2 | Centred display-size number, fiat sub-line, denomination toggle. |

### Rows

| # | Component | Used by | Notes |
|---|-----------|---------|-------|
| 9 | `NetworkRow` `[new]` | R1 DR1L | Chain badge + name + truncated address + copy + QR trailing icons. |
| 10 | `AssetRow` `[015]` | T1 SD1 SD1b SD2d | Gains `trailing` (最大 chip), `dimmed` (SD1b off-chain rows), `selected`, and a balance-label subtitle form. |
| 11 | `ActivityRow` `[015]` | A1 T2 D* | Unchanged. |
| 12 | `ContactPickRow` `[new]` | SD2e | Identicon + name + group chip + address + chevron. Shares `GroupChips` `[018]`. |
| 13 | `RecipientCard` `[new]` | SD2b | Identicon + 收款人 N + name/address + amount + remove. |
| 14 | `FeeTokenRow` `[new]` | SD2f | Badge + symbol/balance + est. fee + check. |
| 15 | `FactRow` `[new]` | A2 A3 SD3 T2 T3b | Label + optional badge/identicon + value + optional copy. The single label-value row for the whole feature. |

### Blocks

| # | Component | Used by | Notes |
|---|-----------|---------|-------|
| 16 | `TokenHeaderCard` `[new]` | SD2 SD2b DSD2L | Token badge + symbol + chain · 余额 + optional 最大. |
| 17 | `AddressCard` `[new]` | R2 R3 DR2L | Identicon + name + two-line mono address + copy. |
| 18 | `QRCard` `[new]` | R2 R3 R4 DR2L | White card, QR, centre badge (token, or account identicon on the share card). |
| 19 | `AmountHero` `[new]` | A2 A3 SD3 T2 | Large amount + fiat line; sign-coloured. |
| 20 | `StatusHero` `[new]` | SD4a SD4b SD4c DSD4L | Circle (spinner / clock / check / cross) + title + captions. |
| 21 | `HintCard` `[new]` | T4 | Bordered card, CTA on top, title + body. |
| 22 | `NoticeBanner` `[new]` | SD1b SD2d | Badge + inline explanatory text on a subtle fill. |
| 23 | `StatusChip` `[new]` | A2 T3 T3b T5 T5b | 已确认 / 已添加 / 兼容 / 不兼容. Four tones. |
| 24 | `GhostPillRow` `[new]` | SD2b | Row of outline pill buttons. |
| 25 | `SummaryLine` `[new]` | SD2b SD2d | Leading caption + trailing figure. |
| 26 | `FeeRow` `[new]` | SD2 SD2b SD2d DSD2L | 网络费 + fee-token badge + amount + chevron into `FeeTokenSheet`. |

### Full-bleed

| # | Component | Used by | Notes |
|---|-----------|---------|-------|
| 27 | `ScanSurface` `[new]` | S1 DS1L | Corner frame + hint + tool row. Full-screen on mobile, centred modal on desktop. |

## State Matrix

Gallery ids, stable across all four platforms.

### Mobile (19)

| id | Mock | State |
|----|------|-------|
| `r1` | R1 | Receive · network list |
| `r2` | R2 | Receive · QR for a network |
| `r2x` | R2x | Receive · QR at 1.35× text scale |
| `r3` | R3 | Receive · QR for a named asset |
| `r4` | R4 | Receive · share card (the saved-image product) |
| `s1` | S1 | Scan · full screen |
| `a1` | A1 | Activity · history |
| `a2` | A2 | Activity · detail, received ERC-20 (has a contract row) |
| `a3` | A3 | Activity · detail, sent native coin (no contract row) |
| `t1` | T1 | Assets · list |
| `t2` | T2 | Assets · token detail |
| `t3` | T3 | Assets · add ERC-20 |
| `t3b` | T3b | Assets · add native coin / network |
| `t4` | T4 | Assets · empty, guided |
| `t5` | T5 | Add token · ERC-20 error states |
| `t5b` | T5b | Add token · network error states |
| `sd1` | SD1 | Send · pick token |
| `sd1b` | SD1b | Send · pick tokens, multi-select |
| `sd2` | SD2 | Send · form, single recipient |
| `sd2b` | SD2b | Send · form, split (one token → N people) |
| `sd2c` | SD2c | Send · import recipients |
| `sd2d` | SD2d | Send · form, sweep (N tokens → one person) |
| `sd2e` | SD2e | Send · pick contact |
| `sd2f` | SD2f | Send · pick fee token |
| `sd3` | SD3 | Send · confirm |
| `sd3b` | SD3b | Send · confirm, 3 recipients |
| `sd3c` | SD3c | Send · confirm, 3 assets |
| `sd4a` | SD4a | Send · receipt, submitting |
| `sd4b` | SD4b | Send · receipt, submitted |
| `sd4c` | SD4c | Send · receipt, confirmed |

### Desktop (14)

`dr1` `dr2` `dr3` · `ds1` · `da1` `da2` `da3` · `dt1` `dt3` `dt3b` `dt4` ·
`dsd1` `dsd2` `dsd2b` `dsd3` `dsd4` — the same content in the third column,
except `ds1` which is a centred modal over a dimmed window.

## Entry Points (the wiring requirement)

Per `SPEC 动效`, and all from the wallet home built in spec 015:

| From | Goes to |
|------|---------|
| Home 「收款」 | `r1` → row QR icon → `r2` |
| Home 「转账」 | `sd1` → token → `sd2` → `sd3` → `sd4` |
| Home 「扫码」 | `s1` |
| Home 活动「全部」 | `a1` → row → `a2` |
| Home 资产「全部 / 添加」 | `t1` → row → `t2`; 添加 → `t3` |
| `t1` 「通过地址添加代币」 | `t3` |
| `t2` 「收款 / 转账」 | `r3` / `sd2` for that token |
| `sd2` 收款人 person icon | `sd2e`; scan icon → `s1` |
| `sd2` 网络费 row | `sd2f` |

Desktop: every one of these opens the third column instead of pushing a screen,
and the panel keeps a back chevron so a stacked flow can unwind.

## New i18n Keys

The corpus already carries `receive.*`, `send.*`, `history.*`, `assets.*`,
`addToken.*`, `tokenDetail.*`, `componentsTx.*` and `componentsUi.scanner.*`
from the legacy React Native app — about 90% of this feature's strings resolve
against keys that already exist. What is genuinely new:

| Key | English | Named by |
|-----|---------|----------|
| `addToken.invalidAddress` | Invalid contract address | SPEC 动效 · 活动与资产 |
| `addToken.notCompatible` | Not compatible | T5b chip |
| `receive.searchNetworkPlaceholder` | Search networks | SPEC 动效 · 收款 |
| `receive.searchNetworkEmpty` | No networks match "{{query}}" | SPEC 动效 · 收款 |
| `receive.qrTitleNetwork` | Use this address to receive assets on {{network}} | R2 |
| `receive.qrTitleAsset` | Use this address to receive {{symbol}} on {{network}} | R3 |
| `receive.shareCardNetworkNote` | {{network}} payments only | R4 |
| `receive.tokenContract` | Token contract | R3 |
| `assets.addByAddress` | Add a token by address | T1 |
| `assets.notShowingTitle` | Received a token but don't see it? | T4 |
| `assets.notShowingBody` | Confirmed transfers are added automatically… | T4 |
| `send.balanceLabel` | Balance {{amount}} | SD2 |
| `send.fromContacts` | From contacts | SD2b |
| `send.splitTotalLabel` | Total | SD2b |
| `send.multiSendChainNotice` | {{network}} selected — a multi-token send… | SD1b |
| `send.multiSendSameRecipient` | Every token goes to the same address | SD2d |
| `send.pickContactTitle` | Choose a contact | SD2e |
| `send.pickContactSearch` | Search contacts or paste an address | SD2e |
| `send.feeTokenHint` | Pays this transfer's network fee… | SD2f |
| `send.feeTokenEstimate` | Estimated fee | SD2f |
| `send.batchRateHint` | Amounts are read as {{code}}… | SD2c |
| `send.batchParsedCount` | {{count}} rows parsed | SD2c |
| `send.confirmTotalLine` | Total ≈ {{fiat}} · {{network}} | SD3c |
| `send.txPreparingBiometric` | Preparing the transaction, waiting for biometrics | SD4a |
| `send.txBackgroundHint` | Closing this page keeps the transaction running | SD4a |
| `send.txCloseBackground` | Close · keep running | SD4a |
| `send.txSubmittedTitle` | Submitted to the network | SD4b |
| `send.txConfirmedTitle` | Sent {{amount}} {{symbol}} | SD4c |
| `componentsUi.scanner.fromGallery` | From photos | S1 |
| `componentsUi.scanner.flipCamera` | Flip | S1 |
| `componentsUi.scanner.torch` | Torch | S1 |
| `componentsUi.scanner.gallery` | Photos | S1 |

## Success Criteria

- **SC-001** Every state id above renders on all four platforms.
- **SC-002** Every entry point in the table works in the real signed-in app,
  not only in the gallery.
- **SC-003** No component in the inventory is implemented twice on one
  platform, and none of the `[015]`/`[018]` components is forked.
- **SC-004** Fixtures are the only data source; nothing calls RPC, signs, or
  spends.
- **SC-005** Each platform's existing gates stay green, and the corpus gates
  (gen / lint / verify / vectors / leaf-count pin / wasm) pass.
