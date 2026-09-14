# Results — 058 iOS ↔ Android Parity

## The founder's four questions, answered

| | Is the iPhone the same as the Android? |
|---|---|
| **转账** | It is now. It was not: one sentence covered preparing, signing and submitting, the ceremony could not be cancelled from the receipt, and a payment **held because fees rose above what was approved** said nothing at all — `hold_reason` was on this client's wire and read by no Swift. |
| **收款** | It is now. R3 — a code for ONE asset, with its contract — has been drawn since 021 and unreachable since; Android has had it since 048. Both phones still lack the amount-carrying request, and **so does Android**: its `receiveMode` / `receiveAmount` have no callers either. |
| **dApp browser** | It was already ahead on machines (`favorite_renamed`, `delete_origin` are iOS-only). Two anti-phishing sentences were missing: an http page is now called insecure, and a site asking to connect is named in the title. |
| **设置** | **The 语言 row did nothing. Ever.** Not "until relaunch" — `vela.language` was written by the settings page and read by no Swift at any point. Fixed, and the app changes language without a relaunch. 存储 and 关于 are measured now instead of drawn. |

## And the compiler

`xcodebuild -configuration Release -destination 'generic/platform=iOS'` →
**BUILD SUCCEEDED**. The Archive blocker is gone.

It was not `CoreStore`'s fault. Three lines reproduce it:

```sh
echo 'final class Box<T> { var value: T; init(_ v: T) { self.value = v } }' > /tmp/r.swift
xcrun swift-frontend -frontend -c /tmp/r.swift -O -swift-version 5 \
  -default-isolation=MainActor -module-name R \
  -target arm64-apple-ios17.4-simulator \
  -sdk "$(xcrun --sdk iphonesimulator --show-sdk-path)" -o /tmp/r.o
```

Any **generic class**, under this target's `-default-isolation MainActor`, at
`-O`: SE-0371 makes the deinit isolated, and inlining the destroying destructor
into the deallocating one walks off the end of
`isCallerAndCalleeLayoutConstraintsCompatible`. Adding an empty `deinit` to
`CoreStore` moved the crash to `UsbPromptsBridge.Box` — which is how the module
turned out to contain exactly three generic classes, one of them in the
Debug-only `Dev/` folder. `nonisolated deinit` restores the pre-6.2 destructor,
which is what both classes always had.

Debug builds at `-Onone`, so the inliner never ran and this was invisible from
051 until somebody tried to archive. `app-ios/scripts/check-generic-class-deinit.mjs`
is the guard; it fails for a generic class without the declaration.

**No toolchain decision is needed.** 057 recorded this as needing one.

## Baselines

| | at 057's end | now |
|---|---|---|
| hermetic tests | 564 in 74 suites | **581 in 75 suites** |
| Release / Archive build | **crashes the compiler** | **succeeds** |
| Android-only Event variants | 8 | **6** |
| Android-only corpus keys | 27 | **19** |
| view fields carried and never read | 21 | **20** |
| web-only (strong) event diffs | 2 | **1** |
| `check-native-reachability.mjs` | **red** (three iOS folders) | green |
| literal-audit violations | 35 | 35 |
| `vela_core_uniffi.swift` | untouched since 051 | **untouched** |
| lines under `rust/crates/vela-core/src/app/` | — | **zero** |
| corpus keys added | — | **zero** |

## What this cut did

**语言.** `Loc` was constructed once from `Locale.preferredLanguages`. `apply`
reads the stored choice at launch and re-resolves on a pick, and `t()` reads
`resolvedLanguage`, so every translated view depends on the language and
redraws. A pinned `VELA_LANG` still wins — found on a simulator the moment it
landed, because the storage page came up in English under `VELA_LANG=zh`.

**转账.** Three sentences for three states; 取消 while the passkey prompt is up,
which does **not** navigate (the prompt has to be answered where it is);
`hold_reason` in the core's own words on both the held and the rejected path.

