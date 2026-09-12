package app.getvela.wallet.feature.settings

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.TabsModel

/**
 * Canonical settings fixtures (spec 023 — the single canon all four platforms
 * port; Android sibling of the web's `src/lib/settings/fixtures.ts`).
 *
 * Numbers, URLs, latencies and brand colours are DATA and identical across
 * platforms, so a reviewer comparing the four clients is comparing the same
 * wallet. Labels resolve through the corpus; components never format.
 *
 * Where a mock shows a composed string ("200 条 · 1.0 MB") the parts are
 * composed HERE — composition order is a translation concern and a component
 * must never learn one.
 */
object SettingsFixtures {

    // --- Canon ---------------------------------------------------------------

    const val ACCOUNT_NAME = "大表哥"
    const val ADDRESS_FULL = "0x14fB1f4E2b9C7a5D8e3F6a1B4c7D9e2F5a8B1D1eA5c"
    const val ADDRESS_DISPLAY = "0x14fB...D1eA5c"

    private const val TOTAL_BALANCE = "\$3,262.40"
    private const val APP_VERSION = "1.0.0"
    private const val APP_COMMIT = "6ab8f"
    private const val NETWORK_COUNT = 12

    private data class AccountFixture(
        val name: String,
        val addressFull: String,
        val addressDisplay: String,
        val amount: String,
    )

    private val ACCOUNTS = listOf(
        AccountFixture(ACCOUNT_NAME, ADDRESS_FULL, ADDRESS_DISPLAY, "\$3,140.22"),
        AccountFixture(
            "旅行基金",
            "0x9a01c4E7b2F5a8D3e6C9b1A4d7F0e3B6c9D277C2b",
            "0x9a01...77C2b",
            "\$122.18",
        ),
        AccountFixture(
            "试验田",
            "0x3Ce4f7A0b3D6e9C2a5F8b1E4d7C0a3F6b9E2A90f1",
            "0x3Ce4...A90f1",
            "\$0.00",
        ),
    )

    /**
     * The eight networks ST9 lists, in order. `colorArgb` is the chain's own
     * brand colour — data, not a theme token: it belongs to Ethereum and BNB,
     * and must not flip with the appearance.
     */
    private data class NetworkFixture(
        val id: String,
        val name: String,
        val letter: String,
        val colorArgb: Long,
        val chainId: Long,
        val latencyMs: Int,
        val custom: Boolean = false,
    )

    private val NETWORKS = listOf(
        NetworkFixture("ethereum", "Ethereum", "E", 0xFF627EEA, 1, 45),
        NetworkFixture("bnb", "BNB Chain", "B", 0xFFF0B90B, 56, 128),
        NetworkFixture("polygon", "Polygon", "P", 0xFF8247E5, 137, 45),
        NetworkFixture("arbitrum", "Arbitrum", "A", 0xFF28A0F0, 42161, 45),
        NetworkFixture("base", "Base", "B", 0xFF0052FF, 8453, 45),
        NetworkFixture("gnosis", "Gnosis", "G", 0xFF2E9E7E, 100, 45),
        NetworkFixture("tempo", "Tempo", "T", 0xFF8C8C8C, 4217, 45),
        NetworkFixture("xlayer", "X Layer", "X", 0xFF8C8C8C, 196, 0, custom = true),
    )

    private fun network(id: String): NetworkFixture =
        NETWORKS.first { it.id == id }

    private fun mark(id: String): ChainMarkModel =
        network(id).let { ChainMarkModel(it.letter, it.colorArgb) }

    /**
     * Language endonyms — NOT corpus strings. A language picker names each
     * language IN that language, so the row reads the same whichever locale the
     * app is in; that is the whole point of showing 日本語 to somebody who
     * cannot read the current UI.
     */
    val LOCALE_ENDONYMS: List<Pair<String, String>> = listOf(
        "en" to "English",
        "zh" to "简体中文",
        "zh-TW" to "繁體中文（台灣）",
        "zh-HK" to "繁體中文（香港）",
        "ja" to "日本語",
        "ko" to "한국어",
        "vi" to "Tiếng Việt",
        "id" to "Bahasa Indonesia",
        "tr" to "Türkçe",
        "es-MX" to "Español (México)",
        "pt-BR" to "Português (Brasil)",
        "fr" to "Français",
        "de" to "Deutsch",
        "ru" to "Русский",
        "it" to "Italiano",
    )

    /**
     * Currency names come from the FX provider, not the corpus: the list is
     * provider-driven (the endpoint decides which currencies exist), so their
     * names are data here rather than 120 translated strings.
     */
    private val CURRENCIES = listOf(
        Triple("USD", "$", "US Dollar"),
        Triple("EUR", "€", "Euro"),
        Triple("GBP", "£", "British Pound"),
        Triple("CNY", "¥", "Chinese Yuan"),
        Triple("JPY", "¥", "Japanese Yen"),
        Triple("KRW", "₩", "South Korean Won"),
        Triple("HKD", "$", "Hong Kong Dollar"),
        Triple("VND", "₫", "Vietnamese Dong"),
    )

    private val NUMBER_SAMPLES =
        listOf("1,234,567.89", "1,234,567.89", "1.234.567,89", "1 234 567,89", "12,34,567.89")
    private val DATE_SAMPLES =
        listOf("2026/06/13", "2026/06/13", "06/13/2026", "13/06/2026", "13.06.2026", "2026-06-13")
    private val TIME_SAMPLES = listOf("13:45", "13:45", "1:45 PM")

    // --- Helpers -------------------------------------------------------------

    /**
     * `45ms`, or `在线 · 45ms` with a prefix. Past a second the unit becomes
     * seconds AND the tone steps down to warning — which is the only way "1.2s"
     * reads as slow rather than as a very small number.
     */
    private fun latency(ms: Int, prefix: String? = null): StatusPillModel {
        val tone = if (ms >= 1000) SettingsTone.Warn else SettingsTone.Ok
        val value = if (ms >= 1000) "%.1fs".format(ms / 1000.0) else "${ms}ms"
        return StatusPillModel(tone, if (prefix == null) value else "$prefix · $value")
    }

