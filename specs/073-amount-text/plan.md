# Implementation Plan: 073 — Amount text

**Branch**: `073-amount-text` | **Spec**: [spec.md](spec.md) | **Tasks**: [tasks.md](tasks.md)

## The shared surface

| | Core | UniFFI (Kotlin / Swift) | wasm | Desktop |
|---|---|---|---|---|
| Clean | `l10n::amount_text::clean` | `amountTextClean(raw, number, previous, pasted?)` | `amountTextClean` | direct |
| Caret | `caret_after_clean` | `amountTextCaret` | `amountTextCaret` | — (no caret in its field) |

`number` is the resolved preset key each shell already has (`comma_dot`,
`dot_comma`, `space_comma`, `indian`); `pasted` is `null` on native fields.

## Per shell

| Shell | Helper | Fields |
|---|---|---|
| Web | `flows/amount-field.ts` (`takeAmount`, `composing`, `pasted`) | `AmountInput`, `RecipientCard`, `AllowanceEditor` |
| Android | `core/format/AmountText.kt` (`cleanAmountEdit`) | `FlowBlocks` send amount, `FlowRows` split share, `SigningComponents` custom cap |
| iOS | `Core/AmountText.swift` (`AmountText.clean`) | `RootView` amount draft, `SplitRows.amountEdited`, `AllowanceEditorView` |
| Desktop | `flows::live::amount_edited` | `page.rs` send amount, `split_amount_edited`, custom cap |

Why in the field and not in the machine: see spec FR-003.

## Gate
Core tests (15), each shell's suite, the phone and the simulator.
