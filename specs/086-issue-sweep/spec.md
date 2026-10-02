# Feature Specification: 086 — Issue sweep (16 tester issues, 2026-09-22 … 28)

**Feature Branch**: `086-issue-sweep` (this branch holds the docs; each fix goes on its own `fix/issue-<N>-<desc>` branch with its own PR)
**Created**: 2026-10-01
**Status**: Draft
**Input**: 16 open GitHub issues from the tester (cici2026) against v0.9.4 / v0.9.5: #334 #333 #332 #331 #330 #329 #328 #326 #322 #321 #310 #312 #314 #315 #317 #318.

Program note: this is the first of four specs (086–089) that together prepare the Android and iOS betas
for Google Play and App Store testing tomorrow. 087 = mobile beta device pass, 088 = store submission
readiness, 089 = Chrome extension pass. No other spec number is used.

## User Scenarios & Testing *(mandatory)*

Each story below is one issue, or one cluster of issues that share a screen. Each can be fixed, tested
and shipped on its own.

### User Story 1 — Pay someone by scanning their code, recipient first (Priority: P1)

Issues #332 (Android), #326 (Web), #312 (Web). A person scans another wallet's Receive code. They first
want to see whom they are paying. Then they choose what to send: from their own assets on the network
the code names, if it names one. They also want to be able to change that choice on the Send form itself.

**Why this priority**: this is money movement. A scan that silently lands on an unrelated token and chain
(XDAI on Gnosis for a BNB Chain code) can make someone send on the wrong network.

**Independent Test**: scan a network-specific Receive code (e.g. BNB Chain) and a plain address code with
a second wallet. The Send form opens with the scanned recipient shown. The asset is chosen from the payer's
holdings on the named network, or the person is asked to choose. The token card on the Send form opens
the asset picker and keeps the recipient.

**Acceptance Scenarios**:

1. **Given** a code that names a network the payer holds assets on, **When** it is scanned, **Then** the
   recipient is shown first, and only the payer's assets on that network are offered (or the single one
   is preselected).
2. **Given** a code naming a network the payer holds nothing on, **When** it is scanned, **Then** the
   payer is told plainly that they have nothing on that network. The app does not switch silently to
   another chain.
3. **Given** a plain address code with no network, **When** it is scanned, **Then** the recipient is shown
   first and the person chooses the asset; the form does not jump straight to a token list with no
   recipient visible.
4. **Given** the Send form opened from a scan, **When** the token card is tapped, **Then** the asset
   picker opens, and choosing keeps the scanned recipient.

---

### User Story 2 — Contacts are easy to edit and import faithfully (Priority: P1)

Issues #334, #310, #333 (Web). Editing a contact can be done from the detail panel a normal click opens,
next to what is being edited. An imported list keeps every name exactly as written, in any script.

**Why this priority**: names in Chinese, Japanese, Cyrillic etc. are the norm for these users. A garbled
name is data loss on import.

**Independent Test**: open a contact by click; Edit is visible near the name and Delete sits close to the
content with no large gap. Import files containing CJK, Cyrillic and emoji names, saved as UTF-8 (with
and without BOM), UTF-16 and the legacy Chinese Windows encoding. Every name survives.

**Acceptance Scenarios**:

1. **Given** the contact detail panel, **When** it opens, **Then** an Edit control is next to the name
   and the destructive action sits after the content, not pinned to the bottom of a tall panel.
2. **Given** a contact file whose names use non-Latin scripts, **When** it is imported, **Then** every
   name appears exactly as in the file, in the list and in the detail panel.
3. **Given** a file whose text cannot be decoded, **When** it is imported, **Then** the person is told,
   and no "�" names are written silently.

---

### User Story 3 — Android everyday screens behave (Priority: P1)

Issues #328, #331, #330, #329, #322, #321 (Android, plus every other shell found with the same defect).

