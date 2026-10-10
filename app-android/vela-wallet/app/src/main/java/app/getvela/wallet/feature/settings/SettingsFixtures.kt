package app.getvela.wallet.feature.settings

import app.getvela.wallet.feature.settings.core.WalletKeys
import app.getvela.wallet.feature.settings.core.RegistryBackup
import app.getvela.wallet.feature.settings.core.NetBoards
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.CreateKeyRow
import app.getvela.wallet.core.data.DebugMode
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.UnreachableNetwork
import app.getvela.wallet.core.diagnostics.BugReport
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

    // The founder's ruling (2026-09-12, spec 047): the settings home's first
    // block carries no 通讯录 row (the tab bar has the book) and no 反馈 row;
    // 反馈 lives in the last block, beside 关于 (spec 081 FR-016, as the web).
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
                // Spec 069: the default transaction speed — between the
                // endpoints and the storage, as the web places it.
                SettingsRowModel(
                    id = FEE_SPEED_ROW,
                    title = s.t(I18nKeys.SettingsUi.FEE_SPEED_TITLE),
                    icon = SettingsIcon.Clock,
                    subtitle = s.t(I18nKeys.SettingsUi.FEE_SPEED_SUBTITLE),
                    value = s.t(I18nKeys.Flows.GAS_TIER_FAST),
                ),
                // Spec 102: the signing pages this device keeps, next to how
                // sends are priced. Which one an account signs on is the
                // account's own "Where you review and sign".
                SettingsRowModel(
                    id = SIGNING_PAGES_ROW,
                    title = s.t("settings.signing.title"),
                    icon = SettingsIcon.FileText,
                    subtitle = s.t("settings.signing.subtitle"),
                ),
                SettingsRowModel(
                    id = "storage",
                    title = s.t(I18nKeys.SettingsUi.STORAGE_TITLE),
                    icon = SettingsIcon.HardDrive,
                    subtitle = s.t(I18nKeys.SettingsUi.STORAGE_SUBTITLE),
                ),
            ),
        ),
        // Community (founder, 2026-09-27): the official accounts, directly above
        // the last group — the page ends … Advanced → Community → About / Feedback.
        SettingsSectionModel(
            label = s.t(I18nKeys.SettingsUi.SECTION_COMMUNITY),
            rows = CommunityLinks.ALL.map { link ->
                SettingsRowModel(
                    id = link.id,
                    title = link.title,
                    icon = link.icon,
                    subtitle = link.handle,
                    // It leaves the app: the external mark, not a chevron.
                    trailing = RowTrailing.External,
                )
            },
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
                // Spec 081 FR-016 / 078 round 3 (the web's placement): 反馈 back on
                // the screen, in the LAST block beside 关于 — the 047 ruling was
                // about the first block, and a report sheet no row opens is a
                // report nobody can send.
                SettingsRowModel(
                    id = "feedback",
                    title = s.t(I18nKeys.SettingsUi.FEEDBACK_TITLE),
                    icon = SettingsIcon.Feedback,
                    subtitle = s.t(I18nKeys.SettingsUi.FEEDBACK_SUBTITLE),
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
                // Spec 098 §5.1: said where the RPC is set — the relay is sent it.
                hint = s.t(I18nKeys.SettingsUi.NETWORK_SAVE_HINT) + " " + s.t(I18nKeys.SettingsUi.NETWORK_RELAY_NOTICE),
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
        // The two refusals are told apart by the signer row (the core's
        // `NetBlocker`): a chain with no P-256 verifier is refused for that
        // and nothing else matters; a chain that has it and lacks contracts
        // can be made ready.
        val noP256 = mode == "no-p256"
        // Four rows in every verdict: a refusal is only legible as an answer
        // if it shows WHICH requirement failed, so the list never shortens.
        // EntryPoint is deployed everywhere and passes in all three.
        val checks = listOf(
            CheckItemModel("EntryPoint v0.7", true),
            CheckItemModel(s.t(I18nKeys.SettingsUi.ADD_CHECK_SAFE), ok),
            CheckItemModel(s.t(I18nKeys.SettingsUi.ADD_CHECK_SIGNER), !noP256),
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
                    hint = s.t(I18nKeys.SettingsUi.NETWORK_RELAY_NOTICE),
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
                // The core's line for the reason (`NetCompatibility.hint_key`).
                callout = CalloutModel(
                    CalloutTone.Warning,
                    s.t(if (noP256) "settingsModals.addNetwork.noP256Hint" else I18nKeys.SettingsUi.ADD_INCOMPATIBLE_HINT),
                ),
                // An outline CTA plus a re-check link, not a greyed-out accent
                // one: an action you cannot take should not be dressed as the
                // action you came for. Chain Setup only where contracts can be
                // deployed — with the core's link, opened on this chain.
                secondary = if (noP256) null else s.t(I18nKeys.SettingsUi.ADD_CHAIN_TOOL),
                secondaryUrl = if (noP256) null else "https://getvela.app/chain-setup?chain=48900",
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
            // Spec 098 §5.1: a key set here rides in the RPC URL the relay is sent.
            description = s.t(I18nKeys.SettingsUi.PROVIDERS_DESCRIPTION) + " " + s.t(I18nKeys.SettingsUi.PROVIDERS_RELAY_NOTICE),
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
                "https://ethereum-data.getvela.app",
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
                "https://vela-relay-cf.getvela.app",
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

    private fun about(s: VelaStrings, state: SettingsScreenState): AboutModel = AboutModel(
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
            // Spec 095 (App Review 5.1.1(i)): the policy, the terms and a way to
            // reach us, reachable after onboarding too — the same three on every
            // shell. The row opens https:// + its value.
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_LINK_PRIVACY),
                "getvela.app/privacy",
                mono = true,
                external = true,
            ),
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_LINK_TERMS),
                "getvela.app/terms",
                mono = true,
                external = true,
            ),
            KeyValueRowModel(
                s.t(I18nKeys.SettingsUi.ABOUT_LINK_SUPPORT),
                "getvela.app/support",
                mono = true,
                external = true,
            ),
        ),
        footer = s.t(I18nKeys.SettingsUi.ABOUT_FOOTER),
        debugMode = DebugModeRowModel(
            title = s.t(I18nKeys.SettingsUi.ABOUT_DEBUG_MODE),
            body = s.t(I18nKeys.SettingsUi.ABOUT_DEBUG_MODE_BODY),
            revealedNotice = s.t(I18nKeys.SettingsUi.ABOUT_DEBUG_MODE_REVEALED),
            // ST14B: revealed and on — the switch in both of its looks is a tap apart.
            mode = if (state == SettingsScreenState.ST14B) DebugMode.On else DebugMode.Hidden,
        ),
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
                        id = app.getvela.wallet.core.data.Preferences.AUTO_LANGUAGE,
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

    /** The Settings row id of the default speed (spec 069). */
    /**
     * Spec 092's three cases in the core's order: last seen holding $4,500,
     * last seen empty, never read — what SR6 draws through the live builder.
     */
    val UNREACHABLE_VIEW = BalanceView(
        display_total_usd = 4_500.0,
        balance_partial = true,
        failed_chain_ids = listOf(1, 56, 137),
        unreachable_networks = listOf(
            UnreachableNetwork(1, "held", 4_500.0, I18nKeys.SettingsUi.LAST_SEEN),
            UnreachableNetwork(56, "empty", null, I18nKeys.SettingsUi.LAST_SEEN_EMPTY),
            UnreachableNetwork(137, "not_read", null, I18nKeys.SettingsUi.NOT_READ_YET),
        ),
        unreachable_key = I18nKeys.Wallet.UNREACHABLE_MANY,
        cached_total_usd = 4_500.0,
    )
    private val UNREACHABLE_NAMES = mapOf(1 to "Ethereum", 56 to "BNB Chain", 137 to "Polygon")

    const val FEE_SPEED_ROW = "fee-speed"

    /** The Settings row of Signing pages (spec 102). */
    const val SIGNING_PAGES_ROW = "signing-pages"


    /**
     * The default speed's sheet: the three speeds, fastest first, each with
     * the line on what it buys — never `rapid`, which nothing offers.
     */
    fun feeSpeedSheet(s: VelaStrings, selected: String) = SelectSheetModel(
        title = s.t(I18nKeys.SettingsUi.FEE_SPEED_SHEET_TITLE),
        subtitle = s.t(I18nKeys.SettingsUi.FEE_SPEED_SHEET_SUBTITLE),
        rows = listOf(
            Triple("fast", I18nKeys.Flows.GAS_TIER_FAST, I18nKeys.Flows.GAS_TIER_HINT_FAST),
            Triple("standard", I18nKeys.Flows.GAS_TIER_STANDARD, I18nKeys.Flows.GAS_TIER_HINT_STANDARD),
            Triple("slow", I18nKeys.Flows.GAS_TIER_SLOW, I18nKeys.Flows.GAS_TIER_HINT_SLOW),
        ).map { (id, name, hint) ->
            SelectRowModel(id = id, label = s.t(name), detail = s.t(hint), selected = id == selected)
        },
    )

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
        stepsPlaceholder = s.t(I18nKeys.SettingsUi.BUG_STEPS_PLACEHOLDER),
        sending = s.t(I18nKeys.SettingsUi.BUG_SENDING),
        successTitle = s.t(I18nKeys.SettingsUi.BUG_SUCCESS_TITLE),
        // Templates the sheet fills per render: the placeholder is handed back as its own value.
        successBodyNew = s.t(I18nKeys.SettingsUi.BUG_SUCCESS_NEW, mapOf("number" to "{{number}}")),
        successBodyDeduped = s.t(I18nKeys.SettingsUi.BUG_SUCCESS_DEDUPED, mapOf("number" to "{{number}}")),
        viewIssue = s.t(I18nKeys.SettingsUi.BUG_VIEW_ISSUE),
        fallbackTitle = s.t(I18nKeys.SettingsUi.BUG_FALLBACK_TITLE),
        fallbackBody = s.t(I18nKeys.SettingsUi.BUG_FALLBACK_BODY),
        openGithub = s.t(I18nKeys.SettingsUi.BUG_OPEN_GITHUB),
        done = s.t(I18nKeys.SettingsUi.BUG_DONE),
        tryAgain = s.t(I18nKeys.SettingsUi.COMMON_TRY_AGAIN),
        screenshotsLabel = s.t(I18nKeys.SettingsUi.BUG_SCREENSHOTS_LABEL),
        addScreenshots = s.t(I18nKeys.SettingsUi.BUG_ADD_SCREENSHOTS),
        screenshotsHint = s.t(I18nKeys.SettingsUi.BUG_SCREENSHOTS_HINT, mapOf("max" to BugReport.MAX_SCREENSHOTS.toString())),
        screenshotsPublic = s.t(I18nKeys.SettingsUi.BUG_SCREENSHOTS_PUBLIC),
        removeScreenshot = s.t(I18nKeys.SettingsUi.BUG_REMOVE_SCREENSHOT, mapOf("index" to "{{index}}")),
        screenshotsLimit = s.t(I18nKeys.SettingsUi.BUG_SCREENSHOTS_LIMIT, mapOf("max" to BugReport.MAX_SCREENSHOTS.toString())),
        screenshotUnsupported = s.t(I18nKeys.SettingsUi.BUG_SCREENSHOT_UNSUPPORTED),
        screenshotsDropped = s.t(I18nKeys.SettingsUi.BUG_SCREENSHOTS_DROPPED),
        fallbackScreenshots = s.t(I18nKeys.SettingsUi.BUG_FALLBACK_SCREENSHOTS),
        viewScreenshot = s.t(I18nKeys.SettingsUi.BUG_VIEW_SCREENSHOT, mapOf("index" to "{{index}}")),
        closeViewer = s.t(I18nKeys.SettingsUi.BUG_CLOSE_VIEWER),
        removeFromViewer = s.t(I18nKeys.SettingsUi.BUG_REMOVE_FROM_VIEWER),
    )

    // --- Rescue --------------------------------------------------------------

    private fun rpcBanner(s: VelaStrings) = RpcBannerModel(
        text = s.t(I18nKeys.Wallet.UNREACHABLE_MANY, mapOf("n" to "2")),
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
                hint = s.t(I18nKeys.SettingsUi.NETWORK_RELAY_NOTICE),
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
        SettingsScreenState.ST10, SettingsScreenState.ST10B, SettingsScreenState.ST10C, SettingsScreenState.ST10D,
        SettingsScreenState.ST10E, SettingsScreenState.ST10F, SettingsScreenState.ST10G,
        SettingsScreenState.ST10H, SettingsScreenState.ST10I, SettingsScreenState.ST10J ->
            Shape(SettingsPage.AddNetwork, SettingsOverlay.None)
        SettingsScreenState.ST11 -> Shape(SettingsPage.RpcProviders, SettingsOverlay.None)
        SettingsScreenState.ST12 -> Shape(SettingsPage.Endpoints, SettingsOverlay.None)
        SettingsScreenState.ST13 -> Shape(SettingsPage.Storage, SettingsOverlay.None)
        SettingsScreenState.ST13B ->
            Shape(SettingsPage.Storage, SettingsOverlay.ClearCaches, backdrop = "storage")
        SettingsScreenState.ST14, SettingsScreenState.ST14B -> Shape(SettingsPage.About, SettingsOverlay.None)
        SettingsScreenState.ST15 -> Shape(SettingsPage.Home, SettingsOverlay.Feedback)
        SettingsScreenState.ST16 -> Shape(SettingsPage.Home, SettingsOverlay.EraseDevice)
        SettingsScreenState.ST17, SettingsScreenState.ST17B, SettingsScreenState.ST17C -> Shape(SettingsPage.Venue, SettingsOverlay.None)
        SettingsScreenState.ST18, SettingsScreenState.ST18B -> Shape(SettingsPage.SigningPages, SettingsOverlay.None)
        SettingsScreenState.SR1 -> Shape(SettingsPage.Home, SettingsOverlay.None, rescue = true)
        SettingsScreenState.SR2, SettingsScreenState.SR2B ->
            Shape(SettingsPage.Home, SettingsOverlay.RpcFix, rescue = true, backdrop = "wallet")
        SettingsScreenState.SR3 ->
            Shape(SettingsPage.Home, SettingsOverlay.BalanceDetail, rescue = true, backdrop = "wallet")
        SettingsScreenState.SR4 ->
            Shape(SettingsPage.Home, SettingsOverlay.Relayer, rescue = true, backdrop = "send")
        SettingsScreenState.SR5 -> Shape(SettingsPage.Home, SettingsOverlay.None, rescue = true)
        SettingsScreenState.SR6, SettingsScreenState.SR7 ->
            Shape(SettingsPage.Home, SettingsOverlay.Unreachable, rescue = true, backdrop = "wallet")
        SettingsScreenState.SK1, SettingsScreenState.SK2, SettingsScreenState.SK3, SettingsScreenState.SK4 ->
            Shape(SettingsPage.Home, SettingsOverlay.None)
    }

    // -- the Keys block and its copy-to-Ethereum row ---------------------------

    /**
     * What the core's backup walk ends on for each SK board: the state and the
     * row that rides with it (`BackupState::row` — its words, tone and tap).
     * Written out here because a board has no chain to ask; the live screen
     * reads the row off the step and maps nothing.
     */
    fun backupCheck(state: SettingsScreenState): RegistryBackup.Check? {
        // The paragraph under the row rides with it, as the core sends it:
        // every state but the one that can never be copied.
        fun row(subtitle: String, tone: RegistryBackup.Tone, action: RegistryBackup.Action, explained: Boolean = true) =
            RegistryBackup.Row(
                "settingsModals.backup.title", "settingsModals.backup.$subtitle", tone, action,
                explainKey = I18nKeys.SettingsUi.BACKUP_EXPLAIN.takeIf { explained },
            )
        return when (state) {
            SettingsScreenState.SK1 -> RegistryBackup.Check(
                RegistryBackup.State.NotBackedUp,
                RegistryBackup.Call(1, "0x0000000000000000000000000000000000000000", "0x"),
                row("notBackedUp", RegistryBackup.Tone.Neutral, RegistryBackup.Action.Copy),
            )
            SettingsScreenState.SK2 -> RegistryBackup.Check(
                RegistryBackup.State.BackedUp, null,
                row("backedUp", RegistryBackup.Tone.Positive, RegistryBackup.Action.None),
            )
            SettingsScreenState.SK3 -> RegistryBackup.COULD_NOT
            SettingsScreenState.SK4 -> RegistryBackup.Check(
                RegistryBackup.State.NotCopyable, null,
                row("cannotCopy", RegistryBackup.Tone.Neutral, RegistryBackup.Action.None, explained = false),
            )
            else -> null
        }
    }

    /** The SK boards' keys: one synced in a vault the catalog names, one bound to a security key. */
    private fun withKeys(model: SettingsScreenModel, state: SettingsScreenState, s: VelaStrings): SettingsScreenModel {
        val check = backupCheck(state) ?: return model
        fun key(name: String, provider: String, method: KeyMethod, synced: Boolean, body: String) = WalletKeys.Row(
            key = CreateKeyRow(name, "platform", "internal", true, synced, "", provider, method),
            synced = synced,
            publicKeyHex = "04" + body.repeat(64),
        )
        val keys = WalletKeys.Result(
            WalletKeys.Source.Registry,
            listOf(
                key(ACCOUNT_NAME, "Google Password Manager", KeyMethod.Platform, true, "3a"),
                key("", "", KeyMethod.SecurityKey, false, "c5"),
            ),
        )
        return SettingsLive.withWalletKeys(model, keys, check, s)
    }

    // -- spec 102: where you review and sign, and the signing pages ----------

    /** The official page, as the core normalises it, and a page on the person's own domain. */
    const val OFFICIAL_PAGE = "https://sign.getvela.app/"
    const val OWN_PAGE = "https://sign.example.com/"

    /** A second self-hosted page, named by the person, whose build is new to Vela: its check asks. */
    const val ASK_PAGE = "https://signer.example.org/"

    /** The full version [ASK_PAGE]'s check asks the person to trust. */
    const val ASK_VERSION = "3f9a1c22b7e4d05a6c8f1e2d3b4a59687766554433221100ffeeddccbbaa9988"

    /**
     * What Settings → Signing pages keeps on the drawn device (the official
     * page is never stored): a self-hosted page the person did not name — so
     * it reads "Self-hosted · sign.example.com" (D6) — whose build they
     * trusted, and one they named whose build is new to Vela.
     */
    val SAVED_PAGES = listOf(
        app.getvela.wallet.feature.settings.core.SigningPage(OWN_PAGE, "", listOf("6ffe9ef2" + "0".repeat(56))),
        app.getvela.wallet.feature.settings.core.SigningPage(ASK_PAGE, "Home"),
    )

    /** 14:32 today — the boards' "checked" time (the web's). */
    private val CHECKED_AT: Long = java.util.Calendar.getInstance().apply {
        set(java.util.Calendar.HOUR_OF_DAY, 14)
        set(java.util.Calendar.MINUTE, 32)
        set(java.util.Calendar.SECOND, 0)
    }.timeInMillis

    private fun line(state: uniffi.vela_core_uniffi.SignerIntegrityState, key: String, version: String, opens: Boolean) =
        uniffi.vela_core_uniffi.SignerIntegrityLine(state, version, CHECKED_AT.toULong(), "componentsUi.signing.integrity.$key", opens)

    /**
     * The integrity lines the boards draw: the official page matches the
     * published build list; the self-hosted page is a build they trusted on
     * this device — or, on ST18b, one that will NOT open.
     */
    fun pageLine(url: String, refused: Boolean = false): uniffi.vela_core_uniffi.SignerIntegrityLine = when {
        url == OFFICIAL_PAGE -> line(uniffi.vela_core_uniffi.SignerIntegrityState.MATCHES, "matches", "0ba8ee8c", true)
        url == ASK_PAGE -> uniffi.vela_core_uniffi.SignerIntegrityLine(
            uniffi.vela_core_uniffi.SignerIntegrityState.ASK_TO_TRUST, ASK_VERSION.take(8), null,
            "componentsUi.signing.integrity.askTrust", false,
        )
        refused -> line(uniffi.vela_core_uniffi.SignerIntegrityState.MISMATCH, "mismatch", "7d41e0b9", false)
        else -> line(uniffi.vela_core_uniffi.SignerIntegrityState.TRUSTED_HERE, "trusted", "6ffe9ef2", true)
    }

    /** The version a board's page asks to be trusted, when it does. */
    fun pageToTrust(url: String): String? = ASK_VERSION.takeIf { url == ASK_PAGE }

    /** ST17C: [OWN_PAGE] redeployed — the build this device has not trusted yet. */
    const val OWN_NEW_VERSION = "9c2e7a41d3b8f6051e4a7c9d2b6f8e30a1c5d7e9f2b4a6c8d0e2f4a6b8c0d2e4"

    /**
     * The account the boards draw — on `getvela.app`, reviewing on the
     * official page — and (ST17b) one on its own domain, locked to its page.
     * Drawn through the live builder over the core's own `signingVenueChoices`,
     * so the board is a screen the core can produce.
     */
    private fun venueAccount(own: Boolean) = app.getvela.wallet.feature.onboarding.core.SessionAccountRow(
        index = 0,
        name = ACCOUNT_NAME,
        address = ADDRESS_FULL,
        signingDomain = if (own) "sign.example.com" else "getvela.app",
        signingVenueJson = """{"type":"page","url":"${if (own) OWN_PAGE else OFFICIAL_PAGE}"}""",
    )

    /** The signing pages the boards' device keeps — Settings → Signing pages, and the create / sign-in picker's list. */
    val SIGNING_PAGES_VIEW = app.getvela.wallet.feature.settings.core.SigningPagesView(
        pages = listOf(
            app.getvela.wallet.feature.settings.core.SigningPageRow(OFFICIAL_PAGE, "", "getvela.app", official = true),
            app.getvela.wallet.feature.settings.core.SigningPageRow(OWN_PAGE, "", "sign.example.com", trusted = SAVED_PAGES[0].trusted),
            app.getvela.wallet.feature.settings.core.SigningPageRow(ASK_PAGE, "Home", "signer.example.org"),
        ),
        saved = SAVED_PAGES,
        loaded = true,
    )

    /** The live builders over the boards' data: what the gallery shows is what a session would. */
    private fun withSigning(model: SettingsScreenModel, state: SettingsScreenState, s: VelaStrings): SettingsScreenModel {
        val refused = state == SettingsScreenState.ST18B
        // ST17C: the account's own page was redeployed — its check asks.
        val redeployed = state == SettingsScreenState.ST17C
        val view = if (refused) SIGNING_PAGES_VIEW.copy(add_error = "duplicate") else SIGNING_PAGES_VIEW
        val lines = { url: String ->
            if (redeployed && url == OWN_PAGE) {
                uniffi.vela_core_uniffi.SignerIntegrityLine(
                    uniffi.vela_core_uniffi.SignerIntegrityState.ASK_TO_TRUST, OWN_NEW_VERSION.take(8), null,
                    "componentsUi.signing.integrity.askTrust", false,
                )
            } else {
                pageLine(url, refused)
            }
        }
        val toTrust = { url: String -> if (redeployed && url == OWN_PAGE) OWN_NEW_VERSION else pageToTrust(url) }
        val pages = SettingsLive.withSigningPages(model, view, lines, s, toTrust)
        val drafted = if (refused) pages.copy(signingPages = pages.signingPages.copy(draft = OWN_PAGE)) else pages
        val own = state == SettingsScreenState.ST17B || redeployed
        return SettingsLive.withVenue(drafted, venueAccount(own = own), SAVED_PAGES, lines, s, toTrust)
    }

    fun buildState(state: SettingsScreenState, s: VelaStrings): SettingsScreenModel =
        withWizardStop(withKeys(withSigning(baseState(state, s), state, s), state, s), state, s)

    /** Which of the wizard's stops a board draws; `null` on every other board. */
    fun wizardStop(state: SettingsScreenState): NetBoards.Stop? = when (state) {
        SettingsScreenState.ST10E -> NetBoards.Stop.ScanMissingContracts
        SettingsScreenState.ST10F -> NetBoards.Stop.ScanNoP256
        SettingsScreenState.ST10G -> NetBoards.Stop.ScanCheckFailed
        SettingsScreenState.ST10H -> NetBoards.Stop.AlreadyAdded
        SettingsScreenState.ST10I -> NetBoards.Stop.NotFound
        SettingsScreenState.ST10J -> NetBoards.Stop.NoRpcEndpoint
        else -> null
    }

    /**
     * ST10E–ST10J: the wizard stopped — the real machine's view of it
     * (`NetBoards`) through [SettingsLive.withWizard], the builder a session
     * draws with. Nothing here words a stop.
     */
    private fun withWizardStop(model: SettingsScreenModel, state: SettingsScreenState, s: VelaStrings): SettingsScreenModel {
        val stop = wizardStop(state) ?: return model
        val search = addNetwork(s, "search").copy(results = emptyList())
        return SettingsLive.withWizard(model.copy(addNetwork = search), NetBoards.view(stop), s)
    }

    private fun baseState(state: SettingsScreenState, s: VelaStrings): SettingsScreenModel {
        val shape = shape(state)
        val addMode = when (state) {
            SettingsScreenState.ST10B -> "compatible"
            SettingsScreenState.ST10C -> "incompatible"
            SettingsScreenState.ST10D -> "no-p256"
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
            textScale = TextScaleModel(s.t(I18nKeys.SettingsUi.TEXT_SCALE), steps = 7, index = 3),
            signOutLabel = s.t(I18nKeys.SettingsUi.SIGN_OUT_BUTTON),
            eraseTitle = s.t(I18nKeys.SettingsUi.ERASE_TITLE),
            eraseSubtitle = s.t(I18nKeys.SettingsUi.ERASE_SUBTITLE),
            networksTitle = s.t(I18nKeys.SettingsUi.NETWORKS_TITLE),
            networksSubtitle = s.t(I18nKeys.SettingsUi.NETWORKS_SUBTITLE),
            networks = networks(s),
            addNetworkLabel = s.t(I18nKeys.SettingsUi.ADD_NETWORK_TITLE),
            removeNetworkLabel = s.t("settingsModals.network.removeTitle"),
            removeNetworkSheet = ConfirmSheetModel(
                title = s.t("settingsModals.network.removeTitle"),
                body = s.t("settingsModals.network.removeBody"),
                confirm = s.t("settingsModals.network.removeConfirm"),
                cancel = s.t("settingsModals.network.removeCancel"),
                danger = true,
            ),
            resetEndpointsSheet = ConfirmSheetModel(
                title = s.t(I18nKeys.SettingsUi.ENDPOINTS_RESET_TITLE),
                body = s.t(I18nKeys.SettingsUi.ENDPOINTS_RESET_BODY),
                confirm = s.t(I18nKeys.SettingsUi.ENDPOINTS_RESET_CONFIRM),
                cancel = s.t(I18nKeys.SettingsUi.ENDPOINTS_RESET_CANCEL),
                danger = true,
            ),
            networkDetail = networkDetail(s, mismatch = state == SettingsScreenState.ST9B),
            addNetwork = addNetwork(s, addMode),
            rpcProviders = rpcProviders(s),
            endpoints = endpoints(s),
            storage = storage(s),
            about = about(s, state),
            accountsSheet = accountsSheet(s),
            signOutSheet = signOutSheet(s, warned = state == SettingsScreenState.ST3B),
            languageSheet = languageSheet(s, "zh"),
            currencySheet = currencySheet(s),
            feeSpeedSheet = feeSpeedSheet(s, "fast"),
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
            // SR6 is drawn through the live builder, from a view shaped like
            // the core's: what the gallery shows is what a session would.
            unreachable = if (state == SettingsScreenState.SR7) {
                // SR7: the real machine's round in which Tempo's token list
                // did not load — one row, what was last read there, no Fix.
                SettingsLive.unreachable(
                    app.getvela.wallet.feature.wallet.core.BalanceBoards.tokenListUnreachable(ADDRESS_FULL, System.currentTimeMillis().toDouble()),
                    CurrencyView(code = "USD", rate = 1.0, committed = true),
                    app.getvela.wallet.feature.wallet.WalletFixtures.TOKEN_LIST_CHAINS,
                    s,
                )
            } else {
                SettingsLive.unreachable(
                    UNREACHABLE_VIEW,
                    CurrencyView(code = "USD", rate = 1.0, committed = true),
                    UNREACHABLE_NAMES,
                    s,
                )
            },
            relayer = relayer(s),
            indexDown = indexDown(s),
            backdropTitle = backdropTitle,
            closeLabel = s.t(I18nKeys.SettingsUi.CLOSE),
        )
    }
}
