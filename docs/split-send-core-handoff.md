# Multi-recipient send: what the core now says, and what each shell still has to draw

Written for: whoever owns the iOS, Android and desktop shells. Issues 204–206 were
fixed on web; the core changes below ship to every shell at its next build.
**Nothing here breaks a build or the wire** — every addition is a new view field,
a new type behind one, or a new event variant. But until a shell reads them, its
screen stays as quiet as web's was.

## Why

One defect, several times over: the core computed a verdict AND its reasons, and
the shell read only the verdict. A dark button with nothing beside it. Web read
`BatchView.can_apply` and none of `over_balance`, `over_cap`, `total_token`,
`total_fiat`, `file_error`, `template_saved`, `priced`, `busy`, `file_name`.
Those were always in the view — check that your shell reads them. The fields
below are the reasons the core did NOT yet give.

## New in `BatchView` (importer)

| Field | Meaning | Draw it as |
| --- | --- | --- |
| `errors: [{ line, raw, reason }]` | The lines the parser refused, in source order. `reason` is `no_address` or `no_amount`. `line` uses the same numbering as `preview[].line`, so the two lists interleave. `raw` is clipped to 120 characters; the list to 100 lines. `rejected` still counts them all, exactly as before, and they are still NOT in `preview`. | A skipped row showing `raw` (mono) and the reason: `send.batchBadAddress` / `send.badAmount`. |

## New in `SendView` (split form)

| Field | Meaning | Draw it as |
| --- | --- | --- |
| `split_row_issues: [{ id, ordinal, address, amount }]` | The rows `Continue` will not take. Each field is `ok`, `empty` or `invalid`. Empty list ⇔ the rows pass the gate. | `invalid` → a sentence on the row in the error colour (`send.batchBadAddress`, `send.badAmount`). `empty` → nothing on the row (it is unfinished, not wrong). Under the total, name the FIRST listed row: `send.splitNeedsAddress` / `send.splitNeedsAmount` with `{{n}}` = `ordinal`. |
| `split_remaining: string \| null` | Balance less the rows' sum, token units. `null` while a row cannot be summed or the sum is over the balance. The fee is not held back. | `send.splitRemaining` ("2.5 ETH left") beside the total. |
| `split_import_room: number` | The cap (60) less the rows already started. | Open the importer with this as `max_recipients`, not a flat 60. |

## Behaviour that changed

- **`can_continue` in a split is now truthful.** An amount was judged by its
  leading digits, so `1,5` armed the button; the sum could not read it and
  `Continue` returned without a word. The gate now closes and
  `split_row_issues` says which row. If your shell showed an armed button that
  did nothing, it no longer can.
- **`TapMax` does nothing in a split.** It wrote the hidden single amount and
  could start a fee estimate for it. Hide the Max button in a split (desktop
  already does); if you leave it, it is now inert instead of misleading.
- **`amount_warning` is `null` in a split.** It judged the single-send figure a
  split leaves behind. `split_over_balance` and `split_row_issues` are the
  split's own verdicts.
- **`EnterSplitMode` twice no longer wipes the rows.** It rebuilt them as
  `[the single recipient, blank]` every call. Desktop's "+ Add recipient" sends
  this event while already in a split (`wallet/page.rs:4301` overwrites the
  handler set at `:4260`) — that press used to delete an imported list. The core
  now ignores it; the desktop handler should still be fixed to append a row.

## New event

`{"type":"append_split_recipients","recipients":[…]}` — the imported (or
whole-group) rows are ADDED to the rows already started; blank rows are dropped;
every appended row gets a fresh id (send `""`); the cap still holds.
`seed_split_recipients` is unchanged and still replaces. Web now appends by
default and offers "Replace them instead" in the importer
(`send.batchAddsToRows` / `batchReplacesRows` / `batchReplaceInstead` /
`batchAddInstead`). A shell that keeps sending the old event keeps the old
behaviour — but then it should SAY it replaces.

## New copy (all 15 locales, `send.*`)

`batchUnitCaption`, `batchTokenHint`, `batchAddsToRows`, `batchReplacesRows`,
`batchReplaceInstead`, `batchAddInstead`, `badAmount`, `splitNeedsAddress`,
`splitNeedsAmount`, `splitRemaining`, `splitFillEmpty`. Also `zh-HK`
`batchRateHint` and `batchParsedCount` were rewritten out of spoken Cantonese.

## Two things web does in the shell that you may want

- **Decimal commas in editable figures** (rate, row amount, single amount): the
  core speaks dot-decimal and strips the rest, so `7,5` typed under a comma
  preset was applied as 75. Web shows the core's figure in the person's decimal
  mark and hands back a dot. Under a DOT preset a comma is passed through
  untouched — stripping makes `1,5` pay 15, converting makes `1,500` pay 1.5 —
  and the core's refusal (above) is what the person sees.
- **"Use 0.5 ETH for the empty rows"**: one press copies the first accepted
  amount into every row with none (`recipients_changed`). A group picked from
  the book arrives with no amounts at all.

Reference implementation: `app-web/vela-wallet/src/lib/flows/live-batch.ts`,
`live-send.ts`, `screens/BatchImport.svelte`, `ui/RecipientCard.svelte`.
Core tests: `rust/crates/vela-core/tests/app_send.rs` ("The split says what it is
doing"), `app_batch_import.rs` ("refused lines").
