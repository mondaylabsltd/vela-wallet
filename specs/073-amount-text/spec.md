# Feature Specification: An amount means the same on every Vela

**Feature Branch**: `073-amount-text` (on `072-settings-parity`)
**Created**: 2026-09-22
**Status**: Done (shells wired; device + simulator verified)
**Input**: Found during the 071 Android device pass — "0,001" typed on the
phone's decimal-comma keypad did not resolve — and traced to every shell.
Owner's rule for this program: *a stable experience in an unstable
environment; a clear, simple, maintainable architecture; less duplicated
code.*

## Why

The core reads one amount shape: ASCII digits and a `.`. Only the web cleaned
what a person typed before it reached the core (issue 231, `amount-text.ts`),
and only in the send figure. Everywhere else the text went through raw:

| Field | Shells that sent it raw | What the core did with "4,5" |
|---|---|---|
| Send amount | Android, iOS, desktop | fiat mode: `parseFloat` → **4** (a different sum, silently); token mode: Continue enabled, refused when the call is built |
| Split row share | all four | refused when the call is built |
| Custom allowance (dApp approve) | all four | `parse_token_amount` drops every comma → **45**, ten times the cap typed |

A phone's decimal pad follows the device's region, so every person whose
region writes a decimal comma (de, fr, es, pt-BR, it, ru, tr, vi, id…) meets
this on the first amount they type.

## User stories

### US1 — What I type is what is sent (P0)
Typing "4,5" on a decimal-comma keypad into any amount field — the send
figure, a split row's share, a custom allowance — reads 4.5 on every shell,
and the field shows the figure the core has ("4.5").

### US2 — A pasted figure is read whole or refused whole (P0)
A pasted "1.234,56" or "1,234.56" is 1234.56 under any preset. A paste with no
reading as one figure ("1.5e-7", "0x10", "4.5.6") is refused and the field keeps
what it had — never salvaged digit by digit into a different sum (1.57).

### US3 — One rule (P1)
The rule lives once, in the core; the web's TypeScript copy is removed.

## Requirements
- **FR-001** `vela_core::l10n::amount_text::clean(raw, preset, entry,
  previous)` — a case-for-case port of the web's rule, plus `Entry::Unknown`
  for a native field that cannot tell a paste: anything but one added
  character reads as a paste. `caret_after_clean` in UTF-16 units.
  `preset_of` reads the shells' wire names.
- **FR-002** UniFFI `amountTextClean(raw, number, previous, pasted?)`,
  `amountTextCaret`; wasm the same.
- **FR-003** Every amount field cleans in its OWN change handler (not in the
  machine) and holds/sends the result. In the machine it would race: a shell
  whose view lags the typing (Android) sends "4,5" after the core already
  read "4," as "4.", and the machine could no longer tell a decimal mark from
  grouping.
- **FR-004** Web: one helper (`flows/amount-field.ts`) for the send figure,
  split rows and the allowance editor; `amount-text.ts` deleted.

## Success criteria
- **SC-001** Each shell has a test that types a decimal comma into an amount
  field and fails without the change (web: browser tests; iOS: unit + UI;
  Android: JVM; desktop: unit).
- **SC-002** On the Android phone (decimal-comma preset) "0,5" in the send
  amount reads 0.5; on the iOS simulator with a German keypad, the same, under
  both a matching and a disagreeing preset.

## Out of scope
- The batch importer's own rate field and CSV cells (`batch_import` has its own
  cleaning, keyed to the file's format).
- Showing the figure in the person's decimal mark while it is typed: the
  field's text stays dot-decimal, as the web has since issue 231.