    private fun VelaStrings.chainMeta(chainId: Long): String =
        t(I18nKeys.SettingsUi.CHAIN_ID, mapOf("chainId" to chainId.toString()))

    // --- Pages ---------------------------------------------------------------

    // The founder's ruling (2026-09-12, spec 047): the settings home carries no
    // 通讯录 row (the tab bar has the book) and no 反馈 row (the crash sheet and
    // About reach the issue tracker). The feedback sheet stays drawn for those.
    private fun sections(s: VelaStrings, advancedOpen: Boolean): List<SettingsSectionModel> = listOf(
        SettingsSectionModel(
            label = s.t(I18nKeys.SettingsUi.SECTION_APPEARANCE),
            appearanceControls = true,
            rows = listOf(
                SettingsRowModel(
                    id = "language",
                    title = s.t(I18nKeys.SettingsUi.LANGUAGE_TITLE),
                    icon = SettingsIcon.Globe,
                    value = "简体中文 · ${s.t(I18nKeys.SettingsUi.COMMON_SYSTEM)}",
                ),
            ),
        ),
        SettingsSectionModel(
            label = s.t(I18nKeys.SettingsUi.SECTION_LOCALIZATION),
            rows = listOf(
                SettingsRowModel(
                    id = "currency",
                    title = s.t(I18nKeys.SettingsUi.CURRENCY_TITLE),
                    icon = SettingsIcon.Coins,
                    value = "USD · \$1,234.56",
                ),
                SettingsRowModel(
                    id = "number-format",
                    title = s.t(I18nKeys.SettingsUi.NUMBER_TITLE),
                    icon = SettingsIcon.Hash,
                    value = NUMBER_SAMPLES[0],
                ),
                SettingsRowModel(
                    id = "date-format",
                    title = s.t(I18nKeys.SettingsUi.DATE_TITLE),
                    icon = SettingsIcon.Calendar,
                    value = DATE_SAMPLES[0],
                ),
                SettingsRowModel(
                    id = "time-format",
                    title = s.t(I18nKeys.SettingsUi.TIME_TITLE),
                    icon = SettingsIcon.Clock,
                    value = TIME_SAMPLES[0],
                ),
            ),
        ),
        SettingsSectionModel(
            label = s.t(I18nKeys.SettingsUi.SECTION_ADVANCED),
            collapsible = true,
            collapsed = !advancedOpen,
            rows = listOf(
                SettingsRowModel(
                    id = "networks",
                    title = s.t(I18nKeys.SettingsUi.NETWORKS_TITLE),
                    icon = SettingsIcon.Network,
                    subtitle = s.t(I18nKeys.SettingsUi.NETWORKS_SUBTITLE),
                    value = s.t(
                        I18nKeys.SettingsUi.NETWORK_COUNT,
                        mapOf("count" to NETWORK_COUNT.toString()),
                    ),
                ),
                SettingsRowModel(
                    id = "rpc-providers",
                    title = s.t(I18nKeys.SettingsUi.RPC_PROVIDERS_TITLE),
                    icon = SettingsIcon.Server,
                    subtitle = s.t(I18nKeys.SettingsUi.RPC_PROVIDERS_SUBTITLE),
                ),
                SettingsRowModel(
                    id = "add-network",
                    title = s.t(I18nKeys.SettingsUi.ADD_NETWORK_TITLE),
                    icon = SettingsIcon.Plus,
                    subtitle = s.t(I18nKeys.SettingsUi.ADD_NETWORK_SUBTITLE),
                ),
                SettingsRowModel(
                    id = "endpoints",
                    title = s.t(I18nKeys.SettingsUi.ENDPOINTS_TITLE),
                    icon = SettingsIcon.Zap,
                    subtitle = s.t(I18nKeys.SettingsUi.ENDPOINTS_SUBTITLE),
                ),
                SettingsRowModel(
                    id = "storage",
                    title = s.t(I18nKeys.SettingsUi.STORAGE_TITLE),
                    icon = SettingsIcon.HardDrive,
                    subtitle = s.t(I18nKeys.SettingsUi.STORAGE_SUBTITLE),
                ),
            ),
        ),
        SettingsSectionModel(
            rows = listOf(
                SettingsRowModel(
                    id = "about",
                    title = s.t(I18nKeys.SettingsUi.ABOUT_TITLE),
                    icon = SettingsIcon.Info,
                    value = s.t(
                        I18nKeys.SettingsUi.ABOUT_SUBTITLE,
                        mapOf("version" to APP_VERSION),
                    ),
                ),
            ),
        ),
    )

    private fun networks(s: VelaStrings): List<NetworkRowModel> = NETWORKS.map { n ->
        NetworkRowModel(
            id = n.id,
            mark = ChainMarkModel(n.letter, n.colorArgb),
            name = n.name,
            meta = s.chainMeta(n.chainId),
            badge = if (n.custom) null else latency(n.latencyMs),
            tag = if (n.custom) s.t(I18nKeys.SettingsUi.NETWORK_CUSTOM) else null,
            removable = n.custom,
        )
    }

    private fun networkDetail(s: VelaStrings, mismatch: Boolean): NetworkDetailModel {
        val eth = network("ethereum")
        return NetworkDetailModel(
            title = eth.name,
            subtitle = "${s.chainMeta(eth.chainId)} · ETH",
            mark = mark("ethereum"),
            name = eth.name,
            note = s.t(I18nKeys.SettingsUi.NETWORK_BUILTIN_NOTE),
            badge = latency(eth.latencyMs, s.t(I18nKeys.SettingsUi.NETWORK_ONLINE)),
            rpc = UrlFieldModel(
                id = "rpc",
                label = s.t(I18nKeys.SettingsUi.FIELD_RPC_URL),
                value = "https://eth.llamarpc.com",
                hint = s.t(I18nKeys.SettingsUi.NETWORK_SAVE_HINT),
                badge = latency(eth.latencyMs),
                tone = if (mismatch) SettingsTone.Error else null,
            ),
            explorer = UrlFieldModel(
                id = "explorer",
                label = s.t(I18nKeys.SettingsUi.FIELD_EXPLORER),
                value = "https://etherscan.io",
            ),
            callout = if (mismatch) {
                CalloutModel(
                    CalloutTone.Danger,
                    s.t(
                        I18nKeys.SettingsUi.RPC_CHAIN_MISMATCH,
                        mapOf("reported" to "56", "expected" to "1"),
                    ),
                )
            } else {
                null
            },
        )
    }

