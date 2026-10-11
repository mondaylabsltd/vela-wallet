# Feature Specification: Contacts UI Components & Preview Galleries

**Feature Branch**: `018-contacts-ui`

**Created**: 2026-08-09

**Status**: Draft

**Input**: User description: "Implement the contacts (通讯录) UI from
`design/contacts/` on all four clients (`app-android/vela-wallet`,
`app-ios/VelaWallet`, `app-web/vela-wallet`, `app-desktop/vela-wallet`).
The ~20 mocks are compositions of a small set of reusable components —
build those components once per platform and assemble the screens from
them. No real business state or logic yet: every state is driven by
fixtures, and every state must be easy to see and test through the
preview gallery each platform already has (specs 014/015 mechanism).
On desktop the third column plays the mobile bottom-sheet role
(contact detail third column; header ⋯ dropdown; group-row context
menu). Avatars are the Nimiq identicon from vela-core."

## Why

Spec 015 built the wallet-home component vocabulary — balance, activity,
assets, sheet vs. third-column modality — and left 通讯录 as a selectable
but empty tab. Contacts is the second screen family users touch on every
transfer, and it is where the *list-management* vocabulary of the product
gets decided: alphabetically sectioned lists with an index rail, group
management, action menus (bottom sheet on mobile, dropdown and context
menu on desktop), destructive confirmation, and add/import/export entry
points. Building that vocabulary once per platform, state-complete and
fixture-driven, before any storage or import pipeline exists, means the
later "wire up real contacts data" feature is a data-plumbing change
rather than a UI rewrite — and reviewers can open every state today from
the same galleries spec 015 established.

Several pieces are deliberately *reused, not rebuilt*: the identicon
avatar, activity rows (a contact's 最近往来 is the same row family as
wallet activity), section headers, empty-state artwork, the mobile tab
bar, the desktop sidebar shell, and the desktop third column. This
feature adds the missing list-management components and assembles the
contacts screens from old plus new parts.

## Design Authority

The mocks in `design/contacts/` are the visual authority:

| Mock | State it defines |
|------|------------------|
| `C1 通讯录 _ 默认.png` | Mobile contacts home: search, 分组 section (管理 ›), A–Z sectioned contact list, index rail, tab bar with 通讯录 selected |
| `C2 通讯录 _ 联系人详情.png` | Mobile contact detail: identicon hero, group chips, 转账/收款/二维码 actions, full address + copy, 最近往来, red 删除联系人 |
| `C3 通讯录 _ 空 · 无联系人.png` | Mobile empty state: icon, title, caption, primary 添加联系人 + secondary 从文件导入 |
| `C4 通讯录 _ 分组详情.png` | Mobile group detail: title + member count, member rows, ghost 添加成员 row, pinned 群发转账 CTA with caption |
| `C5 通讯录 _ 添加与导入导出.png` | Mobile action sheet over C1: 新建联系人 / 从文件导入 / 导出通讯录 / 取消 |
| `C6 通讯录 _ 分组操作.png` | Mobile action sheet over C4: 编辑分组 / 导入到本组 / 导出本组 / 删除分组 (destructive) / 取消 |
| `DC1 … 桌面 · 两栏（默认）.png` | Desktop: sidebar (通讯录 selected) + group rail (全部联系人 8 / 家人 3 / 工作 5 / 交易所 2 / 新建分组) + sectioned list; header search ⌘F, 添加联系人 button, ⋯ button |
| `DC2 … 桌面 · 第三栏打开（联系人详情）.png` | Desktop third column: 联系人 title + ✕, identicon + name + group chip, pill actions, address + copy, 最近往来, 查看全部往来, footer 编辑 / 删除联系人 |
| `DC3 … 桌面 · 空 · 无联系人.png` | Desktop empty: rail shows 全部联系人 0 + 新建分组 only; centered empty state with both CTAs |
| `DC4 … 桌面 · 分组视图（家人）.png` | Desktop group view: content header 家人 3 位成员 + accent 群发转账 + ⋯; member rows; ghost 添加成员; caption line |
| `DC5 … 桌面 · 页头「⋯」菜单.png` | Header dropdown anchored to ⋯: 导入通讯录 / 导出全部通讯录 |
| `DC6 … 桌面 · 分组行右键菜单.png` | Group-row context menu: 重命名分组 / 导入到本组 / 导出本组 / — / 删除分组 (destructive) |
| `M1 菜单 _ 页头「⋯」下拉.png` | Dropdown menu component isolated (2 items) |
| `M2 菜单 _ 分组行右键.png` | Context menu component isolated (4 items, divider, destructive row) |
| `SPEC 动效 _ 通讯录 手机.png` | Mobile motion & interaction spec (index-rail bubble HUD, row swipe actions, sheet timing, reduced-motion, search focus) |
| `SPEC 动效 _ 通讯录 桌面.png` | Desktop motion & interaction spec (third-column 0→400 240ms, hover/selection, keyboard map, right-click, drag-to-group, <1120 overlay, import/export entries) |

