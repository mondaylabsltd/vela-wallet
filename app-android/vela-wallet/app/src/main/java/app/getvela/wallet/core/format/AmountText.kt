package app.getvela.wallet.core.format

import uniffi.vela_core_uniffi.amountTextClean

/**
 * An amount field's edit, made readable for the core before it is sent on
 * (spec 073; the core's `l10n::amount_text` says why).
 *
 * A decimal-comma keypad's "4,5" reached the send machine raw, and in fiat
 * mode it was read as 4 — a different sum, silently; a custom allowance's
 * parser dropped the comma and allowed 45. Every Compose field that takes an
 * amount calls this in `onValueChange`, holds what it returns, and sends THAT
 * on, so what is on screen is what the machine has.
 *
 * `previous` is the field's text before the edit — how one keystroke is told
 * from a paste (a native field cannot say which it was, so anything but one
 * added character is read as a paste). `null` means a paste with no reading
 * as one figure ("1.5e-7"): the field keeps `previous` and nothing is sent.
 */
fun cleanAmountEdit(next: String, previous: String): String? =
    amountTextClean(next, Formats.current.resolvedNumber().wire, previous, null)
