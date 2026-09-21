# Tasks — 073 Amount text

- [x] T001 Core `l10n::amount_text` (clean, caret, preset_of, Entry) — the web suite's cases + the native reading; 15 tests
- [x] T002 UniFFI + wasm `amountTextClean` / `amountTextCaret`
- [x] T003 Web: AmountInput through the core; `amount-text.ts` deleted, its suite runs through the shipped wasm; the gallery and the component tests load the core
- [x] T004 Web: `amount-field.ts`; split rows and the allowance editor cleaned — browser tests fail without it (5/5)
- [x] T005 Android: `cleanAmountEdit` in the send amount, split share, custom cap; JVM 644
- [x] T006 iOS: `AmountText.clean` in the amount draft, `SplitRows`, the cap field; unit 723; UI 2/2 (German keypad) twice
- [x] T007 Desktop: `amount_edited` in the send amount, split share, custom cap; 493
- [x] T008 Android phone pass (see quickstart)
- [x] T009 iOS: `theListenerIsOnTheLoopbackOnly` (071) made robust to a parallel suite's wildcard listener on the same port

## Later
- iOS: the send form's balance line prints "0.46767" under a decimal-comma preset while the fiat line prints "¥3,35" (seen on the simulator).