`D1`/`H1` in the same folder are wallet-home references showing the
shells the contacts screens plug into; they are authoritative only for
shell consistency, not re-implemented here.

Where a mock conflicts with `design-system.md` tokens, the design system
wins and the difference is recorded in the feature's results notes (same
rule as specs 007/009/015). Mocks are dark-appearance; components MUST be
built from theme tokens so both appearances work, with dark expected to
match the mocks.

## The Reusable Component Vocabulary

The twenty mocks decompose into one shared vocabulary. Names are
normative for this spec (each platform keeps its own naming conventions,
but the mapping must be recorded). Components marked **[reuse]** already
exist from spec 015 and MUST be consumed, not duplicated.

1. **ContactRow** — IdenticonAvatar (seeded by address) + name
   (truncates) + secondary line with middle-truncated address. Variants:
   plain (mobile), `hover`/`selected` raised background (desktop),
   member context (identical anatomy; used in group detail). Mobile
   supports swipe-left to reveal 转账 / 删除 actions; 删除 is
   destructive and demands a second confirmation.
2. **GroupRow** (mobile) — leading rounded-square tile with group glyph,
   group name, trailing `N 人` + chevron.
3. **GroupRail** (desktop) — the second column: 全部联系人 row with
   trailing count and selected state, 分组 label, group rows (glyph,
   name, count), and a 新建分组 row with folder-plus glyph. Rows support
   `selected`, `hover`, `drop-target` (raised while a contact/file is
   dragged over) states and right-click → **ContextMenu**.
4. **AlphaSectionList** — contacts grouped under letter headers
   (uppercase letter + hairline). Sorting/grouping strings arrive
   pre-computed in fixtures.
5. **AlphaIndexRail** (mobile) — right-edge A–Z rail; touch/slide jumps
   to the letter's section and shows a letter-bubble HUD near the finger
   (fade-in 120ms / fade-out 80ms, ease-out), one selection haptic per
   letter crossed. Reduced-motion: no bubble animation, direct jump.
6. **SearchField** **[reuse pattern]** — mobile: full-width field
   (搜索名字、ENS 或地址) whose focus raises the keyboard and filters
   the list as typed, clear restores, cancel affordance per platform
   convention; desktop: header field with ⌘F badge (page-local search
   that coexists with the sidebar's global ⌘K).
7. **ActionMenuSheet** (mobile) — bottom sheet: drag handle, icon+label
   rows, optional divider + destructive row (red icon + label), and a
   separate 取消 button. Rise 250ms ease-out over scrim; reduced-motion
   swaps rise for a 150ms opacity crossfade. Hosts C5 and C6 content and
   the delete-confirmation variant.
8. **DropdownMenu** (desktop) — anchored below its trigger (header ⋯):
   raised surface, icon+label rows (M1).
9. **ContextMenu** (desktop) — right-click menu, same visual family as
   DropdownMenu plus divider and destructive row (M2). Used on group
   rows (DC6) and contact rows (转账 / 收款 / 复制地址 / 编辑 /
   移入分组 / 删除 per desktop SPEC).
10. **GroupChips** — on contact detail: one pill per group membership
    (家人) plus a trailing `+ 分组` add chip.
11. **ContactActions** — 转账 only (issue #479, 2026-10-10: 收款 opened the
    wallet's OWN receive screen and 二维码 did on iOS — neither is about the
    contact). Mobile: one full-width card (icon above label) — same anatomy
    as spec 015's ActionButtonRow; desktop: one pill button (icon + label
    inline). No 收款 in the contact row's context menu either. **[reuse]** the
    015 component with new items where the platform implementation
    allows.
