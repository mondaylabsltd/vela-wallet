# Data Model: Contacts UI fixtures (canonical)

**Branch**: `018-contacts-ui` · **Date**: 2026-08-09

This file is the single canon (research.md D7). Each platform ports it
verbatim: `feature/contacts/ContactsFixtures.kt` ·
`Features/Contacts/ContactsFixtures.swift` ·
`src/lib/contacts/fixtures.ts` · `src/contacts/fixtures.rs`.
Display models only — pre-grouped, pre-formatted, no service types.
All i18n-able labels resolve through the keys in
[contracts/i18n-keys.md](contracts/i18n-keys.md); everything below that
is quoted (names, addresses, amounts, dates) is **data, verbatim across
locales** (spec FR-012).

## Contacts (8 位 — the canonical roster)

Identicon seed = the full address (through `normalize_seed`, never
call-site lowercased). Only Alice's full address appears in a mock (C2);
the other seven are **pinned inventions whose first/last four hex chars
match the mock's truncated display** (research.md D7 — identicon
artwork therefore won't pixel-match the mock renders for those seven;
cross-platform parity is what counts).

| # | Section | Name | Display address | Full address (seed) |
|---|---------|------|-----------------|---------------------|
| 1 | A | Alice | `0x9F3c…21aE` | `0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE` |
| 2 | A | 阿豪 | `0x77Bd…4F02` | `0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02` |
| 3 | B | Bartholomew Vanderbilt-Konstantinopoulos.eth | `0x31c9…E77a` | `0x31c9A100517d2436E9E1350D383A7d0aAeC1E77a` |
| 4 | B | Bob · 泵泵 | `0x44Aa…9C21` | `0x44AaF19cE84f22101b5D6cbA918B92DcA5f19C21` |
| 5 | C | Charlie | `0x5eF0…3a9C` | `0x5eF0FF25a1A24E5cCb2a6D939B87F5DAb2003a9C` |
| 6 | D | DAO 金库 | `0xF00d…C0de` | `0xF00dBaBe8712004343cD00926Ab004D6C042C0de` |
| 7 | H | hold on | `0xCafe…F00d` | `0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d` |
| 8 | M | 妈妈 | `0x88Ce…12aB` | `0x88Ce02FdB0e50D9C21e33c0F9B58a3E38f7612aB` |

Group-only member (spec Assumptions — the recorded mock inconsistency):

| Name | Display address | Full address (seed) |
|------|-----------------|---------------------|
| 表弟 | `0xA1c3…88dD` | `0xA1c3D3d7085B90AF14E5d21C86e6dB49F30a88dD` |

Letter sections present: **A, B, C, D, H, M** (in that order). The
index rail always renders the full A–Z alphabet; letters without a
section jump to the nearest existing section.

Row anatomy: identicon (row size) · name (truncates single-line — #3 is
the truncation exerciser) · display address in mono under the name.

## Groups

| Group | Count label | Members (ordered) |
|-------|-------------|-------------------|
| 家人 | 3 人 / 3 位成员 | 妈妈, 表弟, Alice |
| 工作 | 5 人 | *(never opened in mocks — count only)* |
| 交易所 | 2 人 | *(count only)* |

Desktop rail: 全部联系人 `8` (selected in dc1) → 分组 label → 家人 3 ·
工作 5 · 交易所 2 → 新建分组 (folder-plus). Empty state (dc3): 全部联系人
`0` + 新建分组 only.

## Contact detail (Alice — c2 / dc2)

- Hero identicon (**64 mobile / 48 desktop** — measured from C2 and DC2 on
  2026-08-09; the earlier ~96/~64 estimate in this file was wrong and all
  four platforms were re-aligned to the measured values), name **Alice**, short
  address `0x9F3c…21aE` (mobile only), chips: `家人` + `+ 分组`.
