# Results — 057 iOS Founder Pass

## Baselines, and the program's

| | at 050's start | at 057's start | now |
|---|---|---|---|
| hermetic test run | 356 | 558 in 72 suites | **564 in 74 suites** |
| XCUITest methods | 15 | 35 | 35 |
| literal-audit violations | 35 | 35 | **35** |
| `vela_core_uniffi.swift` bytes | 353,772 | 353,772 | **untouched since 051's merge of main** |
| event-parity strong diffs | — | 2 | **2** |

## What this cut did

**Login recovery's shell half.** A refused answer becomes the machine's OWN
failure, exactly once (contract 048 §2). Before it, `CoreDriver` reported the
fault and stopped — the machine had asked a question and was still waiting, so
the session sat in `restoring` forever, which a person sees as a wallet that
will not open. And `AccountStore` now tells "cannot be read" from "there is
none": one sends somebody to create a wallet they already have, the other is a
fault they can act on (the ANDROID-8 hardening).

**The avatar style was stored and read by nothing** — the same defect Android
049 found, and the reason its inventory has a "Shows at" column. 首字母 draws
the first character of the NAME, never of an address: "0x" is every address's
initial and a wall of identical circles is worse than no choice.

**Four dead controls**, from the forty-row audit: the contact page's three
action cards, the token detail's 转账, and the picker's class chips.

**One ambiguity a test caught.** xDAI is a stablecoin and Gnosis's gas coin, so
it matched two chips. Gas wins, and the test now asserts the three classes are a
partition — which is what makes a filter mean anything.

## Success criteria

| | claim | verdict |
|---|---|---|
| SC-001 | a record written by the retired client still opens the wallet | **test-only** — `SessionOldShapeTests`, with and without a `keys` array |
| SC-002 | an unreadable store is not read as an empty one | **test-only** — the same file |
| SC-003 | the forty affordances are each live, or recorded with a reason | **done** — `contracts/ios-affordances.md` carries the state of each; eleven remain dead and every one is named below |
| SC-004 | the nineteen settings rows each show where they are meant to | **half** — `contracts/settings-inventory.md`; 语言's live change, 存储 and 关于 are owed |
| SC-005 | the founder's own passkey sends on the phone | **not done — needs the founder's finger** |
| SC-006 | the whole chain pushes as one branch, one PR | see below |

**Device evidence:** `/tmp/founder.xcresult` — five tests pass on the iPhone 11
(`00008030-001A75961445802E`): the contact saved-and-deleted round trip, the
payroll paste, the sweep pick, and both scanner cases.

## Still dead, and why

| Row | Why |
|---|---|
| 7 · 新建分组 / 编辑分组 | No drawn form. `contacts::GroupSave` is one of the two remaining parity diffs, and the founder's draw-first ruling forbids inventing one. |
| 12 · hero status line → rescue | SR2 / SR3 / SR4 are drawn and unreachable. Needs a decision about which line raises which sheet. |
| 26 · asset-limited receive (r3) | Drawn, unreachable. |
| 28 · activity row swipe 删除 | No swipe on a feed row. |
| 33 · 存储 | Drawn numbers, not real bytes. |
| 34 · 关于 | Drawn, not from the build. |
| 37 · native transaction detail (a3) | Drawn, unreachable. |
| 38 · native-coin token tab (t3b) | Drawn, unreachable — it adds a NETWORK, which is `network_admin`'s wizard. |
| 39 · haptics | No policy written. |
| 40 · crash report | Absent. |
| 10 · identicon viewer | Opens from the wallet header only; eleven other avatars do not raise it. |

## What needs the founder

1. **A real passkey send** (SC-005). Every other transfer in this program went
   through the parallel space's fixed keyset; one with the founder's own key,
   on their own phone, is the last proof and it needs a finger on a sensor.
2. **Tap 允许 on the camera prompt once** (055), so the viewfinder can run.
3. **Hold a receive code up to the lens** (055 SC-005's other half).
4. **A live two-row split** (054 SC-001) — it spends money.
5. **The export share sheet and the document picker** (054 SC-008/SC-009).

## The program, 050 → 057

| Cut | What it did |
|---|---|
| 050 | contacts · network_admin · display_currency |
| 051 | the read path: pool, balances, feed, trust, tokens, receive-watch |
| 052 | money: send, fee_policy, tx_tracker, and the parallel space |
| 053 | the browser and signing: six machines, a real `WKWebView`, EIP-1193 |
| 054 | split, sweep, the payroll importer, and the address book's write half |
| 055 | simulation, message depth, the camera |
| 056 | preferences and formats, the settings page, deep links, two rulers |
| 057 | login recovery, the audit, the inventory |

**Zero lines under `rust/crates/vela-core/src/app/` across all seven.** Zero
corpus delta. Zero lines in the other four clients.

`vela_core_uniffi.swift` is **untouched since `6e693b26`** — 051's merge of
`main`, which regenerated it — and is 353,772 bytes now. So: unchanged across
052–057, the six cuts this branch chain carries. 051 itself did regenerate it,
through `rust/scripts/build-ios-xcframework.sh`, which is the only sanctioned way.

## Recorded for the PR

1. **Release archives crash `EarlyPerfInliner`** (Swift 6.2.4) on
   `CoreStore.deinit` — pre-existing since 051, measured with `-Onone`. This is
   an App Store blocker and needs a toolchain decision.
2. **VND cannot be priced** and the mainnet BNB/USD feed is dead (051's two
   open questions, unchanged).
3. **Two parity diffs and 21 unread view fields**, each with a reason in 056's
   results — the honest residue of seven cuts.
