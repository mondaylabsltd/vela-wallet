package app.getvela.wallet

import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.cleanAmountEdit
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Spec 073: every Compose amount field cleans its edit through the core's
 * rule, with the field's own text as `previous` and no paste flag (a native
 * field cannot say). A decimal-comma pad's "4,5" went out raw: 4 in fiat mode,
 * an allowance of 45 on a custom cap.
 */
class AmountTextTest {
    private val saved = Formats.current

    @After fun restore() {
        Formats.current = saved
    }

    @Test fun `a decimal-comma preset reads the comma as the decimal mark`() {
        Formats.current = Formats(NumberFormatKey.DotComma)
        assertEquals("4.", cleanAmountEdit("4,", "4"))
        assertEquals("4.5", cleanAmountEdit("4.5", "4."))
    }

    @Test fun `one typed comma is the decimal mark when the pad and the preset disagree`() {
        // The phone's region gives the pad "," while the app reads comma-dot.
        Formats.current = Formats(NumberFormatKey.CommaDot)
        assertEquals("4.", cleanAmountEdit("4,", "4"))
        // Typed into a figure that has its point: the comma is dropped.
        assertEquals("12.5", cleanAmountEdit("1,2.5", "12.5"))
    }

    @Test fun `more than one character at once reads as a paste`() {
        Formats.current = Formats(NumberFormatKey.CommaDot)
        assertEquals("1234.56", cleanAmountEdit("1.234,56", ""))
        // A tiny balance as many screens print it: refused, never 1.57.
        assertNull(cleanAmountEdit("1.5e-7", ""))
        assertNull(cleanAmountEdit("0x10", "2"))
    }

    @Test fun `a clean figure is left as typed`() {
        Formats.current = Formats(NumberFormatKey.DotComma)
        for (text in listOf("", "0", "4", "4.", ".5", "0.50", "53.4836")) {
            assertEquals(text, cleanAmountEdit(text, text.dropLast(1)))
        }
    }
}
