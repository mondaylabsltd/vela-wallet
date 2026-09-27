package app.getvela.wallet.feature.signing

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.wallet.WalletFixtures

/**
 * Canonical signing fixtures (spec 022, data-model.md §3 — the single canon all
 * four platforms port; web reference: `src/lib/signing/fixtures.ts`).
 *
 * Amounts, addresses and contract names are verbatim mock content; every label
 * resolves through the corpus. The catalogue doubles as the degradation
 * ladder's regression suite: CS23–CS24 and CS30–CS32 are the rungs below
 * "verified descriptor", and they are here so any change to the renderer has to
 * face what a wallet shows when it does NOT know what a transaction does.
 */
object SigningFixtures {

    private val networkDot = ExploreFixtures.networkDot
    private const val NETWORK = "Ethereum"
    private const val FEE_VALUE = "~0.0021 ETH ≈ $5.40"

    /** Token marks: brand content, exactly like the wallet's chain colours. */
    private object Mark {
        val usdc = TokenMark("U", Color(0xFF2775CA))
        val eth = TokenMark("E", Color(0xFF627EEA))
        val weth = TokenMark("W", Color(0xFF8A92B2))
        val spweth = TokenMark("S", Color(0xFF4C6FFF))
        val usdt = TokenMark("T", Color(0xFF26A17B))
        val contact = TokenMark("A", Color(0xFFE8572A))
    }

    private object Dapp {
        val uniswap = listOf("Uniswap", "app.uniswap.org", "U")
        val oneinch = listOf("1inch", "app.1inch.io", "1")
        val opensea = listOf("OpenSea", "opensea.io", "O")
        val morpho = listOf("Morpho", "app.morpho.org", "M")
        val safe = listOf("Safe", "app.safe.global", "S")
        val ens = listOf("ENS", "app.ens.domains", "E")
        val phish = listOf("opensae-mint", "opensae-mint.xyz", "O")
        val uniswapTint = ExploreFixtures.Brand.uniswap
        val oneinchTint = Color(0xFFC2352D)
        val openseaTint = ExploreFixtures.Brand.opensea
        val morphoTint = Color(0xFF2E5BFF)
        val safeTint = Color(0xFF12FF80)
        val ensTint = ExploreFixtures.Brand.ens
        val unknownTint = Color(0xFF6E6B62)
    }

    private object Addr {
        const val ALICE = "0xaF5e…b3e1"
        const val ALICE_FULL = "0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1"
        const val VITALIK = "0xd8dA…6045"
        const val ONEINCH_ROUTER = "0x1111…0582"
        const val UNIVERSAL_ROUTER = "0x3fC9…7FAD"
        const val UNISWAP_V3 = "0x68b3…4dC5"
        const val BAYC = "0xBC4C…f13D"
        const val CONDUIT = "0x1E00…3c71"
        const val MORPHO_VAULT = "0x38989B…21eB"
        const val UNKNOWN = "0x4e1dC6…A9C1"
        const val REWARDS = "0x067d3D…2ed1"
        const val USDT = "0xdAC1…1ec7"
        const val SAFE = "0x4167…461a"
        const val DEPLOYED = "0x1A2b…9304"
        const val DEEPEST = "0x004C22…6819"
        const val USDC_FULL = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    }

    // --- Small builders -------------------------------------------------------

    private fun VelaStrings.sg(key: String, vars: Map<String, String> = emptyMap()) =
        t("componentsUi.signing.$key", vars)

    private fun VelaStrings.ap(key: String, vars: Map<String, String> = emptyMap()) =
        t("componentsUi.signingApprove.$key", vars)

    private fun chip(id: String, label: String, state: AllowanceChip.ChipState) =
        AllowanceChip(id, label, state)

    private fun VelaStrings.onchainFee() =
        FeeModel.OnChain(t("componentsUi.gas.networkFee"), FEE_VALUE)

    private fun VelaStrings.tech(
        summary: String? = null,
        functionLabel: String? = null,
        signature: String? = null,
        params: List<SigningRow> = emptyList(),
        identities: List<TechIdentity> = emptyList(),
        simResult: SigningRow? = null,
        rawLabel: String? = null,
        rawHex: String? = null,
    ) = TechModel(
        title = sg("advancedToggle"),
        summary = summary,
        functionLabel = functionLabel,
        signature = signature,
        params = params,
        identities = identities,
        simResult = simResult,
        rawLabel = rawLabel,
        rawHex = rawHex,
        copyLabel = sg("copyValue"),
        explorerLabel = sg("viewOnExplorer"),
    )

