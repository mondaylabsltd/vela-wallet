package app.getvela.wallet.core.format

import androidx.compose.runtime.mutableStateOf
import java.math.BigDecimal
import java.math.RoundingMode
import java.text.DecimalFormatSymbols
import java.util.Calendar
import java.util.Locale

/*
 * The web's `locale-format.ts`, ported (spec 047 D2): four number styles,
 * five date shapes, two clocks, and `auto` resolved from the locale — the
 * product's own presets, never `Intl`/`NumberFormat` deciding on their own
 * (project rule: explicit format presets). Nothing here decides money; it
 * only draws figures the core already computed.
 */

enum class NumberFormatKey(val wire: String) {
    Auto("auto"), CommaDot("comma_dot"), DotComma("dot_comma"), SpaceComma("space_comma"), Indian("indian");
    companion object { fun of(wire: String?): NumberFormatKey = entries.firstOrNull { it.wire == wire } ?: Auto }
}

enum class DateFormatKey(val wire: String) {
    Auto("auto"), YmdSlash("ymd_slash"), MdySlash("mdy_slash"), DmySlash("dmy_slash"), DmyDot("dmy_dot"), Iso("iso");
    companion object { fun of(wire: String?): DateFormatKey = entries.firstOrNull { it.wire == wire } ?: Auto }
}

enum class TimeFormatKey(val wire: String) {
    Auto("auto"), H24("h24"), H12("h12");
    companion object { fun of(wire: String?): TimeFormatKey = entries.firstOrNull { it.wire == wire } ?: Auto }
}

/** The six text-scale levels and their factors (the web's `TEXT_SCALE_LEVELS`). */
enum class TextScaleLevel(val wire: String, val factor: Float) {
    Compact("compact", 0.82f), Small("small", 0.91f), Standard("standard", 1.0f),
    Comfortable("comfortable", 1.1f), Large("large", 1.22f), XLarge("xlarge", 1.35f);
    companion object { fun of(wire: String?): TextScaleLevel = entries.firstOrNull { it.wire == wire } ?: Standard }
}