    /** ST10 search, ST10b compatible, ST10c incompatible — one builder. */
    private fun addNetwork(s: VelaStrings, mode: String): AddNetworkModel {
        val title = s.t(I18nKeys.SettingsUi.ADD_NETWORK_TITLE)
        val searchPlaceholder = s.t(I18nKeys.SettingsUi.ADD_SEARCH)
        if (mode == "search") {
            return AddNetworkModel(
                title = title,
                subtitle = s.t(I18nKeys.SettingsUi.ADD_DESCRIPTION),
                searchPlaceholder = searchPlaceholder,
                results = listOf(
                    NetworkRowModel(
                        "zora",
                        ChainMarkModel("Z", 0xFF8C8C8C),
                        "Zora",
                        s.chainMeta(7_777_777),
                    ),
                    NetworkRowModel(
                        "zircuit",
                        ChainMarkModel("Z", 0xFF2E9E7E),
                        "Zircuit",
                        s.chainMeta(48_900),
                    ),
                    NetworkRowModel(
                        "zora-sepolia",
                        ChainMarkModel("Z", 0xFF8C8C8C),
                        "Zora Sepolia",
                        s.chainMeta(999_999_999),
                        tag = s.t(I18nKeys.SettingsUi.ADD_TESTNET),
                    ),
                ),
            )
        }

        val ok = mode == "compatible"
        // Four rows in both verdicts: "incompatible" is only legible as an
        // answer if it shows WHICH requirement failed, so the list never
        // shortens. EntryPoint is deployed everywhere and passes in both.
        val checks = listOf(
            CheckItemModel("EntryPoint v0.7", true),
            CheckItemModel(s.t(I18nKeys.SettingsUi.ADD_CHECK_SAFE), ok),
            CheckItemModel(s.t(I18nKeys.SettingsUi.ADD_CHECK_SIGNER), ok),
            CheckItemModel(
                s.t(I18nKeys.SettingsUi.ADD_CHECK_REMAINING, mapOf("count" to "8")),
                ok,
            ),
        )
        val checksTitle = s.t(I18nKeys.SettingsUi.ADD_COMPATIBILITY_CHECK)

        return if (ok) {
            AddNetworkModel(
                title = title,
                subtitle = "Zora · ${s.chainMeta(7_777_777)}",
                searchPlaceholder = searchPlaceholder,
                candidate = NetworkRowModel(
                    "zora",
                    ChainMarkModel("Z", 0xFF8C8C8C),
                    "Zora",
                    s.t(I18nKeys.SettingsUi.ADD_BEST_RPC, mapOf("latencyMs" to "182")),
                    badge = StatusPillModel(
                        SettingsTone.Ok,
                        s.t(I18nKeys.SettingsUi.ADD_COMPATIBLE),
                    ),
                ),
                checksTitle = checksTitle,
                checks = checks,
                customRpc = UrlFieldModel(
                    id = "custom-rpc",
                    label = s.t(I18nKeys.SettingsUi.ADD_CUSTOM_RPC_TITLE),
                    value = "",
                    placeholder = s.t(I18nKeys.SettingsUi.ADD_CUSTOM_RPC_PLACEHOLDER),
                ),
                primary = s.t(I18nKeys.SettingsUi.ADD_BUTTON),
            )
        } else {
            AddNetworkModel(
                title = title,
                subtitle = "Zircuit · ${s.chainMeta(48_900)}",
                searchPlaceholder = searchPlaceholder,
                candidate = NetworkRowModel(
                    "zircuit",
                    ChainMarkModel("Z", 0xFF2E9E7E),
                    "Zircuit",
                    checksTitle,
                    badge = StatusPillModel(
                        SettingsTone.Error,
                        s.t(I18nKeys.SettingsUi.ADD_INCOMPATIBLE),
                    ),
                ),
                checksTitle = checksTitle,
                checks = checks,
                callout = CalloutModel(
                    CalloutTone.Warning,
                    s.t(I18nKeys.SettingsUi.ADD_INCOMPATIBLE_HINT),
                ),
                // An outline CTA plus a re-check link, not a greyed-out accent
                // one: an action you cannot take should not be dressed as the
                // action you came for.
                secondary = s.t(I18nKeys.SettingsUi.ADD_CHAIN_TOOL),
                recheck = s.t(I18nKeys.SettingsUi.ADD_RECHECK_WITH_RPC),
            )
        }
    }

    private fun rpcProviders(s: VelaStrings): RpcProvidersModel {
        val support = { count: Int ->
            s.t(
                I18nKeys.SettingsUi.PROVIDER_SUPPORTS,
                mapOf("count" to count.toString(), "total" to NETWORK_COUNT.toString()),
            )
        }
        val notSet = s.t(I18nKeys.SettingsUi.PROVIDER_NOT_SET)
        return RpcProvidersModel(
            title = s.t(I18nKeys.SettingsUi.RPC_PROVIDERS_TITLE),
            subtitle = s.t(I18nKeys.SettingsUi.RPC_PROVIDERS_SUBTITLE),
            description = s.t(I18nKeys.SettingsUi.PROVIDERS_DESCRIPTION),
            providers = listOf(
                ProviderCardModel(
                    id = "alchemy",
                    name = "Alchemy",
                    badge = StatusPillModel(
                        SettingsTone.Ok,
                        s.t(I18nKeys.SettingsUi.PROVIDER_CONNECTED),
                    ),
                    field = UrlFieldModel("alchemy", "", "alch_k3y...9fQ2"),
                    action = s.t(I18nKeys.SettingsUi.PROVIDER_CHECK_KEY),
                    support = support(12),
                ),
                ProviderCardModel(
                    id = "drpc",
                    name = "dRPC",
                    badge = StatusPillModel(SettingsTone.Neutral, notSet),
                    field = UrlFieldModel("drpc", "", "", placeholder = notSet),
                    action = s.t(I18nKeys.SettingsUi.PROVIDER_GET_KEY),
                    link = "${s.t(I18nKeys.SettingsUi.PROVIDER_GET_KEY)} →",
                ),
                ProviderCardModel(
                    id = "ankr",
                    name = "Ankr",
                    badge = StatusPillModel(SettingsTone.Neutral, notSet),
                    field = UrlFieldModel("ankr", "", "", placeholder = notSet),
                    action = s.t(I18nKeys.SettingsUi.PROVIDER_GET_KEY),
                    support = support(8),
                ),
            ),
        )
    }