12. **AddressBlock** — 地址 label + full address in mono (wraps to two
    lines on mobile) + trailing copy icon button.
13. **RecentActivity** **[reuse]** — SectionHeader (最近往来 + 全部 ›
    on mobile / plain label + 查看全部往来 link on desktop) over spec
    015 ActivityRow instances (已收到 +50 USDC in green, 已发送
    −0.2 ETH).
14. **EmptyState** **[reuse + extend]** — 015 EmptyState (outline icon
    tile, title, caption) extended with a CTA slot: primary accent
    添加联系人 + secondary outline 从文件导入 (stacked full-width on
    mobile, inline pair on desktop).
15. **DestructiveTextButton** — centered red text action (删除联系人)
    on mobile detail; on desktop the same action sits in the third
    column footer opposite 编辑.
16. **GhostAddRow** — dashed/muted leading circle with +, label
    (添加成员), non-raised.
17. **PinnedCTABar** (mobile) — bottom-pinned accent CTA (群发转账) with
    a caption line underneath (向本组 3 人转账，金额可分别设置。).
18. **PageHeader** — mobile: large title 通讯录 + trailing icon button
    (person-add); detail screens: back chevron + trailing pencil / ⋯.
    Desktop: content-column header with title, search field, accent-less
    添加联系人 button (person-add glyph) and ⋯ icon button.
19. **ThirdPanel** **[reuse]** — desktop third column (fixed width 400,
    open 240ms ease-out with content fade-in delayed 80ms/120ms, close
    200ms, Esc/✕ closes, content swap without column re-animation —
    150ms crossfade; below 1120px width it becomes an overlay with scrim
    instead of squeezing the list). This feature ships one new content:
    **ContactDetailPanel** (DC2).
20. **IdenticonAvatar** **[reuse]** — vela-core Nimiq identicon, seeded
    by the contact's address, circular crop; sizes: row (~40dp), detail
    hero (~96dp mobile / ~64dp desktop). Invalid/empty seed → shared
    placeholder artwork.
21. **TabBar** / **Sidebar** **[reuse]** — 通讯录 becomes the selected
    item (solid icon + accent on mobile; raised row + solid icon in the
    desktop sidebar). Other destinations unchanged.

## User Scenarios & Testing

### User Story 1 - Browse every mobile contacts state in a gallery (Priority: P1)

A developer or designer opens the platform gallery (Android, iOS, web at
mobile widths) and walks the mobile contacts states — default list,
contact detail, empty, group detail, add/import/export sheet, group
actions sheet, swipe-actions revealed, delete confirmation — each
rendered purely from fixtures and each visually matching its `C*` mock.

**Why this priority**: The state-complete contacts screens are the
deliverable; the gallery is how they are reviewed and regression-checked.
Everything else composes into it.

**Independent Test**: Open the gallery on one platform, walk all states,
compare side-by-side with the mocks.

**Acceptance Scenarios**:

1. **Given** the gallery, **when** each C1–C6 fixture state is selected,
   **then** the rendered screen reproduces the corresponding mock's
   structure and content verbatim (分组 家人 3人 / 工作 5人 / 交易所
   2人; 联系人 8 位; Alice `0x9F3c…21aE`; 阿豪; Bartholomew
   Vanderbilt-Konstantinopoulos.eth; Bob · 泵泵; Charlie; DAO 金库;
   hold on; 妈妈 `0x88Ce…12aB`; 表弟 `0xA1c3…88dD`; +50 USDC; −0.2 ETH).
2. **Given** the contact list (C1), **then** contacts render under
   letter section headers with the A–Z index rail present, and the 分组
   section shows 管理 › as a trailing action.