- Actions: 转账 only (card mobile, pill desktop; issue #479 retired 收款 / 二维码).
- 地址 block, mono, mobile wraps as two lines exactly:
  `0x9F3cA71b04E82f5C55d9` / `B21aE00734F8Dd8021aE`; desktop one line;
  trailing copy affordance.
- 最近往来 (mobile trailing action: 全部 › ; desktop trailing link
  below rows: 查看全部往来):

| Kind | Title | Subtitle | Amount | Color |
|------|-------|----------|--------|-------|
| received | 已收到 | 昨天 20:15 · Ethereum | +50 USDC | success |
| sent | 已发送 | 8 月 5 日 · Arbitrum | −0.2 ETH | fg |

(The received entry mirrors the 015 wallet fixture's Alice row — one
story across features. Chain badge colors reuse the 015 chain palette:
Ethereum blue, Arbitrum blue.)
- Mobile footer: centered destructive 删除联系人. Desktop footer:
  编辑 (pencil) left · 删除联系人 (destructive) right.

## Menus (MenuFixture)

| Menu | Kind | Items (icon · label key · destructive) |
|------|------|----------------------------------------|
| addMenu (c5) | sheet | user-round-plus · `contacts.addTitle` · — / download · `contacts.importFile` · — / upload · `contacts.exportTitle` · — |
| groupMenuMobile (c6) | sheet | pencil · `contacts.groupEdit` / download · `contacts.importGroup` / upload · `contacts.exportGroup` / **divider** / trash-2 · `contacts.groupDelete` · ✔destructive |
| headerDropdown (dc5, M1) | dropdown | download · `contacts.importAll` / upload · `contacts.exportAll` |
| groupContext (dc6, M2) | context | pencil · `contacts.groupRename` / download · `contacts.importGroup` / upload · `contacts.exportGroup` / **divider** / trash-2 · `contacts.groupDelete` · ✔destructive |
| contactContext (desktop SPEC, component board only) | context | `componentsUi.dock.send` / `componentsUi.dock.receive` / `componentsUi.identiconViewer.copyAddress` / `contacts.edit` / `contacts.moveGroup` / **divider** / `contacts.delete` · ✔destructive |
| deleteConfirm (c2s) | sheet | title `contacts.deleteTitle`, body `contacts.deleteBody` ({{name}} = Alice), confirm `contacts.delete` (destructive), cancel `contacts.cancel` |

Sheets end with a separate 取消 (`contacts.cancel`) button; dropdown/
context menus have none (outside-click/Esc dismisses).

## Screen states (ContactsStateId — the gallery inventory)

**Mobile** (`MOBILE_STATES`, rendered in the 390×844 frame):

| Id | State | Notes |
|----|-------|-------|
| `c1` | default list | groups + 8 contacts, sections A…M, index rail, tab bar 通讯录 selected |
| `c1s` | swipe revealed | row 阿豪 offset left, 转账 (accent) + 删除 (error) actions visible |
| `c1f` | search active | query `Ali`, list filtered to Alice (section A only), clear affordance visible |
| `c2` | contact detail | Alice, per §Contact detail |
| `c2s` | delete confirm | deleteConfirm sheet over dimmed c2 |
| `c3` | empty | EmptyState + accent 添加联系人 + outline 从文件导入 |
| `c4` | group detail | 家人 · 3 位成员, member rows, ghost 添加成员, pinned 群发转账 + `contacts.batchSendHint` ({{count}}=3) |
| `c5` | add menu | addMenu sheet over dimmed c1 |
| `c6` | group menu | groupMenuMobile sheet over dimmed c4 |

**Desktop** (`DESKTOP_STATES`, ≥1280 stage; `dc2n` in a 1024 stage):

| Id | State | Notes |
|----|-------|-------|
| `dc1` | two columns | rail 全部联系人 8 selected; full list; header search ⌘F + 添加联系人 + ⋯ |
| `dc2` | third column | Alice detail panel (title `contacts.sectionContacts`), list row Alice selected |
| `dc3` | empty | rail 0-state, centered EmptyState with both CTAs |
| `dc4` | group view | header 家人 3 位成员 + accent 群发转账 + ⋯; members; ghost row; caption `contacts.batchSendHintTitled` ({{count}}=3) |
| `dc5` | header menu | headerDropdown anchored under ⋯ |
| `dc6` | group context | groupContext anchored at 家人 row |
| `dc2n` | narrow overlay | dc2 content; third column as overlay + scrim (web live <1120; native desktop N/A — research D6) |

## Component boards (gallery root, per platform)

ContactRow (default · hover · selected · swipe-revealed · member) ·
GroupRow · GroupRail rows (default · selected · hover · drop-target) ·
AlphaIndexRail (idle · bubble-HUD static) · SearchField (idle ·
focused/filtering) · ActionMenuSheet (add · group · delete-confirm) ·
DropdownMenu · ContextMenu (group · contact) · GroupChips · AddressBlock
(mobile 2-line · desktop 1-line) · RecentActivity · EmptyStateCTA (empty
· search-empty via `contacts.noResults` {{query}}=`zzz`) · GhostAddRow ·
PinnedCTABar · identicon board (9 canon seeds + `""` placeholder).

Stable gallery ids (web e2e / screenshots): `gallery-section-contacts-*`
and `gallery-contacts-<component>-<variant>`, same scheme as 015.

## Model shapes (normative names; platform naming may adapt)

```
ContactFixture   { name, addressDisplay, addressFull, sectionKey,
                   groups: [name], activity?: [ActivityFixture] }
GroupFixture     { name, countLabel, membersLabel?, members?: [ContactFixture] }
ContactsPage     { searchPlaceholderKey, query?, groups, totalLabel,
                   sections: [{ letter, contacts }], empty: bool,
                   revealedIndex?, tab: 'contacts' }
MenuFixture      { kind: sheet|dropdown|context, items: [{icon, labelKey,
                   destructive?}], dividersAfter: [ix], anchor }
DetailFixture    { contact, chips, actions, addressLines, activity,
                   footer: mobile|desktop }
PanelFixture     { content: none|contactDetail, narrowOverlay: bool }
```

Builders take `(stateId, messages, identicon)` — messages and identicon
injected so tests can stub (015 convention). Fixture tests pin: state-id
inventory exactly as listed above; zh copy verbatim (群发转账,
向本组 3 人转账，金额可分别设置。, 还没有联系人, 8 位, 3 位成员 …); the
8+1 canon addresses byte-exact.
