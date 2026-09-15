# Feature Specification: iOS ↔ Android Parity — the four areas, and the compiler

**Feature Branch**: `058-ios-android-parity` (stacked on `057-ios-founder-pass`)

**Created**: 2026-09-15

**Status**: Draft

**Input**: The founder, 2026-09-15: "使用 058 spec 来对比 app-android,我们的 ios 版本功能是否
和 android 保持一致性了,转账,收款,dapp browser 以及 设置功能。android 设备我也连接上去了,你
可以截图对比的。缺少的 UI 交互,你也可以去学习一下 android 或者是手机网页版本(app-web)。Eleven
affordances are still drawn-and-unreachable,如果 web android 都没有设计稿的话,那你先出设计稿
(用 html 画出来,我审核通过后,再到里面去实现,包括 web desktop android ios)。在 Release /
Archive 编译时,Swift 6.2.4 编译器在优化 CoreStore.deinit 时自己崩溃了。这个问题也得解决吧"

## Why this exists

040–049 wired Android against the web. 050–057 wired iOS against the web. **The
two phones have never been read against each other**, and the web is not a
proxy for either: it has a mouse, three columns and no camera, so a control it
lacks proves nothing about a phone, and a control it has may belong on a
desktop only.

057 ended with eleven affordances "drawn and unreachable" and one line in the
PR notes: Release and Archive cannot be built at all, because swift-frontend
6.2.4 crashes optimising a destructor. That second one is not a parity question
— it is the difference between an app and an App Store submission — so it is
US1 here.

## The rulers this cut adds

Two, both cross-client, both zero-exit (the table is the deliverable; what a
row MEANS is a judgement that belongs in results.md):

| | what it reads | what it finds |
|---|---|---|
| `scripts/check-ios-android-parity.mjs` | per core machine, the Event variants each phone dispatches | a machine one phone talks to and the other does not |
| `scripts/check-ios-android-copy-parity.mjs` | per corpus area, the KEYS each phone can resolve | a **sentence the iPhone cannot say** — which is usually a control, a state or a whole sheet that is missing, not a translation that is |

The copy ruler is the sharper of the two and it is new thinking: every label in
this app comes from one corpus, by key, on all four clients. A key Android
resolves and iOS never mentions is a screen the iPhone does not draw. It reads
fixture files on purpose — on both phones the fixture layer is where a screen's
copy lives — and it filters candidate strings through `en.json` itself, because
`chevron.right` and `pancakeswap.finance` have the shape of a key and are not.

Both rulers have a known blind spot, recorded rather than papered over: a
controller method that dispatches an event and that **nothing calls** counts as
"dispatched". `WalletController.receiveMode` and `receiveAmount` on Android are
exactly that, and only reading the call sites shows it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — The app can be archived at all (Priority: P1)

A Release or Archive build completes. Today it does not: swift-frontend 6.2.4
segfaults in `EarlyPerfInliner`, and the founder's note names `CoreStore.deinit`
because that is where it landed first.

**Why this priority**: every other item in this spec ships through a build that
currently cannot be made. It is the App Store blocker 057 recorded.

**Independent Test**: `xcodebuild -configuration Release -destination
'generic/platform=iOS'` exits 0.

**Acceptance Scenarios**:

1. **Given** the Release configuration, **When** the app target is compiled,
   **Then** the compiler does not crash and the build succeeds.
2. **Given** somebody later adds a generic class, **When** the guard runs,
   **Then** it names that class before Release does.

---

### User Story 2 — 转账: the iPhone says what is happening to the money (Priority: P1)

A person taps 确认, the passkey sheet comes up, and the screen behind it says
what it is doing. If the network's fee rises above what they approved, the app
says so, and says whether the payment is waiting or was dropped.

**Why this priority**: it is the only area where silence costs money. Android
says 正在等待生物识别…, offers 取消 during the ceremony, and explains a
fee-hold; iOS says one sentence for every pre-submission state and reads
`hold_reason` nowhere at all.

**Independent Test**: drive `SendLive.receipt` with each `tx_status` and with a
receipt carrying a `hold_reason`; the title and captions differ per state.