3. **Given** contact detail (C2), **then** the hero identicon, name,
   short address, group chip 家人 + `+ 分组` chip, the 转账 action
   card (issue #479), the full mono address with copy affordance, 最近往来 with two
   activity rows, and the red 删除联系人 action all render.
4. **Given** the empty state (C3), **then** the search field and header
   remain, and the empty component shows 还没有联系人, the caption, an
   accent 添加联系人 and an outline 从文件导入 button.
5. **Given** group detail (C4), **then** the title 家人 with 3 位成员,
   three member rows, the ghost 添加成员 row, and the pinned 群发转账
   CTA with its caption render.
6. **Given** the C5 state, **then** an action sheet with 新建联系人 /
   从文件导入 / 导出通讯录 and 取消 renders over the dimmed list; the
   C6 state renders 编辑分组 / 导入到本组 / 导出本组 / 删除分组 (red)
   over the dimmed group detail.
7. **Given** the swipe state, **then** a contact row shows the revealed
   转账 / 删除 actions; **given** the delete-confirmation state, **then**
   a destructive confirm sheet names the contact being removed.

---

### User Story 2 - Desktop two-column contacts with third-column detail and menus (Priority: P2)

On desktop the same vocabulary assembles into sidebar + group rail +
sectioned list. Clicking a contact opens the third column with the
contact detail; ✕ or Esc closes it; the header ⋯ opens a dropdown; a
group row right-click opens a context menu; selecting a group swaps the
list to that group's members with the 群发转账 header. The desktop
gallery exposes DC1–DC6 plus the narrow-window overlay state.

**Why this priority**: The third-column + menu patterns are the
desktop-defining interactions; they reuse everything US1 builds.

**Independent Test**: Launch the desktop gallery, walk DC1→DC6 and the
narrow state, compare with the mocks.

**Acceptance Scenarios**:

1. **Given** the default desktop state, **then** the layout matches DC1:
   sidebar with 通讯录 selected, group rail (全部联系人 8 selected, 家人
   3, 工作 5, 交易所 2, 新建分组), sectioned list, header with search
   (⌘F), 添加联系人, ⋯.
2. **Given** the detail state, **then** the third column matches DC2 —
   联系人 title + ✕, identicon + Alice + 家人 chip, three pill actions,
   full address + copy, 最近往来 rows (same ActivityRow component as the
   wallet), 查看全部往来, footer 编辑 + red 删除联系人 — while the list
   column narrows and stays interactive.
3. **Given** the empty state, **then** the rail shows 全部联系人 0 and
   新建分组 only, and the centered empty component matches DC3.
4. **Given** the group view, **then** the content header shows 家人
   3 位成员 with an accent 群发转账 button and ⋯, member rows, ghost
   添加成员 row and the caption line per DC4.
5. **Given** the header-menu state, **then** a dropdown anchored to ⋯
   lists 导入通讯录 / 导出全部通讯录 (DC5); **given** the group-menu
   state, **then** a context menu lists 重命名分组 / 导入到本组 /
   导出本组 / divider / red 删除分组 (DC6).
6. **Given** an open third column, **when** ✕ or Esc is activated,
   **then** the layout returns to DC1; **given** a viewport narrower
   than 1120px, **then** the third column renders as an overlay with
   scrim instead of squeezing the list.

---

### User Story 3 - Motion, input and accessibility behaviours from the SPEC sheets (Priority: P3)

The interaction details the two SPEC mocks define are implemented as
component behaviours (where they are static-state renderable they also
appear as gallery states; timing values are design tokens/constants,
not re-measured per screen).

**Why this priority**: These behaviours make the components feel native
per platform, but they layer onto structures US1/US2 already prove.

**Independent Test**: On a touch platform, slide along the index rail
and observe the bubble HUD + haptics; on desktop, verify keyboard map
and hover/selection treatments.

**Acceptance Scenarios**:

1. **Given** the mobile list, **when** the finger slides along the index
   rail, **then** the list jumps per letter, a bubble HUD appears near
   the finger (120ms in / 80ms out) and one selection haptic fires per
   crossed letter; with reduced motion the jump is direct and the bubble
   does not animate.
2. **Given** a contact row on mobile, **when** swiped left, **then**
   转账 / 删除 reveal in 250ms ease-out; a full swipe stands for 转账
   intent (visual only in this feature); 删除 always raises the
   destructive confirm.
3. **Given** desktop rows, **then** hover transitions to raised in
   ~120ms, selection applies the raised state immediately, and the
   selected contact is the one shown in the third column.
4. **Given** the desktop page, **then** ⌘F focuses page search, ↑/↓
   move through the list, Enter opens the detail column, Esc closes it,
   and the focus ring is the 2px accent treatment; Tab order is search →
   添加联系人 → group rail → list → third column.
5. **Given** reduced-motion on any platform, **then** sheet rise and
   third-column slide are replaced by opacity crossfades (150ms).

---

### User Story 4 - Localized, token-pure components (Priority: P3)

All user-visible strings go through the existing i18n capability (the
mocks are zh; en must also resolve), and all styling goes through theme
tokens. Switching locale or appearance in the gallery re-renders
correctly.

**Why this priority**: Same discipline specs 006–015 established;
cheaper to keep than to regain.

**Acceptance Scenarios**:

1. **Given** any gallery state, **when** locale switches zh ↔ en,
   **then** every string changes and no key leaks to screen (fixture
   names/addresses stay as data).
2. **Given** component source, **then** no hardcoded color/spacing/font
   values appear in page code (tokens only), matching each platform's
   established token layer.

### Edge Cases

- Name longer than the row (Bartholomew Vanderbilt-Konstantinopoulos.eth)
  — truncates with ellipsis on one line, never wraps, never pushes the
  address or trailing affordances out (C1/DC1 show the truncation).
- Full address on mobile detail wraps to exactly the mock's two-line
  mono block; the copy button stays vertically centered against it.
- Empty group (0 members) — group detail shows the ghost 添加成员 row
  and the pinned CTA disabled/caption adjusted (not mocked; gallery
  includes it as an extra fixture, treatment recorded in results).
- Contact with no activity — 最近往来 section shows the reused empty
  treatment instead of rows (not mocked; same rule).
- Contact in multiple groups — chips row wraps.
- Search with no matches — list area shows the search-empty treatment
  (not separately mocked; reuse EmptyState, record in results).
- Delete flows are visual only: confirm sheets render but "confirming"
  in the gallery just returns to the fixture state.
- Index rail letters with no section (fixture has no E/G…) — rail still
  renders the full alphabet; tapping a letter with no section jumps to
  the nearest existing section per platform convention.
- Reduced-motion set at the OS level — all entrance animations degrade
  to crossfades; galleries stay fully navigable.
- Gallery must function fully offline (fixtures only).

## Requirements

### Functional Requirements

- **FR-001**: Each platform MUST provide the component vocabulary above
  as individually reusable units — one authoritative implementation per
  component per platform; screens compose components, never re-implement
  them; existing spec-015 components MUST be consumed where marked
  [reuse].
- **FR-002**: The mobile contacts screens (list, detail, group detail)
  MUST be assembled from those components and render all C1–C6 states
  plus swipe-revealed and delete-confirm states from fixtures alone.
- **FR-003**: The desktop contacts screen MUST implement the group rail
  + sectioned list layout with the closable third-column contact detail,
  header dropdown, and group context menu (DC1–DC6), reusing the shared
  vocabulary and the spec-015 third-column mechanics (width, timing,
  Esc, narrow-window overlay below 1120px).
- **FR-004**: Every platform MUST extend its existing preview gallery
  with (a) each new component and its variants, and (b) the full-screen
  states listed above; each entry reachable in ≤ 2 interactions from the
  gallery root; galleries excluded from or clearly separated in
  production navigation.
- **FR-005**: All screen states MUST be driven by a fixture model (plain
  data, display-ready strings, pre-grouped sections); components MUST
  NOT fetch, sort by collation, resolve ENS, or derive business state.
- **FR-006**: Contact avatars MUST render the Nimiq identicon via each
  platform's existing vela-core consumption route, seeded by the
  contact's address through `normalize_seed` semantics (no call-site
  lowercasing); invalid seeds fall back to the shared placeholder.
