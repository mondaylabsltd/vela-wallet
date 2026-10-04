package app.getvela.wallet.core.format

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import uniffi.vela_core_uniffi.amountTextCaret
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

/**
 * The same edit for a field that holds its caret, which every amount field
 * does (issue #421): what the field shows next, or `null` — refused, and the
 * field keeps [previous].
 *
 * The core's rule can now ADD a character as well as drop one: "." is "0.",
 * as "08" is "8". A `String` field keeps the caret where the key left it, so
 * after "." it sat between the "0" and the ".", and the next "5" landed in
 * front of the point — "05." cleaned to "5.", five where 0.5 was being typed.
 * So the caret goes where the core says ([amountTextCaret]), counted the same
 * way on every client. An edit that cleans to itself is the field's own,
 * caret and composition untouched; a caret move is not an edit at all.
 */
fun cleanAmountFieldEdit(next: TextFieldValue, previous: TextFieldValue): TextFieldValue? {
    if (next.text == previous.text) return next
    val clean = cleanAmountEdit(next.text, previous.text) ?: return null
    if (clean == next.text) return next
    val caret = amountTextCaret(next.text, clean, next.selection.end.toUInt()).toInt()
    return next.copy(text = clean, selection = TextRange(caret))
}

/** A figure set from outside the field (Max, ⇄, the core's echo): the caret after it. */
fun amountFieldOf(text: String): TextFieldValue = TextFieldValue(text, TextRange(text.length))