    private fun endpoints(s: VelaStrings): EndpointsModel = EndpointsModel(
        title = s.t(I18nKeys.SettingsUi.ENDPOINTS_TITLE),
        description = s.t(I18nKeys.SettingsUi.ENDPOINTS_DESCRIPTION),
        fields = listOf(
            UrlFieldModel(
                "chain-data",
                s.t(I18nKeys.SettingsUi.ENDPOINT_CHAIN_DATA),
                "https://ethereum-data.awesometools.dev",
                hint = s.t(I18nKeys.SettingsUi.ENDPOINT_CHAIN_DATA_HINT),
                badge = latency(62),
            ),
            UrlFieldModel(
                "passkey",
                s.t(I18nKeys.SettingsUi.ENDPOINT_PASSKEY),
                "https://p256-index-rs.getvela.app",
                hint = s.t(I18nKeys.SettingsUi.ENDPOINT_PASSKEY_HINT),
                badge = latency(88),
            ),
            UrlFieldModel(
                "relay",
                s.t(I18nKeys.SettingsUi.ENDPOINT_RELAY),
                "https://vela-relay.getvela.app",
                hint = s.t(I18nKeys.SettingsUi.ENDPOINT_RELAY_HINT),
                badge = latency(104),
            ),
            UrlFieldModel(
                "fiat",
                s.t(I18nKeys.SettingsUi.ENDPOINT_FIAT),
                "https://vela-currency.getvela.app/v2/…",
                hint = s.t(I18nKeys.SettingsUi.ENDPOINT_FIAT_HINT),
                badge = latency(1200, s.t(I18nKeys.SettingsUi.NETWORK_SLOW)),
            ),
        ),
        reset = s.t(I18nKeys.SettingsUi.ENDPOINTS_RESET),
    )

    private fun storage(s: VelaStrings): StorageModel {
        val records = { n: Int -> s.t(I18nKeys.SettingsUi.COUNT_RECORDS, mapOf("count" to n.toString())) }
        val clear = s.t(I18nKeys.SettingsUi.STORAGE_CLEAR)
        return StorageModel(
            title = s.t(I18nKeys.SettingsUi.STORAGE_TITLE),
            subtitle = s.t(I18nKeys.SettingsUi.STORAGE_SUBTITLE),
            amount = "2.4",
            unit = "MB",
            summary = s.t(I18nKeys.SettingsUi.STORAGE_SUMMARY, mapOf("count" to "216")),
            segments = listOf(
                StorageSegmentModel("user", s.t(I18nKeys.SettingsUi.LEGEND_USER_DATA), 0.5f, 0xFF5A7CF6),
                StorageSegmentModel("cache", s.t(I18nKeys.SettingsUi.LEGEND_CACHES), 0.3f, 0xFF3DA872),
                StorageSegmentModel("sessions", s.t(I18nKeys.SettingsUi.LEGEND_SESSIONS), 0.2f, 0xFF85827A),
            ),
            groups = listOf(
                StorageGroupModel(
                    label = s.t(I18nKeys.SettingsUi.STORAGE_USER_DATA),
                    items = listOf(
                        StorageItemModel(
                            "transactions",
                            s.t(I18nKeys.SettingsUi.ITEM_TRANSACTIONS),
                            "${records(200)} · 1.0 MB",
                            clear,
                            destructive = true,
                        ),
                        StorageItemModel(
                            "contacts",
                            s.t(I18nKeys.SettingsUi.ITEM_CONTACTS),
                            "${s.t(I18nKeys.SettingsUi.COUNT_CONTACTS, mapOf("count" to "18"))} · 42 KB",
                            clear,
                            destructive = true,
                        ),
                        StorageItemModel(
                            "custom",
                            s.t(I18nKeys.SettingsUi.ITEM_CUSTOM),
                            "${s.t(I18nKeys.SettingsUi.COUNT_ITEMS, mapOf("count" to "5"))} · 12 KB",
                            clear,
                            destructive = true,
                        ),
                        StorageItemModel(
                            "browsing",
                            s.t(I18nKeys.SettingsUi.ITEM_BROWSING),
                            "${records(31)} · 58 KB",
                            clear,
                            destructive = true,
                        ),
                    ),
                ),
                StorageGroupModel(
                    label = s.t(I18nKeys.SettingsUi.STORAGE_CACHES),
                    action = s.t(I18nKeys.SettingsUi.STORAGE_CLEAR_ALL),
                    items = listOf(
                        StorageItemModel("balances", s.t(I18nKeys.SettingsUi.ITEM_BALANCES), "0.6 MB", clear),
                        StorageItemModel("rates", s.t(I18nKeys.SettingsUi.ITEM_RATES), "96 KB", clear),
                        StorageItemModel("scan", s.t(I18nKeys.SettingsUi.ITEM_SCAN), "31 KB", clear),
                    ),
                ),
                StorageGroupModel(
                    label = s.t(I18nKeys.SettingsUi.STORAGE_CONNECTIONS),
                    items = listOf(
                        StorageItemModel(
                            "dapps",
                            s.t(I18nKeys.SettingsUi.ITEM_DAPPS),
                            s.t(I18nKeys.SettingsUi.COUNT_SITES, mapOf("count" to "4")),
                            s.t(I18nKeys.SettingsUi.STORAGE_DISCONNECT_ALL),
                            destructive = true,
                        ),
                    ),
                ),
            ),
        )
    }