- **FR-007**: Menus MUST come in the three modal forms the designs
  define — mobile ActionMenuSheet, desktop DropdownMenu, desktop
  ContextMenu — sharing one visual family (surface, radii, icon+label
  rows, divider, destructive row) per platform.
- **FR-008**: Destructive actions (删除联系人, 删除分组, row-swipe 删除)
  MUST render in the destructive color and MUST be represented in the
  gallery together with their confirmation step.
- **FR-009**: The mobile list MUST include the A–Z index rail with the
  letter-bubble HUD and haptic behaviour where the platform supports
  haptics; web mobile-width MAY omit haptics but keeps the rail and
  bubble.
- **FR-010**: All strings via the platform's i18n layer with zh and en
  resolving; all styling via theme tokens (both appearances build, dark
  matches the mocks).
- **FR-011**: Motion timings (sheet 250ms rise / third column 240ms
  open, 200ms close / content crossfade 150ms / hover 120ms / bubble
  120ms-80ms) MUST live as named constants in each platform's motion/
  token layer and MUST honour the reduced-motion degrade rules from the
  SPEC mocks.
- **FR-012**: Fixture content MUST mirror the mocks verbatim so visual
  diffing against `design/contacts/` is meaningful; states not covered
  by a mock (empty group, no activity, search-empty) reuse existing
  treatments and are recorded in results notes.