class Formats(
    val number: NumberFormatKey = NumberFormatKey.Auto,
    val date: DateFormatKey = DateFormatKey.Auto,
    val time: TimeFormatKey = TimeFormatKey.Auto,
    val locale: Locale = Locale.getDefault(),
) {
    private class NumberStyle(val group: String, val decimal: String, val indian: Boolean)

    private fun style(): NumberStyle = when (resolvedNumber()) {
        NumberFormatKey.CommaDot -> NumberStyle(",", ".", false)
        NumberFormatKey.DotComma -> NumberStyle(".", ",", false)
        NumberFormatKey.SpaceComma -> NumberStyle(" ", ",", false)
        NumberFormatKey.Indian -> NumberStyle(",", ".", true)
        NumberFormatKey.Auto -> NumberStyle(",", ".", false)
    }

    /** `auto` → what the locale itself groups and points with (the web's `detectNumber`). */
    fun resolvedNumber(): NumberFormatKey {
        if (number != NumberFormatKey.Auto) return number
        val symbols = DecimalFormatSymbols.getInstance(locale)
        val group = symbols.groupingSeparator
        val decimal = symbols.decimalSeparator
        return when {
            decimal == ',' && group == '.' -> NumberFormatKey.DotComma
            decimal == ',' -> NumberFormatKey.SpaceComma
            locale.country.equals("IN", ignoreCase = true) || locale.language == "hi" -> NumberFormatKey.Indian
            else -> NumberFormatKey.CommaDot
        }
    }

    fun resolvedDate(): DateFormatKey {
        if (date != DateFormatKey.Auto) return date
        val pattern = runCatching {
            (java.text.DateFormat.getDateInstance(java.text.DateFormat.SHORT, locale) as? java.text.SimpleDateFormat)?.toPattern()
        }.getOrNull().orEmpty()
        val order = pattern.filter { it == 'y' || it == 'M' || it == 'd' }.map { it.lowercaseChar() }.distinct().joinToString("")
        val sep = pattern.firstOrNull { it == '/' || it == '-' || it == '.' } ?: '/'
        return when (order) {
            "ymd" -> if (sep == '-') DateFormatKey.Iso else DateFormatKey.YmdSlash
            "dmy" -> if (sep == '.') DateFormatKey.DmyDot else DateFormatKey.DmySlash
            else -> DateFormatKey.MdySlash
        }
    }

    fun resolvedTime(): TimeFormatKey {
        if (time != TimeFormatKey.Auto) return time
        val pattern = runCatching {
            (java.text.DateFormat.getTimeInstance(java.text.DateFormat.SHORT, locale) as? java.text.SimpleDateFormat)?.toPattern()
        }.getOrNull().orEmpty()
        return if (pattern.contains('a')) TimeFormatKey.H12 else TimeFormatKey.H24
    }

    /** Digits grouped by the style: `1234567` → `1,234,567` / `1.234.567` / `12,34,567`. */
    fun groupDigits(digits: String): String {
        val style = style()
        if (digits.length <= 3) return digits
        if (!style.indian) return digits.reversed().chunked(3).joinToString(style.group).reversed()
        val last3 = digits.takeLast(3)
        val head = digits.dropLast(3).reversed().chunked(2).joinToString(style.group).reversed()
        return head + style.group + last3
    }

    /** A decimal string (`1234.5`) drawn with the style's separators, `minFraction..maxFraction` places, rounding down (money never rounds up). */
    fun number(plain: String, minFraction: Int = 0, maxFraction: Int = 8): String {
        val value = plain.toBigDecimalOrNull() ?: return plain
        return number(value, minFraction, maxFraction)
    }

    fun number(value: BigDecimal, minFraction: Int = 0, maxFraction: Int = 8): String {
        val style = style()
        val scaled = value.setScale(maxFraction, RoundingMode.DOWN).stripTrailingZeros()
        val fixed = if (scaled.scale() < minFraction) scaled.setScale(minFraction) else scaled
        val text = fixed.toPlainString()
        val negative = text.startsWith("-")
        val body = text.removePrefix("-")
        val dot = body.indexOf('.')
        val intPart = if (dot >= 0) body.substring(0, dot) else body
        val frac = if (dot >= 0) body.substring(dot + 1) else ""
        val grouped = groupDigits(intPart)
        val out = if (frac.isNotEmpty()) grouped + style.decimal + frac else grouped
        return if (negative) "-$out" else out
    }

    /** Money's two places: `1234.5` → `1,234.50`. */
    fun fixed2(value: Double): String = number(BigDecimal(value), minFraction = 2, maxFraction = 2)

    fun date(epochMs: Long): String {
        val c = Calendar.getInstance(locale).apply { timeInMillis = epochMs }
        val y = c.get(Calendar.YEAR).toString()
        val m = (c.get(Calendar.MONTH) + 1).toString().padStart(2, '0')
        val d = c.get(Calendar.DAY_OF_MONTH).toString().padStart(2, '0')
        return when (resolvedDate()) {
            DateFormatKey.YmdSlash -> "$y/$m/$d"
            DateFormatKey.MdySlash -> "$m/$d/$y"
            DateFormatKey.DmySlash -> "$d/$m/$y"
            DateFormatKey.DmyDot -> "$d.$m.$y"
            DateFormatKey.Iso -> "$y-$m-$d"
            DateFormatKey.Auto -> "$y-$m-$d"
        }
    }

    fun time(epochMs: Long): String {
        val c = Calendar.getInstance(locale).apply { timeInMillis = epochMs }
        val minute = c.get(Calendar.MINUTE).toString().padStart(2, '0')
        return when (resolvedTime()) {
            TimeFormatKey.H12 -> {
                val h = c.get(Calendar.HOUR).let { if (it == 0) 12 else it }
                "$h:$minute ${if (c.get(Calendar.AM_PM) == Calendar.AM) "AM" else "PM"}"
            }
            else -> "${c.get(Calendar.HOUR_OF_DAY).toString().padStart(2, '0')}:$minute"
        }
    }

    fun dateTime(epochMs: Long): String = "${date(epochMs)} ${time(epochMs)}"

    /** The resolved preset's decimal mark. */
    fun decimalMark(): String = style().decimal

    /**
     * A plain decimal string (`0.001`) with the preset's decimal mark and NO
     * grouping — the web's `trimBalance` rule for token amounts: a decimal
     * comma read as a thousands separator is a hundredfold mistake, but the
     * mocks draw token amounts ungrouped. Money (`number`/`fixed2`) gets both.
     */
    fun plain(text: String): String {
        val mark = decimalMark()
        return if (mark == ".") text else text.replaceFirst(".", mark)
    }

    /** The settings row's sample — the web's `formatNumber(1234567.89, 2, 2)`. */
    fun example(): String = number(BigDecimal("1234567.89"), 2, 2)
    fun dateExample(): String = date(sampleMs())
    fun timeExample(): String = time(sampleMs())

    /** 2026-06-13 13:45 local — the web's `FORMAT_SAMPLE`. */
    private fun sampleMs(): Long = Calendar.getInstance(locale).run {
        clear()
        set(2026, Calendar.JUNE, 13, 13, 45, 0)
        timeInMillis
    }

    companion object {
        /**
         * The process-wide formats the live builders read (spec 047 D2). Set
         * by the preferences once loaded and on every change; tests set it
         * explicitly. Rendering-only state, so a holder rather than a machine.
         *
         * Spec 049: Compose state, not a `@Volatile` field. A screen that read
         * a static holder was drawn once and never told the preset changed —
         * the founder picked `1.234.567,89` and the home kept its dots until
         * the next balance refresh. Read in composition this subscribes the
         * reader; a `remember` that bakes a figure in must key on it too.
         */
        private val holder = mutableStateOf(Formats())

        var current: Formats
            get() = holder.value
            set(value) {
                holder.value = value
            }
    }
}