**Acceptance Scenarios**:

1. **Given** `tx_status == "signing"`, **When** the receipt renders, **Then**
   its title is `send.txSigning` and its CTA is 取消, not 在后台继续.
2. **Given** a receipt whose `hold_reason` is set and whose status is failed,
   **Then** the caption is `send.txRejectedFees`, not the generic failure hint.
3. **Given** a held (not failed) receipt, **Then** `send.txHeldFees` explains
   that the payment will go out by itself once fees settle.

---

### User Story 3 — 收款: a code for THIS asset (Priority: P1)

From a token's page, 收款 shows a code for that token on that network, titled
with the asset and carrying the contract it means. Today the iPhone shows the
same network-level code from everywhere, so a person asking to be paid USDC on
Gnosis hands over a code that says "Gnosis".

**Why this priority**: r3 is drawn, translated and unreachable on iOS, and live
on Android since 048. The asset is what the other person needs.

**Independent Test**: `payment_request`'s `asset_picked` reaches the QR sheet;
the title uses `receive.qrTitleAsset` and a contract row appears.

**Acceptance Scenarios**:

1. **Given** a token detail, **When** 收款 is tapped, **Then** the sheet names
   that token and that network and shows the token contract.
2. **Given** a network row's QR button, **Then** the sheet names the network and
   shows no contract row.

---

### User Story 4 — dApp browser: an unencrypted site says so (Priority: P2)

A page served over http is described as insecure, and the consent sheet names
the host that is asking.

**Why this priority**: 056 recorded "there is no corpus sentence for insecure"
and that was wrong — `connect.browser.a11yInsecure` exists, Android uses it,
and its absence leaves an http dApp looking like any other. Same for
`connect.browser.title`, which is the anti-phishing line: **Connect to {host}**.

**Independent Test**: build the connection model for an `http://` origin and for
a consent request; assert both strings.

**Acceptance Scenarios**:

1. **Given** an `http://` origin, **Then** the status line reads the insecure
   sentence rather than the bare host.
2. **Given** a site asking to connect, **Then** the sheet's title names it.

---

### User Story 5 — 设置: the language row changes the language (Priority: P1)

A person picks 日本語. The app is in Japanese — now, and after a relaunch.

**Why this priority**: this is worse than 057's "half" recorded. `Loc` is
constructed once from `Locale.preferredLanguages`, and **nothing reads
`vela.language` ever**: the stored choice changes no label at launch, on
relaunch, or at any other moment. The row is a picture. Android has had it live
since 047, and 049's whole lesson was that a setting is verified where its
effect shows, never on the settings page.

**Independent Test**: `Loc` boots from the stored tag; changing it re-resolves
without relaunch.

**Acceptance Scenarios**:

1. **Given** `vela.language = "ja"` in the store, **When** the app launches,
   **Then** labels are Japanese.
2. **Given** the app running, **When** a different language is picked, **Then**
   the visible labels change without a relaunch.
3. **Given** `auto`, **Then** the device's language decides, as before.

---

### User Story 6 — 设置: 存储 and 关于 tell the truth (Priority: P2)

存储 shows what this device actually holds, and 关于 shows the version and
commit of the build being run.

**Why this priority**: both are drawn numbers on iOS and real on Android
(`DeviceStorage.kt`, `withAbout`). An invented byte count is worse than no
byte count: 清除缓存 next to it claims to free something nobody measured.

**Acceptance Scenarios**:

1. **Given** the 存储 page, **Then** each row's size comes from the store's own
   keys, and 清除 removes exactly those keys.
2. **Given** 关于, **Then** the version and commit are the running build's.

---

### User Story 7 — The dead controls do what they draw (Priority: P2)

The contact page's 转账 / 收款 / 二维码, every avatar's identicon viewer, the
hero's status line, and a transaction's 删除.

**Why this priority**: 057 left eleven; this cut closes the ones with a
precedent and records the rest with a reason. Each has a design already, on
Android or on the web — **no screen in this cut is invented**:

