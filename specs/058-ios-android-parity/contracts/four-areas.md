# Contract — the four areas, iPhone against Android

The founder named four: 转账, 收款, dApp browser, 设置. Each row is a
difference found by one of the three rulers or by reading both call sites, with
the file that proves it. "Before 058" is the state on 2026-09-15.

Rulers used, in order of sharpness:

```sh
node scripts/check-ios-android-copy-parity.mjs --list   # sentences one phone cannot say
node scripts/check-ios-android-parity.mjs               # machines one phone does not talk to
node app-ios/scripts/check-ios-dropped-judgement.mjs    # view fields the core computes and nobody reads
```

## 转账 · send

| # | Difference | Android | iOS before 058 |
|---|---|---|---|
| S1 | The receipt's pre-submission states | 正在准备 / 正在等待生物识别 / 正在提交, per `tx_status` (`SendLive.kt:606`) | one sentence, `send.txSubmitting`, for all three (`SendLive.swift:651`) |
| S2 | Cancel during the passkey ceremony | the CTA becomes 取消 while signing — the core's checkpoint (`SendLive.kt:615`) | always 在后台继续; the ceremony cannot be called off from the receipt |
| S3 | A fee-held or fee-rejected payment | `hold_reason` decides the caption: 等待费用回落 or 未发送 (`SendLive.kt:648`) | `holdReason` is on the wire (`SendWire.swift:193`) and **read by no Swift at all** |
| S4 | `send.splitTotalLabel`, `send.batchImportFile` | resolved | never mentioned |
| S5 | Fee-asset choice (the reverse debt) | `FeeEvent`: 2 of 6 dispatched — no `select_fee_asset`, no `requote` | 5 of 6; the iPhone picks its fee coin and Android does not |

S5 is recorded, not fixed: it is Android's debt, and this branch is the iOS
chain.

## 收款 · receive

| # | Difference | Android | iOS before 058 |
|---|---|---|---|
| R1 | A code for one ASSET (r3) | live since 048: a token detail's 收款 and a network row's QR both push a `PaymentRequestEvent.AssetPicked`; the sheet's title, centre mark and 代币合约 row follow the asset (`FlowLive.kt:39`, `VelaNavHost.kt:921`) | drawn, unreachable. `FlowsLive.receiveQr` takes a `ChainMeta` and nothing else, and says so in its own comment |
| R2 | `payment_request`'s other three events | `mode_changed`, `amount_changed`, `acknowledge` exist as controller methods with **no callers** — dead on Android too | not dispatched |
| R3 | The view fields | — | `qrValue`, `copyPayload`, `hasAmount` carried and never read (dropped-judgement ruler) |

R2 is the rulers' blind spot made visible: the event ruler scores Android 6/6
on `PaymentRequestEvent` and only the call sites show that half of it is dead
code. Both phones are missing the amount-carrying request; neither is behind
the other.

## dApp browser · 探索

| # | Difference | Android | iOS before 058 |
|---|---|---|---|
| B1 | An http page | `connect.browser.a11yInsecure` — "Insecure site — not encrypted" (`ExploreLive.kt:120`) | the bare host. 056 recorded "there is no corpus sentence for this"; there is, and Android uses it |
| B2 | The consent sheet's title | `connect.browser.title` — **Connect to {host}** (`ExploreLive.kt:139`) | the generic `explore.connectionTitle` |
| B3 | Everything else | `ExploreEvent` 11 dispatched, `BhistEvent` 3, `DpermEvent` 9 | 12, 4, 9 — the iPhone is **ahead**: `favorite_renamed` and `delete_origin` are iOS-only |

The browser is the one area where iOS leads. B1 and B2 are the whole gap and
both are anti-phishing copy.

## 设置 · settings

| # | Difference | Android | iOS before 058 |
|---|---|---|---|
| T1 | 语言 | `prefsStore.setLanguage(id)` and the composition re-resolves (`VelaNavHost.kt:1566`) | **the choice is stored and read by nothing, ever.** `Loc()` is built once from `Locale.preferredLanguages` (`VelaWalletApp.swift:11`); `vela.language` has no reader at launch or after |
| T2 | 存储 | real: every key, its bytes, its record count, filed under the drawn rows; clearing removes exactly those keys (`DeviceStorage.kt`) | invented numbers |
| T3 | 关于 | the build's version and commit (`SettingsLive.kt:457`) | drawn |
| T4 | Copy | — | zero Android-only keys in `settings` and `settingsModals`: the rows themselves match |

T1 is the founder's 049 lesson repeating on the other phone: a setting was
"verified" by watching its own row change.

## What the screenshots showed

Read-only, on the Xiaomi `9d5f42fb`, signed in to the founder's real account —
so nothing was tapped that spends:

- `and-home.png` — 通讯录 with 新建分组 in the section header, the affordance
  iOS row 7 is missing.
- `and-receive.png` — R1's network list: copy and QR on every row. The QR is
  the door to the asset-limited sheet.
- `and-receive-qr.png` — R2 for Optimism: 保存图片 and 在区块浏览器中查看
  under the code. Both live on iOS since 051.
