package app.getvela.wallet.core.format

/** How a figure lands on the ladder's last place. */
enum class TokenRounding {
    /** Every figure that is READ: a balance, an amount sent. */
    HalfUp,

    /**
     * A CEILING the person may type back ("you can send up to …"): rounded
     * up it would be a figure the balance cannot cover.
     */
    Down,
}

/**
 * A token amount as every screen writes it — ONE rule on all four shells
 * (spec 078): the core's ladder, digit for digit (`l10n::number::
 * format_token_amount`, the Max figure's `send::max_figure`, the web's
 * `tokenAmountText`).
 *
 * - 6 places under 1, 4 under 1000, 2 from 1000;
 * - rounded half up (or [TokenRounding.Down] for a ceiling), trailing zeros
 *   dropped;
 * - an amount too small to survive six places keeps two significant digits,
 *   cut, instead of reading `0`;
 * - the preset's decimal mark, never grouped (a decimal comma read as a
 *   thousands separator is a hundredfold mistake).
 *
 * String arithmetic only — never a `Double`: an 18-decimal balance must not
 * pass through binary floating point on its way to the screen. Anything that
 * is not a plain non-negative decimal comes back as it was given.
 *
 * The asset list's rows, the Send picker, the token card's balance, the
 * single-send confirm figure, the receipt, the sweep rows and the "≈ token"
 * line all call THIS, so one holding never reads as two numbers.
 */
fun tokenAmountText(
    amount: String,
    rounding: TokenRounding = TokenRounding.HalfUp,
    decimalMark: String = Formats.current.decimalMark(),
): String {
    val exact = amount.trim()
    val dot = exact.indexOf('.')
    val intRaw = if (dot == -1) exact else exact.substring(0, dot)
    val fracRaw = if (dot == -1) "" else exact.substring(dot + 1)
    if ((intRaw.isEmpty() && fracRaw.isEmpty()) || !intRaw.all { it in '0'..'9' } || !fracRaw.all { it in '0'..'9' }) {
        return exact
    }
    val intDigits = intRaw.trimStart('0')
    val places = when {
        intDigits.isEmpty() -> 6
        intDigits.length <= 3 -> 4
        else -> 2
    }
    val digits = (intDigits + fracRaw.padEnd(places, '0').take(places)).map { it - '0' }.toMutableList()
    val next = fracRaw.getOrNull(places)
    if (rounding == TokenRounding.HalfUp && next != null && next >= '5') {
        var i = digits.size
        while (true) {
            if (i == 0) {
                digits.add(0, 1)
                break
            }
            i -= 1
            if (digits[i] == 9) {
                digits[i] = 0
            } else {
                digits[i] += 1
                break
            }
        }
    }
    val split = digits.size - places
    val intPart = digits.subList(0, split).joinToString("").ifEmpty { "0" }
    val fracPart = digits.subList(split, digits.size).joinToString("").trimEnd('0')
    if (intPart == "0" && fracPart.isEmpty()) {
        // Below the ladder's last place: two significant digits, cut.
        val lead = fracRaw.takeWhile { it == '0' }.length
        if (lead == fracRaw.length) return "0"
        val keep = minOf(lead + 2, fracRaw.length)
        return "0$decimalMark${fracRaw.substring(0, keep).trimEnd('0')}"
    }
    return if (fracPart.isEmpty()) intPart else "$intPart$decimalMark$fracPart"
}
