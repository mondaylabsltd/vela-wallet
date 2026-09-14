# 056 — iOS Port Completion

**Branch:** `056-ios-port-completion` (stacked on `055-ios-signing-depth`)
**Mirrors:** Android 047, web 028 phases 4/9/10, 038's stability list
**Machines:** `payment_request`, plus the preferences that have **no machine**
and must not be given one.

## Why this cut

050–055 wired every machine iOS had been ignoring. What is left is the part
with no machine behind it and the part where a drawn control still leads
nowhere:

- **The settings page is mostly a picture.** The currency picker cannot pick
  (`SettingsStore.chooseCurrency` has zero call sites), the RPC-provider and
  endpoint pages are drawn over six live executor operations nobody calls,
  storage shows invented numbers, and language, theme, avatar and text size all
  render and change nothing.
- **Preferences and formats do not exist on this client.** Web has
  `locale-format.ts` and Android has `Formats.kt`; iOS prints
  `String(format:)` and `ISO8601` wherever a number or a date is shown. Four
  number formats, five date formats and a 24/12-hour choice are storage keys
  this app writes on every other platform and has never read here.
- **Deep links are declared and unhandled.** `applinks:getvela.app` is in the
  entitlement and there is no `CFBundleURLTypes`, no `onOpenURL`, and nothing
  that turns a pay-link into a prefilled send.
- **A list of drawn-but-unreachable screens** that 051 recorded and nothing has
  cleared: the native transaction detail, the native-coin token tab, an
  asset-limited receive, the identicon viewer from every avatar, the category
  chips, and the copy controls that copy nothing.

## User stories

### US1 — a person's own preferences are theirs (P1)

Language, theme, avatar style, text size, display currency, and the three
formats are chosen in settings, written to the SAME storage keys every other
client uses, and take effect **without a relaunch** — and are verified where
they SHOW, not on the settings page (049's lesson: a format picker that
previews itself is not evidence that the hero reads it).

### US2 — the settings page does what it draws (P1)

Every row either acts or is not drawn. The RPC pages drive the six live
operations, storage reports real bytes and can clear them, and the account list
switches accounts.

### US3 — a link opens the wallet where it means to (P2)

`velawallet://` and `https://getvela.app/…` reach the app, a pay-link prefills
the send, and `velawallet://open?url=` reaches the browser.

### US4 — the unreachable becomes reachable (P2)

The screens 051 drew and could not reach, each from where a person would look.

### US5 — two rulers and the handover (P3)

`check-ios-event-parity.mjs` and `check-ios-dropped-judgement.mjs`, derived from
the Android pair, run to zero strong differences with every remaining unread
field given a written reason; plus `docs/ios/`.

## Success criteria

| | claim |
|---|---|
| SC-001 | a currency chosen in settings changes the hero, the asset rows and the transaction detail |
| SC-002 | a number format chosen in settings changes every figure, including a remembered flow model |
| SC-003 | a language chosen in settings takes effect without a relaunch |
| SC-004 | the RPC provider and endpoint pages drive the core, and restore defaults |
| SC-005 | storage reports real bytes and clearing one line changes them |
| SC-006 | a pay-link opens a prefilled send |
| SC-007 | every row on the settings page acts, or is not drawn |
| SC-008 | both rulers report zero strong differences |

## Out of scope

Erasing the founder's device (drawn, verified by reading, never run), the Safari
extension, and any change to `vela-core`.
