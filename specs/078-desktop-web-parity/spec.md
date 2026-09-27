# Feature Specification: The desktop wallet is the web wallet

**Feature Branch**: `078-desktop-web-parity`
**Created**: 2026-09-24
**Status**: Specified; first fixes landed on `fix/desktop-windows`, carried here
**Input**: Owner, 2026-09-24, on the first day the desktop ran on Windows —
"全屏下，明显 web 版更好看，桌面版很丑" · "学学 web" · "你的桌面版真的是一个残次品" ·
"我感觉我这样提很累啊，你能不能自己去比对桌面网页和客户端版本，感受差别，并修复保持一致呢" ·
"请用 github speckit 工作流，078 来解决这些交互 UI 视觉问题".

## Why

The web wallet (`app-web/vela-wallet`) is the reference. The gpui desktop
(`app-desktop/vela-wallet`) was built screen by screen from the same mocks, but
nobody used it next to the web for an hour until today, and in that hour the
owner found, one at a time: rows stretched across a full-screen monitor, a list
cut off with no way to scroll to it, a sidebar crowded to half the web's
rhythm, a count that disagreed with the web (16 vs 23 for the same wallet),
chips that did nothing, a network filter drawn twice, a saved receive card
wearing somebody else's identicon, an avatar that does not open, a header that
does not open the account switcher, a list that goes blank on switching back,
a Max that overflows its field, scan buttons that look nothing like the web's,
a side panel whose Close does not close it.

Each was a one-line report; each was fixed as it came. That is the failure
this spec exists to end: **the owner should not be the diff.** The desktop is
compared against the web systematically — every surface, every interaction —
and the differences are fixed as one piece of work, verified on screen by the
one making them.

## User stories

### US1 — Everything I can click on the web, I can click on the desktop (P0)
Every control the web draws on a surface the desktop also draws does the same
thing there: the sidebar header opens the account switcher; an identicon opens
the identicon viewer; a status line opens what it points at; chips narrow; a
Close closes. A control the desktop cannot back yet is not drawn.

### US2 — It looks like the same product (P0)
Side by side at the same window size, a desktop surface and its web
counterpart have the same structure, the same spacing, type sizes and weights,
the same surfaces (raised / sunken), radii and colours, taken from the web's
design tokens (`app-web/vela-wallet/src/lib/tokens/tokens.css`) — not
approximated by eye.

### US3 — It behaves under real conditions (P1)
A short window scrolls rather than crushes; a full-screen window keeps the
web's measure (content ≤ 800px, two-ended rows ≤ 560px); switching accounts
shows the last-known holdings at once and refreshes behind them; long values
(a Max of an 18-decimal coin, a 42-character address) step down or wrap the
way the web's do, never clip at both ends.

### US4 — The money is right (P0)
What Max fills is exactly what can be sent: the fee is held back when the coin
being sent pays it, not when another coin does, and a Max tapped while the fee
is being checked does not leave Continue stuck.

## Requirements

### Process
- **FR-001** An audit (research.md) lists every gap between web and desktop,
  per area — wallet home, flows (send / receive / scan / activity / add
  token), contacts, settings, explore, signing — each with the web's
  behaviour and file:line, the desktop's, and the fix with the web token
  values.
- **FR-002** Every visual fix is checked by a screenshot of the running
  desktop next to the web at the same size before it is called done; the
  gallery states of both apps are the fixtures for this.
- **FR-003** No regression in the desktop suite or clippy; each fix lands as a
  commit that says what the web does and what changed.

### Interactions (from the owner's reports; the audit adds the rest)
- **FR-010** The sidebar header opens the account switcher, as the web's
  `AccountSwitcher` does on a wide layout: a centred 520px dialog — summary,
  one row per account (identicon, name, short address, total, check on the
  active one, remove), create / sign in — built from the switcher Settings →
  Account already draws.
- **FR-011** Every identicon the web makes openable (the sidebar avatar, the
  receive card, address cards) opens the identicon viewer: the large
  identicon, its caption, copy address, close.
- **FR-012** The "back up public keys to Ethereum" surface's Close closes it.
- **FR-013** The asset detail panel shows the token's logo, name and figures
  exactly as the web's `AssetDetailPanel` does.
- **FR-014** The scanner's buttons and layout match the web's scan surface.

### Money
- **FR-020** Max: the core's reserve rule (1× native, 1.5× an ERC-20 fee coin,
  nothing when another coin pays) is kept; the gaps the audit found are fixed
  in the core for every shell — Max during Continue's fee check no longer
  leaves `estimating_gas` stuck; Max after a fee-coin switch uses the new
  coin's quote; Max without a quote estimates the real transfer.
- **FR-021** The figure Max writes is displayed on the web's ladder (drawn
  length, 46/38/31, ellipsis at the end), in the person's decimal mark.

### Done on this branch before the spec (kept, not redone)
Windows build; the `velawallet://` relay (named pipe) and the empty-path
callback; `dist/` bytes; sidebar: Sign Out / ⌘K removed, chain logos, 44px
rows, list only on Wallet, All Networks counts holdings, rows never shrink;
Wallet column: 800 / 560 measure, scrolls below the caption strip with a bar;
Settings: chain logos, scrolls below the caption strip; clipboard chords
everywhere; address-bar caret; send picker: class chips live, no second
network filter; send form restyled after the web's; share card identicon
normalised; switching back shows last-known holdings; the hero figure's
ladder.

## Success criteria
- **SC-001** For every surface in research.md, a screenshot pair (web, desktop,
  same size) shows no structural or token-level difference the audit named.
- **SC-002** Every web interaction listed in research.md for a surface the
  desktop draws works on the desktop, or the control is absent there.
- **SC-003** The owner can use the desktop for an hour next to the web and
  report nothing that the audit should have found.

## Out of scope
Linux's `velawallet://` relay (the Unix-socket half of `scheme_relay`); the
camera on Windows/Linux (the scanner's file path is the floor there); new
features the web does not have.
