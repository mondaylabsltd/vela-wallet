package app.getvela.wallet.feature.settings

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.core.CurrencyCatalog
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetProbeHealth
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetProviderId
import app.getvela.wallet.feature.settings.core.NetView

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
     * The networks a person has, the endpoints they point at, and the provider
     * keys they hold — all from the core's store rather than from the eleven
     * chains the ST9 mock happens to draw.
     *
     * **Health is measured or it is absent.** `rpc_health` was `null` for the
     * whole of spec 040 and the rows carried no pill, because an unmeasured
     * endpoint drawn as fast is the one lie this screen is most able to tell by
     * accident. Spec 041 gives the shell a way to probe; a row still carries no
     * pill until its own probe has answered. (FR-013.)
     */
    /**
     * An endpoint's health, as a pill — or nothing.
     *
     * `null` means nobody has asked yet, and it stays blank. "Checking" is also
     * blank rather than a spinner in a list of twelve rows: a screenful of
     * spinners reads as a broken screen, and the answer arrives in about a
     * second.
     *
     * The wording and the one-second threshold are the drawn design's, taken
     * from `SettingsFixtures.latency` rather than reinvented — the same pill a
     * person has been looking at, now with a number somebody measured.
     */
    private fun healthPill(health: NetProbeHealth?, strings: VelaStrings): StatusPillModel? =
        when (health) {
            null, NetProbeHealth.Checking -> null
            is NetProbeHealth.Ok -> {
                val slow = health.latency_ms >= SLOW_MS
                val value = if (slow) {
                    "%.1fs".format(health.latency_ms / 1000.0)
                } else {
                    "${health.latency_ms}ms"
                }
                val word = strings.t(
                    if (slow) I18nKeys.SettingsUi.NETWORK_SLOW else I18nKeys.SettingsUi.NETWORK_ONLINE,
                )
                StatusPillModel(
                    tone = if (slow) SettingsTone.Warn else SettingsTone.Ok,
                    label = "$word · $value",
                )
            }
            NetProbeHealth.Error ->
                StatusPillModel(SettingsTone.Error, strings.t(I18nKeys.SettingsUi.NETWORK_OFFLINE))
        }

    /** A second is where the drawn design calls an endpoint slow. */
    private const val SLOW_MS = 1_000L

    fun withNetworks(
        model: SettingsScreenModel,
        view: NetView,
        strings: VelaStrings,
    ): SettingsScreenModel {
        if (!view.loaded) return model
        return model.copy(
            // The row that opens the list said "12 个网络" from the fixture
            // while the list beside it was live — so adding a custom network
            // left the count untouched. Found on a device (spec 040 phase 11).
            sections = model.sections.map { section ->
                section.copy(
                    rows = section.rows.map { row ->
                        if (row.id == "networks") {
                            row.copy(
                                value = row.value.orEmpty()
                                    .replaceFirst(Regex("\\d+"), view.networks.size.toString()),
                            )
                        } else {
                            row
                        }
                    },
                )
            },
            networks = view.networks.map { row ->
                NetworkRowModel(
                    id = row.id,
                    mark = ChainMarkModel(
                        letter = row.display_name.take(1).uppercase(),
                        colorArgb = markColour(row.chain_id),
                    ),
                    name = row.display_name,
                    meta = strings.t(
                        I18nKeys.SettingsUi.CHAIN_ID,
                        mapOf("chainId" to row.chain_id.toString()),
                    ),
                    badge = healthPill(row.rpc_health, strings),
                    tag = if (row.is_custom) {
                        strings.t(I18nKeys.SettingsUi.NETWORK_CUSTOM)
                    } else {
                        null
                    },
                    removable = row.is_custom,
                )
            },
            endpoints = model.endpoints.copy(
                fields = view.endpoints.map { endpoint ->
                    UrlFieldModel(
                        id = endpoint.field.name,
                        label = endpointLabel(endpoint.field, model),
                        value = endpoint.value,
                        // The default is the placeholder: an unset endpoint
                        // shows what it WOULD use, greyed, not an empty box.
                        placeholder = endpoint.default_value,
                    )
                },
            ),
            rpcProviders = model.rpcProviders.copy(
                providers = view.providers.mapIndexed { index, provider ->
                    val card = model.rpcProviders.providers.getOrNull(index)
                        ?: model.rpcProviders.providers.firstOrNull()
                    card?.copy(
                        id = provider.provider.name,
                        name = providerName(provider.provider),
                        // The field's id is what the host maps back to a
                        // machine event, so it must be the core's own name for
                        // this provider rather than the fixture's label.
                        field = card.field.copy(
                            id = provider.provider.name,
                            value = provider.key,
                        ),
                    )
                }.filterNotNull(),
            ),
        )
    }

    /**
     * A stable colour per chain, so a network keeps its mark between launches.
     *
     * The fixtures hand-picked a colour per chain because they knew all eleven.
     * A live list does not, and a random colour that changes on every read
     * would make the list flicker.
     */
    private fun markColour(chainId: Long): Long = MARKS[(chainId % MARKS.size).toInt()]

    private val MARKS = listOf(
        0xFF6C7BFF, 0xFF2E9E7E, 0xFFE0A03A, 0xFF8C8C8C,
        0xFFCF5C7A, 0xFF4A9BD1, 0xFF9B6CD1, 0xFF3FA37A,
    )

    /** The label the drawn endpoints page already uses, in field order. */
    private fun endpointLabel(field: NetEndpointField, model: SettingsScreenModel): String {
        val drawn = model.endpoints.fields
        val index = NetEndpointField.entries.indexOf(field)
        return drawn.getOrNull(index)?.label ?: field.name
    }

    private fun providerName(provider: NetProviderId): String = when (provider) {
        NetProviderId.Alchemy -> "Alchemy"
        NetProviderId.Drpc -> "dRPC"
        NetProviderId.Ankr -> "Ankr"
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
