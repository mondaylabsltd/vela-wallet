# Data Model: Canonical Wallet-Home Fixtures

**Branch**: `015-wallet-home-ui` · This file is the single canon every
platform ports (`WalletFixtures.kt`, `WalletFixtures.swift`,
`src/lib/wallet/fixtures.ts`, `src/wallet/fixtures.rs`). Content is
verbatim from the mocks (spec FR-012). Components receive only these
display models — no service types (spec SC-005).

## Types (conceptual)

```
WalletIdentity   { name, addressDisplay, addressFull, identiconSeed }
NetworkFilter    = AllNetworks { dotColors[3], total } | Single { chain }
BalanceState     = Normal | ZeroLive | Loading | Hidden
StatusLine       = None | Warning(key) | Refreshing(key)
BalanceFixture   { state, statusLine, integerPart, decimalPart, currency }
ActivityKind     = Sent | Received | Dapp
ActivityEntry    { kind, titleKey, subtitle: SubtitleParts, amount, unit,
                   positive: bool, masked: bool, badgeColor }
SubtitleParts    { direction: To(name) | From(name) | Plain(text),
                   time?: { day: Today|Yesterday|Literal(text), clock?: text } }
DayGroup         { day: Today|Yesterday, entries[] }
AssetEntry       { ticker, chainName, badgeColor, balance,
                   fiat: Value(text) | NoPrice | Masked, masked: bool }
ChainEntry       { name, dotColor, count, selected }
PanelState       = None | Receive | AssetDetail
ScreenState (mobile) = H1 | H1s | H2 | H3 | H4 | H5 | H6 | H7 | H7x | H8
ScreenState (desktop) = D1 | D2 | D3
```

## Chain palette (fixture-only colors; not theme tokens)

| Chain | Dot |
|---|---|
| BNB Chain | `#F0B90B` |
| Ethereum | `#627EEA` |
| Arbitrum | `#28A0F0` |
| Gnosis | `#21BCA5` |
| Base | `#0052FF` |
| Polygon | `#8247E5` |
| All-networks pill dots | `#627EEA`, `#8247E5`, `#F0B90B` |

## Identity

- **default**: name 大表哥 · addressDisplay `0x14fB1f…D1eA5c` ·
  addressFull `0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c` (also the
  identicon seed, normalized by vela-core)
- **long** (H7): name 这是一个非常长 — same addresses
- Identicon board seeds (US3): the default address,
  `0xd8da6bf26964af9d7eed9e03e53415d37aa96045`, `alice`, `bob`,
  `0x9F3c…21aE` full form `0x9F3c00000000000000000000000000000000021aE`
  → normalize; plus empty string (placeholder case).

## Balance per state

| State | balance | status line |
|---|---|---|
| H1/H1s/H5/H8/D* | `$1,383` + `.28` USD, Normal (H5: Hidden) | None |
| H2 | `$0` + `.00`, ZeroLive (`home.liveIndicator`) | None |
| H3 | Loading | None |
| H4 | `$1,383` + `.46` | Warning `home.balanceUnpriced` |
| H6 | `$1,383` + `.28` | Refreshing `home.balanceStale` |
| H7/H7x | `$1,234,567` + `.89` | None |

## Activity

**default** (H1 shows group 今天 rows 1–2; H1s/D1 show the newest three — the core's
`FeedView.home_rows`, `HOME_ACTIVITY_ITEMS` = 3, issue #469; "All" opens History, which draws
every row):

| group | kind | title key | subtitle | amount |
|---|---|---|---|---|
| 今天 | Sent | `history.labelSent` | 至 hold on (desktop + · 今天 14:02) | −2 POL |
| 今天 | Received | `history.labelReceived` | 来自 0x9F3c…21aE (desktop + · 今天 11:20) | +120 USDT (green) |
| 今天 | Dapp | `history.txLabelDappTx` | PancakeSwap · BNB Chain (desktop: PancakeSwap · 今天 09:41) | −0.05 BNB |
| 昨天 | Received | `history.labelReceived` | 来自 Alice (desktop + · 昨天 20:15) | +50 USDC (green) |

Badges: POL row purple `#8247E5`, USDT row Ethereum blue, BNB rows BNB
yellow, USDC row Base blue `#0052FF`.

**extreme** (H7/H7x):

| group | kind | title key | subtitle | amount |
|---|---|---|---|---|
| 今天 | Sent | `history.labelSent` | 至 Alexandra | −1234.5678 POL |
| 今天 | Dapp | `history.txLabelDappTx` | app.uniswap.org · BNB | −0.0000001 BNB |

**H5**: default rows with `masked = true` (amount dots ····, unit kept;
received rows keep success color).

## Assets

**default** (H1 shows first ~3; H1s/D1 all six):

| ticker | chain | balance | fiat |
|---|---|---|---|
| BNB | BNB Chain | 0.8533 | $496.46 |
| ETH | Arbitrum | 0.2253 | $422.62 |
| ETH | Ethereum | 0.0689 | $129.25 |
| XDAI | Gnosis | 74.3965 | $74.38 |
| USDT | Ethereum | 53.4836 | $53.48 |
| USDC | Polygon | 12.04 | $12.04 |

**partial-price** (H4): rows 1–2 above, then CAKE · BNB Chain · 18.20 ·
NoPrice (`home.balanceDetailNoPrice`, warning color).

**extreme** (H7): WBTC · 以太坊主网 Ethereum · 0.00000042 · $0.03; USDT
· Ethereum · 1,234,567.8901 · $1,234,567.89.

**H5**: default rows with `masked = true` (both lines dotted).

## Chains (H8 sheet & desktop sidebar)

所有网络 ✓ 8 · BNB Chain 1 · Ethereum 3 · Arbitrum 1 · Gnosis 1 ·
Base 1 · Polygon 1. H7 uses `NetworkFilter.Single(BNB Chain)`.

## Desktop panels

**ReceivePanel** (D2): token row BNB / `receive.networkDetail`(BNB
Chain, 56) with disclosure; QRPlaceholder + `componentsUi.qrPlaceholder.
caption`; `receive.addressLabel`; addressFull in mono box; copy button
`componentsUi.identiconViewer.copyAddress`; warning card
`receive.warningTitle` + `receive.warningReminder` +
`receive.networksLine`(count 8).

**AssetDetailPanel** (D3): header 0.8533 BNB / $496.46 · BNB Chain;
actions `tokenDetail.send` / `tokenDetail.receive`; facts — 名称 BNB ·
价格 `tokenDetail.priceValue`(BNB, $581.85) · 合约
`addToken.labelNativeToken` · 精度 18; link `tokenDetail.viewOnExplorer`;
交易记录 (`tokenDetail.labelTransactions`): the two BNB rows — dApp 交易
· PancakeSwap · 今天 09:41 · −0.05 BNB; 已收到 · 来自 0x21aE…9F3c ·
8月1日 (literal) · +0.9 BNB (green).

## QR placeholder pattern

Deterministic 21×21 module grid: three 7×7 finder squares (top-left,
top-right, bottom-left — standard QR geometry, with the ring at offset 1
left empty), remaining cells filled by a seeded PRNG — `xorshift32`
seeded with `0x5EED` — identical output on every platform, so
screenshots diff cleanly. Never encodes data; always paired with the
caption key.
