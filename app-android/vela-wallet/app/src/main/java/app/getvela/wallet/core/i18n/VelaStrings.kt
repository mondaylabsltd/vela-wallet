package app.getvela.wallet.core.i18n

import androidx.compose.runtime.staticCompositionLocalOf

/**
 * Translation access for UI code. The only implementation shipped in the app is
 * [I18nRuntime] (vela-core engine); previews substitute a sample-copy fake.
 */
interface VelaStrings {
    fun t(key: String): String

    fun t(key: String, vars: Map<String, String>): String

    /**
     * A PLURAL key: [count] chooses the form — `_one`, `_few`, `_many` or
     * `_other`, by the language's CLDR rule, in the core — and fills
     * `{{count}}`.
     *
     * The `vars` overload cannot stand in for this: it fills `{{count}}` as
     * text and never selects a form, so a plural key handed to it has no value
     * (issue #409). Never pick the suffix here either — `count == 1` is not
     * Russian's rule (2–4 is `_few`), nor Chinese's (one form for every count).
     */
    fun t(key: String, count: Int): String

    /**
     * How long ago, in the active language — "now", "2m", "3h", a short
     * weekday under a week, else the date: the core's
     * `I18n::format_relative_time`, which no shell ports (issue 462).
     *
     * [tsSeconds] is the moment in WHOLE seconds (milliseconds here read as
     * the future, which is "now"); [nowMs] the clock in milliseconds;
     * [utcOffsetMinutes] what to add to UTC for local time at that moment;
     * [dateFormat] the person's date preset as stored, `auto` already
     * resolved. `RelativeTime.ago` fills all four from the device.
     */
    fun relativeTime(tsSeconds: Long, nowMs: Long, utcOffsetMinutes: Int, dateFormat: String): String
}

val LocalVelaStrings = staticCompositionLocalOf<VelaStrings> {
    error("VelaStrings not provided — wrap content in CompositionLocalProvider(LocalVelaStrings provides …)")
}