| Row | Precedent |
|---|---|
| 5 · contact 转账 / 收款 / 二维码 | Android `contacts.action.*` in `VelaNavHost` |
| 10 · identicon viewer from every avatar | Android's `LocalIdenticonViewer` — the avatar component itself opens it |
| 12 · hero status line → rescue | Android and web agree on the rule: a failed chain opens its RPC fix, anything else the balance breakdown |
| 28 · activity 删除 | web deletes from the **transaction detail**, not by swiping a row (`deleteSelectedTx`); Android has the controller method and no caller either |
| 7 · 新建 / 编辑分组 | Android `GroupSheets.kt` |
| 33 · 存储 · 34 · 关于 | Android, above |
| 39 · haptics · 40 · crash report | Android `VelaHaptic.kt`, `CrashReport.kt` / `CrashSheet.kt` |

Two have no precedent anywhere, and neither wants one:

- **38 · native-coin token tab (t3b)** is dead on **Android too** — both live
  builders hard-code the ERC-20 tab, because what the native tab describes is
  adding a NETWORK, which `network_admin`'s wizard already does properly. The
  recommendation is to delete the drawn tab, which is the founder's call.
- **37 · native transaction detail (a3)** is reachable in substance: the live
  detail renders both directions from the feed. What a3 draws and the live
  sheet omits is the 代币合约 row, and that is a data limit
  (`LocalTransaction` stores a symbol, not a contract) on all four clients.

---

### Edge Cases

- A language whose catalog fails to load must fall back to English cleanly
  rather than render half a screen.
- A token 收款 on a chain with no explorer must not offer 在区块浏览器中查看.
- Deleting the open transaction must leave the detail sheet with nothing to
  show — it steps back to the list, as the web's does.
- The insecure sentence must not appear for `file://` or the in-app demo page.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The Release configuration MUST compile. A generic class in the app
  target MUST declare `nonisolated deinit`, and a guard MUST enforce it.
- **FR-002**: The send receipt MUST distinguish preparing, signing and
  submitting, and MUST offer cancel while the passkey ceremony is up.
- **FR-003**: A receipt's `hold_reason` MUST be shown, in the core's own words.
- **FR-004**: A receive code MUST be able to name one asset, with its contract.
- **FR-005**: An insecure origin MUST be described as insecure; a consent sheet
  MUST name the host asking.
- **FR-006**: The stored language MUST decide the app's language at launch and
  MUST take effect without relaunch.
- **FR-007**: 存储 MUST report the store's real sizes; 关于 MUST report the
  build's real version and commit.
- **FR-008**: The contact page's three action cards, every avatar's viewer, the
  hero status line and a transaction's 删除 MUST each do what they draw.
- **FR-009**: No new screen may be invented. Every ported interaction MUST name
  its precedent on Android or the web.
- **FR-010**: Zero lines under `rust/crates/vela-core/src/app/`, and zero corpus
  keys added — as in every cut of this program.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `xcodebuild -configuration Release -destination
  'generic/platform=iOS'` succeeds. **The Archive blocker is gone.**
- **SC-002**: `node app-ios/scripts/check-generic-class-deinit.mjs` exits 0, and
  exits 1 for a generic class without the declaration.
- **SC-003**: The copy ruler's Android-only count for `send`, `receive`,
  `explore`, `connect` and `settings` is zero, or every remaining key has a
  reason in results.md.
- **SC-004**: The eleven dead affordances are each live, deleted, or recorded
  with a reason naming who else does or does not have it.
- **SC-005**: A language picked on the phone changes the phone — verified on
  the iPhone, not in a test.
- **SC-006**: The Android device is read for each of the four areas and the
  screenshots are the evidence, not a claim.

## Assumptions

- The founder's iPhone 11 remains the device of record for iOS; the Xiaomi
  alioth `9d5f42fb` is the Android reference. The Android device is signed in
  to a **real** account with real money, so every comparison on it is read-only.
- `payment_request`, `activity_feed` and the preferences machines are unchanged;
  this cut is a shell cut, as 050–057 were.
- Anything requiring a fresh design would be drawn in HTML for the founder
  first. **Nothing in this cut does** — that finding is itself a deliverable.
