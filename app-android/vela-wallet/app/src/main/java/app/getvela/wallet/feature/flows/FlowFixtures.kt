package app.getvela.wallet.feature.flows

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.ActivityGroupModel
import app.getvela.wallet.feature.wallet.ActivityKind
import app.getvela.wallet.feature.wallet.ActivityRowModel
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.AssetRowModel
import app.getvela.wallet.feature.wallet.WalletFixtures

/**
 * Canonical wallet-flow fixtures (spec 021 — the Android port of the web's
 * `src/lib/flows/fixtures.ts`, byte-for-byte the same canon).
 *
 * Pure data plus assembly. Nothing here fetches, signs, formats a number or
 * decides a business rule.
 *
 * Where a mock invented content the product already has a canon for, the canon
 * wins: the contact picker uses spec 018's roster and every address is spec
 * 015's or spec 018's, so identicon artwork matches across features and across
 * clients. Chain-dot hex values are FIXTURE DATA, exempt from the tokens-only
 * rule exactly as spec 015's are.
 */
object FlowFixtures {

    // --- Canon ---------------------------------------------------------------

    /** The two chains R1 draws that the home screen never held a balance on. */
    object ExtraChainColors {
        val optimism = Color(0xFFFF0420) // #FF0420
        val avalanche = Color(0xFFE84142) // #E84142
    }

    data class NetworkFixture(
        val name: String,
        val code: String,
        val color: Color,
        val chainId: String,
    )

    /** The eight supported networks, in the order R1 lists them. */
    val NETWORKS: List<NetworkFixture> = listOf(
        NetworkFixture("Ethereum", "ETH", WalletFixtures.ChainColors.ethereum, "1"),
        NetworkFixture("BNB Chain", "BNB", WalletFixtures.ChainColors.bnb, "56"),
        NetworkFixture("Polygon", "POL", WalletFixtures.ChainColors.polygon, "137"),
        NetworkFixture("Arbitrum", "ARB", WalletFixtures.ChainColors.arbitrum, "42161"),
        NetworkFixture("Optimism", "OP", ExtraChainColors.optimism, "10"),
        NetworkFixture("Base", "BASE", WalletFixtures.ChainColors.base, "8453"),
        NetworkFixture("Avalanche", "AVAX", ExtraChainColors.avalanche, "43114"),
        NetworkFixture("Gnosis", "GNO", WalletFixtures.ChainColors.gnosis, "100"),
    )

    /** USDT on Ethereum — the real contract, as the mocks print it. */
    const val USDT_CONTRACT = "0xdAC17F958D2ee523a2206206994597C13D831ec7"
    const val USDT_CONTRACT_SHORT = "0xdAC1…1ec7"

    // Spec 018's roster, reused rather than re-invented.
    private const val ALICE_DISPLAY = "0x9F3c…21aE"
    private const val ALICE_FULL = "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE"
    private const val A_HAO_FULL = "0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02"
    private const val HOLD_ON_DISPLAY = "0xCafe…F00d"
    private const val HOLD_ON_FULL = "0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d"

    private const val TX_HASH_RECEIVED = "0x8f3a…c21d"
    private const val TX_HASH_SENT = "0x3c2d…8e1f"

    /**
     * Split a 0x address into the two lines the mocks wrap it into.
     * 42 characters, so 21 and 21 — an even break rather than one that leaves a
     * stub on the second line.
     */
    fun addressLines(address: String): Pair<String, String> {
        val half = (address.length + 1) / 2
        return address.substring(0, half) to address.substring(half)
    }

    val MOBILE_STATES: List<FlowState> = FlowState.entries

    private data class AssetFixture(
        val ticker: String,
        val chain: String,
        val color: Color,
        val balance: String,
        val fiat: String,
    )

    /** The assets T1 lists, verbatim from the mock. */
    private val ASSETS = listOf(
        AssetFixture("BNB", "BNB Chain", WalletFixtures.ChainColors.bnb, "0.8533", "$496.46"),
        AssetFixture("ETH", "Arbitrum", WalletFixtures.ChainColors.arbitrum, "0.2253", "$422.62"),
        AssetFixture("ETH", "Ethereum", WalletFixtures.ChainColors.ethereum, "0.0689", "$129.25"),
        AssetFixture("XDAI", "Gnosis", WalletFixtures.ChainColors.gnosis, "74.3965", "$74.38"),
        AssetFixture("USDT", "Ethereum", WalletFixtures.ChainColors.ethereum, "53.4836", "$53.48"),
        AssetFixture("USDC", "Polygon", WalletFixtures.ChainColors.polygon, "12.04", "$12.04"),
    )

    /** SD1's order differs from T1's: the picker leads with what you'd send. */
    private val SEND_ASSETS = listOf(
        ASSETS[4],
        ASSETS[2],
        AssetFixture("USDC", "Ethereum", WalletFixtures.ChainColors.ethereum, "18.20", "$18.20"),
        ASSETS[0],
        ASSETS[3],
    )

    private fun AssetFixture.row(): AssetRowModel = AssetRowModel(
        ticker = ticker,
        chain = chain,
        badgeColor = color,
        balance = balance,
        fiat = AssetFiatModel.Value(fiat),
        masked = false,
    )

    // --- Assembly ------------------------------------------------------------

    private fun pill(s: VelaStrings) = FlowPillModel(
        dots = WalletFixtures.PILL_DOTS,
        label = s.t(I18nKeys.Flows.PILL_ALL),
    )

