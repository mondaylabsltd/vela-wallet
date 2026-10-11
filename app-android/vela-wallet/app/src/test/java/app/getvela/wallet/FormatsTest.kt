package app.getvela.wallet

import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TimeFormatKey
import java.util.Locale
import java.util.TimeZone
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The web's presets, read back (spec 047 D2). */
class FormatsTest {
    private val noon = run {
        val c = java.util.Calendar.getInstance(TimeZone.getTimeZone("UTC"))
        c.set(2026, 5, 13, 13, 45, 0); c.set(java.util.Calendar.MILLISECOND, 0); c.timeInMillis
    }

    @Test
    fun `the four number styles`() {
        assertEquals("1,234,567.89", Formats(NumberFormatKey.CommaDot).number("1234567.891", 2, 2))
        assertEquals("1.234.567,89", Formats(NumberFormatKey.DotComma).number("1234567.891", 2, 2))
        assertEquals("1 234 567,89", Formats(NumberFormatKey.SpaceComma).number("1234567.891", 2, 2))
        assertEquals("12,34,567.89", Formats(NumberFormatKey.Indian).number("1234567.891", 2, 2))
        assertEquals("0.001", Formats(NumberFormatKey.CommaDot).number("0.0010"))
        assertEquals("-1,000", Formats(NumberFormatKey.CommaDot).number("-1000"))
        // Money: the exact binary value rounded HALF UP — the web's
        // `toFixed(2)`, digit for digit (spec 078 round 2). Never cut: 12.34
        // (really 12.3399…) stays 12.34.
        val money = Formats(NumberFormatKey.CommaDot)
        assertEquals("0.45", money.fixed2(0.449))
        assertEquals("0.44", money.fixed2(0.444))
        assertEquals("12.34", money.fixed2(12.34))
        assertEquals("1.00", money.fixed2(1.005))
        assertEquals("2.67", money.fixed2(2.675))
        assertEquals("110.44", money.fixed2(110.445))
        assertEquals("0.13", money.fixed2(0.125))
        assertEquals("abc", Formats(NumberFormatKey.CommaDot).number("abc"))
    }

    @Test
    fun `auto follows the locale`() {
        assertEquals(NumberFormatKey.CommaDot, Formats(locale = Locale.US).resolvedNumber())
        assertEquals(NumberFormatKey.DotComma, Formats(locale = Locale.GERMANY).resolvedNumber())
        assertEquals(NumberFormatKey.SpaceComma, Formats(locale = Locale.FRANCE).resolvedNumber())
        assertEquals(NumberFormatKey.Indian, Formats(locale = Locale("en", "IN")).resolvedNumber())
        assertEquals(DateFormatKey.MdySlash, Formats(locale = Locale.US).resolvedDate())
        assertEquals(TimeFormatKey.H12, Formats(locale = Locale.US).resolvedTime())
        assertEquals(TimeFormatKey.H24, Formats(locale = Locale.GERMANY).resolvedTime())
    }

    @Test
    fun `the date shapes and the two clocks`() {
        val utc = Locale.US
        val f = { d: DateFormatKey, t: TimeFormatKey -> Formats(date = d, time = t, locale = utc) }
        val tz = TimeZone.getDefault(); TimeZone.setDefault(TimeZone.getTimeZone("UTC"))
        try {
            assertEquals("2026/06/13", f(DateFormatKey.YmdSlash, TimeFormatKey.H24).date(noon))
            assertEquals("06/13/2026", f(DateFormatKey.MdySlash, TimeFormatKey.H24).date(noon))
            assertEquals("13/06/2026", f(DateFormatKey.DmySlash, TimeFormatKey.H24).date(noon))
            assertEquals("13.06.2026", f(DateFormatKey.DmyDot, TimeFormatKey.H24).date(noon))
            assertEquals("2026-06-13", f(DateFormatKey.Iso, TimeFormatKey.H24).date(noon))
            assertEquals("13:45", f(DateFormatKey.Iso, TimeFormatKey.H24).time(noon))
            assertEquals("1:45 PM", f(DateFormatKey.Iso, TimeFormatKey.H12).time(noon).plain())
            // PR 3 final note F5 — the clock is the CORE's, in the language's
            // own day-period convention: this shell wrote "1:45 PM" in every
            // language, and Settings → Time format showed that as its Chinese
            // example while the core's own lines said 「下午」.
            fun clock(tag: String, key: TimeFormatKey = TimeFormatKey.H12) =
                Formats(date = DateFormatKey.Iso, time = key, locale = Locale.forLanguageTag(tag)).time(noon).plain()
            assertEquals("下午 1:45", clock("zh-CN"))
            assertEquals("下午 1:45", clock("zh"))
            assertEquals("午後 1:45", clock("ja"))
            assertEquals("ÖS 1:45", clock("tr"))
            assertEquals("13:45", clock("zh-CN", TimeFormatKey.H24))
            // The morning, and midnight's twelve.
            assertEquals("上午 9:05", Formats(time = TimeFormatKey.H12, locale = Locale.SIMPLIFIED_CHINESE).time(noon - (4 * 60 + 40) * 60_000L).plain())
            assertEquals("12:10 AM", f(DateFormatKey.Iso, TimeFormatKey.H12).time(noon - (13 * 60 + 35) * 60_000L).plain())
            // The language can be named apart from the format's locale: the app's.
            assertEquals("下午 1:45", f(DateFormatKey.Iso, TimeFormatKey.H12).time(noon, "zh").plain())
            // One unit on a line: the core's no-break space, and a joiner beside a CJK period.
            assertEquals("1:45\u00A0PM", f(DateFormatKey.Iso, TimeFormatKey.H12).time(noon))
            assertTrue(Formats(time = TimeFormatKey.H12, locale = Locale.SIMPLIFIED_CHINESE).time(noon).contains('\u00A0'))
            assertEquals("2026-06-13 13:45", f(DateFormatKey.Iso, TimeFormatKey.H24).dateTime(noon))
        } finally {
            TimeZone.setDefault(tz)
        }
    }

    /** A time as read: the core's no-break space as a space, its zero-width joiners gone. */
    private fun String.plain(): String = replace('\u00A0', ' ').replace("\u2060", "")
}
