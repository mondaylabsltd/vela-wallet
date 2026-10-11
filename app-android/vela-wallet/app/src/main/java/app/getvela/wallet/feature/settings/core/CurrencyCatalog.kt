package app.getvela.wallet.feature.settings.core

/**
 * The currencies this wallet offers: their codes and their names.
 *
 * **Not their symbols.** What a currency's figure is written with is the
 * core's rule (`fiatSymbol(code, language)`), and Settings' row and picker
 * ask it like every figure does. This catalog used to keep a glyph per code —
 * "¥" for the yuan AND the yen, "$" for the US AND the Hong Kong dollar — so
 * two rows of one picker read alike, and the row said 「CNY · ¥」 over a
 * balance written 「CN¥188.44」.
 *
 * Which currencies a picker is worth filling with is shell content: the
 * `display_currency` machine decides which code is current and whether it can
 * be priced. It lives here rather than in `SettingsFixtures` because live
 * code must not import from a fixture file. Same list, same order as
 * `app-web/vela-wallet/src/lib/settings/fixtures.ts`.
 */
object CurrencyCatalog {

    data class Entry(val code: String, val caption: String)

    /**
     * Eight, in this order.
     *
     * The captions are English here and Chinese in the web sibling, which is a
     * real inconsistency — neither is localised — recorded as a debt in this
     * feature's results rather than fixed silently, because fixing it means
     * choosing i18n keys for currency names and that is a content decision.
     */
    val entries: List<Entry> = listOf(
        Entry("USD", "US Dollar"),
        Entry("EUR", "Euro"),
        Entry("GBP", "British Pound"),
        Entry("CNY", "Chinese Yuan"),
        Entry("JPY", "Japanese Yen"),
        Entry("KRW", "South Korean Won"),
        Entry("HKD", "Hong Kong Dollar"),
        Entry("VND", "Vietnamese Dong"),
    )
}