**收款.** The asset goes through `payment_request` (`asset_picked`, so the core
re-clamps the request's precision), and the sheet names the coin, marks the code
with it and prints the contract. A network row's QR tells the machine the
chain's own coin, so the mark and the machine cannot disagree.

**Copies that copy.** The receive code's address and contract showed a checkmark
and left the clipboard as it was — and so did **every fact row in the app**: the
counterparty, the transaction hash, a token's contract. `FactRowModel` carries
`copyValue` now (the whole address, never the ellipsed one), as Android's has
since 043, and every copy goes through `velaCopy`.

**删除记录** on the open transaction, where the web puts it — not a swipe. It was
dead on all three clients: the web draws the button and never sets its label,
Android's `deleteActivity` has no caller. `history.deleteRecord` was in the
corpus the whole time.

**Every avatar opens the viewer**, through an environment opener the avatar
itself reads (Android's `LocalIdenticonViewer`). Threading a callback through
twelve call sites is how it came to open from one.

**存储 · 关于.** `DeviceStorage` is the port of Android's: the page weighs the
store's own keys and each row's 清除 removes exactly those. 关于 reads the
running build — and the commit is a **build setting**, because
`ENABLE_USER_SCRIPT_SANDBOXING` forbids a build phase from reading `.git`:

```sh
xcodebuild archive … VELA_GIT_COMMIT=$(git rev-parse --short HEAD)
```

With none it says `build 1`, which is true and checkable, rather than a hash
nobody can look up.

**The hero's status line** opens the RPC fix for a chain that failed and the
breakdown for anything else — Android's rule and the web's, and not arbitrary:
rate limiting resolves itself, and offering "swap in your own RPC" there would
offer a fix for a problem that is not the person's (invariant ④). Both sheets
are **live**, because opening a drawn one would have been a new lie.

**分组.** `GroupSave` — one of 056's two parity diffs — has a form: the
platform's own prompt, the shape this client's explore tab already uses for
新建分组. The groups header said 管理 and led nowhere; it says 新建分组 now.

**An http dApp is called insecure.** 056 recorded that no corpus sentence
existed. `connect.browser.a11yInsecure` does, translated into all fifteen
languages, and Android has used it since 044. The copy ruler found it.

## Success criteria

| | claim | verdict |
|---|---|---|
| SC-001 | Release builds | **done** — `BUILD SUCCEEDED`, device destination, code signing off |
| SC-002 | the guard catches the next generic class | **done** — exits 1 for a probe class, 0 now |
| SC-003 | no Android-only copy in the four areas | **done for receive, explore, settings and connect** (0 each); `send` keeps two and `contacts` two — named below |
| SC-004 | the eleven dead affordances | **seven live, two recorded, two owed** — table below |
| SC-005 | a language picked on the phone changes the phone | **done on the iPhone 11** — a stored `ja` renders the whole app in Japanese, `de` in German. A person TAPPING the row is simulator-and-test only |
| SC-006 | the Android device read for each area | **done** — three screenshots, read-only |

## The eleven, after

| Row | State |
|---|---|
| 7 · 新建 / 编辑分组 | **live** |
| 10 · identicon viewer from every avatar | **live** |
| 12 · hero status line → rescue | **live**, both sheets live with it |
| 26 · asset-limited receive (r3) | **live** |
| 28 · activity 删除 | **live on iOS**, from the detail. Owed on web (set `deleteLabel`) and Android (call `deleteActivity`) |
| 33 · 存储 | **live** — measured |
| 34 · 关于 | **live** — the running build |
| 37 · native transaction detail (a3) | **recorded**: reachable in substance — the live detail renders both directions. What a3 draws and live omits is the 代币合约 row, and no client can fill it (`LocalTransaction` stores a symbol, not a contract) |
| 38 · native-coin token tab (t3b) | **recorded**: dead on **Android too** — both live builders hard-code the ERC-20 tab. What it describes is adding a NETWORK, which `network_admin`'s wizard does properly. Recommend deleting the drawn tab; that is the founder's call |
| 39 · haptics | **half** — the five-name vocabulary exists (`VelaHaptics.swift`) and every copy uses it. The four pre-existing call sites are not yet converted |
| 40 · crash report | **owed** — Android's `CrashReport.kt` / `CrashSheet.kt` have no iOS counterpart |

## Design work: none was needed

The founder's instruction was to draw HTML for anything web and Android both
lack. **Nothing in this cut needed it.** Every interaction had a precedent —
`contracts/dead-affordances.md` names each one — and the two rows with no
precedent (37, 38) want a decision rather than a drawing.

## What needs the founder

1. **A real passkey send on the iPhone**, still owed from 057 (SC-005 there).
2. **語言 on the device**: pick 日本語 and see the app change. Verified on a
   simulator; the founder's phone is the record.
3. **A per-item 清除 has no confirmation.** Matched to Android deliberately —
   it clears on the tap, no second question — but "Contacts and groups · Clear"
   removes the address book in one touch. Say whether that wants a gate; a
   confirm sheet would be new UI, so it is not built here.
4. **t3b**: delete the drawn native-coin tab, or keep it as a picture.
5. **Row 28 on the other two clients.** One line each, but they are not this
   branch's tree: web `TxDetail` needs `deleteLabel` set, Android's
   `deleteActivity` needs a caller.

## The honest residue

**Six Android-only Event variants**, and half of them are dead on Android:

| Machine | Variants | Why |
|---|---|---|
| `payment_request` | `mode_changed`, `amount_changed`, `acknowledge` | **dead on Android too** — `WalletController.receiveMode` / `receiveAmount` / `acknowledgeReceive` have no callers. Neither phone has the amount-carrying request; neither is behind the other |
| `contacts` | `add_group_members`, `remove_group_member` | iOS replaces the whole membership (`set_group_members`) from the member picker, which is what that sheet asks about. The incremental pair is Android's own door |
| `batch_import` | `set_fiat_code` | the payroll importer's currency picker. Owed |

**Nineteen Android-only corpus keys.** Ten are Android's own platform prompts
(OTG, location permission, the USB key sheets) and have no iPhone meaning. The
rest: `send.splitTotalLabel` and `send.batchImportFile`;
`contacts.moveGroup`; `addToken.labelDecimals` and `addToken.notFoundTitle`;
`onboarding.login.successTitle` / `successMessage`;
`componentsUi.scanner.errorImage`; `home.cancel`.

**Eight iOS-only Event variants** — the reverse debt, and real: Android's
`FeeEvent` dispatches 2 of 6, so **the iPhone picks its fee coin and the
Android cannot**. That belongs to the 04x chain.

**Twenty view fields carried and never read.** The ruler has a blind spot worth
writing down: it sweeps top-level view fields only, so a NESTED one is
invisible to it — `hold_reason` lives on `SendReceiptView` and this cut found it
by reading call sites, not by the ruler.

**A shared gap neither phone has:** the groups header, and so 新建分组, appears
only when a group already exists — on **both** clients. A person with none
cannot make their first one. Not fixed here (showing an empty section is a
drawing decision).

## Device evidence

**On the founder's iPhone 11** (`ABC`, `00008030-001A75961445802E`), Debug
installed and driven by `VelaWalletUITests/DeviceParityTests` — four tests, all
passing, screenshots attached to the `.xcresult`. Nothing spent, nothing
erased, and nothing written to the founder's own preferences: the language
cases go through the ARGUMENT domain (`-vela.language ja`), which outranks the
stored value without persisting one.

| What | What the phone showed |
|---|---|
| 存储 | **102 KB · 共 158 条记录** — 交易记录 154 条 · 99 KB, 联系人与分组 0 位 · 67 B, 自定义代币与网络 1 项 · 204 B, 浏览记录 1 条 · 529 B, 余额与代币缓存 96 B, 汇率缓存 1 KB. Each row its own 清除. The drawing said 2.4 MB · 216 records on every phone |
| 关于 | the running build, not `v1.0.0 (6ab8f)` |
| 语言 | a stored `ja` put the **whole app** into Japanese — 設定 / 言語 · 日本語 / 外観 / 通貨 CNY · ¥8,284.02 / 数値の形式 — and a stored `de` into German. This is the row that did nothing at all before |
| 通讯录 | **the empty state**, which proves the shared gap: with no contacts the groups section is not drawn, so 新建分组 cannot be reached on either phone |

Read-only on the Xiaomi `9d5f42fb`, signed in to the founder's real account, so
nothing was tapped that spends: the contacts home with 新建分组 in its header,
the receive network list, and Optimism's code sheet.

On a booted iPhone 15 Pro simulator, on the way: the storage page at 131 KB ·
204 records, and 关于 reading v1.0 (build 1) in Chinese — which is what caught
the `VELA_LANG` regression.

**Still owed on the device:** a real passkey send (057's SC-005), and the
screens that need a wallet — the receipt's three stages, a token's receive
code, the hero's status line. They need a session, and a session needs a
finger.
