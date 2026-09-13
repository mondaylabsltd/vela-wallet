package app.getvela.wallet.feature.settings.core

/**
 * The currencies this wallet offers, and the glyph each one shows.
 *
 * **Shell content, not a core rule** — and deliberately so. The
 * `display_currency` machine decides *which* code is current and whether it
 * can be priced; it has no opinion about what a `¥` looks like or which eight
 * currencies a picker is worth filling with. The core's own comment says as
 * much: "the shell derives the symbol from its catalog and owns all
 * formatting".
 *
 * It lives here rather than in `SettingsFixtures` because live code must not
 * import from a fixture file: a fixture is a picture of a screen, and the
 * moment production reads one, deleting a picture breaks the product. Same
 * list, same order as `app-web/vela-wallet/src/lib/settings/fixtures.ts`, so
 * the two clients offer a person the same choices.
 */
object CurrencyCatalog {

    data class Entry(val code: String, val glyph: String, val caption: String)

    /**
     * Eight, in this order.
     *
     * The captions are English here and Chinese in the web sibling, which is a
     * real inconsistency — neither is localised — recorded as a debt in this
     * feature's results rather than fixed silently, because fixing it means
     * choosing i18n keys for currency names and that is a content decision.
     */
    val entries: List<Entry> = listOf(
        Entry("USD", "$", "US Dollar"),
        Entry("EUR", "€", "Euro"),
        Entry("GBP", "£", "British Pound"),
        Entry("CNY", "¥", "Chinese Yuan"),
        Entry("JPY", "¥", "Japanese Yen"),
        Entry("KRW", "₩", "South Korean Won"),
        Entry("HKD", "$", "Hong Kong Dollar"),
        Entry("VND", "₫", "Vietnamese Dong"),
    )

    /** The glyph for a code, or the code itself when it is one we do not stock. */
    fun glyph(code: String): String =
        entries.firstOrNull { it.code == code }?.glyph ?: code
}