### Key Entities

- **ContactFixture**: display name, middle-truncated address, full
  address (mono block), identicon seed, group memberships (names),
  letter-section key, optional recent-activity entries.
- **GroupFixture**: name, member count, member list (ContactFixture
  refs), selected flag (desktop rail).
- **ContactsPageFixture**: search placeholder state, groups list, total
  count (8 位), pre-grouped letter sections, empty flag.
- **MenuFixture**: menu kind (sheet / dropdown / context), items (icon,
  label, destructive flag), divider positions, anchor description.
- **ActivityFixture** **[reuse]**: the spec-015 shape (kind, title,
  subtitle, signed amount, unit, color) — contacts detail feeds it
  已收到/已发送 entries.
- **PanelFixture** (desktop): third-column content (none / contact
  detail) plus narrow-overlay flag.

## Success Criteria

### Measurable Outcomes

- **SC-001**: For each of the 14 C/DC/M mocks there exists a gallery
  state on the corresponding platform(s) that a reviewer can open and
  match against the mock; a full walkthrough on one platform takes under
  3 minutes.
- **SC-002**: Zero business-logic imports (storage, network, collation,
  ENS, address validation) in the new component/screen modules —
  verified by reading the module dependency list per platform.
- **SC-003**: The same contact addresses render identical identicons on
  all four platforms (visual check; vela-core conformance suite remains
  green).
- **SC-004**: Each platform builds green with the feature included
  (existing suites pass; new components carry previews/tests per that
  platform's convention).
- **SC-005**: A later "connect real contacts" feature would replace only
  the fixture layer: components consume plain display models exclusively
  (checked by reviewing component inputs for absence of service types).
- **SC-006**: Spec-015 reuse is real: the contacts screens introduce no
  second implementation of identicon avatar, activity row, section
  header, empty-state artwork, tab bar, sidebar, or third-column shell
  (verified per platform by import inspection).

## Assumptions

- The mocks are dark-appearance; light appearance derives from the token
  system and is not separately mocked.
- The C1 count 8 位 vs. the seven rows visible in DC1 is a mock
  inconsistency; the canonical fixture is eight contacts — Alice, 阿豪,
  Bartholomew Vanderbilt-Konstantinopoulos.eth, Bob · 泵泵, Charlie,
  DAO 金库, hold on, 妈妈 (M section) — with 表弟 existing only inside
  the 家人 group fixture. Recorded here so visual diffing knows the one
  intentional delta against DC1.
- 转账 / 收款 / 二维码 / 群发转账 / 管理 › / 全部 › taps do not navigate
  anywhere yet (no destination in scope); menu items likewise render and
  dismiss only. Import/export never touches real files in this feature.
- Contact-row right-click items on desktop (转账 / 收款 / 复制地址 /
  编辑 / 移入分组 / 删除) render as a ContextMenu fixture; none execute.
- Edit flows (编辑 pencil, 编辑分组, 新建联系人, 新建分组 forms) are
  out of scope — the entry points render, the forms are a later feature.
- Drag-to-group and drag-file-import are desktop SPEC behaviours whose
  full interaction is out of scope; the `drop-target` visual state of
  GroupRail rows IS in scope as a gallery-visible variant.
- Sorting, pinyin/letter grouping and search filtering are fixture-side:
  fixtures ship pre-grouped sections and a pre-filtered "search active"
  variant where needed.
- Existing per-platform gallery mechanisms (specs 014/015) are reused;
  no new gallery infrastructure is introduced.
