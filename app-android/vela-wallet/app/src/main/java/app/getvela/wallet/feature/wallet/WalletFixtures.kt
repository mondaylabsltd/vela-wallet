package app.getvela.wallet.feature.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings

/**
 * Canonical wallet-home fixtures (spec 015, data-model.md — the single canon
 * all four platforms port; Android port of the web's `fixtures.ts`). Content is
 * verbatim from the `docs/design/wallet/` mocks (FR-012); builders merge it with
 * resolved corpus strings into display-ready view models. Pure data + assembly:
 * no fetching, no formatting rules, no business state.
 *
 * Chain-dot hex values are FIXTURE DATA (data-model.md "fixture-only colors;
 * not theme tokens") — they are exempt from the tokens-only rule the same way
 * ticker strings are.
 */
object WalletFixtures {

    // --- Canon ----------------------------------------------------------------

    object ChainColors {
        val bnb = Color(0xFFF0B90B) // #F0B90B
        val ethereum = Color(0xFF627EEA) // #627EEA
        val arbitrum = Color(0xFF28A0F0) // #28A0F0
        val gnosis = Color(0xFF21BCA5) // #21BCA5
        val base = Color(0xFF0052FF) // #0052FF
        val polygon = Color(0xFF8247E5) // #8247E5
    }

    val PILL_DOTS: List<Color> =
        listOf(ChainColors.ethereum, ChainColors.polygon, ChainColors.bnb)

    const val NAME = "大表哥"
    const val LONG_NAME = "这是一个非常长"
    const val ADDRESS_DISPLAY = "0x14fB1f…D1eA5c"
    const val ADDRESS_FULL = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"

    /** Identicon-board seeds (US3): cross-platform eyeball parity set. */
    val IDENTICON_BOARD_SEEDS: List<String> = listOf(
        ADDRESS_FULL,
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        "alice",
        "bob",
        "0x9F3c00000000000000000000000000000000021aE",
        "",
    )

    const val MASK = "••••"
    const val BALANCE_MASK = "••••••"
    const val NETWORK_COUNT = 8

    private enum class Day { Today, Yesterday }

    private sealed interface Direction {
        data class To(val name: String) : Direction
        data class From(val name: String) : Direction
        data class Plain(val text: String) : Direction
    }

    private data class ActivityFixture(
        val kind: ActivityKind,
        val direction: Direction,
        val day: Day,
        val amount: String,
        val unit: String,
        val positive: Boolean,
        val badgeColor: Color,
    )

    private val DEFAULT_ACTIVITY = listOf(
        ActivityFixture(
            kind = ActivityKind.Sent,
            direction = Direction.To("hold on"),
            day = Day.Today,
            amount = "−2",
            unit = "POL",
            positive = false,
            badgeColor = ChainColors.polygon,
        ),
        ActivityFixture(
            kind = ActivityKind.Received,
            direction = Direction.From("0x9F3c…21aE"),
            day = Day.Today,
            amount = "+120",
            unit = "USDT",
            positive = true,
            badgeColor = ChainColors.ethereum,
        ),
        ActivityFixture(
            kind = ActivityKind.Dapp,
            direction = Direction.Plain("PancakeSwap · BNB Chain"),
            day = Day.Today,
            amount = "−0.05",
            unit = "BNB",
            positive = false,
            badgeColor = ChainColors.bnb,
        ),
        ActivityFixture(
            kind = ActivityKind.Received,
            direction = Direction.From("Alice"),
            day = Day.Yesterday,
            amount = "+50",
            unit = "USDC",
            positive = true,
            badgeColor = ChainColors.base,
        ),
    )

    private val EXTREME_ACTIVITY = listOf(
        ActivityFixture(
            kind = ActivityKind.Sent,
            direction = Direction.To("Alexandra"),
            day = Day.Today,
            amount = "−1234.5678",
            unit = "POL",
            positive = false,
            badgeColor = ChainColors.polygon,
        ),
        ActivityFixture(
            kind = ActivityKind.Dapp,
            direction = Direction.Plain("app.uniswap.org · BNB"),
            day = Day.Today,
            amount = "−0.0000001",
            unit = "BNB",
            positive = false,
            badgeColor = ChainColors.bnb,
        ),
    )

