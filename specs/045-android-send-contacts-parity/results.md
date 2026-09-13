# Results — 045 Android Send Parity, Batch and Contacts Parity

**Branch**: `045-android-send-contacts-parity` (stacked on 044) · **Device**: Xiaomi
alioth `9d5f42fb`, parallel space, Gnosis, Safe `0x88cC…6894`.

## What landed, phase by phase

| Phase | Delivered | Device evidence |
| --- | --- | --- |
| 0 wires | `BatchImportCore` bridged; `BatchWire` (10/3/6 + view) drift-gated; `ContactsWire` + 7 events, 3 view fields, 5 types; smoke opens the batch core | installed, launched |
| 1 split | `SplitRows` (whole-list `RecipientsChanged`, ids/names kept, empty id minted by the core); editable row cards; total from `confirm_amount`; confirm names the count + every person; receipt lists the parts, title = the sum | SC-001: XDAI 0.001 → founder + 0.001 → Safe, ONE operation, receipt `0x914b5efe…ad994b`, feed one row −0.002 |
| 2 sweep pick | `SweepPick` (first tick pins via `SetMultiNetwork`, dimmed row = no event, last clear unpins, select-all pins first visible valuable chain); live pick ticks/greying/notice/counting CTA; sweep form from `multi_specs` | SC-002: XDAI ticked → 「已选 Gnosis …」+ 「发送 1 个 · Gnosis →」; clear → unpinned; Back leaves the pick. Only one token held on Gnosis, so greying is test-covered (`SweepPickTest`, `SendLiveTest`), the sweep send too |
| 3 batch | `BatchExecutor` (rate through the wallet's waterfall, never 1; picker CSV/TXT text, XLSX matrix via `XlsxMatrix`; creator for the template); `ActivityDocumentPorts` + FileProvider; sheet from the core's view, editable paste + rate; Apply seeds the split | SC-003: `two-rows.csv` picked in DocumentsUI, 「解析结果 · 2 条」, applied (names kept), sent as one op, receipt `0x0162c7ce…97aa56`; template 「模板已保存」 through the system creator; live rate 1 XDAI = 0.7394 GBP |
| 4 treasury/stale | 「暂不」beside the core's retry → `DismissTreasurySheet`; stale flip re-asks the last quote once per flip while confirm is open and idle | SC-004: two TTL expiries on the confirm page → two `send.fee re-quote on stale` log lines, quotes re-fetched, the confirm sent (`0xaa668113…eca784`). The treasury pause itself cannot be provoked on Gnosis (relay funded): test-covered |
| 5 drawn | `ContactFormSheet` (018 vocabulary), the star, the inspection row; gallery C7/C8/C9 | screenshots `p45-5-C7/C8/C9` |
| 6 contacts | form → `Save`; star → `ToggleFavorite`; export → share sheet → `ExportTaken`; import → picker → `ImportFile` → report/refusal sheet → `ImportAcknowledged`; `AddGroupMembers`/`RemoveGroupMember` (picker / swipe), `SetContactGroups` (detail chips); `InspectRecipient` on open (Gnosis) | SC-005: Founder saved, starred, renamed 「Founder 2」, kept across force-stop. SC-007: founder → 「钱包」(paid before, so no first-time line); USDC contract → 「合约」. SC-006: see below |

## SC-006 and groups on the device

- Export: ⋯ → 导出通讯录 → the system share sheet came up with
  `vela-contacts-2026-09-12.json` (5 contacts, `version`/`exportedAt`/
  `contacts`/`groups`), written by the core, handed by `ActivityDocumentPorts`.
- Import of that same file (pushed to Downloads, found through the picker's
  search): 「导入完成 · 新增 0 个，5 个已存在。」— existing wins, the book
  unchanged. A file with one new contact and a `Team` group (new member +
  the founder) created the group — the core creates groups only for NEWLY
  added members (`contact-io.ts:227-241` ported), so the founder was not
  in it until added by hand.
- Groups both ways: the group's member picker (the whole book, ticked)
  removed USDC (`RemoveGroupMember`) and added Founder 2
  (`AddGroupMembers`) — the group reads 「Team · 2 位成员 · Team Mate,
  Founder 2」; the contact's 「+ 分组」 picker toggled 「✓ Team」 off (chip
  gone) and on (chip back) — `SetContactGroups`.
- A refused file (`not json, not a table`) is test-covered
  (`ContactsMachineTest`): `import_failure` set, cleared on acknowledge.

## Gates

| Gate | Result |
| --- | --- |
| Unit suite (`testDebugUnitTest`) | 480 run, 0 failures (455 at the start of 045) |
| `CoreWireDriftTest` | batch family (events/ops/results/file content exhaustive, view subset) + contacts additions, green |
| `check-native-reachability.mjs` | green; `documents` exempted with its reason (an Activity-bound port, no screen) |
| `build-web.mjs --check` | current (wasm 3,734,673 bytes; the bridge object moved the fingerprint once) |
| Device `.so` arm64 | 19,297,248 bytes (+390,328 for `batch_import`; ceiling 19.5 MB holds) |

## Owed / not done here

- The sweep SEND on the device: the golden Safe holds one Gnosis token, so the
  multi-token operation is covered by tests, not the phone (SC-002 as specified).
- The treasury pause's 「暂不」 exit on the device: the relay's treasury is
  funded on Gnosis; the exit is test-covered (`SendLiveTest`), the core's
  `DismissTreasurySheet` unchanged.
- The batch's paste path on the device (typed text): the file path was driven;
  paste is the same core parse (`BridgeSmokeTest` pastes two rows).
- Groups are CREATED only by import on the phone (the manage/rename sheet is
  unwired, out of 045's scope); membership both ways is wired and verified.
- Recipient inspection asks Gnosis (the parallel space's chain) rather than
  the browser's chain.
- First-launch-after-install: the sweep door tapped before the balances had
  arrived once showed no rows for the wait's length; the plain pick and a
  second cold start showed rows in ~2 s. Not reproduced since; noted.

## Findings worth keeping

- The confirm page's CTA is a BUTTON, not a slider (earlier scripts "swiped"
  and it registered as a tap; a swipe along the label row does nothing).
- The core's split receipt keeps `amount` as the single-send scalar; a
  split's title must use `confirm_amount` (the SUM) — the phone did.
- DocumentsUI lists a pushed file only after a media scan; JSON pushed to
  Downloads was found through the picker's search.
