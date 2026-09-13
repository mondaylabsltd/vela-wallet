package app.getvela.wallet

import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TimeFormatKey
import app.getvela.wallet.feature.signing.core.ClearDateFormat
import app.getvela.wallet.feature.signing.core.ClearLocale
import app.getvela.wallet.feature.signing.core.ClearNumberFormat
import app.getvela.wallet.feature.signing.core.ClearTimeFormat
import java.util.Locale
import java.util.TimeZone
import org.junit.Assert.assertEquals
import org.junit.Test

/** Spec 049: the clear-signing locale is the person's presets, resolved, plus the phone's offset. */
class ClearLocaleTest {
    @Test
    fun `explicit presets map one to one`() {
        val locale = ClearLocale.fromFormats(Formats(NumberFormatKey.SpaceComma, DateFormatKey.DmyDot, TimeFormatKey.H12))
        assertEquals(ClearNumberFormat.SpaceComma, locale.number_format)
        assertEquals(ClearDateFormat.DmyDot, locale.date_format)
        assertEquals(ClearTimeFormat.H12, locale.time_format)
    }

    @Test
    fun `auto resolves before it reaches the core`() {
        val de = ClearLocale.fromFormats(Formats(locale = Locale.GERMANY))
        assertEquals(ClearNumberFormat.DotComma, de.number_format)
        assertEquals(ClearDateFormat.DmyDot, de.date_format)
        assertEquals(ClearTimeFormat.H24, de.time_format)
        val us = ClearLocale.fromFormats(Formats(locale = Locale.US))
        assertEquals(ClearNumberFormat.CommaDot, us.number_format)
        assertEquals(ClearDateFormat.MdySlash, us.date_format)
        assertEquals(ClearTimeFormat.H12, us.time_format)
    }

    @Test
    fun `the offset is minutes to add to UTC`() {
        val tz = TimeZone.getDefault()
        try {
            TimeZone.setDefault(TimeZone.getTimeZone("Asia/Shanghai"))
            assertEquals(480, ClearLocale.fromFormats(Formats(), nowMs = 1_800_000_000_000L).tz_offset_minutes)
            TimeZone.setDefault(TimeZone.getTimeZone("UTC"))
            assertEquals(0, ClearLocale.fromFormats(Formats(), nowMs = 1_800_000_000_000L).tz_offset_minutes)
        } finally {
            TimeZone.setDefault(tz)
        }
    }
}
