package app.getvela.wallet.core.format

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import kotlin.math.floor
import kotlin.math.roundToLong

/**
 * The core's compact relative time — "now", "2m", "3h" — for the hero's
 * "Updated <ago>" (issue 462).
 *
 * The rule is `I18n::format_relative_time` in vela-core (the desktop calls
 * it directly through `loc.relative_time`); the uniffi surface exports no
 * formatter, so these are its first three branches over the same corpus
 * keys (`time.now`, `time.minutesShort`, `time.hoursShort`) and the same
 * arithmetic: whole seconds, under 45 s is "now", minutes and hours rounded.
 * Past a day the core names a weekday, then a date; this draws the person's
 * own date format for both — a refresh a day old on a visible home is the
 * rare case, and a date is never wrong where a weekday could be.
 */
object RelativeTime {
    fun ago(atMs: Double, nowMs: Long, strings: VelaStrings, formats: Formats = Formats.current): String {
        val diff = (Math.floorDiv(nowMs, 1000L) - floor(atMs / 1000.0).toLong()).coerceAtLeast(0L)
        return when {
            diff < NOW_SECONDS -> strings.t(I18nKeys.Wallet.TIME_NOW)
            diff < HOUR_SECONDS -> strings.t(
                I18nKeys.Wallet.TIME_MINUTES_SHORT,
                mapOf("n" to (diff / 60.0).roundToLong().toString()),
            )
            diff < DAY_SECONDS -> strings.t(
                I18nKeys.Wallet.TIME_HOURS_SHORT,
                mapOf("n" to (diff / 3_600.0).roundToLong().toString()),
            )
            else -> formats.date(atMs.toLong())
        }
    }

    private const val NOW_SECONDS = 45L
    private const val HOUR_SECONDS = 3_600L
    private const val DAY_SECONDS = 86_400L
}