    private fun receiveList(s: VelaStrings) = ReceiveListModel(
        header = FlowHeaderModel(
            title = s.t(I18nKeys.Flows.RECEIVE_TITLE),
            backLabel = s.t(I18nKeys.Flows.BACK),
        ),
        subtitle = s.t(
            I18nKeys.Flows.RECEIVE_NETWORKS_LINE,
            mapOf("count" to WalletFixtures.NETWORK_COUNT.toString()),
        ),
        searchPlaceholder = s.t(I18nKeys.Flows.RECEIVE_SEARCH),
        emptyText = s.t(I18nKeys.Flows.RECEIVE_SEARCH_EMPTY),
        rows = NETWORKS.map { n ->
            NetworkRowModel(
                name = n.name,
                code = n.code,
                badgeColor = n.color,
                addressDisplay = WalletFixtures.ADDRESS_DISPLAY,
                copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS),
                qrLabel = s.t(I18nKeys.Flows.SCAN_TITLE),
            )
        },
    )

    private fun receiveQr(s: VelaStrings, asset: Boolean): ReceiveQrModel {
        val network = NETWORKS[0]
        return ReceiveQrModel(
            title = if (asset) {
                s.t(
                    I18nKeys.Flows.RECEIVE_QR_ASSET,
                    mapOf("network" to network.name, "symbol" to "USDT"),
                )
            } else {
                s.t(I18nKeys.Flows.RECEIVE_QR_NETWORK, mapOf("network" to network.name))
            },
            closeLabel = s.t(I18nKeys.Flows.CLOSE),
            contract = if (asset) {
                ContractLineModel(
                    label = s.t(I18nKeys.Flows.RECEIVE_TOKEN_CONTRACT),
                    value = USDT_CONTRACT_SHORT,
                    copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS),
                )
            } else {
                null
            },
            account = AddressCardModel(
                name = WalletFixtures.NAME,
                identiconSeed = WalletFixtures.ADDRESS_FULL,
                lines = addressLines(WalletFixtures.ADDRESS_FULL),
                copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS),
            ),
            centre = if (asset) {
                TokenMarkModel("USDT", WalletFixtures.ChainColors.gnosis)
            } else {
                TokenMarkModel(network.code, network.color)
            },
            warning = s.t(I18nKeys.Flows.RECEIVE_WARNING),
            saveImage = s.t(I18nKeys.Flows.RECEIVE_SAVE_IMAGE),
            viewOnExplorer = s.t(I18nKeys.Flows.VIEW_ON_EXPLORER),
        )
    }

    private fun shareCard(s: VelaStrings): ShareCardModel {
        val network = NETWORKS[0]
        return ShareCardModel(
            headline = s.t(I18nKeys.Flows.SHARE_CARD_HEADLINE),
            name = WalletFixtures.NAME,
            lines = addressLines(WalletFixtures.ADDRESS_FULL),
            networkNote = s.t(
                I18nKeys.Flows.SHARE_CARD_NETWORK_NOTE,
                mapOf("network" to network.name),
            ),
            networkMark = TokenMarkModel(network.code, network.color),
            identiconSeed = WalletFixtures.ADDRESS_FULL,
            wordmark = "Vela Wallet",
        )
    }

    fun scan(s: VelaStrings): ScanModel = ScanModel(
        title = s.t(I18nKeys.Flows.SCAN_TITLE),
        hint = s.t(I18nKeys.Flows.SCAN_HINT),
        closeLabel = s.t(I18nKeys.Flows.CLOSE),
        tools = listOf(
            ScanToolModel(ScanTool.Gallery, s.t(I18nKeys.Flows.SCAN_GALLERY)),
            ScanToolModel(ScanTool.Torch, s.t(I18nKeys.Flows.SCAN_TORCH)),
            ScanToolModel(ScanTool.Flip, s.t(I18nKeys.Flows.SCAN_FLIP)),
        ),
    )

    private fun historyGroups(s: VelaStrings): List<ActivityGroupModel> {
        val sent = s.t(I18nKeys.Flows.LABEL_SENT)
        val received = s.t(I18nKeys.Flows.LABEL_RECEIVED)
        fun to(name: String, clock: String) =
            "${s.t(I18nKeys.Flows.TO_NAME, mapOf("name" to name))} · $clock"

        fun from(name: String, clock: String) =
            "${s.t(I18nKeys.Flows.FROM_NAME, mapOf("name" to name))} · $clock"

        return listOf(
            ActivityGroupModel(
                label = s.t(I18nKeys.Flows.DAY_TODAY),
                rows = listOf(
                    ActivityRowModel(
                        kind = ActivityKind.Sent,
                        title = sent,
                        subtitle = to("hold on", "14:02"),
                        amount = "−2",
                        unit = "POL",
                        positive = false,
                        masked = false,
                        badgeColor = WalletFixtures.ChainColors.polygon,
                    ),
                    ActivityRowModel(
                        kind = ActivityKind.Received,
                        title = received,
                        subtitle = from(ALICE_DISPLAY, "11:20"),
                        amount = "+120",
                        unit = "USDT",
                        positive = true,
                        masked = false,
                        badgeColor = WalletFixtures.ChainColors.ethereum,
                    ),
                ),
            ),
            ActivityGroupModel(
                label = s.t(I18nKeys.Flows.DAY_YESTERDAY),
                rows = listOf(
                    ActivityRowModel(
                        kind = ActivityKind.Received,
                        title = received,
                        subtitle = from("Alice", "20:15"),
                        amount = "+50",
                        unit = "USDC",
                        positive = true,
                        masked = false,
                        badgeColor = WalletFixtures.ChainColors.base,
                    ),
                    ActivityRowModel(
                        kind = ActivityKind.Sent,
                        title = sent,
                        subtitle = to("Bob", "09:12"),
                        amount = "−0.4",
                        unit = "XDAI",
                        positive = false,
                        masked = false,
                        badgeColor = WalletFixtures.ChainColors.gnosis,
                    ),
                ),
            ),
            // A literal date once the run of named days ends — the mock's 8月12日.
            ActivityGroupModel(
                label = "8/12",
                rows = listOf(
                    ActivityRowModel(
                        kind = ActivityKind.Received,
                        title = received,
                        subtitle = from("0x21aE…9F3c", "08:44"),
                        amount = "+0.9",
                        unit = "BNB",
                        positive = true,
                        masked = false,
                        badgeColor = WalletFixtures.ChainColors.bnb,
                    ),
                ),
            ),
        )
    }

    private fun history(s: VelaStrings) = HistoryModel(
        header = FlowHeaderModel(
            title = s.t(I18nKeys.Flows.HISTORY_TITLE),
            backLabel = s.t(I18nKeys.Flows.BACK),
            pill = pill(s),
        ),
        mode = HistoryMode.Rows,
        emptyText = s.t(I18nKeys.Flows.HISTORY_EMPTY_FILTER),
        groups = historyGroups(s),
    )

    private fun txDetail(s: VelaStrings, received: Boolean): TxDetailModel {
        val network = if (received) NETWORKS[0] else NETWORKS[2]
        val facts = buildList {
            add(
                FactRowModel(
                    label = if (received) {
                        s.t(I18nKeys.Flows.DETAIL_FROM)
                    } else {
                        s.t(I18nKeys.Flows.DETAIL_TO)
                    },
                    value = if (received) ALICE_DISPLAY else "hold on",
                    lead = FactLead.Identicon(if (received) ALICE_FULL else HOLD_ON_FULL),
                    mono = received,
                    copy = s.t(I18nKeys.Flows.COPY_ADDRESS),
                ),
            )
            add(
                FactRowModel(
                    label = s.t(I18nKeys.Flows.DETAIL_CHAIN),
                    value = network.name,
                    lead = FactLead.Token(TokenMarkModel(network.code, network.color)),
                ),
            )
            // Only an ERC-20 transfer has a contract. A3's native coin does not,
            // and an empty row there invites "which contract?".
            if (received) {
                add(
                    FactRowModel(
                        // 代币合约, not 合约: the token sheet is already about a
                        // token, so it says "contract"; a transaction has to say
                        // WHICH contract.
                        label = s.t(I18nKeys.Flows.RECEIVE_TOKEN_CONTRACT),
                        value = USDT_CONTRACT_SHORT,
                        mono = true,
                        copy = s.t(I18nKeys.Flows.COPY_ADDRESS),
                    ),
                )
            }
            add(
                FactRowModel(
                    label = s.t(I18nKeys.Flows.DETAIL_DATE),
                    value = "${s.t(I18nKeys.Flows.DAY_TODAY)} ${if (received) "11:20" else "14:02"}",
                ),
            )
            add(
                FactRowModel(
                    label = s.t(I18nKeys.Flows.DETAIL_HASH),
                    value = if (received) TX_HASH_RECEIVED else TX_HASH_SENT,
                    mono = true,
                    copy = s.t(I18nKeys.Flows.COPY_ADDRESS),
                ),
            )
        }
        return TxDetailModel(
            title = if (received) {
                s.t(I18nKeys.Flows.TX_LABEL_RECEIVED, mapOf("symbol" to "USDT"))
            } else {
                s.t(I18nKeys.Flows.TX_LABEL_SENT, mapOf("symbol" to "POL"))
            },
            status = StatusChipModel(s.t(I18nKeys.Flows.STATUS_CONFIRMED), StatusTone.Success),
            closeLabel = s.t(I18nKeys.Flows.CLOSE),
            amount = if (received) "+120 USDT" else "−2 POL",
            fiat = if (received) "≈ $120.00" else "≈ $0.98",
            positive = received,
            facts = facts,
            viewOnExplorer = s.t(I18nKeys.Flows.VIEW_ON_EXPLORER),
        )
    }

    private fun assets(s: VelaStrings, empty: Boolean) = AssetsModel(
        header = FlowHeaderModel(
            title = s.t(I18nKeys.Flows.ASSETS_TITLE),
            backLabel = s.t(I18nKeys.Flows.BACK),
            action = s.t(I18nKeys.Flows.ASSETS_ADD),
            pill = pill(s),
        ),
        searchPlaceholder = s.t(I18nKeys.Flows.ASSETS_SEARCH),
        rows = if (empty) emptyList() else ASSETS.map { it.row() },
        addByAddress = s.t(I18nKeys.Flows.ASSETS_ADD_BY_ADDRESS),
        empty = if (empty) assetsEmpty(s) else null,
    )

    /**
     * T4's guided-empty body. Also what T1 shows when the chosen network holds
     * nothing while others do (issue #266) — the web's `emptyCopy`, the same
     * words in both places.
     */
    fun assetsEmpty(s: VelaStrings) = AssetsEmptyModel(
        title = s.t(I18nKeys.Flows.ASSETS_EMPTY_TITLE),
        caption = s.t(I18nKeys.Flows.ASSETS_EMPTY_SUBTEXT),
        cta = s.t(I18nKeys.Flows.ADD_TOKEN_TITLE),
        hintTitle = s.t(I18nKeys.Flows.ASSETS_NOT_SHOWING_TITLE),
        hintBody = s.t(I18nKeys.Flows.ASSETS_NOT_SHOWING_BODY),
    )

    private fun tokenDetail(s: VelaStrings) = TokenDetailModel(
        mark = TokenMarkModel("USDT", WalletFixtures.ChainColors.ethereum),
        symbol = "USDT",
        chain = "Ethereum",
        closeLabel = s.t(I18nKeys.Flows.CLOSE),
        balance = "53.4836 USDT",
        fiat = "$53.48",
        receive = s.t(I18nKeys.Flows.TOKEN_RECEIVE),
        send = s.t(I18nKeys.Flows.TOKEN_SEND),
        facts = listOf(
            FactRowModel(
                label = s.t(I18nKeys.Flows.TOKEN_PRICE),
                value = s.t(
                    I18nKeys.Flows.TOKEN_PRICE_VALUE,
                    mapOf("symbol" to "USDT", "value" to "$1.00"),
                ),
            ),
            FactRowModel(
                label = s.t(I18nKeys.Flows.TOKEN_CONTRACT),
                value = USDT_CONTRACT_SHORT,
                mono = true,
                copy = s.t(I18nKeys.Flows.COPY_ADDRESS),
            ),
            FactRowModel(label = s.t(I18nKeys.Flows.TOKEN_DECIMALS), value = "6"),
            FactRowModel(label = s.t(I18nKeys.Flows.ADD_LABEL_NETWORK), value = "Ethereum"),
        ),
        transactionsTitle = s.t(I18nKeys.Flows.TOKEN_TRANSACTIONS),
        rows = listOf(
            ActivityRowModel(
                kind = ActivityKind.Received,
                title = s.t(I18nKeys.Flows.LABEL_RECEIVED),
                subtitle = "${s.t(I18nKeys.Flows.FROM_NAME, mapOf("name" to ALICE_DISPLAY))} · " +
                    s.t(I18nKeys.Flows.DAY_TODAY),
                amount = "+120",
                unit = "USDT",
                positive = true,
                masked = false,
                badgeColor = WalletFixtures.ChainColors.polygon,
            ),
            ActivityRowModel(
                kind = ActivityKind.Sent,
                title = s.t(I18nKeys.Flows.LABEL_SENT),
                subtitle = "${s.t(I18nKeys.Flows.TO_NAME, mapOf("name" to "Alice"))} · 8/10",
                amount = "−30",
                unit = "USDT",
                positive = false,
                masked = false,
                badgeColor = WalletFixtures.ChainColors.polygon,
            ),
        ),
        viewOnExplorer = s.t(I18nKeys.Flows.TOKEN_EXPLORER),
    )

    /** T3 / T3b and their T5 / T5b failure variants. */
    enum class AddVariant { Erc20, Native, Erc20Invalid, Erc20NotFound, NativeIncompatible }

    private fun addToken(s: VelaStrings, variant: AddVariant): AddTokenModel {
        val native = variant == AddVariant.Native || variant == AddVariant.NativeIncompatible
        val avax = NETWORKS[6]

        if (native) {
            return AddTokenModel(
                title = s.t(I18nKeys.Flows.ADD_TOKEN_TITLE),
                closeLabel = s.t(I18nKeys.Flows.CLOSE),
                tab = AddTokenTab.Native,
                tabErc20 = s.t(I18nKeys.Flows.ADD_TAB_ERC20),
                tabNative = s.t(I18nKeys.Flows.ADD_TAB_NATIVE),
                fieldLabel = s.t(I18nKeys.Flows.ADD_NET_SEARCH_LABEL),
                fieldValue = "Avalanche",
                fieldPlaceholder = s.t(I18nKeys.Flows.ADD_NET_SEARCH_PLACEHOLDER),
                result = AddTokenResult.Network(
                    mark = TokenMarkModel(avax.code, avax.color),
                    name = avax.name,
                    chip = if (variant == AddVariant.NativeIncompatible) {
                        StatusChipModel(s.t(I18nKeys.Flows.ADD_NOT_COMPATIBLE), StatusTone.Error)
                    } else {
                        StatusChipModel(s.t(I18nKeys.Flows.ADD_COMPATIBLE), StatusTone.Success)
                    },
                    facts = listOf(
                        FactRowModel(
                            label = s.t(I18nKeys.Flows.ADD_CHAIN_ID),
                            value = avax.chainId,
                        ),
                        FactRowModel(
                            label = s.t(I18nKeys.Flows.ADD_NATIVE_TOKEN),
                            value = avax.code,
                        ),
                    ),
                    link = if (variant == AddVariant.NativeIncompatible) {
                        "${s.t(I18nKeys.Flows.ADD_ERROR_NOT_COMPATIBLE)} · " +
                            s.t(I18nKeys.Flows.ADD_DEPLOY_CONTRACTS)
                    } else {
                        null
                    },
                ),
                cta = s.t(I18nKeys.Flows.ADD_NETWORK_BTN),
                ctaDisabled = variant != AddVariant.Native,
            )
        }

        return AddTokenModel(
            title = s.t(I18nKeys.Flows.ADD_TOKEN_TITLE),
            closeLabel = s.t(I18nKeys.Flows.CLOSE),
            tab = AddTokenTab.Erc20,
            tabErc20 = s.t(I18nKeys.Flows.ADD_TAB_ERC20),
            tabNative = s.t(I18nKeys.Flows.ADD_TAB_NATIVE),
            network = AddTokenNetworkModel(
                mark = TokenMarkModel(NETWORKS[0].code, NETWORKS[0].color),
                name = NETWORKS[0].name,
                pickLabel = s.t(I18nKeys.Flows.ADD_NET_PICKER_SEARCH),
            ),
            fieldLabel = s.t(I18nKeys.Flows.ADD_TOKEN_ADDRESS),
            fieldValue = when (variant) {
                AddVariant.Erc20Invalid -> USDT_CONTRACT.dropLast(4)
                AddVariant.Erc20NotFound -> "0x1234…abcd"
                else -> USDT_CONTRACT
            },
            fieldPlaceholder = USDT_CONTRACT,
            fieldError = if (variant == AddVariant.Erc20Invalid) {
                s.t(I18nKeys.Flows.ADD_INVALID_ADDRESS)
            } else {
                null
            },
            result = when (variant) {
                AddVariant.Erc20Invalid -> AddTokenResult.None
                AddVariant.Erc20NotFound -> AddTokenResult.NotFound(
                    "${s.t(I18nKeys.Flows.ADD_NOT_FOUND_TITLE)} — " +
                        s.t(I18nKeys.Flows.ADD_NOT_FOUND_MESSAGE),
                )
                else -> AddTokenResult.Token(
                    mark = TokenMarkModel("USDT", WalletFixtures.ChainColors.ethereum),
                    name = "Tether USD",
                    detail = "USDT · ${s.t(I18nKeys.Flows.TOKEN_DECIMALS)} 6 · Ethereum",
                )
            },
            cta = s.t(I18nKeys.Flows.ADD_TO_WALLET),
            ctaDisabled = variant != AddVariant.Erc20,
        )
    }

    private fun sendPick(s: VelaStrings, multi: Boolean) = SendPickModel(
        header = FlowHeaderModel(
            title = if (multi) {
                s.t(I18nKeys.Flows.MULTI_SEND_TITLE)
            } else {
                s.t(I18nKeys.Flows.SELECT_TOKEN_TITLE)
            },
            backLabel = s.t(I18nKeys.Flows.BACK),
            pill = pill(s),
        ),
        searchPlaceholder = s.t(I18nKeys.Flows.SEND_SEARCH),
        filters = listOf(
            FilterChipModel("all", s.t(I18nKeys.Flows.FILTER_ALL), true),
            FilterChipModel("stable", s.t(I18nKeys.Flows.FILTER_STABLE), false),
            FilterChipModel("gas", s.t(I18nKeys.Flows.FILTER_GAS), false),
            FilterChipModel("other", s.t(I18nKeys.Flows.FILTER_OTHER), false),
        ),
        notice = if (multi) {
            SendNoticeModel(
                mark = TokenMarkModel(NETWORKS[0].code, NETWORKS[0].color),
                text = s.t(
                    I18nKeys.Flows.MULTI_SEND_NOTICE,
                    mapOf("network" to NETWORKS[0].name),
                ),
            )
        } else {
            null
        },
        rows = SEND_ASSETS.map { it.row() },
        selection = if (multi) {
            // The first three rows are on Ethereum; the last two are not, which
            // is exactly what the greying is there to explain.
            SendSelectionModel(
                selected = listOf(true, true, true, false, false),
                dimmed = listOf(false, false, false, true, true),
                selectAll = s.t(I18nKeys.Flows.SELECT_ALL_VALUABLE),
            )
        } else {
            null
        },
        cta = if (multi) {
            SendCtaModel(
                s.t(
                    I18nKeys.Flows.MULTI_SEND_CONTINUE,
                    mapOf("n" to "3", "chain" to NETWORKS[0].name),
                ),
                accent = true,
            )
        } else {
            SendCtaModel(s.t(I18nKeys.Flows.MULTI_SEND_TITLE), accent = false)
        },
    )

    private fun sendForm(s: VelaStrings, mode: SendFormMode): SendFormModel {
        val fee = FeeRowModel(
            label = s.t(I18nKeys.Flows.NETWORK_FEE),
            mark = TokenMarkModel("ETH", WalletFixtures.ChainColors.ethereum),
            value = when (mode) {
                SendFormMode.Single -> "0.0021 ETH · ≈$0.55"
                SendFormMode.Split -> "0.0034 ETH · ≈$0.89"
                SendFormMode.Sweep -> "0.0041 ETH · ≈$1.07"
            },
            openLabel = s.t(I18nKeys.Flows.FEE_TOKEN_LABEL),
        )
        val header = FlowHeaderModel(
            title = if (mode == SendFormMode.Sweep) {
                s.t(I18nKeys.Flows.MULTI_SEND_TITLE)
            } else {
                s.t(I18nKeys.Flows.SEND_TITLE, mapOf("symbol" to "USDT"))
            },
            backLabel = s.t(I18nKeys.Flows.BACK),
        )

        if (mode == SendFormMode.Sweep) {
            return SendFormModel(
                header = header,
                mode = mode,
                sweepSummary = s.t(
                    I18nKeys.Flows.MULTI_SEND_SUMMARY,
                    mapOf("n" to "3", "chain" to NETWORKS[0].name),
                ),
                sweepRows = listOf(
                    SweepRowModel(
                        TokenMarkModel("USDT", WalletFixtures.ChainColors.ethereum),
                        "USDT",
                        s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "53.4836")),
                        "53.4836",
                        s.t(I18nKeys.Flows.MAX),
                    ),
                    SweepRowModel(
                        TokenMarkModel("ETH", WalletFixtures.ChainColors.ethereum),
                        "ETH",
                        s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "0.0689")),
                        "0.05",
                        s.t(I18nKeys.Flows.MAX),
                    ),
                    SweepRowModel(
                        TokenMarkModel("USDC", WalletFixtures.ChainColors.ethereum),
                        "USDC",
                        s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "18.20")),
                        "18.20",
                        s.t(I18nKeys.Flows.MAX),
                    ),
                ),
                recipient = RecipientFieldModel(
                    label = s.t(I18nKeys.Flows.RECIPIENT_LABEL),
                    lines = ALICE_DISPLAY to "",
                    identiconSeed = ALICE_FULL,
                    pickLabel = s.t(I18nKeys.Flows.RECIPIENT_PICK_ARIA),
                    scanLabel = s.t(I18nKeys.Flows.SCAN_ARIA),
                    note = s.t(I18nKeys.Flows.MULTI_SEND_SAME_RECIPIENT),
                ),
                fee = fee,
                cta = s.t(I18nKeys.Flows.CONTINUE),
            )
        }

        val token = SendTokenCardModel(
            mark = TokenMarkModel("USDT", WalletFixtures.ChainColors.ethereum),
            symbol = "USDT",
            detail = "Ethereum · ${
                s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "53.4836"))
            }",
            max = if (mode == SendFormMode.Single) s.t(I18nKeys.Flows.MAX) else null,
        )

        if (mode == SendFormMode.Split) {
            return SendFormModel(
                header = header,
                mode = mode,
                token = token,
                recipients = listOf(
                    RecipientCardModel(
                        s.t(I18nKeys.Flows.RECIPIENT_N, mapOf("n" to "1")),
                        ALICE_DISPLAY,
                        ALICE_FULL,
                        "50",
                        s.t(I18nKeys.Flows.REMOVE_RECIPIENT),
                    ),
                    RecipientCardModel(
                        s.t(I18nKeys.Flows.RECIPIENT_N, mapOf("n" to "2")),
                        "Alice",
                        A_HAO_FULL,
                        "30",
                        s.t(I18nKeys.Flows.REMOVE_RECIPIENT),
                    ),
                    RecipientCardModel(
                        s.t(I18nKeys.Flows.RECIPIENT_N, mapOf("n" to "3")),
                        "hold on",
                        HOLD_ON_FULL,
                        "40",
                        s.t(I18nKeys.Flows.REMOVE_RECIPIENT),
                    ),
                ),
                recipientActions = listOf(
                    RecipientActionModel(RecipientAction.Add, s.t(I18nKeys.Flows.ADD_RECIPIENT)),
                    RecipientActionModel(
                        RecipientAction.Contacts,
                        s.t(I18nKeys.Flows.FROM_CONTACTS),
                    ),
                    RecipientActionModel(RecipientAction.Import, s.t(I18nKeys.Flows.BATCH_IMPORT)),
                ),
                summary = SummaryLineModel(
                    label = "${s.t(I18nKeys.Flows.SPLIT_TOTAL)} · " +
                        s.t(I18nKeys.Flows.RECIPIENT_COUNT, mapOf("count" to "3")),
                    value = "120 USDT · ≈$120.00",
                ),
                fee = fee,
                cta = s.t(I18nKeys.Flows.CONTINUE),
            )
        }

        return SendFormModel(
            header = header,
            mode = mode,
            token = token,
            amount = AmountFieldModel("120", "≈ $120.00", s.t(I18nKeys.Flows.FEE_TOKEN_LABEL)),
            recipient = RecipientFieldModel(
                label = s.t(I18nKeys.Flows.RECIPIENT_LABEL),
                lines = addressLines(ALICE_FULL),
                identiconSeed = ALICE_FULL,
                pickLabel = s.t(I18nKeys.Flows.RECIPIENT_PICK_ARIA),
            ),
            addRecipient = s.t(I18nKeys.Flows.ADD_RECIPIENT),
            fee = fee,
            cta = s.t(I18nKeys.Flows.CONTINUE),
        )
    }

    private fun contactPick(s: VelaStrings) = ContactPickModel(
        title = s.t(I18nKeys.Flows.PICK_CONTACT_TITLE),
        closeLabel = s.t(I18nKeys.Flows.CLOSE),
        searchPlaceholder = s.t(I18nKeys.Flows.PICK_CONTACT_SEARCH),
        scanRow = s.t(I18nKeys.Flows.SCAN_TO_FILL),
        groupsTitle = s.t(I18nKeys.Flows.CONTACTS_GROUPS),
        groups = listOf(
            ContactGroupModel(
                "家人",
                s.t(I18nKeys.Flows.GROUP_MEMBERS, mapOf("count" to "3")),
                WalletFixtures.ChainColors.polygon to WalletFixtures.ChainColors.bnb,
            ),
            ContactGroupModel(
                "工作",
                s.t(I18nKeys.Flows.GROUP_MEMBERS, mapOf("count" to "5")),
                WalletFixtures.ChainColors.gnosis to WalletFixtures.ChainColors.arbitrum,
            ),
        ),
        contactsTitle = s.t(I18nKeys.Flows.CONTACTS_TITLE),
        contacts = listOf(
            ContactEntryModel("Alice", "家人", ALICE_DISPLAY, ALICE_FULL),
            ContactEntryModel("阿豪", null, "0x77Bd…4F02", A_HAO_FULL),
            ContactEntryModel("hold on", null, HOLD_ON_DISPLAY, HOLD_ON_FULL),
        ),
    )

    private fun feeTokenPick(s: VelaStrings) = FeeTokenPickModel(
        title = s.t(I18nKeys.Flows.FEE_TOKEN_LABEL),
        closeLabel = s.t(I18nKeys.Flows.CLOSE),
        hint = s.t(I18nKeys.Flows.FEE_TOKEN_HINT),
        estimateLabel = s.t(I18nKeys.Flows.FEE_TOKEN_ESTIMATE),
        rows = listOf(
            FeeTokenRowModel(
                TokenMarkModel("ETH", WalletFixtures.ChainColors.ethereum),
                "ETH",
                s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "0.0689")),
                "~0.0021 ETH",
                selected = true,
            ),
            FeeTokenRowModel(
                TokenMarkModel("USDC", WalletFixtures.ChainColors.ethereum),
                "USDC",
                s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "18.20")),
                "~0.55 USDC",
                selected = false,
            ),
            FeeTokenRowModel(
                TokenMarkModel("USDT", WalletFixtures.ChainColors.ethereum),
                "USDT",
                s.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "53.4836")),
                "~0.55 USDT",
                selected = false,
            ),
        ),
    )

    private fun batchImport(s: VelaStrings) = BatchImportModel(
        title = s.t(I18nKeys.Flows.BATCH_TITLE),
        closeLabel = s.t(I18nKeys.Flows.CLOSE),
        unitFiat = s.t(I18nKeys.Flows.BATCH_UNIT_FIAT, mapOf("code" to "CNY")),
        unitToken = s.t(I18nKeys.Flows.BATCH_UNIT_TOKEN, mapOf("sym" to "USDT")),
        unit = BatchUnit.Fiat,
        pasteValue = "0xabc… , 5000\n0xdef… , 8000",
        pastePlaceholder = s.t(I18nKeys.Flows.BATCH_PASTE_PLACEHOLDER),
        importFile = "${s.t(I18nKeys.Flows.BATCH_IMPORT_FILE)} (xlsx / csv / txt)",
        template = s.t(I18nKeys.Flows.BATCH_TEMPLATE),
        rateSection = s.t(I18nKeys.Flows.BATCH_RATE_SECTION),
        rateLabel = s.t(I18nKeys.Flows.BATCH_RATE_LABEL, mapOf("sym" to "USDT")),
        rateValue = "7.25 CNY",
        rateHint = s.t(I18nKeys.Flows.BATCH_RATE_HINT, mapOf("code" to "CNY", "sym" to "USDT")),
        parsedLabel = s.t(I18nKeys.Flows.BATCH_PARSED_COUNT, mapOf("n" to "3")),
        rows = listOf(
            BatchRowModel(true, ALICE_DISPLAY, "5,000 CNY → 689.66"),
            BatchRowModel(true, "0x21aE…9F3c", "8,000 CNY → 1,103.45"),
            BatchRowModel(false, "0x12zz…${s.t(I18nKeys.Flows.BATCH_BAD_ADDRESS)}", "—"),
        ),
        rejectedText = s.t(I18nKeys.Flows.BATCH_REJECTED_ONE, mapOf("count" to "1")),
        // Two of three rows parsed, so the button offers two — never three.
        cta = s.t(I18nKeys.Flows.BATCH_APPLY_OTHER, mapOf("count" to "2")),
        ctaDisabled = false,
    )

    private fun sendConfirm(s: VelaStrings, mode: SendFormMode): SendConfirmModel {
        val facts = listOf(
            FactRowModel(
                label = s.t(I18nKeys.Flows.FROM_LABEL),
                value = WalletFixtures.NAME,
                lead = FactLead.Identicon(WalletFixtures.ADDRESS_FULL),
            ),
            FactRowModel(
                label = s.t(I18nKeys.Flows.TO_LABEL),
                value = if (mode == SendFormMode.Split) {
                    s.t(I18nKeys.Flows.RECIPIENT_COUNT, mapOf("count" to "3"))
                } else {
                    ALICE_DISPLAY
                },
                lead = if (mode == SendFormMode.Split) null else FactLead.Identicon(ALICE_FULL),
                mono = mode != SendFormMode.Split,
            ),
            FactRowModel(
                label = s.t(I18nKeys.Flows.DETAIL_CHAIN),
                value = NETWORKS[0].name,
                lead = FactLead.Token(TokenMarkModel(NETWORKS[0].code, NETWORKS[0].color)),
            ),
            FactRowModel(
                label = s.t(I18nKeys.Flows.EST_FEE),
                value = when (mode) {
                    SendFormMode.Single -> "~0.0021 ETH · ≈$0.55"
                    SendFormMode.Split -> "~0.0034 ETH · ≈$0.89"
                    SendFormMode.Sweep -> "~0.0041 ETH · ≈$1.07"
                },
            ),
        )
        val header = FlowHeaderModel(
            title = s.t(I18nKeys.Flows.CONFIRM_TITLE),
            backLabel = s.t(I18nKeys.Flows.BACK),
        )

        return when (mode) {
            SendFormMode.Sweep -> SendConfirmModel(
                header = header,
                amount = s.t(I18nKeys.Flows.ASSETS_COUNT, mapOf("n" to "3")),
                subline = s.t(
                    I18nKeys.Flows.CONFIRM_TOTAL_LINE,
                    mapOf("fiat" to "$200.90", "network" to NETWORKS[0].name),
                ),
                facts = facts,
                breakdown = listOf(
                    BreakdownRowModel(
                        lead = TokenMarkModel("USDT", WalletFixtures.ChainColors.ethereum),
                        label = "USDT",
                        value = "53.4836 USDT · ≈$53.48",
                    ),
                    BreakdownRowModel(
                        lead = TokenMarkModel("ETH", WalletFixtures.ChainColors.ethereum),
                        label = "ETH",
                        value = "0.05 ETH · ≈$93.79",
                    ),
                    BreakdownRowModel(
                        lead = TokenMarkModel("USDC", WalletFixtures.ChainColors.ethereum),
                        label = "USDC",
                        value = "18.20 USDC · ≈$18.20",
                    ),
                ),
                cta = s.t(I18nKeys.Flows.CONFIRM_SEND),
            )
            SendFormMode.Split -> SendConfirmModel(
                header = header,
                mark = TokenMarkModel("USDT", WalletFixtures.ChainColors.ethereum),
                amount = "120 USDT",
                subline = "≈ $120.00",
                facts = facts,
                breakdown = listOf(
                    BreakdownRowModel(
                        identiconSeed = ALICE_FULL,
                        label = ALICE_DISPLAY,
                        value = "50 USDT",
                    ),
                    BreakdownRowModel(
                        identiconSeed = A_HAO_FULL,
                        label = "Alice",
                        value = "30 USDT",
                    ),
                    BreakdownRowModel(
                        identiconSeed = HOLD_ON_FULL,
                        label = "hold on",
                        value = "40 USDT",
                    ),
                ),
                cta = s.t(I18nKeys.Flows.CONFIRM_SEND),
            )
            SendFormMode.Single -> SendConfirmModel(
                header = header,
                mark = TokenMarkModel("USDT", WalletFixtures.ChainColors.ethereum),
                amount = "120 USDT",
                subline = "≈ $120.00",
                facts = facts,
                cta = s.t(I18nKeys.Flows.CONFIRM_SEND),
            )
        }
    }

    private fun sendReceipt(s: VelaStrings, stage: ReceiptStage): SendReceiptModel {
        val header = FlowHeaderModel(
            title = s.t(I18nKeys.Flows.SEND_TITLE, mapOf("symbol" to "USDT")),
            backLabel = s.t(I18nKeys.Flows.BACK),
        )
        return when (stage) {
            ReceiptStage.Submitting -> SendReceiptModel(
                header = header,
                stage = stage,
                title = s.t(I18nKeys.Flows.TX_SUBMITTING),
                captions = listOf(
                    s.t(I18nKeys.Flows.TX_PREPARING_BIOMETRIC),
                    s.t(I18nKeys.Flows.TX_BACKGROUND_HINT),
                ),
                cta = s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND),
                ctaAccent = false,
            )
            ReceiptStage.Submitted, ReceiptStage.Failed -> SendReceiptModel(
                header = header,
                stage = ReceiptStage.Submitted,
                title = s.t(I18nKeys.Flows.TX_SUBMITTED_TITLE),
                captions = listOf(
                    s.t(I18nKeys.Flows.TX_WAITING_CONFIRM),
                    s.t(
                        I18nKeys.Flows.TX_TYPICAL_TIME,
                        mapOf("chainName" to NETWORKS[0].name, "estSecs" to "12"),
                    ),
                ),
                cta = s.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND),
                ctaAccent = false,
            )
            ReceiptStage.Confirmed -> SendReceiptModel(
                header = header,
                stage = stage,
                title = s.t(
                    I18nKeys.Flows.TX_CONFIRMED_TITLE,
                    mapOf("amount" to "120", "symbol" to "USDT"),
                ),
                captions = listOf(
                    "${s.t(I18nKeys.Flows.TO_NAME, mapOf("name" to ALICE_DISPLAY))} · " +
                        NETWORKS[0].name,
                ),
                hash = ReceiptHashModel(
                    label = s.t(I18nKeys.Flows.TX_HASH),
                    value = TX_HASH_RECEIVED,
                    copyLabel = s.t(I18nKeys.Flows.COPY_ADDRESS),
                ),
                viewOnExplorer = s.t(I18nKeys.Flows.VIEW_ON_EXPLORER),
                cta = s.t(I18nKeys.Flows.DONE),
                ctaAccent = true,
            )
        }
    }

    // --- Builder -------------------------------------------------------------

    /** Build one state (spec.md's state matrix). */
    fun build(state: FlowState, s: VelaStrings): FlowScreenModel {
        val scale = if (state == FlowState.R2X) 1.35f else 1f
        fun screen(base: FlowBase, sheet: FlowSheet? = null) =
            FlowScreenModel(state, base, sheet, scale)

        return when (state) {
            FlowState.R1 -> screen(FlowBase.Receive(receiveList(s)))
            FlowState.R2, FlowState.R2X -> screen(
                FlowBase.Receive(receiveList(s)),
                FlowSheet.ReceiveQr(receiveQr(s, asset = false)),
            )
            FlowState.R3 -> screen(
                FlowBase.Receive(receiveList(s)),
                FlowSheet.ReceiveQr(receiveQr(s, asset = true)),
            )
            FlowState.R4 -> screen(FlowBase.Share(shareCard(s)))
            FlowState.S1 -> screen(FlowBase.Scan(scan(s)))
            FlowState.A1 -> screen(FlowBase.History(history(s)))
            FlowState.A2 -> screen(
                FlowBase.History(history(s)),
                FlowSheet.TxDetail(txDetail(s, received = true)),
            )
            FlowState.A3 -> screen(
                FlowBase.History(history(s)),
                FlowSheet.TxDetail(txDetail(s, received = false)),
            )
            FlowState.T1 -> screen(FlowBase.Assets(assets(s, empty = false)))
            FlowState.T2 -> screen(
                FlowBase.Assets(assets(s, empty = false)),
                FlowSheet.TokenDetail(tokenDetail(s)),
            )
            FlowState.T3 -> screen(
                FlowBase.Assets(assets(s, empty = false)),
                FlowSheet.AddToken(addToken(s, AddVariant.Erc20)),
            )
            FlowState.T3B -> screen(
                FlowBase.Assets(assets(s, empty = false)),
                FlowSheet.AddToken(addToken(s, AddVariant.Native)),
            )
            FlowState.T4 -> screen(FlowBase.Assets(assets(s, empty = true)))
            FlowState.T5 -> screen(
                FlowBase.Assets(assets(s, empty = false)),
                FlowSheet.AddToken(addToken(s, AddVariant.Erc20Invalid)),
            )
            FlowState.T5B -> screen(
                FlowBase.Assets(assets(s, empty = false)),
                FlowSheet.AddToken(addToken(s, AddVariant.NativeIncompatible)),
            )
            FlowState.SD1 -> screen(FlowBase.SendPick(sendPick(s, multi = false)))
            FlowState.SD1B -> screen(FlowBase.SendPick(sendPick(s, multi = true)))
            FlowState.SD2 -> screen(FlowBase.SendForm(sendForm(s, SendFormMode.Single)))
            FlowState.SD2B -> screen(FlowBase.SendForm(sendForm(s, SendFormMode.Split)))
            FlowState.SD2D -> screen(FlowBase.SendForm(sendForm(s, SendFormMode.Sweep)))
            FlowState.SD2C -> screen(
                FlowBase.SendForm(sendForm(s, SendFormMode.Split)),
                FlowSheet.BatchImport(batchImport(s)),
            )
            FlowState.SD2E -> screen(
                FlowBase.SendForm(sendForm(s, SendFormMode.Single)),
                FlowSheet.ContactPick(contactPick(s)),
            )
            FlowState.SD2F -> screen(
                FlowBase.SendForm(sendForm(s, SendFormMode.Single)),
                FlowSheet.FeeToken(feeTokenPick(s)),
            )
            FlowState.SD3 -> screen(FlowBase.SendConfirm(sendConfirm(s, SendFormMode.Single)))
            FlowState.SD3B -> screen(FlowBase.SendConfirm(sendConfirm(s, SendFormMode.Split)))
            FlowState.SD3C -> screen(FlowBase.SendConfirm(sendConfirm(s, SendFormMode.Sweep)))
            FlowState.SD4A -> screen(
                FlowBase.SendReceipt(sendReceipt(s, ReceiptStage.Submitting)),
            )
            FlowState.SD4B -> screen(FlowBase.SendReceipt(sendReceipt(s, ReceiptStage.Submitted)))
            FlowState.SD4C -> screen(FlowBase.SendReceipt(sendReceipt(s, ReceiptStage.Confirmed)))
        }
    }
}
