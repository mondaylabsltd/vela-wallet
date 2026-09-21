package app.getvela.wallet.feature.send.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `fee_policy` machine's wire, transcribed from
 * `rust/crates/vela-core/src/app/fee_policy.rs` (spec 043).
 *
 * This is the machine that prices a send. The shell fetches numbers — gas
 * signals, the relay's quote, the in-band fee assets, the fee recipient, a
 * gas estimate — and the core turns them into ONE estimate that the confirm
 * screen renders and the submit signs (the "displayed = signed" gate,
 * invariant ①). Every quantity crosses as a decimal string; the shell never
 * does arithmetic on any of them.
 *
 * Numerics from the Rust: `chain_id`, `decimals`, `ms` are `u32` → `Int`.
 */

// -- value types -------------------------------------------------------------

/** One call of a batch: `to`, `value` in wei (decimal string), `data` 0x-hex. */
@Serializable
data class FeeCall(val to: String, val value: String = "0", val data: String = "0x")

@Serializable
enum class FeeTier {
    @SerialName("slow") Slow,

    @SerialName("standard") Standard,

    @SerialName("rapid") Rapid,

    @SerialName("fast") Fast,
}

@Serializable
enum class FeeAssetKind {
    @SerialName("native") Native,

    @SerialName("erc20") Erc20,
}

/** What the fee is paid in — the chain's coin, or an ERC-20 the relay accepts. */
@Serializable
sealed class FeeAssetView {
    @Serializable
    @SerialName("native")
    data object Native : FeeAssetView()

    @Serializable
    @SerialName("erc20")
    data class Erc20(
        val token: String,
        val decimals: Int,
        /** The fee in the token's smallest unit, decimal string. */
        val amount: String,
        val symbol: String? = null,
    ) : FeeAssetView()
}

/**
 * The estimate the confirm screen shows and the submit signs. Handed back to
 * the send machine unchanged (`SendEvent.FeeUpdated`), which is the whole
 * point: there is one of these per attempt and no second derivation.
 */
@Serializable
data class FeeEstimateView(
    val chain_id: Int,
    val total_wei: String,
    val max_fee_per_gas: String,
    val network_fee_per_gas: String,
    val relayer_fee_per_gas: String,
    val bundler_gas_price: String,
    val in_band_gas_basis: String,
    val total_gas: String,
    val deployed: Boolean,
    val tier: FeeTier,
    val quoted: Boolean,
    val fee_asset: FeeAssetView,
    val fee_recipient: String? = null,
)

@Serializable
data class FeeBundlerQuote(
    val max_fee_per_gas: String,
    val network_fee_per_gas: String? = null,
    val relayer_fee_per_gas: String? = null,
)

/** One in-band fee asset the relay quoted for this account on this chain. */
@Serializable
data class FeeAssetQuote(
    val recipient: String,
    val asset: FeeAssetKind,
    val fee_token: String? = null,
    val balance: String,
    val decimals: Int,
    val symbol: String,
    val usd_balance: String,
    val usd_price: String? = null,
)

@Serializable
sealed class FeeGasOutcome {
    @Serializable
    @SerialName("estimated")
    data class Estimated(
        val verification_gas_limit: String,
        val call_gas_limit: String,
        val pre_verification_gas: String,
    ) : FeeGasOutcome()

    @Serializable
    @SerialName("simulation_failed")
    data object SimulationFailed : FeeGasOutcome()

    @Serializable
    @SerialName("context_unavailable")
    data object ContextUnavailable : FeeGasOutcome()
}

@Serializable
enum class FeeFailure {
    @SerialName("missing_public_key") MissingPublicKey,

    @SerialName("fee_token_unavailable") FeeTokenUnavailable,

    @SerialName("quote_unavailable") QuoteUnavailable,

    @SerialName("calculation_failed") CalculationFailed,

    @SerialName("estimate_failed") EstimateFailed,

    @SerialName("gas_quote_too_high") GasQuoteTooHigh,
}

/** One row of the fee-token sheet, already judged (`insufficient`, `selected`). */
@Serializable
data class FeeOptionView(
    val symbol: String,
    val contract: String? = null,
    val decimals: Int,
    val balance: String,
    val recipient: String,
    val usd_balance: String,
    val usd_price: String? = null,
    /** The fee in this asset, when the quote could price it. */
    val amount: String? = null,
    val insufficient: Boolean = false,
    val selected: Boolean = false,
)