- **#328**: after a token detail sheet is closed with ×, tapping another asset opens its sheet.
- **#331**: on a multi-recipient send, tapping the amount field never removes the recipient.
- **#330**: hidden Explore groups can always be shown again.
- **#329**: a favorite is never named after an engine error page. It gets the site's name, or its host.
- **#322**: Sign Out looks like a control.
- **#321**: the Receive screen fits one phone screen, with a margin above the bottom edge.

**Why this priority**: the beta goes to store testers tomorrow. A tap that does nothing, or one that
deletes a recipient, reads as broken.

**Independent Test**: on the connected Android phone (and the iPhone, for any shared defect), reproduce
each issue's steps before the fix and after it, with screenshots.

**Acceptance Scenarios**:

1. **Given** a token detail sheet closed with ×, **When** another asset is tapped, **Then** its sheet opens
   (repeat 10 times in a row).
2. **Given** two or more recipients, **When** the amount field is tapped anywhere on its visible area,
   **Then** the field takes focus. The remove control is separated, with a touch target that does not
   overlap the field.
3. **Given** every Explore group is hidden, **When** the Explore screen shows, **Then** a visible entry
   still opens Manage groups.
4. **Given** a page that failed to load, **When** it is saved as a favorite, **Then** the name is the
   site's last good title, or its host. It is never the error page's title.
5. **Given** Settings, **When** the Sign Out row shows, **Then** it carries the same control affordance as
   its neighbours.
6. **Given** a phone of typical height (≈ 2400 px tall at the default text size), **When** Receive opens,
   **Then** the code, address and actions all show without scrolling, with a bottom margin.

---

### User Story 4 — The backup public keys confirmation is easy to read (Priority: P2)

Issue #314 (Android, and the same sheet on other shells). The sheet groups what matters: what is being
registered, and for which account. The technical details recede, and the confirm action is clear.

**Independent Test**: open Back up public keys → confirm on Android and iOS. The fields are grouped under
clear headings, with one emphasis for the outcome and the technical facts folded under a secondary row.

**Acceptance Scenarios**:

1. **Given** the confirmation sheet, **When** it opens, **Then** the outcome (what is registered, which
   account, which network) leads, and the technical details are visually secondary.

---

### User Story 5 — The extension connects the account you are signed in to, in your language (Priority: P1)

Issues #315, #317 (Chrome extension).

- **#315**: a dApp always connects the account the extension is signed in to. After switching accounts,
  connected sites learn of the change, and a reconnect never hands out a previous account.
- **#317**: the signing sheet follows the extension's language setting.

**Why this priority**: connecting a different account than the one shown is a trust and money issue. A
sheet in the wrong language undermines "what you see is what you sign".

**Independent Test**: sign in to account A, connect a dApp, sign out, then sign in to account B. The dApp
sees B (`accountsChanged`), and a reconnect yields B. Set the extension to English and open a dApp
signing sheet: every string is English.

**Acceptance Scenarios**:

1. **Given** account B is signed in after A was used, **When** a dApp connects or reconnects, **Then** it
   receives B only.
2. **Given** the extension language is English, **When** a dApp signing sheet opens, **Then** no string in
   it is in another language.

---

### User Story 6 — The trusted signer works on iOS (Priority: P1)

Issue #318 (iOS). On iOS, choosing the trusted signer opens the page, signs, and returns the answer to
the wallet, as on Android.