    private data class AssetFixture(
        val ticker: String,
        val chain: String,
        val badgeColor: Color,
        val balance: String,
        /** null = no price (H4's CAKE row). */
        val fiat: String?,
    )

    private val DEFAULT_ASSETS = listOf(
        AssetFixture("BNB", "BNB Chain", ChainColors.bnb, "0.8533", "$496.46"),
        AssetFixture("ETH", "Arbitrum", ChainColors.arbitrum, "0.2253", "$422.62"),
        AssetFixture("ETH", "Ethereum", ChainColors.ethereum, "0.0689", "$129.25"),
        AssetFixture("XDAI", "Gnosis", ChainColors.gnosis, "74.3965", "$74.38"),
        AssetFixture("USDT", "Ethereum", ChainColors.ethereum, "53.4836", "$53.48"),
        AssetFixture("USDC", "Polygon", ChainColors.polygon, "12.04", "$12.04"),
    )

    private val PARTIAL_PRICE_ASSETS = listOf(
        DEFAULT_ASSETS[0],
        DEFAULT_ASSETS[1],
        AssetFixture("CAKE", "BNB Chain", ChainColors.bnb, "18.20", null),
    )

    private val EXTREME_ASSETS = listOf(
        AssetFixture("WBTC", "以太坊主网 Ethereum", ChainColors.ethereum, "0.00000042", "$0.03"),
        AssetFixture("USDT", "Ethereum", ChainColors.ethereum, "1,234,567.8901", "$1,234,567.89"),
    )

    private data class ChainFixture(val name: String, val dot: Color, val count: Int)

    private val CHAINS = listOf(
        ChainFixture("BNB Chain", ChainColors.bnb, 1),
        ChainFixture("Ethereum", ChainColors.ethereum, 3),
        ChainFixture("Arbitrum", ChainColors.arbitrum, 1),
        ChainFixture("Gnosis", ChainColors.gnosis, 1),
        ChainFixture("Base", ChainColors.base, 1),
        ChainFixture("Polygon", ChainColors.polygon, 1),
    )

    // --- Assembly -------------------------------------------------------------

    private fun subtitle(strings: VelaStrings, f: ActivityFixture): String = when (val d = f.direction) {
        // Mobile rows carry no time part (desktop adds "· 今天 14:02"; data-model).
        is Direction.To -> strings.t(I18nKeys.Wallet.TO_NAME, mapOf("name" to d.name))
        is Direction.From -> strings.t(I18nKeys.Wallet.FROM_NAME, mapOf("name" to d.name))
        is Direction.Plain -> d.text
    }

    private fun activityRow(
        strings: VelaStrings,
        f: ActivityFixture,
        masked: Boolean,
    ): ActivityRowModel = ActivityRowModel(
        kind = f.kind,
        title = when (f.kind) {
            ActivityKind.Sent -> strings.t(I18nKeys.Wallet.LABEL_SENT)
            ActivityKind.Received -> strings.t(I18nKeys.Wallet.LABEL_RECEIVED)
            ActivityKind.Dapp -> strings.t(I18nKeys.Wallet.LABEL_DAPP_TX)
        },
        subtitle = subtitle(strings, f),
        amount = if (masked) MASK else f.amount,
        unit = f.unit,
        positive = f.positive,
        masked = masked,
        badgeColor = f.badgeColor,
    )

    private fun groupByDay(
        strings: VelaStrings,
        fixtures: List<ActivityFixture>,
        masked: Boolean = false,
    ): List<ActivityGroupModel> {
        val groups = mutableListOf<ActivityGroupModel>()
        for (f in fixtures) {
            val label = when (f.day) {
                Day.Today -> strings.t(I18nKeys.Wallet.DAY_TODAY)
                Day.Yesterday -> strings.t(I18nKeys.Wallet.DAY_YESTERDAY)
            }
            val row = activityRow(strings, f, masked)
            val last = groups.lastOrNull()
            if (last != null && last.label == label) {
                groups[groups.lastIndex] = last.copy(rows = last.rows + row)
            } else {
                groups += ActivityGroupModel(label, listOf(row))
            }
        }
        return groups
    }

