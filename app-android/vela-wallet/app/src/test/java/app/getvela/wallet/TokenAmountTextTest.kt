package app.getvela.wallet

import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TokenRounding
import app.getvela.wallet.core.format.tokenAmountText
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The ONE token-amount rule (spec 078), pinned to the core's own vectors
 * (`app_send.rs::a_max_reads_on_the_balance_lines_ladder`) — the rule the
 * core's Max figure (`send::max_figure`), the desktop's rows
 * (`format_token_amount`) and the web's `tokenAmountText` all follow, so a
 * holding reads as one number on every shell and every screen.
 */
class TokenAmountTextTest {

    @Test
    fun `the core's vectors, digit for digit`() {
        listOf(
            "0.043790209243313861" to "0.04379",
            "0.0439686" to "0.043969",
            "1.22456789123456789" to "1.2246",
            "1234.567" to "1234.57",
            "2" to "2",
            "0" to "0",
            "0.9999996" to "1",
            "999.99996" to "1000",
            "0.0000001234" to "0.00000012",
            "5.000000" to "5",
        ).forEach { (exact, shown) -> assertEquals(exact, shown, tokenAmountText(exact, decimalMark = ".")) }
    }

    @Test
    fun `a ceiling the person may type back is rounded down`() {
        assertEquals("0.043969", tokenAmountText("0.0439699", TokenRounding.Down, decimalMark = "."))
        assertEquals("999.9999", tokenAmountText("999.99996", TokenRounding.Down, decimalMark = "."))
        assertEquals("1234.56", tokenAmountText("1234.567", TokenRounding.Down, decimalMark = "."))
    }

    @Test
    fun `the preset's decimal mark, never grouped, and anything else as it came`() {
        val saved = Formats.current
        Formats.current = Formats(NumberFormatKey.DotComma)
        try {
            assertEquals("1234,57", tokenAmountText("1234.567"))
            assertEquals("0,04379", tokenAmountText("0.043790209243313861"))
        } finally {
            Formats.current = saved
        }
        assertEquals("—", tokenAmountText("—", decimalMark = "."))
        assertEquals("1e-7", tokenAmountText("1e-7", decimalMark = "."))
        assertEquals("", tokenAmountText("", decimalMark = "."))
    }
}
