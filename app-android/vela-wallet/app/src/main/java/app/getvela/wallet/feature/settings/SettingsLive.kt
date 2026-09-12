package app.getvela.wallet.feature.settings

import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.send.core.SendTreasuryStatus
import app.getvela.wallet.feature.send.core.SendTreasuryAsset
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.settings.core.DeviceStorage
import app.getvela.wallet.core.format.TextScaleLevel
import app.getvela.wallet.core.marks.Marks
import app.getvela.wallet.core.format.Formats
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

    /**
     * The add-network wizard: what the chain index answered for what was typed.
     *
     * **An empty query shows nothing.** The fixture drew three results under an
     * empty search box, which reads as "these are your options" — and two of
     * them were invented chains. A search nobody has performed has no results.
     */
    fun withWizard(
        model: SettingsScreenModel,
        view: NetView,
        strings: VelaStrings,
    ): SettingsScreenModel {
        val wizard = view.wizard
        val info = wizard.chain_info
        val compat = wizard.compat
        return model.copy(
            addNetwork = model.addNetwork.copy(
                query = wizard.query,
                results = if (wizard.query.isBlank()) {
                    emptyList()
                } else {
                    wizard.suggestions.map { entry ->
                        NetworkRowModel(
                            // The chain id IS the identity: two chains can
                            // share a name, and the tap must reach the right
                            // one.
                            id = entry.chain_id.toString(),
                            mark = ChainMarkModel(
                                letter = entry.name.take(1).uppercase(),
                                colorArgb = markColour(entry.chain_id),
                                logoUrl = Marks.chainLogoUrl(entry.chain_id.toInt()),
                            ),
                            name = entry.name,
                            meta = strings.t(
                                I18nKeys.SettingsUi.CHAIN_ID,
                                mapOf("chainId" to entry.chain_id.toString()),
                            ) + " · " + entry.native_currency_symbol,
                        )
                    }
                },
                candidate = info?.let {
                    NetworkRowModel(
                        id = it.chain_id.toString(),
                        mark = ChainMarkModel(
                            letter = it.name.take(1).uppercase(),
                            colorArgb = markColour(it.chain_id),
                            logoUrl = Marks.chainLogoUrl(it.chain_id.toInt()),
                        ),
                        name = it.name,
                        meta = strings.t(
                            I18nKeys.SettingsUi.CHAIN_ID,
                            mapOf("chainId" to it.chain_id.toString()),
                        ) + " · " + it.native_symbol,
                        // **No verdict until one was reached.** While the
                        // checks are running there is no pill: a chain drawn as
                        // compatible before anything was checked is the same
                        // lie as a latency nobody measured.
                        badge = compat?.let { result ->
                            StatusPillModel(
                                tone = if (result.compatible) SettingsTone.Ok else SettingsTone.Error,
                                label = strings.t(
                                    if (result.compatible) {
                                        I18nKeys.SettingsUi.ADD_COMPATIBLE
                                    } else {
                                        I18nKeys.SettingsUi.ADD_INCOMPATIBLE
                                    },
                                ),
                            )
                        },
                        tag = if (it.is_testnet) {
                            strings.t(I18nKeys.SettingsUi.ADD_TESTNET)
                        } else {
                            null
                        },
                    )
                },
                // Each contract the core looked for, and whether it is there.
                // The names are the core's; this only says found or not.
                checks = compat?.contracts?.map { contract ->
                    CheckItemModel(label = contract.name, ok = contract.deployed)
                }.orEmpty(),
                checksTitle = compat?.let {
                    strings.t(I18nKeys.SettingsUi.ADD_COMPATIBILITY_CHECK)
                },
                // Only offered when the core says this chain can be added.
                // The button is what writes a network somebody's money will be
                // read from, and it must not be reachable on a chain whose
                // contracts are not deployed.
                primary = if (wizard.can_add) {
                    strings.t(I18nKeys.SettingsUi.ADD_BUTTON)
                } else {
                    null
                },
            ),
        )
    }

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
                        logoUrl = Marks.chainLogoUrl(row.chain_id.toInt()),
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

    // -- Spec 047 US1: the rows that read the device, not a fixture -----------------

    private val NUMBER_KEYS = listOf("auto", "comma_dot", "dot_comma", "space_comma", "indian")
    private val DATE_KEYS = listOf("auto", "ymd_slash", "mdy_slash", "dmy_slash", "dmy_dot", "iso")
    private val TIME_KEYS = listOf("auto", "h24", "h12")

    /** The sheets' rows are drawn samples indexed 0..n in the web's key order; the selected one is the preference. */
    private fun selectSheet(sheet: SelectSheetModel, keys: List<String>, chosen: String): SelectSheetModel {
        val index = keys.indexOf(chosen).coerceAtLeast(0)
        return sheet.copy(rows = sheet.rows.mapIndexed { i, row -> row.copy(selected = i == index) })
    }

    fun formatKeyAt(keys: List<String>, rowId: String): String? = rowId.toIntOrNull()?.let { keys.getOrNull(it) }
    fun numberKeyAt(rowId: String) = formatKeyAt(NUMBER_KEYS, rowId)
    fun dateKeyAt(rowId: String) = formatKeyAt(DATE_KEYS, rowId)
    fun timeKeyAt(rowId: String) = formatKeyAt(TIME_KEYS, rowId)

    /**
     * Language, the three formats, the text scale and the avatar style, from
     * the person's preferences: the rows say the choice, the sheets tick it,
     * the controls sit on it. `system` shows the resolved language beside it.
     */
    fun withPreferences(
        model: SettingsScreenModel,
        prefs: app.getvela.wallet.core.data.PrefsView,
        activeLanguage: String,
        strings: VelaStrings,
        theme: String,
    ): SettingsScreenModel {
        val endonym = { tag: String -> SettingsFixtures.LOCALE_ENDONYMS.firstOrNull { it.first == tag }?.second ?: tag }
        val languageValue = if (prefs.language == "system") "${endonym(activeLanguage)} · ${strings.t(I18nKeys.SettingsUi.COMMON_SYSTEM)}" else endonym(prefs.language)
        val numberSheet = selectSheet(model.numberSheet, NUMBER_KEYS, prefs.numberFormat.wire)
        val dateSheet = selectSheet(model.dateSheet, DATE_KEYS, prefs.dateFormat.wire)
        val timeSheet = selectSheet(model.timeSheet, TIME_KEYS, prefs.timeFormat.wire)
        fun sample(sheet: SelectSheetModel) = sheet.rows.firstOrNull { it.selected }?.label ?: sheet.rows.firstOrNull()?.label.orEmpty()
        return model.copy(
            sections = model.sections.map { section ->
                section.copy(
                    rows = section.rows.map { row ->
                        when (row.id) {
                            "language" -> row.copy(value = languageValue)
                            "number-format" -> row.copy(value = sample(numberSheet))
                            "date-format" -> row.copy(value = sample(dateSheet))
                            "time-format" -> row.copy(value = sample(timeSheet))
                            else -> row
                        }
                    },
                )
            },
            languageSheet = model.languageSheet.copy(
                rows = model.languageSheet.rows.map { row -> row.copy(selected = row.id == prefs.language) },
            ),
            numberSheet = numberSheet,
            dateSheet = dateSheet,
            timeSheet = timeSheet,
            theme = model.theme.copy(selected = theme),
            avatar = model.avatar.copy(selected = prefs.avatarStyle),
            textScale = model.textScale.copy(steps = TextScaleLevel.entries.size, index = prefs.textScale.ordinal),
        )
    }

    /** The storage page from the device's own keys (spec 047 D4). */
    fun withStorage(model: SettingsScreenModel, report: DeviceStorage.Report, strings: VelaStrings): SettingsScreenModel {
        val total = report.totalBytes
        val (amount, unit) = bytesText(total)
        fun size(bytes: Long) = bytesText(bytes).let { "${it.first} ${it.second}" }
        val groups = model.storage.groups.map { group ->
            group.copy(
                items = group.items.map { item ->
                    val measured = report.items.firstOrNull { it.id == item.id } ?: return@map item
                    val count = measured.records?.let { n ->
                        when (item.id) {
                            "contacts" -> strings.t(I18nKeys.SettingsUi.COUNT_CONTACTS, mapOf("count" to n.toString()))
                            "custom", "dapps" -> strings.t(I18nKeys.SettingsUi.COUNT_ITEMS, mapOf("count" to n.toString()))
                            else -> strings.t(I18nKeys.SettingsUi.COUNT_RECORDS, mapOf("count" to n.toString()))
                        }
                    }
                    item.copy(meta = listOfNotNull(count, size(measured.bytes)).joinToString(" · "))
                },
            )
        }
        val user = report.bytesOf(DeviceStorage.Group.User); val cache = report.bytesOf(DeviceStorage.Group.Cache); val sessions = report.bytesOf(DeviceStorage.Group.Sessions)
        val denom = (user + cache + sessions).coerceAtLeast(1)
        return model.copy(
            storage = model.storage.copy(
                amount = amount,
                unit = unit,
                summary = strings.t(I18nKeys.SettingsUi.STORAGE_SUMMARY, mapOf("count" to report.totalRecords.toString())),
                segments = model.storage.segments.map { seg ->
                    seg.copy(fraction = when (seg.id) { "user" -> user; "cache" -> cache; else -> sessions }.toFloat() / denom)
                },
                groups = groups,
            ),
        )
    }

    internal fun bytesText(bytes: Long): Pair<String, String> = when {
        bytes >= 1_000_000 -> Formats.current.number(java.math.BigDecimal(bytes).divide(java.math.BigDecimal(1_000_000)), 1, 1) to "MB"
        bytes >= 1_000 -> Formats.current.number(java.math.BigDecimal(bytes).divide(java.math.BigDecimal(1_000)), 0, 0) to "KB"
        else -> bytes.toString() to "B"
    }

    /** About: the build's version and commit, the wallet's network count. */
    fun withAbout(model: SettingsScreenModel, version: String, commit: String, networkCount: Int, strings: VelaStrings): SettingsScreenModel = model.copy(
        about = model.about.copy(
            version = strings.t(I18nKeys.SettingsUi.ABOUT_VERSION, mapOf("version" to version, "commit" to commit)),
            rows = model.about.rows.map { row ->
                if (row.label == strings.t(I18nKeys.SettingsUi.ABOUT_NETWORKS_LABEL)) row.copy(value = strings.t(I18nKeys.SettingsUi.ABOUT_NETWORKS_VALUE, mapOf("count" to networkCount.toString()))) else row
            },
        ),
        sections = model.sections.map { section ->
            section.copy(rows = section.rows.map { row -> if (row.id == "about") row.copy(value = strings.t(I18nKeys.SettingsUi.ABOUT_VERSION, mapOf("version" to version, "commit" to commit))) else row })
        },
    )

    /** Feedback: the preview lines are the device's (version, platform, language, failed chains, recent failures). */
    fun withFeedback(model: SettingsScreenModel, version: String, commit: String, platform: String, language: String, failedChains: List<String>, failures: List<String>, strings: VelaStrings): SettingsScreenModel = model.copy(
        feedback = model.feedback.copy(
            previewLines = listOf(
                "${strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_VERSION)}: v$version ($commit)",
                "${strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_PLATFORM)}: $platform",
                "${strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_LANGUAGE)}: $language",
                "${strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_RPC)}: ${failedChains.ifEmpty { listOf(strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_NONE)) }.joinToString(", ")}",
                "${strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_FAILURES)}: ${failures.ifEmpty { listOf(strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_NONE)) }.joinToString("; ")}",
            ),
        ),
    )

    /** The relayer panel from the treasury probe: the address to fund and the shortfall, in the chain's coin. */
    fun withRelayer(model: SettingsScreenModel, chainName: String, chainId: Int, symbol: String, status: SendTreasuryStatus?, strings: VelaStrings): SettingsScreenModel {
        if (status == null) return model
        val decimals = if (status.asset == SendTreasuryAsset.PathUsd) 6 else 18
        val short = (status.floor.toBigDecimalOrNull() ?: java.math.BigDecimal.ZERO) - (status.balance.toBigDecimalOrNull() ?: java.math.BigDecimal.ZERO)
        val amount = SendLive.fromBase(short.max(java.math.BigDecimal.ZERO).toPlainString(), decimals)
        return model.copy(
            relayer = model.relayer.copy(
                mark = ChainMarkModel(symbol.take(3).uppercase(), WalletLive.badge(chainId.toLong()).value.toLong() and 0xFFFFFFFFL, Marks.chainLogoUrl(chainId.toInt())),
                name = chainName,
                amountHint = strings.t(I18nKeys.SettingsUi.RELAYER_AMOUNT_HINT, mapOf("amount" to amount, "symbol" to (if (status.asset == SendTreasuryAsset.PathUsd) "pathUSD" else symbol))),
                addressDisplay = ExploreLive.shortAddress(status.address),
            ),
        )
    }

    /** The RPC banner names the pool's failed chains; absent when none failed. */
    fun withBanner(model: SettingsScreenModel, failedChains: List<Int>, chainNames: Map<Int, String>, strings: VelaStrings): SettingsScreenModel {
        if (failedChains.isEmpty()) return model.copy(rpcBanner = null)
        val drawn = model.rpcBanner ?: return model
        return model.copy(
            rpcBanner = drawn.copy(
                chips = failedChains.map { id ->
                    val name = chainNames[id] ?: "chain-$id"
                    RpcBannerChipModel(id = id.toString(), mark = ChainMarkModel(name.take(1).uppercase(), WalletLive.badge(id.toLong()).value.toLong() and 0xFFFFFFFFL, Marks.chainLogoUrl(id)), name = name, action = drawn.chips.firstOrNull()?.action.orEmpty())
                },
            ),
        )
    }

    /**
     * Spec 048: the network detail page for THIS network — name, chain, the
     * RPC and explorer overrides as the core holds them. Before this the page
     * showed the fixture's Ethereum whichever row was tapped.
     */
    fun networkDetail(fallback: NetworkDetailModel, row: NetNetworkRow, strings: VelaStrings): NetworkDetailModel = fallback.copy(
        title = row.display_name,
        subtitle = strings.t(I18nKeys.SettingsUi.CHAIN_ID, mapOf("chainId" to row.chain_id.toString())) + " · " + row.native_symbol,
        mark = ChainMarkModel(row.display_name.take(1).uppercase(), markColour(row.chain_id), Marks.chainLogoUrl(row.chain_id.toInt())),
        name = row.display_name,
        rpc = fallback.rpc.copy(value = row.rpc_url, badge = null, tone = null),
        explorer = fallback.explorer.copy(value = row.explorer_url),
        callout = null,
        chainId = row.chain_id,
    )

    /** The accounts sheet: this device's accounts, the active one ticked; the drawn amount is not known here and stays blank. */
    fun withAccounts(model: SettingsScreenModel, accounts: List<Pair<String, String>>, activeIndex: Int, strings: VelaStrings): SettingsScreenModel = model.copy(
        accountsSheet = model.accountsSheet.copy(
            // The count line ends in the separator that precedes a total; this sheet shows none, so the separator goes too.
            summary = strings.t(I18nKeys.SettingsUi.ACCOUNTS_COUNT, mapOf("count" to accounts.size.toString())).trimEnd(' ', '·'),
            rows = accounts.mapIndexed { i, (name, address) ->
                AccountsSheetRowModel(name = name.ifBlank { ExploreLive.shortAddress(address) }, addressDisplay = ExploreLive.shortAddress(address), addressFull = address, amount = "", selected = i == activeIndex)
            },
        ),
    )
}
