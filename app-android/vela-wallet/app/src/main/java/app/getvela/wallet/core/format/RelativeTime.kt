package app.getvela.wallet.core.format

import app.getvela.wallet.core.i18n.VelaStrings
import java.util.TimeZone
import kotlin.math.floor

/**
 * The hero's "Updated <ago>" (issue 462): the core's relative time —
 * `I18n::format_relative_time` through [VelaStrings.relativeTime] — so
 * "now", "2m", "3h", the weekday under a week and the date past it are the
 * core's rule in every language, on every shell. This keeps no copy of it.
 *
 * What it adds is only what the device knows: the moment in whole seconds,
 * the zone's offset at that moment, and the person's date preset with `auto`
 * resolved from the locale (the desktop's `loc.relative_time` does the same).
 */
object RelativeTime {
    fun ago(
        atMs: Double,
        nowMs: Long,
        strings: VelaStrings,
        formats: Formats = Formats.current,
        zone: TimeZone = TimeZone.getDefault(),
    ): String {
        val tsSeconds = floor(atMs / 1000.0).toLong()
        return strings.relativeTime(
            tsSeconds = tsSeconds,
            nowMs = nowMs,
            utcOffsetMinutes = zone.getOffset(tsSeconds * 1000L) / 60_000,
            dateFormat = formats.resolvedDate().wire,
        )
    }
}