    private fun assetRow(
        strings: VelaStrings,
        f: AssetFixture,
        masked: Boolean = false,
    ): AssetRowModel = AssetRowModel(
        ticker = f.ticker,
        chain = f.chain,
        badgeColor = f.badgeColor,
        balance = if (masked) MASK else f.balance,
        fiat = when {
            masked -> AssetFiatModel.Masked
            f.fiat == null -> AssetFiatModel.NoPrice(strings.t(I18nKeys.Wallet.NO_PRICE))
            else -> AssetFiatModel.Value(f.fiat)
        },
        masked = masked,
    )

    private fun chainRows(strings: VelaStrings): List<ChainRowModel> = buildList {
        add(
            ChainRowModel(
                name = strings.t(I18nKeys.Wallet.ALL_NETWORKS),
                dot = null,
                count = NETWORK_COUNT,
                selected = true,
            ),
        )
        CHAINS.forEach { add(ChainRowModel(it.name, it.dot, it.count, selected = false)) }
    }

    private fun balance(
        strings: VelaStrings,
        state: BalanceStateKind,
        integer: String? = null,
        decimals: String? = null,
        status: BalanceStatusModel? = null,
    ): BalanceModel = BalanceModel(
        label = strings.t(I18nKeys.Wallet.TOTAL_BALANCE),
        currency = "USD",
        state = state,
        integer = if (state == BalanceStateKind.Hidden) BALANCE_MASK else integer,
        decimals = if (state == BalanceStateKind.Hidden) null else decimals,
        liveText = if (state == BalanceStateKind.ZeroLive) {
            strings.t(I18nKeys.Wallet.LIVE_INDICATOR)
        } else {
            null
        },
        status = status,
        a11yHide = strings.t(I18nKeys.Wallet.A11Y_HIDE_BALANCE),
        a11yShow = strings.t(I18nKeys.Wallet.A11Y_SHOW_BALANCE),
    )

    private fun header(long: Boolean): WalletHeaderModel = WalletHeaderModel(
        name = if (long) LONG_NAME else NAME,
        addressDisplay = ADDRESS_DISPLAY,
        identiconSeed = ADDRESS_FULL,
    )

