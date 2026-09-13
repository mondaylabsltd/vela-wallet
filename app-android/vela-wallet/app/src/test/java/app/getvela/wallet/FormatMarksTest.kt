package app.getvela.wallet

import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TimeFormatKey
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Test

/** Spec 049: the mark, the plain token amount, and the settings examples. */
class FormatMarksTest {
    @Test
    fun `the decimal mark is the preset's`() {
        assertEquals(".", Formats(NumberFormatKey.CommaDot).decimalMark())
        assertEquals(",", Formats(NumberFormatKey.DotComma).decimalMark())
        assertEquals(",", Formats(NumberFormatKey.SpaceComma).decimalMark())
        assertEquals(".", Formats(NumberFormatKey.Indian).decimalMark())
        assertEquals(",", Formats(locale = Locale.GERMANY).decimalMark())
    }

    @Test
    fun `a token amount swaps the mark and keeps its digits ungrouped`() {
        assertEquals("0,001", Formats(NumberFormatKey.DotComma).plain("0.001"))
        assertEquals("1234567,5", Formats(NumberFormatKey.SpaceComma).plain("1234567.5"))
        assertEquals("1234567.5", Formats(NumberFormatKey.CommaDot).plain("1234567.5"))
        assertEquals("12", Formats(NumberFormatKey.DotComma).plain("12"))
        assertEquals("", Formats(NumberFormatKey.DotComma).plain(""))
    }

    @Test
    fun `the settings examples are the web's samples`() {
        assertEquals("1,234,567.89", Formats(NumberFormatKey.CommaDot).example())
        assertEquals("1.234.567,89", Formats(NumberFormatKey.DotComma).example())
        assertEquals("1 234 567,89", Formats(NumberFormatKey.SpaceComma).example())
        assertEquals("12,34,567.89", Formats(NumberFormatKey.Indian).example())
        assertEquals("1,234,567.89", Formats(locale = Locale.US).example())
        assertEquals("2026/06/13", Formats(date = DateFormatKey.YmdSlash).dateExample())
        assertEquals("13.06.2026", Formats(date = DateFormatKey.DmyDot).dateExample())
        assertEquals("1:45 PM", Formats(time = TimeFormatKey.H12).timeExample())
        assertEquals("13:45", Formats(time = TimeFormatKey.H24).timeExample())
    }

    @Test
    fun `the process-wide holder is settable and read back`() {
        val saved = Formats.current
        try {
            Formats.current = Formats(NumberFormatKey.DotComma)
            assertEquals("1.234,50", Formats.current.fixed2(1234.5))
        } finally {
            Formats.current = saved
        }
    }
}