    /** CS1/CS29's five-layer panel — the one the founder asked to see in full. */
    private fun VelaStrings.transferTech() = tech(
        functionLabel = sg("techFunction"),
        signature = "transfer(address to, uint256 value)",
        params = listOf(
            SigningRow("to", Addr.ALICE, mono = true),
            SigningRow(
                "value",
                sg("techRawUnits", mapOf("value" to "1000000000", "n" to "6")),
                mono = true,
            ),
        ),
        identities = listOf(
            TechIdentity(sg("techIdentityToken"), "USD Coin", Addr.USDC_FULL, Mark.usdc),
            TechIdentity(sg("techIdentityRecipient"), "Alice Chen", Addr.ALICE_FULL, Mark.contact),
        ),
        simResult = SigningRow(sg("simResultLabel"), "−1,000 USDC · ${sg("balanceMatchesHero")}"),
        rawLabel = "${sg("techRawData")} · ${sg("byteSize", mapOf("n" to "68"))}",
        rawHex = "0xa9059cbb000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1" +
            "00000000000000000000000000000000000000000000000000000003b9aca00",
    )

    private fun VelaStrings.model(
        state: SigningScreenState,
        dapp: List<String>,
        tint: Color,
        blocks: List<SigningBlock>,
        confirmAction: String,
        tech: TechModel = tech(),
        techOpen: Boolean = false,
        fee: FeeModel = onchainFee(),
        confirmEnabled: Boolean = true,
    ) = SigningScreenModel(
        state = state,
        dappName = dapp[0],
        dappHost = dapp[1],
        dappLetter = dapp[2],
        dappTint = tint,
        networkName = NETWORK,
        networkDot = networkDot,
        blocks = blocks,
        tech = tech,
        techOpen = techOpen,
        fee = fee,
        signerLabel = sg("signingAccount"),
        signerName = WalletFixtures.NAME,
        signerSeed = WalletFixtures.ADDRESS_FULL,
        confirmHint = sg("slideToConfirm"),
        confirmAction = confirmAction,
        confirmEnabled = confirmEnabled,
        panelTitle = sg("signatureRequest"),
    )

    // --- The catalogue --------------------------------------------------------

    @Suppress("CyclomaticComplexMethod", "LongMethod")
    fun build(state: SigningScreenState, s: VelaStrings): SigningScreenModel = with(s) {
        val unknownDapp = listOf(sg("unverifiedTag"), "dapp.example.com", "D")
        val verified = PartyBadge(sg("verifiedTag"), SigningTone.Success)
        val unverified = PartyBadge(sg("unverifiedTag"), SigningTone.Caution)

        when (state) {
            SigningScreenState.CS1, SigningScreenState.CS29 -> model(
                state, Dapp.uniswap, Dapp.uniswapTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentSend"), SigningTone.Neutral),
                    SigningBlock.Amount(
                        AmountLine("", "1,000", "USDC", Mark.usdc, "≈ $1,000.00"),
                    ),
                    SigningBlock.Sentence(
                        sg("summarySend", mapOf("amount" to "1,000 USDC", "to" to "Alice Chen")),
                        SigningTone.Accent,
                    ),
                    SigningBlock.Party(
                        sg("recipientLabel"), "Alice Chen", Addr.ALICE,
                        PartyBadge(sg("contactTag"), SigningTone.Neutral),
                    ),
                ),
                confirmAction = sg("confirmSend"),
                tech = transferTech(),
                techOpen = state == SigningScreenState.CS29,
            )

            SigningScreenState.CS2 -> model(
                state, Dapp.uniswap, Dapp.uniswapTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentSend"), SigningTone.Neutral),
                    SigningBlock.Amount(AmountLine("", "10", "ETH", Mark.eth, "≈ $25,604.00")),
                    SigningBlock.Sentence(
                        sg("summarySend", mapOf("amount" to "10 ETH", "to" to Addr.VITALIK)),
                        SigningTone.Accent,
                    ),
                    SigningBlock.Party(
                        sg("recipientLabel"), "vitalik.eth", Addr.VITALIK,
                        PartyBadge(sg("firstTimeTag"), SigningTone.Caution),
                    ),
                ),
                confirmAction = sg("confirmSend"),
            )

