# Feature Specification: Android Settings Audit and the Send Path

**Feature Branch**: `049-android-settings-audit-send` (stacked on `048-android-founder-pass-login`)
**Created**: 2026-09-13
**Status**: Draft
**Input**: The founder, 2026-09-13: "创建 spec 049 来检查设置中每一项设置是否都正常。我现在发现不正常的有：1. 首字母/图形头像 2. 数字格式。我希望你彻底检查一遍并修复。对了还要验证一遍主流程，就是转账是否通畅。"

## Why this exists

047 made every settings row read the device; 048 made every button do what it
says. What neither pass checked is whether a **choice takes effect where the
person looks for it**. The founder found two that do not: the avatar style
switches its segment and changes no avatar anywhere, and the number format
ticks a row while the home hero still prints `CN¥3` + `.63` with the choice
`1.234.567,89` stored on the phone. Both were "verified" earlier by looking at
the settings page itself, which is the one screen a preference is not for.

So this spec is an audit with a fixed shape: for **every** setting, name the
places in the app where it must show, drive the phone through each, and fix
what does not follow. The transfer path is checked on-chain at the end for the
same reason — a settings pass that touches the amount rendering of every
screen must end with a real payment landing.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A chosen avatar style shows everywhere an avatar is drawn (Priority: P1)

A person picks 首字母 under 头像样式. Every avatar in the app that stands for
an account or a contact — the settings account row, the accounts sheet, the
wallet header, the account switcher, contact rows and the contact detail, the
send form's recipient and split cards, the transaction detail's counterparty,
the signing sheet's signer row, the browser's account chip, the done screen —
becomes a letter on an accent disc: the first letter of the name, or `V` when
there is no name to take one from. Picking 图形头像 brings the identicon back.
The identicon viewer follows the same choice. The change is immediate, needs
no restart, and survives one.

**Why this priority**: the control exists, is labelled, and does nothing. A
visible control that lies is worse than none.

**Independent Test**: on the phone, pick 首字母 → photograph the settings
account row, the wallet header, a contact row, the send recipient; each shows
a letter disc. Pick 图形头像 → the same four show identicons. Relaunch → the
last choice holds.

**Acceptance Scenarios**:

1. **Given** 图形头像 is chosen, **When** the person picks 首字母, **Then**
   every account/contact avatar on screen redraws as a letter disc within one
   frame of the tap, and the segment shows 首字母 selected.
2. **Given** 首字母 is chosen, **When** a contact named "觉得九点半" is listed,
   **Then** its disc shows 觉; an unnamed address shows `V`.
3. **Given** 首字母 is chosen, **When** the app is killed and relaunched,
   **Then** the avatars are still letter discs.
4. **Given** 首字母 is chosen, **When** an avatar is tapped, **Then** the
   viewer opens with the same letter disc, the whole address underneath.

---

### User Story 2 — The number, date and time formats apply to every figure (Priority: P1)

A person picks `1.234.567,89`. The home hero reads `CN¥3,63`, not `CN¥3` `.63`;
asset rows read `€0,40`; token amounts in the feed, the assets list, the send
form, the confirm screen, the receipt and the transaction detail use the comma
as the decimal mark (`−0,001 XDAI`); the clear-signing sheet's amounts follow
too. The 数字格式 row on the settings home shows the current example, and the
sheet's rows are the real examples of each preset, not a drawn string. The
same holds for 日期格式 (day headers, detail dates, contact activity) and
时间格式 (detail times). Changing any of the three re-renders the screens
already built — no relaunch, no waiting for the next balance refresh.

**Why this priority**: money is where a wrong separator is a hundredfold
misreading. The founder stored `dot_comma` and the hero showed a mark that
reads as a second thousands separator.

**Independent Test**: on the phone, pick `1.234.567,89` → the hero, an asset
row, a feed row, the send form and confirm, and a transaction detail all show
the comma mark and dot grouping; pick `1,234,567.89` → they flip back without
leaving the screen. Pick `1:45 PM` → a detail's time shows AM/PM.

**Acceptance Scenarios**:

1. **Given** `1.234.567,89` is chosen, **When** the home is shown, **Then** the
   hero's decimal mark is a comma and its grouping a dot.
2. **Given** `1.234.567,89` is chosen, **When** a feed row shows `0.001 XDAI`
   in the core's figure, **Then** the row prints `−0,001 XDAI`.
3. **Given** a format is changed on the settings page, **When** the person
   returns to the home, **Then** it already shows the new format, and a screen
   left open under the sheet re-renders without a data refresh.
4. **Given** the 数字格式 sheet is open, **Then** each row's label is that
   preset's rendering of 1234567.89, and 自动 shows what the device's locale
   resolves to.
5. **Given** `1.234.567,89` is chosen, **When** a dApp transaction is being
   signed, **Then** the clear-signing amounts use the comma mark.

---

### User Story 3 — Every other setting does what its row says (Priority: P1)

The audit proper: language, currency, theme, text size, the 高级 disclosure,
networks (list, detail, overrides, delete, add), RPC providers, endpoints,
storage (measure, clear one, clear caches), about, sign out, erase device, and
the accounts sheet. Each is driven on the phone and its effect read from the
screen it applies to, not from the settings page.