    private fun about(s: VelaStrings): AboutModel = AboutModel(
        title = s.t(I18nKeys.SettingsUi.ABOUT_TITLE),
        tagline = s.t(I18nKeys.SettingsUi.ABOUT_TAGLINE),
        version = s.t(
            I18nKeys.SettingsUi.ABOUT_VERSION,
            mapOf("version" to APP_VERSION, "commit" to APP_COMMIT),
        ),
        sectionTechnical = s.t(I18nKeys.SettingsUi.ABOUT_SECTION_TECHNICAL),
        rows = listOf(
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_WALLET_LABEL),
                s.t(I18nKeys.SettingsUi.ABOUT_WALLET_VALUE),
                mono = true,
            ),
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_AUTH_LABEL),
                s.t(I18nKeys.SettingsUi.ABOUT_AUTH_VALUE),
                mono = true,
            ),
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_ACCOUNT_LABEL),
                s.t(I18nKeys.SettingsUi.ABOUT_ACCOUNT_VALUE),
            ),
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_SIGNER_LABEL),
                s.t(I18nKeys.SettingsUi.ABOUT_SIGNER_VALUE),
            ),
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_NETWORKS_LABEL),
                s.t(
                    I18nKeys.SettingsUi.ABOUT_NETWORKS_VALUE,
                    mapOf("count" to NETWORK_COUNT.toString()),
                ),
            ),
        ),
        links = listOf(
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_LINK_WEBSITE),
                "getvela.app",
                mono = true,
                external = true,
            ),
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_LINK_GITHUB),
                "github.com/mondaylabsltd/vela-wallet",
                mono = true,
                external = true,
            ),
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_LINK_SAFE),
                "safe.global",
                mono = true,
                external = true,
            ),
        ),
        footer = s.t(I18nKeys.SettingsUi.ABOUT_FOOTER),
    )

    // --- Overlays ------------------------------------------------------------

    private fun accountsSheet(s: VelaStrings) = AccountsSheetModel(
        title = s.t(I18nKeys.SettingsUi.ACCOUNTS_TITLE),
        summary = s.t(I18nKeys.SettingsUi.ACCOUNTS_COUNT, mapOf("count" to ACCOUNTS.size.toString())) +
            s.t(I18nKeys.SettingsUi.ACCOUNTS_TOTAL, mapOf("amount" to TOTAL_BALANCE)),
        rows = ACCOUNTS.mapIndexed { i, a ->
            AccountsSheetRowModel(a.name, a.addressDisplay, a.addressFull, a.amount, i == 0)
        },
        primary = s.t(I18nKeys.SettingsUi.ACCOUNT_CREATE),
        secondary = s.t(I18nKeys.SettingsUi.ACCOUNT_SIGN_IN),
    )

    private fun signOutSheet(s: VelaStrings, warned: Boolean) = ConfirmSheetModel(
        title = s.t(I18nKeys.SettingsUi.SIGN_OUT_TITLE),
        body = s.t(I18nKeys.SettingsUi.SIGN_OUT_DESC),
        note = s.t(I18nKeys.SettingsUi.SIGN_OUT_KEEPS),
        callout = if (warned) {
            CalloutModel(CalloutTone.Warning, s.t(I18nKeys.SettingsUi.SIGN_OUT_WARNING))
        } else {
            null
        },
        confirm = s.t(
            if (warned) I18nKeys.SettingsUi.SIGN_OUT_ANYWAY else I18nKeys.SettingsUi.SIGN_OUT_BUTTON,
        ),
        cancel = s.t(I18nKeys.SettingsUi.SIGN_OUT_CANCEL),
        danger = true,
    )

    private fun languageSheet(s: VelaStrings, current: String): SelectSheetModel {
        val label = LOCALE_ENDONYMS.firstOrNull { it.first == current }?.second ?: current
        return SelectSheetModel(
            title = s.t(I18nKeys.SettingsUi.LANGUAGE_PICKER_TITLE),
            subtitle = s.t(I18nKeys.SettingsUi.LANGUAGE_PICKER_SUBTITLE),
            rows = buildList {
                add(
                    SelectRowModel(
                        id = "system",
                        label = s.t(I18nKeys.SettingsUi.LANGUAGE_FOLLOW_SYSTEM),
                        note = "${s.t(I18nKeys.SettingsUi.COMMON_SYSTEM)} · $label",
                        selected = true,
                    ),
                )
                LOCALE_ENDONYMS.forEach { (id, endonym) -> add(SelectRowModel(id, endonym)) }
            },
            footerNote = s.t(I18nKeys.SettingsUi.LANGUAGE_CONTRIBUTE_NOTE),
            footerLink = s.t(I18nKeys.SettingsUi.LANGUAGE_CONTRIBUTE_CTA),
        )
    }

    private fun currencySheet(s: VelaStrings) = SelectSheetModel(
        title = s.t(I18nKeys.SettingsUi.CURRENCY_SHEET_TITLE),
        searchPlaceholder = s.t(I18nKeys.SettingsUi.CURRENCY_SEARCH),
        rows = CURRENCIES.mapIndexed { i, (code, glyph, name) ->
            SelectRowModel(code, code, glyph = glyph, caption = name, selected = i == 0)
        },
    )

    /**
     * The three format pickers. Row 0 is always 自动 — it shows the sample the
     * system would give, with the "自动 · 系统" note; the rest are explicit
     * choices. One builder with three sample lists, not three near-identical
     * ones.
     */
    private fun formatSheet(
        s: VelaStrings,
        title: String,
        subtitle: String?,
        samples: List<String>,
        notes: Map<Int, String> = emptyMap(),
    ) = SelectSheetModel(
        title = title,
        subtitle = subtitle,
        rows = samples.mapIndexed { i, sample ->
            SelectRowModel(
                id = i.toString(),
                label = sample,
                mono = true,
                note = if (i == 0) {
                    "${s.t(I18nKeys.SettingsUi.COMMON_AUTOMATIC)} · ${s.t(I18nKeys.SettingsUi.COMMON_SYSTEM)}"
                } else {
                    notes[i]
                },
                selected = i == 0,
            )
        },
    )

    private fun feedback(s: VelaStrings) = FeedbackModel(
        title = s.t(I18nKeys.SettingsUi.BUG_TITLE),
        subtitle = s.t(I18nKeys.SettingsUi.BUG_SUBTITLE),
        placeholder = s.t(I18nKeys.SettingsUi.BUG_PLACEHOLDER),
        addSteps = s.t(I18nKeys.SettingsUi.BUG_ADD_STEPS),
        previewToggle = s.t(I18nKeys.SettingsUi.BUG_PREVIEW_TOGGLE),
        // Label AND value on every line: the point of this block is that the
        // person can read what is about to leave their device, and a bare list
        // of values is not readable.
        previewLines = listOf(
            "${s.t(I18nKeys.SettingsUi.BUG_PREVIEW_VERSION)}: v$APP_VERSION ($APP_COMMIT)",
            "${s.t(I18nKeys.SettingsUi.BUG_PREVIEW_PLATFORM)}: Android 16",
            "${s.t(I18nKeys.SettingsUi.BUG_PREVIEW_LANGUAGE)}: zh",
            "${s.t(I18nKeys.SettingsUi.BUG_PREVIEW_RPC)}: ${s.t(I18nKeys.SettingsUi.BUG_PREVIEW_NONE)}",
            "${s.t(I18nKeys.SettingsUi.BUG_PREVIEW_FAILURES)}: ${s.t(I18nKeys.SettingsUi.BUG_PREVIEW_NONE)}",
        ),
        consent = s.t(I18nKeys.SettingsUi.BUG_CONSENT),
        send = s.t(I18nKeys.SettingsUi.BUG_SEND),
        githubLink = s.t(I18nKeys.SettingsUi.BUG_GITHUB),
    )

    // --- Rescue --------------------------------------------------------------

    private fun rpcBanner(s: VelaStrings) = RpcBannerModel(
        text = s.t(I18nKeys.SettingsUi.RPC_UNAVAILABLE_MULTIPLE, mapOf("count" to "2")),
        chips = listOf("polygon", "gnosis").map { id ->
            RpcBannerChipModel(id, mark(id), network(id).name, s.t(I18nKeys.SettingsUi.RPC_FIX))
        },
    )

    /** SR2 (failing) and SR2b (restored) are one model with a flag. */
    private fun rpcFix(s: VelaStrings, restored: Boolean): RpcFixModel {
        val polygon = network("polygon")
        return RpcFixModel(
            title = s.t(I18nKeys.SettingsUi.RPC_FIX_TITLE),
            mark = mark("polygon"),
            name = polygon.name,
            meta = "${s.chainMeta(polygon.chainId)} · POL",
            badge = if (restored) {
                latency(96, s.t(I18nKeys.SettingsUi.NETWORK_ONLINE))
            } else {
                StatusPillModel(SettingsTone.Error, s.t(I18nKeys.SettingsUi.NETWORK_OFFLINE))
            },
            callout = if (restored) {
                CalloutModel(CalloutTone.Success, s.t(I18nKeys.SettingsUi.RPC_FIX_RESTORED))
            } else {
                CalloutModel(CalloutTone.Warning, s.t(I18nKeys.SettingsUi.RPC_FIX_WARNING))
            },
            field = UrlFieldModel(
                id = "rpc",
                label = s.t(I18nKeys.SettingsUi.RPC_FIX_LABEL),
                value = "https://polygon-rpc.com",
                badge = if (restored) latency(96) else null,
                tone = if (restored) SettingsTone.Ok else SettingsTone.Error,
            ),
            primary = s.t(
                if (restored) I18nKeys.SettingsUi.COMMON_DONE else I18nKeys.SettingsUi.RPC_FIX_SAVE,
            ),
            // Nothing left to go and get once it works.
            providersLabel = if (restored) null else s.t(I18nKeys.SettingsUi.RPC_PROVIDERS_HINT),
            providers = if (restored) {
                emptyList()
            } else {
                listOf("Alchemy", "QuickNode", "dRPC", "Chainlist")
            },
            report = if (restored) null else s.t(I18nKeys.SettingsUi.RPC_REPORT),
        )
    }

    private fun balanceDetail(s: VelaStrings) = BalanceDetailModel(
        title = s.t(I18nKeys.SettingsUi.BALANCE_DETAIL_TITLE),
        summary = s.t(I18nKeys.SettingsUi.BALANCE_DETAIL_TOTAL, mapOf("amount" to TOTAL_BALANCE)),
        sectionPending = s.t(I18nKeys.SettingsUi.BALANCE_DETAIL_NETWORKS),
        pendingNote = s.t(I18nKeys.SettingsUi.BALANCE_DETAIL_NOTE),
        // Rate-limiting gets a grey line and no button because it resolves
        // itself; a dead RPC gets a red line and 立即重试 because it does not.
        pending = listOf(
            BalanceDetailRowModel(
                "polygon",
                mark("polygon"),
                network("polygon").name,
                status = s.t(I18nKeys.SettingsUi.BALANCE_DETAIL_RETRYING),
                tone = SettingsTone.Neutral,
            ),
            BalanceDetailRowModel(
                "gnosis",
                mark("gnosis"),
                network("gnosis").name,
                status = s.t(I18nKeys.SettingsUi.BALANCE_DETAIL_FAILED),
                tone = SettingsTone.Error,
                action = s.t(I18nKeys.SettingsUi.BALANCE_DETAIL_RETRY),
            ),
        ),
        sectionDone = s.t(I18nKeys.SettingsUi.BALANCE_DETAIL_UPDATED),
        done = listOf(
            BalanceDetailRowModel("ethereum", mark("ethereum"), "Ethereum", amount = "\$2,412.11"),
            BalanceDetailRowModel("bnb", mark("bnb"), "BNB Chain", amount = "\$850.29"),
        ),
    )

    private fun relayer(s: VelaStrings) = RelayerModel(
        title = s.t(I18nKeys.SettingsUi.RELAYER_TITLE),
        lead = s.t(I18nKeys.SettingsUi.RELAYER_LEAD),
        mark = mark("gnosis"),
        name = network("gnosis").name,
        amountHint = s.t(
            I18nKeys.SettingsUi.RELAYER_AMOUNT_HINT,
            mapOf("amount" to "0.02", "symbol" to "xDAI"),
        ),
        qrCaption = s.t(I18nKeys.SettingsUi.RELAYER_ADDRESS_LABEL),
        addressDisplay = "0x7Bd0...4E9c",
        copyLabel = s.t(I18nKeys.SettingsUi.RELAYER_COPY),
        callout = CalloutModel(CalloutTone.Warning, s.t(I18nKeys.SettingsUi.RELAYER_DISCLAIMER)),
        primary = s.t(I18nKeys.SettingsUi.RELAYER_RETRY),
    )

    private fun indexDown(s: VelaStrings) = IndexDownModel(
        title = s.t(I18nKeys.SettingsUi.INDEX_DOWN_TITLE),
        subtitle = s.t(I18nKeys.SettingsUi.INDEX_DOWN_SUBTITLE),
        callout = CalloutModel(CalloutTone.Warning, s.t(I18nKeys.SettingsUi.INDEX_DOWN_WARNING)),
        field = UrlFieldModel(
            id = "endpoint",
            label = s.t(I18nKeys.SettingsUi.INDEX_DOWN_ENDPOINT_LABEL),
            value = "https://p256-index-rs.getvela.app",
            badge = StatusPillModel(SettingsTone.Error, s.t(I18nKeys.SettingsUi.NETWORK_OFFLINE)),
        ),
        primary = s.t(I18nKeys.SettingsUi.COMMON_TRY_AGAIN),
        secondary = s.t(I18nKeys.SettingsUi.INDEX_DOWN_EDIT),
        footer = s.t(I18nKeys.SettingsUi.INDEX_DOWN_FOOTER),
    )

    // --- State table ---------------------------------------------------------

    /** Which page + overlay each mock is. The screen reads only this. */
    private data class Shape(
        val page: SettingsPage,
        val overlay: SettingsOverlay,
        val rescue: Boolean = false,
        val backdrop: String? = null,
    )

    private fun shape(state: SettingsScreenState): Shape = when (state) {
        SettingsScreenState.ST1, SettingsScreenState.ST1B ->
            Shape(SettingsPage.Home, SettingsOverlay.None)
        SettingsScreenState.ST2 -> Shape(SettingsPage.Home, SettingsOverlay.Accounts)
        SettingsScreenState.ST3, SettingsScreenState.ST3B ->
            Shape(SettingsPage.Home, SettingsOverlay.SignOut)
        SettingsScreenState.ST4 -> Shape(SettingsPage.Home, SettingsOverlay.Language)
        SettingsScreenState.ST5 -> Shape(SettingsPage.Home, SettingsOverlay.Currency)
        SettingsScreenState.ST6 -> Shape(SettingsPage.Home, SettingsOverlay.NumberFormat)
        SettingsScreenState.ST7 -> Shape(SettingsPage.Home, SettingsOverlay.DateFormat)
        SettingsScreenState.ST8 -> Shape(SettingsPage.Home, SettingsOverlay.TimeFormat)
        SettingsScreenState.ST9 -> Shape(SettingsPage.Networks, SettingsOverlay.None)
        SettingsScreenState.ST9B -> Shape(SettingsPage.NetworkDetail, SettingsOverlay.None)
        SettingsScreenState.ST10, SettingsScreenState.ST10B, SettingsScreenState.ST10C ->
            Shape(SettingsPage.AddNetwork, SettingsOverlay.None)
        SettingsScreenState.ST11 -> Shape(SettingsPage.RpcProviders, SettingsOverlay.None)
        SettingsScreenState.ST12 -> Shape(SettingsPage.Endpoints, SettingsOverlay.None)
        SettingsScreenState.ST13 -> Shape(SettingsPage.Storage, SettingsOverlay.None)
        SettingsScreenState.ST13B ->
            Shape(SettingsPage.Storage, SettingsOverlay.ClearCaches, backdrop = "storage")
        SettingsScreenState.ST14 -> Shape(SettingsPage.About, SettingsOverlay.None)
        SettingsScreenState.ST15 -> Shape(SettingsPage.Home, SettingsOverlay.Feedback)
        SettingsScreenState.ST16 -> Shape(SettingsPage.Home, SettingsOverlay.EraseDevice)
        SettingsScreenState.SR1 -> Shape(SettingsPage.Home, SettingsOverlay.None, rescue = true)
        SettingsScreenState.SR2, SettingsScreenState.SR2B ->
            Shape(SettingsPage.Home, SettingsOverlay.RpcFix, rescue = true, backdrop = "wallet")
        SettingsScreenState.SR3 ->
            Shape(SettingsPage.Home, SettingsOverlay.BalanceDetail, rescue = true, backdrop = "wallet")
        SettingsScreenState.SR4 ->
            Shape(SettingsPage.Home, SettingsOverlay.Relayer, rescue = true, backdrop = "send")
        SettingsScreenState.SR5 -> Shape(SettingsPage.Home, SettingsOverlay.None, rescue = true)
    }

    fun buildState(state: SettingsScreenState, s: VelaStrings): SettingsScreenModel {
        val shape = shape(state)
        val addMode = when (state) {
            SettingsScreenState.ST10B -> "compatible"
            SettingsScreenState.ST10C -> "incompatible"
            else -> "search"
        }
        val backdropTitle = when (shape.backdrop) {
            "wallet" -> s.t(I18nKeys.SettingsUi.NAV_WALLET)
            "send" -> s.t(I18nKeys.SettingsUi.ACTION_SEND)
            "storage" -> s.t(I18nKeys.SettingsUi.STORAGE_TITLE)
            else -> s.t(I18nKeys.SettingsUi.TITLE)
        }

        return SettingsScreenModel(
            state = state,
            title = s.t(I18nKeys.SettingsUi.TITLE),
            page = shape.page,
            overlay = shape.overlay,
            selectedTab = if (shape.rescue) "wallet" else "settings",
            tabs = TabsModel(
                wallet = s.t(I18nKeys.SettingsUi.NAV_WALLET),
                contacts = s.t(I18nKeys.SettingsUi.NAV_CONTACTS),
                explore = s.t(I18nKeys.SettingsUi.NAV_EXPLORE),
                settings = s.t(I18nKeys.SettingsUi.NAV_SETTINGS),
            ),
            account = AccountRowModel(
                name = ACCOUNT_NAME,
                addressDisplay = ADDRESS_DISPLAY,
                addressFull = ADDRESS_FULL,
                action = s.t(I18nKeys.SettingsUi.ACCOUNT_SWITCH),
            ),
            sections = sections(s, advancedOpen = state == SettingsScreenState.ST1B),
            theme = SegmentedModel(
                label = s.t(I18nKeys.SettingsUi.THEME_TITLE),
                selected = "dark",
                segments = listOf(
                    SegmentModel("light", s.t(I18nKeys.SettingsUi.THEME_LIGHT), SettingsIcon.Sun),
                    SegmentModel("dark", s.t(I18nKeys.SettingsUi.THEME_DARK), SettingsIcon.Moon),
                    SegmentModel("auto", s.t(I18nKeys.SettingsUi.THEME_AUTO), SettingsIcon.Monitor),
                ),
            ),
            avatar = SegmentedModel(
                label = s.t(I18nKeys.SettingsUi.AVATAR_TITLE),
                selected = "identicon",
                segments = listOf(
                    SegmentModel("initials", s.t(I18nKeys.SettingsUi.AVATAR_INITIALS)),
                    SegmentModel("identicon", s.t(I18nKeys.SettingsUi.AVATAR_IDENTICON)),
                ),
            ),
            textScale = TextScaleModel(s.t(I18nKeys.SettingsUi.TEXT_SCALE), steps = 7, index = 3),
            signOutLabel = s.t(I18nKeys.SettingsUi.SIGN_OUT_BUTTON),
            eraseTitle = s.t(I18nKeys.SettingsUi.ERASE_TITLE),
            eraseSubtitle = s.t(I18nKeys.SettingsUi.ERASE_SUBTITLE),
            networksTitle = s.t(I18nKeys.SettingsUi.NETWORKS_TITLE),
            networksSubtitle = s.t(I18nKeys.SettingsUi.NETWORKS_SUBTITLE),
            networks = networks(s),
            addNetworkLabel = s.t(I18nKeys.SettingsUi.ADD_NETWORK_TITLE),
            networkDetail = networkDetail(s, mismatch = state == SettingsScreenState.ST9B),
            addNetwork = addNetwork(s, addMode),
            rpcProviders = rpcProviders(s),
            endpoints = endpoints(s),
            storage = storage(s),
            about = about(s),
            accountsSheet = accountsSheet(s),
            signOutSheet = signOutSheet(s, warned = state == SettingsScreenState.ST3B),
            languageSheet = languageSheet(s, "zh"),
            currencySheet = currencySheet(s),
            numberSheet = formatSheet(
                s,
                s.t(I18nKeys.SettingsUi.NUMBER_TITLE),
                s.t(I18nKeys.SettingsUi.NUMBER_SUBTITLE),
                NUMBER_SAMPLES,
                mapOf(4 to s.t(I18nKeys.SettingsUi.NOTE_INDIAN)),
            ),
            dateSheet = formatSheet(
                s,
                s.t(I18nKeys.SettingsUi.DATE_TITLE),
                s.t(I18nKeys.SettingsUi.DATE_SUBTITLE),
                DATE_SAMPLES,
            ),
            timeSheet = formatSheet(
                s,
                s.t(I18nKeys.SettingsUi.TIME_TITLE),
                s.t(I18nKeys.SettingsUi.TIME_SUBTITLE),
                TIME_SAMPLES,
                mapOf(
                    1 to s.t(I18nKeys.SettingsUi.NOTE_H24),
                    2 to s.t(I18nKeys.SettingsUi.NOTE_H12),
                ),
            ),
            clearCachesSheet = ConfirmSheetModel(
                title = s.t(I18nKeys.SettingsUi.STORAGE_CLEAR_TITLE),
                body = s.t(I18nKeys.SettingsUi.STORAGE_CLEAR_BODY),
                confirm = s.t(I18nKeys.SettingsUi.STORAGE_CLEAR_CONFIRM),
                cancel = s.t(I18nKeys.SettingsUi.COMMON_CANCEL),
                danger = false,
            ),
            eraseSheet = ConfirmSheetModel(
                title = s.t(I18nKeys.SettingsUi.ERASE_TITLE),
                body = s.t(I18nKeys.SettingsUi.ERASE_DESC),
                note = s.t(I18nKeys.SettingsUi.ERASE_KEEPS),
                callout = CalloutModel(CalloutTone.Danger, s.t(I18nKeys.SettingsUi.ERASE_LOSES)),
                confirm = s.t(I18nKeys.SettingsUi.ERASE_CONFIRM),
                cancel = s.t(I18nKeys.SettingsUi.ERASE_CANCEL),
                danger = true,
            ),
            feedback = feedback(s),
            rpcBanner = if (state == SettingsScreenState.SR1) rpcBanner(s) else null,
            rpcFix = rpcFix(s, restored = state == SettingsScreenState.SR2B),
            balanceDetail = balanceDetail(s),
            relayer = relayer(s),
            indexDown = indexDown(s),
            backdropTitle = backdropTitle,
            closeLabel = s.t(I18nKeys.SettingsUi.CLOSE),
        )
    }
}