**Independent Test**: on the iPhone, sign a message and a transaction through the trusted signer (with a
person's Face ID, or up to the passkey prompt when no one is present). The page opens, the answer
returns, and the wallet completes.

**Acceptance Scenarios**:

1. **Given** the trusted signer is chosen on iOS, **When** a signature is requested, **Then** the page
   opens and the signed answer returns to the wallet. When it cannot, the person is told why, never
   left on a dead screen.

### Edge Cases

- A code that names a network the app does not support: say so; never fall back to a different chain.
- A code carrying an amount or token (EIP-681 style): keep its token and amount when the payer holds it.
- Contact files with mixed scripts, emoji, a BOM, CRLF line endings, or an empty name.
- Explore with every group hidden, after a restart.
- A favorite saved while the page is still loading (no title yet): use the host.
- An extension already connected to a site when the account changes, while the site's tab is closed.
- The trusted signer page unreachable on iOS: the existing "page never opened" handling applies.
- Large text size and small screens (Receive fit, multi-recipient rows).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: After a scan, the recipient MUST be visible before or together with the asset choice, on every shell that scans.
- **FR-002**: When a scanned code names a network, the asset offered MUST be one of the payer's assets on that network. When the payer holds none there, the app MUST say so and MUST NOT switch to another network silently.
- **FR-003**: The Send form's asset card MUST open the asset picker, and choosing MUST keep the recipient.
- **FR-004**: The rule for which asset a scan preselects MUST be decided once (in the shared core) and used by every shell.
- **FR-005**: The contact detail panel MUST offer Edit next to the name. Edit and Delete MUST sit with the content, not across a large gap.
- **FR-006**: Contact import MUST keep names exactly in UTF-8 (with or without BOM) and UTF-16 files. A file it cannot decode faithfully MUST be refused with a message, never imported with "�".
- **FR-007**: Closing a detail sheet by any means (×, swipe, back) MUST leave every list row able to open its sheet.
- **FR-008**: A multi-recipient row's remove control MUST NOT share or overlap the amount field's touch area.
- **FR-009**: Manage groups MUST stay reachable when every Explore group is hidden.
- **FR-010**: A favorite or tab MUST NOT be named from a page that failed to load. It takes the site's last good title, or its host.
- **FR-011**: Sign Out MUST look and behave like a control.
- **FR-012**: The Receive screen MUST fit one screen on a typical phone at the default text size, with a bottom margin, and scroll only when it truly cannot fit.
- **FR-013**: The backup public keys confirmation MUST lead with the outcome and group the technical facts as secondary.
- **FR-014**: The extension MUST connect, and report to connected sites, only the signed-in account. An account change MUST be announced to connected sites.
- **FR-015**: The extension's dApp signing sheet MUST render in the extension's chosen language.
- **FR-016**: The trusted signer MUST work end to end on iOS, or fail with a clear reason.
- **FR-017**: Every fix MUST check the other shells (Android, iOS, web/extension, desktop) for the same defect, and fix it there too when present.
- **FR-018**: Every fix MUST come with an automated test for the reported failure when one is possible. Otherwise it MUST record repeatable manual steps and device evidence.
- **FR-019**: Each issue MUST land as its own branch `fix/issue-<N>-<desc>` and PR (`Fixes #<N>`). Issues on the same screen may share one PR that fixes both, and the PR names both.

### Key Entities

- **Scanned payment code**: a recipient address, and optionally a network, token and amount.
- **Contact import file**: rows of name, address and group, in some text encoding.
- **Explore group**: Favorites or Recent dApps, each visible or hidden.
- **Favorite**: URL plus a display name. The name is never taken from an error page.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All 16 issues have a merged-ready PR (or a documented "already fixed on main" with proof), each linked with `Fixes #N`.
- **SC-002**: Each issue's original steps, replayed after the fix on the affected platform, no longer show the defect. Device issues have before/after screenshots.
- **SC-003**: Scanning a code for network X never opens Send on a network other than X (0 occurrences in 10 scans across 3 networks).
- **SC-004**: Importing a 50-name file mixing 5 scripts keeps 50 of 50 names byte-exact.
- **SC-005**: In 20 consecutive taps on multi-recipient amount fields, 0 recipients are removed.
- **SC-006**: The full test suites (core, web, Android, iOS, desktop, extension) stay green on every fix branch.

## Assumptions

- "Typical phone" means the test devices: Xiaomi alioth (1080×2400) and iPhone 11 (828×1792), at the default text size.
- The issue reports are against v0.9.4/0.9.5. Some may already be fixed on `main` (67e2d193d); those are closed with proof instead of a change.
- The legacy Chinese Windows encoding (GBK/GB18030) is decoded when the platform's decoder can do so faithfully. Otherwise the file is refused with a message (FR-006).
- Device checks run on the connected Xiaomi and iPhone 11, with parallel-space fixed keys where signing is needed. No real funds move without the owner.