**Why this priority**: the founder asked for 彻底检查. Two of eighteen were
wrong; the rest are asserted, not shown, until this pass.

**Independent Test**: the inventory in `contracts/settings-inventory.md`
drives one scripted pass; every row ends "seen on the phone" or "recorded
with a reason".

**Acceptance Scenarios**:

1. **Given** any row in the inventory, **When** its control is used,
   **Then** the listed effect is visible where the inventory says it shows.
2. **Given** a setting that cannot be exercised on the phone (a provider key
   the founder does not hold), **Then** the results record the reason.

---

### User Story 4 — A transfer still lands (Priority: P1)

After the rendering changes above, a real send of 0.001 XDAI on Gnosis from the
fixture Safe to the phone's own account goes: pick → form → confirm → receipt,
the receipt confirms, the feed shows the row, and the balances on both sides
move by the amount and the in-band fee.

**Why this priority**: every screen on the send path draws an amount this spec
re-touches; the transfer is the proof nothing on the path broke.

**Independent Test**: the scripted send on the phone in the parallel space;
Gnosis balance before/after read from RPC; the receipt screen photographed.

**Acceptance Scenarios**:

1. **Given** the fixture Safe holds ≥0.05 XDAI, **When** 0.001 XDAI is sent to
   a contact, **Then** the confirm shows the amount in the chosen format, the
   receipt reaches 已确认, and the on-chain balance drops by 0.001 plus the
   displayed fee.

---

### Edge Cases

- A name that begins with a digit or an address-looking string (`0x…`) is
  not a name: the initials disc shows `V`.
- A number preset change while a send form has a typed amount: the editable
  field keeps its raw digits (typing must not reflow); only display strings
  change.
- `自动` under a locale whose separators are `,`/`.` (zh, en) resolves to
  `1,234,567.89`; under de to `1.234.567,89`; the sheet's 自动 row says which.
- The clear-signing locale carries the phone's UTC offset at the moment of
  the request, not a cached one.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001** Every account/contact avatar MUST honour the stored avatar
  style, immediately on change and after relaunch; the identicon viewer MUST
  follow it.
- **FR-002** The initials style MUST draw the name's first letter on an accent
  disc at the web's proportions (letter ≈ 0.34 of the diameter), `V` without a
  usable name.
- **FR-003** Every figure drawn from a preference — hero, asset rows, fiat
  lines, token amounts, fees, receipts, detail facts, day headers, times —
  MUST use the chosen number/date/time preset, and MUST re-render on change
  without a data refresh.
- **FR-004** The hero MUST draw its decimal mark from the number preset, and
  token amounts MUST use the preset's decimal mark (grouping stays off on
  token amounts, as on the web).
- **FR-005** The 数字/日期/时间 rows and sheets MUST show live examples of the
  current and of each preset, in the device's locale for 自动.
- **FR-006** The clear-signing locale MUST be built from the resolved
  presets and the current UTC offset, not a constant.
- **FR-007** The transaction detail's fiat line MUST be in the chosen display
  currency and format, never a hard-coded `$`.
- **FR-008** Every row of the settings inventory MUST be driven on the phone
  with its effect observed where it applies; a row that cannot be driven MUST
  be recorded with the reason.
- **FR-009** A real 0.001 XDAI transfer MUST land on Gnosis from the phone
  after the changes, with the confirm/receipt figures in the chosen format.

### Key Entities

- **Avatar style**: `initials | identicon`, stored under the web's key.
- **Format presets**: number (auto, comma_dot, dot_comma, space_comma,
  indian), date (auto, ymd_slash, mdy_slash, dmy_slash, dmy_dot, iso), time
  (auto, h24, h12); `auto` resolved by the shell from the device locale.
- **Settings inventory**: one row per setting — control, where its effect
  shows, how it is read back.

## Success Criteria *(mandatory)*

- **SC-001** Avatar style: 4 named sites photographed in each style on the
  phone; relaunch keeps the choice.
- **SC-002** Number format: the hero, an asset row, a feed row, the send form,
  the confirm and a tx detail photographed under `dot_comma` show comma
  marks; flipping back shows dots — 0 stale screens.
- **SC-003** Every inventory row ends in "seen on the phone" or a recorded
  reason; 0 rows unaccounted for.
- **SC-004** The send lands: receipt 已确认 on the phone, on-chain balance
  delta = amount + displayed fee.
- **SC-005** Unit suite green on Android; the two rulers show no new strong
  diffs.

## Assumptions

- The web is the checklist: where the web applies a preference, Android
  applies it the same way; where the web also hard-codes (the clear-signing
  locale), Android fixes its side and records the web gap.
- The parallel space on the Xiaomi with the fixture Safe (dust on Gnosis) is
  the test bed; a real send costs real xDAI (the founder has authorised this
  before).
- The device's Java default locale is the `auto` source, as the browser's is
  on the web; the app language does not change it.

## Out of Scope

- New settings; desktop or iOS; the web's own clear-signing locale constant
  (recorded, not changed here).
- The signing sheet's fixture-driven scenarios (only the live locale feed
  changes).
