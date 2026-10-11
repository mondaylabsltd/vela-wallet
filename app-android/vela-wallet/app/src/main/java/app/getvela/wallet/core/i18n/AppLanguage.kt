package app.getvela.wallet.core.i18n

/**
 * The app's language, as its strings engine resolved it ("zh", "zh-TW",
 * "en"…) — for a formatter that has no strings in hand.
 *
 * One thing reads it today: the symbol a fiat figure is written with
 * (`fiatSymbol(code, language)`, the core's rule). "¥" alone belongs to the
 * reader's own of the two currencies that share it — the yuan in Chinese, the
 * yen everywhere else — so the same balance is 「¥188.44」 in Chinese and
 * "CN¥188.44" in English, and a figure has to know which it is drawn in.
 *
 * Set by the app's own [I18nRuntime], BEFORE it publishes a language change:
 * whatever redraws because the language changed finds this already changed.
 * A holder, like `Formats.current`: rendering-only state, not a machine. A
 * builder with strings in hand passes `strings.language` instead.
 */
object AppLanguage {
    @Volatile
    var tag: String = I18nRuntime.FALLBACK
}
