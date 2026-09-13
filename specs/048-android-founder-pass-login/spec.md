# Feature Specification: Android Founder Pass and Login Recovery

**Feature Branch**: `048-android-founder-pass-login` (stacked on `047-android-port-completion`)
**Created**: 2026-09-12
**Status**: Draft
**Input**: The founder's notes of 2026-09-12 on the Android build (ten items,
from the settings text-size control to 群发转账), the interaction audit that
followed — every tappable thing the web has on a phone screen, checked against
the Android shell (40 findings, 9 of them the founder's) — and one bug that is
not Android's at all but blocks people on the web today: after a passkey signs,
the wallet does nothing, and only clearing the browser's storage lets them in.
Plus a written haptics policy, because the founder asked for vibration in the
right places and not everywhere.

## Why this exists

047 finished the port: every screen reads live data. This pass is about the
screens doing what they *say*. The audit's shape is consistent — controls were
drawn from the mocks in the 01x specs, the live wiring of 04x reached the
primary path of each screen, and the second-tier affordances (copy, view on
explorer, filters, the identicon viewer, group management) were left with
their default no-op callbacks. Nothing crashed, nothing warned; the buttons
just did nothing. The founder found nine of them in an afternoon.

The login bug is different in kind: a person who created a wallet in the
retired client, on the same web address, now cannot sign in on the new one,
and the app tells them nothing. That is the worst failure a wallet can have
short of losing money, and it goes first.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A wallet made in the old client still opens (Priority: P1)

A person created their wallet with the retired client at the wallet's web
address. They open the same address today, tap sign in, their passkey prompts
and signs — and the page must open their wallet, exactly as if nothing had
changed underneath. If their stored records cannot be read for any reason, the
page must say so and offer a way forward (sign in again, or reset this
browser's copy), never sit silent with a disabled button.

**Why this priority**: It blocks existing people from their money on the
public address, and the failure is invisible. Everything else in this spec is
a dead button; this is a locked door.

**Independent Test**: Seed a browser with an account record written in the old
client's shape (its field names, with and without the per-key list), sign in
with the fixture passkey → the wallet home opens. Seed a record that cannot be
read at all → a visible message with a recovery action, no spinner that never
ends. On Android, seed the same old-shaped record in the app's store → the
session leaves its loading state visibly (a wallet, or a message), never a
blank forever.

**Acceptance Scenarios**:

1. **Given** the browser holds accounts in the old client's shape, **When** the
   person signs in with their passkey, **Then** the wallet opens and the
   records are rewritten in the current shape so the next visit does not
   depend on the translation.
2. **Given** the browser holds accounts in the old shape without a per-key
   list, **When** they sign in, **Then** the wallet opens and their single
   key is treated as the account's key.
3. **Given** the stored accounts cannot be read (corrupt or foreign), **When**
   the page loads or a sign-in completes, **Then** within 2 seconds the person
   sees a message that this browser's wallet data cannot be read, with
   "sign in again" and "reset this browser's copy" as the ways out, and the
   sign-in button is pressable again.
4. **Given** a returning visitor whose stored accounts cannot be read,
   **When** they open the wallet page, **Then** they see the welcome screen
   (not an endless loading state) and the same message.
5. **Given** an Android install whose stored accounts cannot be read, **When**
   the app starts, **Then** the failure is logged and shown, never a silent
   loading screen.
6. **Given** the desktop app finds one unreadable record among readable ones,
   **When** it starts, **Then** the behaviour is unchanged (the record is
   skipped) and the skip is written to its log — recorded here, not fixed.

---

### User Story 2 — Every button on the phone does what it says (Priority: P1)

The founder's list and the audit's P1 findings: taps that do nothing today on
the phone, each of which the web does. A person copying an address gets it on
the clipboard; tapping a row in the history or assets list opens that item;
the saved receive image carries a real code; a contact's 转账 / 收款 / 二维码
work; a group can be created, renamed, deleted and sent to as a batch.

**Why this priority**: These are the phone's everyday paths, and a copy button
that only shows a tick is a lie the person discovers at the worst moment — when
they paste.

**Independent Test**: For each affordance, tap it on the phone and observe the
effect: clipboard content read back, the detail sheet open with the right
item, the saved image decoded by a code reader, the send form open with the
group's members, the group listed after creation.

**Acceptance Scenarios**:

1. **Given** any copy control (receive network rows, the receive address and
   contract, a transaction's address and hash, a token's facts, the receipt
   hash, a contact's address), **When** tapped, **Then** the clipboard holds
   exactly that value and the control shows its copied state.
2. **Given** the history list or the assets list, **When** a row is tapped,
   **Then** that row's detail sheet opens; one Back closes it and one more
   leaves the list.
3. **Given** the receive screen's 保存图片, **When** tapped, **Then** an image
   appears in the phone's gallery that a code reader decodes to the wallet
   address, composed like the web's card (white curved foot, the app icon and
   wordmark, the network's logo in its pill, the name and address).
4. **Given** a contact's detail, **When** 转账 / 收款 / 二维码 is tapped,
   **Then** the send form opens to that contact / the receive screen opens /
   the contact's code sheet opens.
5. **Given** a group's detail, **When** 群发转账 is tapped, **Then** the send
   form opens with one split row per member; a group of one opens the plain
   form to that member.
6. **Given** the contacts list, **When** the section action beside 分组 is
   tapped, **Then** a new-group sheet opens and saving it lists the group;
   **Given** a group's ⋯ menu, **When** rename or delete is chosen, **Then**
   the change is reflected in the list after confirmation.
7. **Given** the identicon in any place the web makes it tappable (contact
   rows and detail, settings account rows, both account sheets, the receive
   address card, the send recipient field, split cards, contact-pick rows,
   fact-row leads, the breakdown, signer rows), **When** tapped, **Then** the
   identicon viewer opens with that address.

---

### User Story 3 — Filters, links and the second-tier controls (Priority: P2)

The affordances that shape what a screen shows or take the person somewhere
else: the class and network filters on the token pick and the two lists, the
"view on explorer" buttons, the token detail's own send and receive, the home
hero's tap-to-hide and its status line's rescue sheet, the scanner's camera
flip, the settings pages' editable fields and links.

**Why this priority**: Wrong or missing, but the task can still be done another
way (the picker instead of a preselected form, the settings page instead of
the hero's rescue sheet).

**Independent Test**: Each control tapped on the phone changes the screen the
way the web does.

**Acceptance Scenarios**:

1. **Given** the token pick, **When** 稳定币 / Gas 币 / 其他 is chosen,
   **Then** only that class of holdings is listed; 全部 restores the list.
2. **Given** the token pick, the history list or the assets list, **When** the
   全部网络 pill is tapped, **Then** a network sheet opens and choosing one
   narrows the list to that network; the pill shows the choice.
3. **Given** the receive code sheet, a transaction detail or a token detail,
   **When** 在区块浏览器中查看 is tapped, **Then** the network's explorer opens
   on that address / hash / token.
4. **Given** a token detail, **When** 转账 is tapped, **Then** the send form
   opens with that token already chosen; **When** 收款 is tapped, **Then**
   that token's own receive code opens; **When** one of its activity rows is
   tapped, **Then** that transaction's detail opens.
5. **Given** the home, **When** the balance amount is tapped, **Then** the
   figures hide (and unhide on the next tap); **When** the status line
   (offline, refreshing, a failed network) is tapped, **Then** the matching
   rescue sheet opens.
6. **Given** the scanner, **When** 翻转 is tapped, **Then** the preview
   switches to the other camera and back.
7. **Given** the send recipient picker, **When** the 扫码 row is tapped,
   **Then** the scanner opens and a scanned address fills the recipient;
   **When** a group row is tapped, **Then** the members become split rows.
8. **Given** a split row, **When** its contact pick is tapped, **Then** the
   picker fills that row.
9. **Given** the add-token screen, **When** 原生币 is chosen and a network
   picked, **Then** the native coin of that network is added.
10. **Given** settings, **When** a network's RPC or explorer override is edited,
    the list-foot "+ 添加网络" is tapped, a custom RPC is typed and 重新检查
    tapped, a provider's 检查密钥 / 获取密钥 or link is tapped, or the language
    sheet's contribute link is tapped, **Then** each does what its label says.
11. **Given** a contact detail, **When** 最近往来 · 全部 is tapped, **Then** the
    history filtered to that contact opens; **Given** a contact row swiped,
    **When** 转账 or 删除 is tapped, **Then** the send form opens / the delete
    confirmation appears; **Given** 导出, **Then** CSV or JSON can be chosen.

---

### User Story 4 — The text-size control is a slider, and the phone vibrates where it should (Priority: P2)

The settings text-size control is dragged as well as tapped and snaps to its
steps with a light tick at each; the preview text grows as the thumb moves.
Across the app, vibration follows one written policy: it marks what the eye
cannot confirm and what really changed, and nothing else.

**Why this priority**: The founder's first complaint, and the rule that stops
the next fifty controls from each inventing their own buzz.

**Independent Test**: On the phone, drag the slider across its range and feel
one tick per step; tap a dot and feel one tick; switch a filter, pick a
network, copy an address and feel one light click each; scroll a list, switch
a tab, open and close a sheet and feel nothing. The scripted pass reads the
haptic log line for each.

**Acceptance Scenarios**:

1. **Given** the text-size row, **When** the thumb is dragged, **Then** it
   snaps to the nearest step, the sample text scales live, and one tick is
   felt per step crossed; a tap on a step does the same.
2. **Given** any selection that takes effect (a switch, a class or network
   filter, a network, fee token or account pick, a favourite, a copy),
   **When** made by the person, **Then** exactly one light click is felt.
3. **Given** a success or a refusal the core decides (money sent, money
   arrived, a rejected signature), **When** it happens, **Then** the existing
   two-beat or heavy pattern plays — unchanged.
4. **Given** scrolling, a row tap that navigates, a tab switch, Back, a sheet
   opening or closing, typing, an animation, or a value the app changed by
   itself, **Then** nothing is felt.
5. **Given** the phone's system haptics are off, **Then** the app is silent.

---

### User Story 5 — Visual parity where the founder noticed (Priority: P3)

The scanner's look (bracket weight, a single status line), the share card's
composition, the settings home without the 通讯录 and 反馈 rows on the live
route, and the small settings polish (the feedback box editable).

**Why this priority**: Looks, not function.

**Independent Test**: Side-by-side screenshots against the web at phone width.

**Acceptance Scenarios**:

1. **Given** the scanner, **Then** the frame's brackets match the web's weight
   and there is one status line under the hint.
2. **Given** the settings home on the live route, **Then** the 通讯录 and 反馈
   rows are absent, as ruled on 2026-09-12.

### Edge Cases

- An old-shape account record that also has an unknown extra field: it still
  reads; unknown fields are ignored, never fatal.
- Two records for the same address, one old-shaped and one new: both read; the
  rewrite keeps the order and drops nothing.
- A copy on a device with no clipboard service: the control shows no copied
  state and nothing crashes.
- The gallery refuses the image (no space): the person sees the refusal; the
  share sheet remains a way to get the image out.
- A group of zero members sent as a batch: the send form opens empty with the
  split entry, not an error.
- The scanner on a device with one camera: 翻转 is absent or disabled, not a
  dead button.
- The network filter chosen on the pick, then the sweep pins a different
  chain: the sweep's pinned chain wins and the filter follows it.
- Haptics during a fast drag across all steps: one tick per step crossed, never
  a burst that outlasts the gesture.

## Requirements *(mandatory)*

### Functional Requirements

**Login recovery**

- **FR-001** Account records written by the retired client (old field names,
  with or without the per-key list) MUST be read by the current wallet on the
  web and on Android without any action by the person.
- **FR-002** After such records are read on the web, they MUST be rewritten
  once in the current shape; subsequent visits MUST not depend on the
  translation.
- **FR-003** When the stored accounts cannot be read, the web MUST show a
  visible message with two actions — sign in again, reset this browser's copy
  — within 2 seconds of the page loading or the sign-in completing, and the
  sign-in control MUST be usable again. The session MUST fall back to the
  welcome screen, never remain loading.
- **FR-004** Any answer from the app's storage that the core refuses MUST be
  turned into the matching failure for that machine and logged, on the web
  and on Android; no machine may be left waiting for an answer that was
  refused.
- **FR-005** Regression coverage MUST include: the core reading both shapes;
  the web's account loading and rewrite; the web's effect loop turning a
  refused answer into a failure; a web end-to-end sign-in from an old-shaped
  seed; an Android session that leaves loading from an old-shaped store.

**Founder's list and P1 findings**

- **FR-006** Every copy control in the flows and contacts MUST write its value
  to the clipboard through one shared helper and show its copied state.
- **FR-007** Row taps on the history list and the assets list MUST open that
  row's detail; no extra navigation step may be left behind.
- **FR-008** 保存图片 MUST produce an image in the phone's gallery whose code
  decodes to the wallet address, composed like the web's card.
- **FR-009** A contact's 转账 / 收款 / 二维码 MUST open the send form to that
  contact, the receive screen, and the contact's code sheet respectively.
- **FR-010** 群发转账 on a group MUST open the send form with the members as
  split rows; a single member MUST open the plain form to them.
- **FR-011** The contacts list MUST offer creating a group (labelled as the web
  labels it), and a group's menu MUST offer rename and delete, all through the
  contacts machine.
- **FR-012** The identicon viewer MUST open from every identicon that carries
  an address, in every place the web makes it tappable.

**P2 findings**

- **FR-013** The token pick MUST filter by class (全部 / 稳定币 / Gas 币 /
  其他), and the token pick, history list and assets list MUST filter by
  network through the 全部网络 pill and its sheet; the pill MUST show the
  chosen network.
- **FR-014** 在区块浏览器中查看 on the receive code sheet, transaction detail
  and token detail MUST open the explorer on the address, hash or token.
- **FR-015** A token detail's 转账 MUST open the send form with that token
  chosen; its 收款 MUST open that token's own code; its activity rows MUST open
  their transaction detail.
- **FR-016** The home hero's amount MUST toggle hiding on tap; its status line
  MUST open the matching rescue sheet.
- **FR-017** The scanner's 翻转 MUST switch cameras where two exist, and MUST
  be absent or disabled where one does.
- **FR-018** The send recipient picker MUST offer 扫码 and group rows in the
  live send; split rows MUST offer a per-row contact pick.
- **FR-019** The add-token screen's 原生币 tab and network pick MUST work.
- **FR-020** In settings, network overrides MUST be editable, the list-foot
  "+ 添加网络" MUST open the add-network flow, a custom RPC MUST be typable and
  re-checkable, provider key actions and links MUST act, the language sheet's
  contribute link MUST open, and the feedback box MUST be editable.
- **FR-021** Contact detail's 最近往来 · 全部, the swiped 转账 / 删除, and an
  export format choice MUST work.

**Slider and haptics**

- **FR-022** The text-size control MUST be draggable and tappable, snap to its
  steps, scale its sample live, and tick once per step crossed.
- **FR-023** Haptics MUST follow the written policy: detents, effective
  selections and core-decided outcomes only; never scrolling, navigating
  taps, tab switches, Back, sheets, typing, animation or programmatic
  changes; one per gesture; none on a control that also navigates; honouring
  the system haptics switch. The policy MUST be written into the design-system
  documentation, and every haptic MUST leave a debug log line.

**Visual parity**

- **FR-024** The scanner's brackets and status line, the share card, and the
  settings home rows MUST match the web at phone width as listed in User
  Story 5.

### Key Entities

- **Account record (stored)**: id, name, address, public key, creation time,
  and the list of keys (credential id, public key, name, transports). Two
  spellings of the field names exist in the wild; the current one is the only
  one written.
- **Chain filter**: the network a list is narrowed to, or all; shown on the
  全部网络 pill; owned by the core, one per list.
- **Class filter**: 全部 / 稳定币 / Gas 币 / 其他 on the token pick.
- **Haptic class**: Detent, Select, Success, Reject — the only four the app
  performs.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001** A sign-in from an old-shaped seed opens the wallet on the web
  (end-to-end) and the same seed leaves loading on Android within 5 seconds
  of launch; an unreadable seed shows the message within 2 seconds on both.
- **SC-002** All 9 founder items reproduce as fixed on the phone; all 40 audit
  rows are closed or recorded with a reason, and the audit table in the
  results shows the after-state per row.
- **SC-003** Every copy control's clipboard content read back from the phone
  equals the shown value (0 mismatches over the full list).
- **SC-004** The saved receive image decodes to the wallet address with a
  code reader on the pulled file.
- **SC-005** The haptic log shows exactly one line per step crossed on a
  slider drag, one per effective selection, and zero for a scroll, a tab
  switch, a Back and a sheet open/close, in one scripted pass.
- **SC-006** The unit suites stay green on every shell touched (core, web,
  Android), and the two rulers show no new strong diffs.

## Assumptions

- The retired client wrote only the account list, the active index and the
  pending uploads in the old spelling at the web address; other keys it wrote
  are read by no current screen or fail quietly — a follow-up sweep, not this
  spec.
- "Saved to the gallery" on the phone means the image appears in the system
  gallery without a permission prompt; the share sheet stays as the way to
  send it elsewhere.
- The chain and class filters already exist in the core (the web uses them);
  the phone wires them, it does not invent them.
- The desktop app is not exposed to the old spelling (its own file, written
  only by the current core) and is not changed here.

## Out of Scope

- Explore and the dApp browser; WalletPair; desktop UI work.
- New core machines: only the tolerant reading of old records and the
  refused-answer contract touch the core.
- The sheet-animation difference on the home rows (the sheet slides after the
  list appears): recorded as P3, not changed.
- Third-party audits, store submission, anything outside the phone and the
  web sign-in.