    /** Assemble the mobile home view model for one H-state. */
    fun buildMobileState(state: WalletScreenState, strings: VelaStrings): WalletHomeModel {
        val extreme = state == WalletScreenState.H7 || state == WalletScreenState.H7X

        fun sections(mode: SectionMode): Pair<SectionModel, SectionModel> = Pair(
            SectionModel(
                title = strings.t(I18nKeys.Wallet.SECTION_ACTIVITY),
                action = strings.t(I18nKeys.Wallet.FILTER_ALL),
                mode = mode,
                empty = EmptyStateModel(
                    title = strings.t(I18nKeys.Wallet.EMPTY_NO_ACTIVITY),
                    caption = strings.t(I18nKeys.Wallet.EMPTY_ACTIVITY_SUBTITLE),
                ),
            ),
            SectionModel(
                title = strings.t(I18nKeys.Wallet.SECTION_ASSETS),
                action = strings.t(I18nKeys.Wallet.FILTER_ALL),
                mode = mode,
                empty = EmptyStateModel(
                    title = strings.t(I18nKeys.Wallet.ASSETS_EMPTY_TITLE),
                    caption = strings.t(I18nKeys.Wallet.ASSETS_EMPTY_SUBTEXT),
                ),
            ),
        )

        val (rowsActivity, rowsAssets) = sections(SectionMode.Rows)
        val base = WalletHomeModel(
            state = state,
            header = header(long = extreme),
            pill = if (extreme) {
                NetworkPillModel.Single(dot = ChainColors.bnb, label = "BNB Chain")
            } else {
                NetworkPillModel.All(dots = PILL_DOTS, label = strings.t(I18nKeys.Wallet.PILL_ALL))
            },
            balance = balance(strings, BalanceStateKind.Normal, integer = "$1,383", decimals = "28"),
            actions = ActionsModel(
                receive = strings.t(I18nKeys.Wallet.ACTION_RECEIVE),
                send = strings.t(I18nKeys.Wallet.ACTION_SEND),
                scan = strings.t(I18nKeys.Wallet.ACTION_SCAN),
            ),
            activitySection = rowsActivity,
            activityGroups = groupByDay(strings, DEFAULT_ACTIVITY),
            assetsSection = rowsAssets,
            assetRows = DEFAULT_ASSETS.map { assetRow(strings, it) },
            tabs = TabsModel(
                wallet = strings.t(I18nKeys.Wallet.NAV_WALLET),
                contacts = strings.t(I18nKeys.Wallet.NAV_CONTACTS),
                explore = strings.t(I18nKeys.Wallet.NAV_EXPLORE),
                settings = strings.t(I18nKeys.Wallet.NAV_SETTINGS),
            ),
            textScale = if (state == WalletScreenState.H7X) 1.35f else 1f,
        )

        return when (state) {
            // First screen: two activity rows / three asset rows visible; scroll shows the rest.
            WalletScreenState.H1, WalletScreenState.H1S -> base

            WalletScreenState.H2 -> {
                val (activityEmpty, assetsEmpty) = sections(SectionMode.Empty)
                base.copy(
                    balance = balance(strings, BalanceStateKind.ZeroLive, integer = "$0", decimals = "00"),
                    activitySection = activityEmpty,
                    activityGroups = emptyList(),
                    assetsSection = assetsEmpty,
                    assetRows = emptyList(),
                )
            }

            WalletScreenState.H3 -> {
                val (activityLoading, assetsLoading) = sections(SectionMode.Loading)
                base.copy(
                    balance = balance(strings, BalanceStateKind.Loading),
                    activitySection = activityLoading,
                    activityGroups = emptyList(),
                    assetsSection = assetsLoading,
                    assetRows = emptyList(),
                )
            }

            WalletScreenState.H4 -> base.copy(
                balance = balance(
                    strings,
                    BalanceStateKind.Normal,
                    integer = "$1,383",
                    decimals = "46",
                    status = BalanceStatusModel(
                        kind = BalanceStatusKind.Warning,
                        text = strings.t(I18nKeys.Wallet.BALANCE_UNPRICED),
                    ),
                ),
                activityGroups = groupByDay(strings, DEFAULT_ACTIVITY.take(2)),
                assetRows = PARTIAL_PRICE_ASSETS.map { assetRow(strings, it) },
            )

            WalletScreenState.H5 -> base.copy(
                balance = balance(strings, BalanceStateKind.Hidden),
                activityGroups = groupByDay(strings, DEFAULT_ACTIVITY, masked = true),
                assetRows = DEFAULT_ASSETS.map { assetRow(strings, it, masked = true) },
            )

            WalletScreenState.H6 -> base.copy(
                balance = balance(
                    strings,
                    BalanceStateKind.Normal,
                    integer = "$1,383",
                    decimals = "28",
                    status = BalanceStatusModel(
                        kind = BalanceStatusKind.Refreshing,
                        text = strings.t(I18nKeys.Wallet.BALANCE_STALE),
                    ),
                ),
            )

            WalletScreenState.H7, WalletScreenState.H7X -> base.copy(
                balance = balance(strings, BalanceStateKind.Normal, integer = "$1,234,567", decimals = "89"),
                activityGroups = groupByDay(strings, EXTREME_ACTIVITY),
                assetRows = EXTREME_ASSETS.map { assetRow(strings, it) },
            )

            WalletScreenState.H8 -> base.copy(
                sheet = SheetModel(
                    title = strings.t(I18nKeys.Wallet.SELECT_CHAIN),
                    rows = chainRows(strings),
                ),
            )
        }
    }
}