@Serializable
data class FeeView(
    val busy: Boolean = false,
    val failed: FeeFailure? = null,
    val fee: FeeEstimateView? = null,
    val stale: Boolean = false,
    val fee_token: String? = null,
    val options: List<FeeOptionView> = emptyList(),
    val confirm_fee_ready: Boolean = false,
)

// -- what the machine asks for -----------------------------------------------

@Serializable
sealed class FeeOperation {
    @Serializable
    @SerialName("fetch_gas_price")
    data class FetchGasPrice(val chain_id: Int, val want_tip: Boolean) : FeeOperation()

    @Serializable
    @SerialName("fetch_bundler_quote")
    data class FetchBundlerQuote(val chain_id: Int, val tier: FeeTier) : FeeOperation()

    @Serializable
    @SerialName("fetch_in_band_quotes")
    data class FetchInBandQuotes(val chain_id: Int, val account: String) : FeeOperation()

    @Serializable
    @SerialName("fetch_fee_recipient")
    data class FetchFeeRecipient(val chain_id: Int, val account: String) : FeeOperation()

    @Serializable
    @SerialName("estimate_user_op_gas")
    data class EstimateUserOpGas(
        val chain_id: Int,
        val account: String,
        val deployed: Boolean,
        val calls: List<FeeCall> = emptyList(),
    ) : FeeOperation()

    /**
     * `eth_estimateGas` for each call on its own, from the Safe (`from`) —
     * only the contract calls, issued beside the estimate so the quote prices
     * the `callGasLimit` the submit will raise to.
     */
    @Serializable
    @SerialName("measure_inner_calls")
    data class MeasureInnerCalls(
        val chain_id: Int,
        val from: String,
        val calls: List<FeeCall> = emptyList(),
    ) : FeeOperation()

    @Serializable
    @SerialName("start_ttl")
    data class StartTtl(val ms: Int) : FeeOperation()
}

// -- what the shell observed -------------------------------------------------

@Serializable
sealed class FeeShellResult {
    /** Each `null` = that signal could not be read; the core prices with what it has. */
    @Serializable
    @SerialName("gas_price")
    data class GasPrice(
        val eth_gas_price: String? = null,
        val base_fee: String? = null,
        val priority_fee: String? = null,
    ) : FeeShellResult()

    @Serializable
    @SerialName("bundler_quote")
    data class BundlerQuote(val quote: FeeBundlerQuote? = null) : FeeShellResult()

    /** `quotes = null` = the relay could not be asked; `[]` = it offers nothing here. */
    @Serializable
    @SerialName("in_band_quotes")
    data class InBandQuotes(val quotes: List<FeeAssetQuote>? = null) : FeeShellResult()

    @Serializable
    @SerialName("fee_recipient")
    data class FeeRecipient(val recipient: String? = null) : FeeShellResult()

    @Serializable
    @SerialName("user_op_gas")
    data class UserOpGas(val outcome: FeeGasOutcome) : FeeShellResult()

    /** One entry per measured call, in order, decimal gas; `null` = nobody could measure it. */
    @Serializable
    @SerialName("inner_calls_measured")
    data class InnerCallsMeasured(val gas: List<String?> = emptyList()) : FeeShellResult()

    @Serializable
    @SerialName("ttl_elapsed")
    data object TtlElapsed : FeeShellResult()
}

// -- what the shell tells it -------------------------------------------------

@Serializable
sealed class FeeEvent {
    @Serializable
    @SerialName("quote_requested")
    data class QuoteRequested(
        val chain_id: Int,
        val account: String,
        val deployed: Boolean,
        val public_key_available: Boolean,
        val tier: FeeTier,
        val calls: List<FeeCall> = emptyList(),
        val fee_token: String? = null,
    ) : FeeEvent()

    @Serializable
    @SerialName("select_fee_asset")
    data class SelectFeeAsset(val token: String? = null) : FeeEvent()

    @Serializable
    @SerialName("requote")
    data object Requote : FeeEvent()

    @Serializable
    @SerialName("leave_confirm")
    data object LeaveConfirm : FeeEvent()

    @Serializable
    @SerialName("chain_changed")
    data class ChainChanged(val chain_id: Int) : FeeEvent()

    @Serializable
    @SerialName("quote_expired")
    data object QuoteExpired : FeeEvent()
}
