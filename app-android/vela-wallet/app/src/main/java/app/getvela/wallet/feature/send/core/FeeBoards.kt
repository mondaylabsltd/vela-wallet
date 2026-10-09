package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import uniffi.vela_core_uniffi.FeePolicyCore

/**
 * The fee failures the gallery boards draw (PR 2 note 1), each a view the
 * REAL `fee_policy` machine wrote — a question asked, its account read
 * answered as the case says, its timers left running — so a board shows what
 * the core says about a failure (the row's figure and reason, whether it
 * retries by itself, the footer's line), not a drawing of it. Never on a live
 * surface.
 */
object FeeBoards {
    enum class Case {
        /** The chain's nodes did not answer the account read: the core retries by itself. */
        ChainDown,

        /** The read never left the app: retried by itself too, worded as Vela's own fault. */
        Internal,

        /** [ChainDown], and the core's own re-ask is out now: the reason stays, the sign turns. */
        Retrying,

        /** An undeployed account with no key to price it with: only a tap asks again. */
        TapOnly,

        /**
         * PR 2 polish: the relay answered that the operation fails paid in
         * USDC (the coin chosen), and the chain's own coin is untried: a tap
         * opens the coins — "Pay with another coin".
         */
        WouldFailChooseCoin,

        /**
         * The same answer for every coin on offer — nobody chose one, and the
         * machine tried the chain's own and then USDC: no other coin is left,
         * so the row keeps its dash and is no control.
         */
        WouldFailNothing,
    }

    /** The boards' fee coin, chosen by the person (`WouldFail*`). */
    const val USDC = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"

    /** The relay's fee recipient on the boards. */
    private const val RECIPIENT = "0x2222222222222222222222222222222222222222"

    /** A Universal Router: a call the relay simulates (`WouldFail*`). */
    private const val ROUTER = "0x66a9893cc07d91d95644aedd05d03f95e1dba8af"

    private fun native() = FeeAssetQuote(
        recipient = RECIPIENT, asset = FeeAssetKind.Native, balance = "1200000000000000000", decimals = 18,
        symbol = "ETH", usd_balance = "3072.00", usd_price = "2560",
    )

    private fun usdc() = FeeAssetQuote(
        recipient = RECIPIENT, asset = FeeAssetKind.Erc20, fee_token = USDC, balance = "1000000000", decimals = 6,
        symbol = "USDC", usd_balance = "1000.00", usd_price = "1",
    )

    /** The view, as the core writes it, for [case] — a transfer on [chainId] from [account]. */
    fun view(case: Case, chainId: Int, account: String): FeeView =
        Wire.json.decodeFromString(FeeView.serializer(), viewJson(case, chainId, account))

    /** [view]'s JSON exactly as the core wrote it — what `signConfirmState` reads back. */
    fun viewJson(case: Case, chainId: Int, account: String): String {
        if (case == Case.WouldFailChooseCoin || case == Case.WouldFailNothing) return wouldFail(case, chainId, account)
        val read = when (case) {
            Case.Internal -> DeploymentRead.Internal(kind = "board")
            else -> DeploymentRead.Unreachable(rate_limited = false)
        }
        var reads = 0
        val script = CoreScript(FeePolicyCore().asBridge()) { operation ->
            when {
                // The first read only: a re-ask's own read stays out.
                operation.optString("type") == "read_deployment" && reads++ == 0 ->
                    Wire.json.encodeToString(FeeShellResult.serializer(), FeeShellResult.Deployment(read))
                // The run's deadline and the core's re-ask timer stay out:
                // the board is the moment the failure is on screen.
                else -> null
            }
        }
        val question = FeeEvent.QuoteRequested(
            chain_id = chainId,
            account = account,
            deployed = false,
            public_key_available = case != Case.TapOnly,
            tier = FeeTier.Standard,
            calls = listOf(FeeCall(to = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141", value = "1000000000000000")),
            read_deployment = case != Case.TapOnly,
        )
        script.dispatch(Wire.json.encodeToString(FeeEvent.serializer(), question))
        if (case == Case.Retrying) {
            // The core's own timer ran out: its re-ask goes, and its read is
            // left out — the moment the row keeps its reason and turns.
            script.resolve("start_ttl", Wire.json.encodeToString(FeeShellResult.serializer(), FeeShellResult.TtlElapsed))
        }
        return script.viewJson()
    }

    /**
     * The relay's answer that the operation fails: a deployed account's
     * contract call (one the relay simulates — a router `execute`, past the
     * estimation line), gathered as a calm network answers it — the gas
     * price, the bundler's quote, the relay's in-band coins (the chain's own
     * and USDC) — and every simulation refused (`FeeGasOutcome.Refused`).
     * Paid in USDC by the person's choice, the chain's coin is left untried
     * ([Case.WouldFailChooseCoin]); with nobody's choice the machine tries
     * both ([Case.WouldFailNothing]). What the row and the footer then say is
     * the core's.
     */
    private fun wouldFail(case: Case, chainId: Int, account: String): String {
        val wire = Wire.json
        val rows = listOf(native(), usdc())
        val chosen = case == Case.WouldFailChooseCoin
        val script = CoreScript(FeePolicyCore().asBridge()) { operation ->
            when (operation.optString("type")) {
                "fetch_gas_price" -> wire.encodeToString(
                    FeeShellResult.serializer(),
                    FeeShellResult.GasPrice(eth_gas_price = "1000000000", base_fee = "0", priority_fee = "0"),
                )
                "fetch_bundler_quote" -> wire.encodeToString(
                    FeeShellResult.serializer(),
                    FeeShellResult.BundlerQuote(
                        FeeBundlerQuote(max_fee_per_gas = "2000000000", network_fee_per_gas = "1000000000", relayer_fee_per_gas = "1000000000"),
                    ),
                )
                "fetch_in_band_quotes" -> wire.encodeToString(FeeShellResult.serializer(), FeeShellResult.InBandQuotes(rows))
                "fetch_fee_recipient" -> wire.encodeToString(FeeShellResult.serializer(), FeeShellResult.FeeRecipient(RECIPIENT))
                "estimate_user_op_gas" -> wire.encodeToString(FeeShellResult.serializer(), FeeShellResult.UserOpGas(FeeGasOutcome.Refused))
                "measure_inner_calls" -> wire.encodeToString(
                    FeeShellResult.serializer(),
                    FeeShellResult.InnerCallsMeasured(List(operation.optJSONArray("calls")?.length() ?: 0) { null }),
                )
                // The run's deadline and the block-time timer stay out.
                else -> null
            }
        }
        val question = FeeEvent.QuoteRequested(
            chain_id = chainId,
            account = account,
            deployed = true,
            public_key_available = true,
            tier = FeeTier.Standard,
            calls = listOf(FeeCall(to = ROUTER, data = "0x3593564c" + "ab".repeat(1_200))),
            fee_token = if (chosen) USDC else null,
            auto_fee_token = !chosen,
        )
        script.dispatch(wire.encodeToString(FeeEvent.serializer(), question))
        return script.viewJson()
    }
}