            SigningScreenState.CS3 -> model(
                state, Dapp.safe, Dapp.safeTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentSend"), SigningTone.Neutral),
                    SigningBlock.Amount(AmountLine("", "0.5", "ETH", Mark.eth, "≈ $1,280.20")),
                    SigningBlock.Positive(sg("balanceSelfTransfer")),
                    SigningBlock.Party(
                        sg("recipientLabel"),
                        sg("selfName", mapOf("name" to WalletFixtures.NAME)),
                        WalletFixtures.ADDRESS_DISPLAY,
                        PartyBadge(sg("walletTag"), SigningTone.Success),
                    ),
                ),
                confirmAction = sg("confirmSend"),
            )

            SigningScreenState.CS4 -> model(
                state, Dapp.uniswap, Dapp.uniswapTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentSend"), SigningTone.Neutral),
                    SigningBlock.Amount(AmountLine("", "100", "USDC", Mark.usdc, "≈ $100.00")),
                    SigningBlock.Sentence(
                        sg("summarySendFrom", mapOf("amount" to "100 USDC", "to" to Addr.VITALIK)),
                        SigningTone.Accent,
                    ),
                    SigningBlock.Rows(listOf(SigningRow(sg("labelFrom"), Addr.ALICE, mono = true))),
                    SigningBlock.Party(sg("recipientLabel"), "vitalik.eth", Addr.VITALIK),
                ),
                confirmAction = sg("confirmSend"),
            )

            SigningScreenState.CS5 -> model(
                state, Dapp.oneinch, Dapp.oneinchTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentApprove"), SigningTone.Danger),
                    SigningBlock.Allowance(
                        ap("spendingCap"), ap("unlimitedValue"), SigningTone.Danger,
                        // The site's own ask, preselected (2026-09-26): Permit2
                        // bundles revert when the wallet re-encodes the
                        // approve. A cap is one chip away.
                        listOf(
                            chip("requested", ap("requested"), AllowanceChip.ChipState.Selected),
                            chip("balance", ap("balanceCap"), AllowanceChip.ChipState.Idle),
                            chip("custom", ap("custom"), AllowanceChip.ChipState.Idle),
                            chip("revoke", ap("revoke"), AllowanceChip.ChipState.Idle),
                        ),
                    ),
                    SigningBlock.Party(
                        sg("spenderLabel"), "1inch Router", Addr.ONEINCH_ROUTER, verified,
                    ),
                    SigningBlock.Warning(SigningTone.Danger, sg("unlimitedWarning")),
                ),
                confirmAction = sg("intentApprove"),
                // Unlimited, seen and said — signable as asked.
                confirmEnabled = true,
            )

            SigningScreenState.CS6 -> model(
                state, Dapp.oneinch, Dapp.oneinchTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentApprove"), SigningTone.Neutral),
                    SigningBlock.Allowance(
                        ap("spendingCap"), "1,240 USDC", SigningTone.Neutral,
                        listOf(
                            chip("requested", ap("requested"), AllowanceChip.ChipState.Idle),
                            chip("balance", ap("balanceCap"), AllowanceChip.ChipState.Selected),
                            chip("custom", ap("custom"), AllowanceChip.ChipState.Idle),
                            chip("revoke", ap("revoke"), AllowanceChip.ChipState.Idle),
                        ),
                    ),
                    SigningBlock.Sentence(
                        ap(
                            "capSummary",
                            mapOf("spender" to "1inch Router", "amount" to "1,240 USDC"),
                        ),
                        SigningTone.Neutral,
                    ),
                    SigningBlock.Party(
                        sg("spenderLabel"), "1inch Router", Addr.ONEINCH_ROUTER, verified,
                    ),
                ),
                confirmAction = sg("intentApprove"),
            )

            SigningScreenState.CS7 -> model(
                state, Dapp.uniswap, Dapp.uniswapTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentApprove"), SigningTone.Neutral),
                    SigningBlock.Allowance(
                        ap("spendingCap"), "+100 USDC", SigningTone.Neutral,
                        listOf(
                            chip("requested", ap("requested"), AllowanceChip.ChipState.Selected),
                            chip("balance", ap("balanceCap"), AllowanceChip.ChipState.Idle),
                            chip("custom", ap("custom"), AllowanceChip.ChipState.Idle),
                            chip("revoke", ap("revoke"), AllowanceChip.ChipState.Idle),
                        ),
                        // increaseAllowance is an INCREMENT: the number that
                        // matters is the one it lands on, so the sheet adds up.
                        resultingTotal = SigningRow(ap("resultingTotal"), "350 USDC"),
                    ),
                    SigningBlock.Party(
                        sg("spenderLabel"), "Uniswap Router", Addr.UNIVERSAL_ROUTER, verified,
                    ),
                ),
                confirmAction = sg("intentApprove"),
            )

            SigningScreenState.CS8 -> model(
                state, Dapp.oneinch, Dapp.oneinchTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentRevoke"), SigningTone.Neutral),
                    SigningBlock.Allowance(
                        ap("spendingCap"), ap("revokeValue"), SigningTone.Neutral,
                        listOf(
                            chip("requested", ap("requested"), AllowanceChip.ChipState.Idle),
                            chip("balance", ap("balanceCap"), AllowanceChip.ChipState.Idle),
                            chip("custom", ap("custom"), AllowanceChip.ChipState.Idle),
                            chip("revoke", ap("revoke"), AllowanceChip.ChipState.Selected),
                        ),
                    ),
                    SigningBlock.Sentence(
                        ap("revokeSummary", mapOf("spender" to "1inch Router")),
                        SigningTone.Neutral,
                    ),
                    SigningBlock.Party(
                        sg("spenderLabel"), "1inch Router", Addr.ONEINCH_ROUTER, verified,
                    ),
                ),
                confirmAction = sg("intentRevoke"),
            )

            SigningScreenState.CS9 -> model(
                state, Dapp.opensea, Dapp.openseaTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentTransferNft"), SigningTone.Neutral),
                    SigningBlock.Nft("#6529", "Bored Ape Yacht Club"),
                    SigningBlock.Sentence(
                        sg("summaryTransferNft", mapOf("id" to "#6529", "to" to "Alice Chen")),
                        SigningTone.Accent,
                    ),
                    SigningBlock.Party(
                        sg("recipientLabel"), "Alice Chen", Addr.ALICE,
                        PartyBadge(sg("contactTag"), SigningTone.Neutral),
                    ),
                ),
                confirmAction = sg("confirmLabel"),
            )

            SigningScreenState.CS10 -> model(
                state, Dapp.opensea, Dapp.openseaTint,
                blocks = listOf(
                    SigningBlock.Intent(ap("verbApproveAll"), SigningTone.Danger),
                    SigningBlock.Allowance(
                        ap("spendingCap"), ap("allNfts"), SigningTone.Danger,
                        // setApprovalForAll has no finite form to offer — two
                        // chips are the only honest choices.
                        listOf(
                            chip("revoke", ap("revokeAccess"), AllowanceChip.ChipState.Idle),
                            chip("grant", ap("grantAllAnyway"), AllowanceChip.ChipState.Selected),
                        ),
                    ),
                    SigningBlock.Sentence(
                        sg("summaryApproveNft", mapOf("operator" to "OpenSea Conduit")),
                        SigningTone.Accent,
                    ),
                    SigningBlock.Party(
                        ap("collectionLabel"), "Bored Ape Yacht Club", Addr.BAYC, verified,
                    ),
                    SigningBlock.Party(ap("operatorLabel"), "OpenSea Conduit", Addr.CONDUIT),
                    SigningBlock.Warning(SigningTone.Caution, ap("setApprovalAllWarn")),
                ),
                confirmAction = ap("verbApproveAll"),
            )

            SigningScreenState.CS11, SigningScreenState.CS33 -> model(
                state, Dapp.oneinch, Dapp.oneinchTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentSwap"), SigningTone.Neutral),
                    SigningBlock.Swap(
                        AmountLine("−", "1,000", "USDC", Mark.usdc, "≈ $1,000.00", sg("labelPay")),
                        AmountLine(
                            "+", "0.3042", "WETH", Mark.weth, "≈ $778.90",
                            sg("labelMinReceived"), SigningTone.Success,
                        ),
                    ),
                    SigningBlock.Sentence(
                        sg(
                            "summarySwap",
                            mapOf("pay" to "1,000 USDC", "receive" to "0.3042 WETH"),
                        ),
                        SigningTone.Accent,
                    ),
                    SigningBlock.Party(
                        sg("interactingLabel"), "1inch Aggregation Router · 1inch Network",
                        Addr.ONEINCH_ROUTER, verified,
                    ),
                ),
                confirmAction = sg("confirmSwap"),
                fee = if (state == SigningScreenState.CS33) {
                    FeeModel.OnChain(
                        t("componentsUi.gas.networkFee"), FEE_VALUE,
                        selectorTitle = sg("feeTokenTitle"),
                        options = listOf(
                            FeeTokenOption(
                                "eth", Mark.eth, "ETH",
                                "${t("componentsUi.gas.rowBalance")} 0.0689",
                                "~0.0021 ETH", selected = true,
                            ),
                            FeeTokenOption(
                                "usdc", Mark.usdc, "USDC",
                                "${t("componentsUi.gas.rowBalance")} 1,240.00",
                                "~5.55 USDC", selected = false,
                            ),
                        ),
                    )
                } else {
                    onchainFee()
                },
            )

            SigningScreenState.CS12 -> model(
                state, Dapp.uniswap, Dapp.uniswapTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentSwap"), SigningTone.Neutral),
                    SigningBlock.Swap(
                        AmountLine("−", "0.5", "ETH", Mark.eth, "≈ $1,280.20", sg("labelPay")),
                        AmountLine(
                            "+", "1,278.11", "USDC", Mark.usdc, "≈ $1,278.11",
                            sg("labelMinReceived"), SigningTone.Success,
                        ),
                    ),
                    SigningBlock.Sentence(
                        sg("summarySwap", mapOf("pay" to "0.5 ETH", "receive" to "1,278.11 USDC")),
                        SigningTone.Accent,
                    ),
                    SigningBlock.Party(
                        sg("interactingLabel"), "Uniswap V3 Router", Addr.UNISWAP_V3, verified,
                    ),
                ),
                confirmAction = sg("confirmSwap"),
            )

            SigningScreenState.CS13 -> model(
                state, Dapp.uniswap, Dapp.uniswapTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentSwap"), SigningTone.Neutral),
                    SigningBlock.Swap(
                        AmountLine("−", "1,000", "USDC", Mark.usdc, caption = sg("labelPay")),
                        AmountLine(
                            "+", "0.3042", "WETH", Mark.weth, caption = sg("labelMinReceived"),
                            tone = SigningTone.Success,
                        ),
                    ),
                    SigningBlock.Rows(
                        listOf(
                            SigningRow(
                                sg("labelDeadline"),
                                sg("expiredValue", mapOf("time" to "2026-08-14 18:00")),
                                SigningTone.Caution,
                            ),
                        ),
                    ),
                    SigningBlock.Warning(SigningTone.Caution, sg("expiredWarning")),
                    SigningBlock.Warning(SigningTone.Danger, sg("simWillFail")),
                ),
                confirmAction = sg("confirmSwap"),
            )

            SigningScreenState.CS14 -> model(
                state, Dapp.morpho, Dapp.morphoTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentDeposit"), SigningTone.Neutral),
                    SigningBlock.Swap(
                        AmountLine(
                            "−", "2", "WETH", Mark.weth, "≈ $5,120.80", sg("depositAsset"),
                        ),
                        AmountLine(
                            "+", "1.9631", "spWETH", Mark.spweth,
                            caption = sg("sharesReceived"), tone = SigningTone.Success,
                        ),
                    ),
                    SigningBlock.Warning(SigningTone.Caution, sg("unverifiedWarning")),
                    SigningBlock.Party(
                        sg("interactingLabel"), "Morpho Vault · Morpho Labs",
                        Addr.MORPHO_VAULT, verified,
                    ),
                ),
                confirmAction = sg("confirmDeposit"),
            )

            SigningScreenState.CS15 -> model(
                state, Dapp.morpho, Dapp.morphoTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentWithdraw"), SigningTone.Neutral),
                    SigningBlock.Amount(
                        AmountLine(
                            "+", "2", "WETH", Mark.weth, "≈ $5,120.80",
                            tone = SigningTone.Success,
                        ),
                    ),
                    SigningBlock.Sentence(
                        sg("summaryReceive", mapOf("amount" to "2 WETH")), SigningTone.Accent,
                    ),
                    SigningBlock.Party(
                        sg("interactingLabel"), "Morpho Vault · Morpho Labs",
                        Addr.MORPHO_VAULT, verified,
                    ),
                ),
                confirmAction = sg("confirmWithdraw"),
            )

            SigningScreenState.CS16 -> model(
                state, Dapp.uniswap, Dapp.uniswapTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("permitIntent"), SigningTone.Danger),
                    SigningBlock.Sentence(
                        sg(
                            "summaryPermitUnlimited",
                            mapOf("spender" to "Universal Router", "token" to "USDC"),
                        ),
                        SigningTone.Danger,
                    ),
                    SigningBlock.Party(
                        sg("spenderLabel"), "Universal Router", Addr.UNIVERSAL_ROUTER, verified,
                    ),
                    SigningBlock.Rows(
                        listOf(
                            SigningRow(
                                ap("spendingCap"), "${ap("unlimitedValue")} USDC",
                                SigningTone.Danger,
                            ),
                            SigningRow(ap("expiresLabel"), "2026-09-14 19:30"),
                        ),
                    ),
                    // The whole reason this is danger and not caution: there is
                    // no editor to offer, because a signature cannot be capped.
                    SigningBlock.Warning(SigningTone.Danger, ap("permitCantCap")),
                ),
                confirmAction = sg("signLabel"),
                fee = FeeModel.OffChain(sg("noNetworkFee")),
            )

            SigningScreenState.CS17 -> model(
                state, Dapp.uniswap, Dapp.uniswapTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("permitIntent"), SigningTone.Neutral),
                    SigningBlock.Sentence(
                        sg(
                            "summaryPermit",
                            mapOf("spender" to "Universal Router", "amount" to "1,000 USDC"),
                        ),
                        SigningTone.Accent,
                    ),
                    SigningBlock.Party(
                        sg("spenderLabel"), "Universal Router", Addr.UNIVERSAL_ROUTER, verified,
                    ),
                    SigningBlock.Rows(
                        listOf(
                            SigningRow(ap("spendingCap"), "1,000 USDC"),
                            SigningRow(sg("labelDeadline"), "2030-03-14 08:26"),
                        ),
                    ),
                ),
                confirmAction = sg("signLabel"),
                fee = FeeModel.OffChain(sg("noNetworkFee")),
            )

            SigningScreenState.CS18 -> model(
                state, unknownDapp, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("typedDataIntent"), SigningTone.Neutral),
                    SigningBlock.Warning(SigningTone.Caution, sg("blindTypedWarning")),
                    SigningBlock.Rows(
                        listOf(
                            SigningRow(sg("typedDomain"), "CoolProtocol · v2"),
                            SigningRow(sg("typeLabel"), "Order"),
                            SigningRow(sg("signingFor"), "dapp.example.com", SigningTone.Accent),
                        ),
                    ),
                    SigningBlock.Code(
                        listOf(
                            "{ \"maker\": \"0x14fB1f…D1eA5c\",",
                            "  \"taker\": \"0x0000…0000\",",
                            "  \"makerAmount\": \"1000000000\", … }",
                        ),
                    ),
                ),
                confirmAction = sg("signLabel"),
                tech = tech(summary = sg("byteSize", mapOf("n" to "412"))),
                fee = FeeModel.OffChain(sg("noNetworkFee")),
            )

            SigningScreenState.CS19 -> model(
                state, Dapp.ens, Dapp.ensTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("signInIntent"), SigningTone.Neutral),
                    SigningBlock.Rows(
                        listOf(
                            SigningRow(sg("siweDomain"), "app.ens.domains"),
                            SigningRow(sg("siweStatement"), "登录以管理你的 ENS 名称"),
                        ),
                    ),
                    SigningBlock.Code(
                        listOf(
                            "app.ens.domains wants you to sign in",
                            "with your Ethereum account:",
                            WalletFixtures.ADDRESS_DISPLAY,
                        ),
                    ),
                    SigningBlock.Positive(sg("siweOk", mapOf("domain" to "app.ens.domains"))),
                ),
                confirmAction = sg("signLabel"),
                fee = FeeModel.OffChain(sg("noNetworkFee")),
            )

            SigningScreenState.CS20 -> model(
                state, Dapp.phish, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("signInIntent"), SigningTone.Danger),
                    // The mismatch goes ABOVE the facts: by the time somebody
                    // has read a login screen they have already decided.
                    SigningBlock.Warning(
                        SigningTone.Danger,
                        sg(
                            "siweMismatch",
                            mapOf("domain" to "opensea.io", "origin" to "opensae-mint.xyz"),
                        ),
                    ),
                    SigningBlock.Rows(
                        listOf(
                            SigningRow(sg("siweDomain"), "opensea.io", SigningTone.Danger),
                            SigningRow(sg("siweOrigin"), "opensae-mint.xyz", mono = true),
                            SigningRow(sg("siweStatement"), "登录以查看你的 NFT"),
                        ),
                    ),
                    SigningBlock.Code(
                        listOf(
                            "opensea.io wants you to sign in",
                            "with your Ethereum account:",
                            WalletFixtures.ADDRESS_DISPLAY,
                        ),
                    ),
                ),
                confirmAction = sg("signLabel"),
                fee = FeeModel.Hidden,
            )

            SigningScreenState.CS21 -> model(
                state, unknownDapp, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("messageIntent"), SigningTone.Neutral),
                    SigningBlock.Warning(SigningTone.Caution, sg("hexMessageWarning")),
                    SigningBlock.Code(
                        listOf(
                            "0xdeadbeefcafebabe0102030405",
                            "060708091011121314151617181920",
                            "2122232425262728293031…",
                        ),
                        note = "(${sg("byteSize", mapOf("n" to "80"))})",
                    ),
                    SigningBlock.Rows(
                        listOf(SigningRow(sg("signingFor"), "dapp.example.com")),
                    ),
                ),
                confirmAction = sg("signLabel"),
                fee = FeeModel.Hidden,
            )

            SigningScreenState.CS22 -> model(
                state, unknownDapp, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("ethSignIntent"), SigningTone.Danger),
                    SigningBlock.Sentence(sg("ethSignBody"), SigningTone.Danger),
                    SigningBlock.Code(
                        listOf(
                            "0x9c22ff5f21f0b81b113e63f7db6da9",
                            "4fedef11b2119b4088b89664fb9a3c",
                            "b658",
                        ),
                    ),
                    SigningBlock.Warning(SigningTone.Danger, sg("ethSignWarning")),
                ),
                confirmAction = sg("confirmLabel"),
                fee = FeeModel.Hidden,
            )

            SigningScreenState.CS23 -> model(
                state, unknownDapp, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentContractCall"), SigningTone.Neutral),
                    SigningBlock.Warning(
                        SigningTone.Caution, sg("blindDecodeWarning", mapOf("bytes" to "196")),
                    ),
                    SigningBlock.Rows(
                        listOf(SigningRow(sg("labelAmount"), "0.1 ETH ≈ $256.04")),
                    ),
                    SigningBlock.Party(
                        sg("interactingLabel"), sg("unverifiedTag"), Addr.UNKNOWN, unverified,
                    ),
                    SigningBlock.Balances(
                        sg("balanceChangesTitle"),
                        listOf(BalanceDeltaRow("ETH", "−0.1", SigningTone.Neutral)),
                        sg("blindButSimulated"),
                    ),
                ),
                confirmAction = sg("confirmLabel"),
            )

            SigningScreenState.CS24 -> model(
                state, unknownDapp, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentContractCall"), SigningTone.Danger),
                    SigningBlock.Sentence(sg("drainSummary"), SigningTone.Danger),
                    SigningBlock.Balances(
                        sg("balanceChangesTitle"),
                        listOf(
                            BalanceDeltaRow("USDC", "−8,450", SigningTone.Danger),
                            BalanceDeltaRow("ETH", "−0.8", SigningTone.Danger),
                        ),
                        sg("drainWarning"),
                        SigningTone.Danger,
                    ),
                    SigningBlock.Party(
                        sg("interactingLabel"), sg("unverifiedTag"), Addr.UNKNOWN, unverified,
                    ),
                    SigningBlock.Warning(
                        SigningTone.Danger, sg("blindDecodeWarning", mapOf("bytes" to "4")),
                    ),
                ),
                confirmAction = sg("confirmLabel"),
            )

            SigningScreenState.CS25 -> model(
                state, Dapp.safe, Dapp.safeTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("deployIntent"), SigningTone.Neutral),
                    SigningBlock.Sentence(sg("summaryDeploy"), SigningTone.Accent),
                    SigningBlock.Rows(
                        listOf(
                            SigningRow(
                                sg("deployBytecode"), sg("byteSize", mapOf("n" to "246")),
                            ),
                            SigningRow(sg("deployPredictedAddress"), Addr.DEPLOYED, mono = true),
                        ),
                    ),
                ),
                confirmAction = sg("confirmLabel"),
            )

            SigningScreenState.CS26 -> model(
                state, Dapp.oneinch, Dapp.oneinchTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("batchIntent"), SigningTone.Neutral),
                    SigningBlock.Sentence(
                        sg("batchSubtitle", mapOf("count" to "2")), SigningTone.Accent,
                    ),
                    SigningBlock.Card(
                        sg("batchStep", mapOf("index" to "1", "action" to sg("intentApprove"))),
                        listOf(
                            SigningRow(ap("spendingCap"), "100 USDC"),
                            SigningRow(sg("spenderLabel"), "1inch Router"),
                        ),
                        SigningTone.Neutral,
                    ),
                    SigningBlock.Card(
                        sg("batchStep", mapOf("index" to "2", "action" to sg("intentSwap"))),
                        listOf(
                            SigningRow(sg("labelPay"), "−100 USDC"),
                            SigningRow(sg("labelMinReceived"), "+0.0304 WETH"),
                        ),
                        SigningTone.Neutral,
                    ),
                    SigningBlock.Balances(
                        sg("balanceChangesTitle"),
                        listOf(
                            BalanceDeltaRow("USDC", "−100", SigningTone.Neutral),
                            BalanceDeltaRow("WETH", "+0.0304", SigningTone.Success),
                        ),
                        sg("balanceMatchesHero"),
                    ),
                ),
                confirmAction = sg("confirmLabel"),
            )

            SigningScreenState.CS27 -> model(
                state, Dapp.safe, Dapp.safeTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("safeIntent"), SigningTone.Neutral),
                    SigningBlock.Sentence(sg("safeSummary"), SigningTone.Accent),
                    // Safe's calldata nests, so the sheet decodes the inner call
                    // too: a wrapper that showed only the outer call would show
                    // nothing at all.
                    SigningBlock.Card(
                        sg("safeInnerCall", mapOf("action" to sg("intentSend"))),
                        listOf(
                            SigningRow(sg("labelAmount"), "250 USDC"),
                            SigningRow(sg("recipientLabel"), "Alice Chen"),
                        ),
                        SigningTone.Neutral,
                    ),
                    SigningBlock.Party(
                        sg("interactingLabel"), "Safe 1.4.1 · Safe Ecosystem", Addr.SAFE, verified,
                    ),
                ),
                confirmAction = sg("confirmLabel"),
            )

            SigningScreenState.CS28 -> model(
                state, unknownDapp, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentSend"), SigningTone.Danger),
                    SigningBlock.Amount(
                        AmountLine("", "500", "USDT", Mark.usdt, tone = SigningTone.Danger),
                        card = true,
                        note = sg("sendingToTokenContract"),
                    ),
                    SigningBlock.Party(
                        sg("recipientLabel"), "Tether USD", Addr.USDT,
                        PartyBadge(sg("contractTag"), SigningTone.Danger),
                    ),
                    SigningBlock.Warning(SigningTone.Danger, sg("tokenToContractWarning")),
                ),
                confirmAction = sg("confirmSend"),
            )

            SigningScreenState.CS30 -> model(
                state, unknownDapp, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentContractCall"), SigningTone.Neutral),
                    SigningBlock.Sentence(
                        sg("bestEffortSummary", mapOf("fn" to "execute(…)")), SigningTone.Accent,
                    ),
                    SigningBlock.Warning(SigningTone.Caution, sg("bestEffortWarning")),
                    SigningBlock.Rows(
                        listOf(
                            SigningRow(
                                sg("techFunction"), "execute(bytes,bytes[],uint256)", mono = true,
                            ),
                            SigningRow(
                                sg("techParam", mapOf("index" to "1", "name" to "bytes")),
                                "0x0b00… (2)",
                            ),
                            SigningRow(
                                sg("techParam", mapOf("index" to "2", "name" to "bytes[]")), "2",
                            ),
                            SigningRow(
                                sg("techParam", mapOf("index" to "3", "name" to "deadline")),
                                "2026-08-15 20:00",
                            ),
                        ),
                    ),
                    SigningBlock.Party(
                        sg("interactingLabel"), sg("unverifiedTag"), Addr.UNKNOWN, unverified,
                    ),
                    SigningBlock.Balances(
                        sg("balanceChangesTitle"),
                        listOf(
                            BalanceDeltaRow("ETH", "−0.1", SigningTone.Neutral),
                            BalanceDeltaRow("USDC", "+255.8", SigningTone.Success),
                        ),
                        sg("bestEffortSimulated"),
                    ),
                ),
                confirmAction = sg("confirmLabel"),
            )

            SigningScreenState.CS31 -> model(
                state, unknownDapp, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentContractCall"), SigningTone.Neutral),
                    SigningBlock.Sentence(sg("verifiedAbiSummary"), SigningTone.Neutral),
                    SigningBlock.Rows(
                        listOf(
                            SigningRow("claimRewards · ids", "[128, 129, 130]"),
                            SigningRow(
                                "beneficiary",
                                sg(
                                    "selfName",
                                    mapOf("name" to WalletFixtures.ADDRESS_DISPLAY),
                                ),
                                mono = true,
                            ),
                            SigningRow("restake", "true"),
                        ),
                    ),
                    SigningBlock.Party(
                        sg("interactingLabel"), "RewardsVault", Addr.REWARDS,
                        PartyBadge(sg("contractTag"), SigningTone.Neutral),
                    ),
                    SigningBlock.Warning(SigningTone.Caution, sg("verifiedAbiWarning")),
                    SigningBlock.Balances(
                        sg("balanceChangesTitle"),
                        listOf(BalanceDeltaRow("stETH", "+4.21", SigningTone.Success)),
                        sg("balanceMatchesHero"),
                    ),
                ),
                confirmAction = sg("confirmLabel"),
            )

            SigningScreenState.CS32 -> model(
                state, unknownDapp, Dapp.unknownTint,
                blocks = listOf(
                    SigningBlock.Intent(sg("intentContractCall"), SigningTone.Neutral),
                    // The deepest rung: neither decode nor simulation. Both
                    // failures are stated plainly, and the amount is still
                    // shown — facts that ARE knowable are never withheld
                    // because the rest is not.
                    SigningBlock.Warning(
                        SigningTone.Caution, sg("selectorNotListed", mapOf("bytes" to "4")),
                    ),
                    SigningBlock.Warning(SigningTone.Danger, sg("simUnavailableWarning")),
                    SigningBlock.Rows(
                        listOf(SigningRow(sg("labelAmount"), "0.25 ETH ≈ $640.10")),
                    ),
                    SigningBlock.Party(
                        sg("interactingLabel"), sg("unverifiedTag"), Addr.DEEPEST, unverified,
                    ),
                    SigningBlock.Code(
                        listOf(
                            "0x8fabe4c2000000000000000000000000",
                            "d400866e00b055b20752a826cd5c89b8",
                            "11de130b…",
                        ),
                        note = "(${sg("byteSize", mapOf("n" to "132"))})",
                    ),
                ),
                confirmAction = sg("confirmLabel"),
            )
        }
    }
}
