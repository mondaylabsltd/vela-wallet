package app.getvela.wallet.feature.settings

import app.getvela.wallet.feature.settings.core.CurrencyCatalog
import app.getvela.wallet.feature.settings.core.CurrencyView

/**
 * The live settings builders: what the core has ruled → the display models the
 * drawn components already accept.
 *
 * The sibling of [SettingsFixtures], and the division between them is the
 * point: fixtures are the canon the gallery renders, so they keep every state
 * including the ones no wallet is currently in. This file renders **one**
 * state — the person's.
 *
 * It grows a machine at a time. Spec 040 phase 3 brings the display currency;
 * phase 5 brings the networks, endpoints and providers. Anything a builder here
 * has not claimed yet is still fixture-fed and says so at its call site.
 */
object SettingsLive {

    /**
     * The display currency, as the settings surface shows it in two places:
     * the localisation row's value, and which row the currency sheet ticks.
     *
     * **What this does not do is convert anything.** `view.rate` is `null`
     * until spec 041 gives the shell a way to price a currency, and the row
     * shows the code alone rather than a converted sample when it is. Showing
     * "¥1,234.56" against a rate nobody could fetch would be inventing an
     * exchange rate on the settings screen.
     */
    fun withCurrency(model: SettingsScreenModel, view: CurrencyView): SettingsScreenModel {
        val code = view.code
        return model.copy(
            sections = model.sections.map { section ->
                section.copy(
                    rows = section.rows.map { row ->
                        if (row.id == "currency") row.copy(value = currencyRowValue(view)) else row
                    },
                )
            },
            currencySheet = model.currencySheet.copy(
                rows = CurrencyCatalog.entries.map { entry ->
                    SelectRowModel(
                        id = entry.code,
                        label = entry.code,
                        glyph = entry.glyph,
                        caption = entry.caption,
                        selected = entry.code == code,
                    )
                },
            ),
        )
    }

    /**
     * The row's right-hand value.
     *
     * `committed == false` means the core is still showing its USD placeholder
     * rather than a settled choice — the row says the code it would use and
     * nothing more, because a sample amount implies a rate and there is not one
     * yet.
     */
    private fun currencyRowValue(view: CurrencyView): String =
        "${view.code} · ${CurrencyCatalog.glyph(view.code)}"
}
