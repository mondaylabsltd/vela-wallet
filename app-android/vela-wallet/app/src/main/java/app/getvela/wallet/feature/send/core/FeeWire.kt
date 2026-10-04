package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.crux.Wire
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull

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
    /**
     * What this tier bids per gas now, and how high it will go — the two ends
     * of the gas bid the speed control draws (issues 684/685), wei as decimal
     * strings. `null` means nothing honest to show, never 0. Carried so the
     * estimate handed back to the core (the send machine, the speed machine)
     * is whole: a subset here would silently drop the figure.
     */
    val effective_gas_price: String? = null,
    val max_gas_price: String? = null,
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
    /**
     * The tip the relay will SIGN this tier with — the only per-tier number
     * that buys priority, and one half of the gas bid the core publishes
     * (issue 684). `null` when the row omits it; never 0 as a stand-in.
     */
    val max_priority_fee_per_gas: String? = null,
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
    /**
     * The native coin's price from THIS shell's own source, for the $0.01
     * native minimum only (issue 682). Omitted here: the core's default
     * answer is the one Android gave before.
     */
    val native_usd_floor_price: String? = null,
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

    /**
     * Spec 083 fee: the relay ANSWERED that this operation fails. Declared so
     * the wire matches the core; this shell does not tell a refusal from an
     * unreachable relay and answers [SimulationFailed] for both, as before.
     */
    @Serializable
    @SerialName("refused")
    data object Refused : FeeGasOutcome()
}

/**
 * Why there is no quote. Six plain words cross as strings; since spec 082
 * (RJ13) one carries a field and crosses as an object, the way serde writes an
 * externally tagged enum: `{"chain_read":{"rate_limited":true}}`. A value this
 * build has no name for fails the whole view (the wire's strict rule), never
 * reads as some default.
 */
@Serializable(with = FeeFailure.WireSerializer::class)
sealed class FeeFailure {
    /** The name the core writes for this failure (`quote_unavailable`, `chain_read`, …). */
    abstract val name: String

    data object MissingPublicKey : FeeFailure() { override val name = "missing_public_key" }

    data object FeeTokenUnavailable : FeeFailure() { override val name = "fee_token_unavailable" }

    data object QuoteUnavailable : FeeFailure() { override val name = "quote_unavailable" }

    data object CalculationFailed : FeeFailure() { override val name = "calculation_failed" }

    data object EstimateFailed : FeeFailure() { override val name = "estimate_failed" }

    data object GasQuoteTooHigh : FeeFailure() { override val name = "gas_quote_too_high" }

    /**
     * Spec 083 fee: the relay answered that the operation fails — its answer,
     * where [FeeGasOutcome.SimulationFailed] is no answer at all. The core
     * says it only to a shell that reports [FeeGasOutcome.Refused], which
     * this one does not yet.
     */
    data object WouldFail : FeeFailure() { override val name = "would_fail" }

    /**
     * A chain read the quote needs (the account's deployment) got no answer
     * from the chain's nodes (spec 082 RJ13, G48): it is the chain node, not
     * Vela's relay, that is out of reach — [rate_limited] when the nodes
     * answered only with rate limits. The shells produce it; the fee machine
     * never does. Asked again on the same schedule as a relay failure.
     */
    data class ChainRead(val rate_limited: Boolean) : FeeFailure() { override val name = CHAIN_READ }

    /**
     * What the core's `feeRequoteDelayMs` and `feeFailureReasonKey` take: the
     * wire name, or the whole `ChainRead` JSON.
     */
    val wire: String
        get() = when (this) {
            is ChainRead -> Wire.json.encodeToString(WireSerializer, this)
            else -> name
        }

    object WireSerializer : KSerializer<FeeFailure> {
        override val descriptor: SerialDescriptor = JsonElement.serializer().descriptor

        override fun deserialize(decoder: Decoder): FeeFailure {
            val json = decoder as? JsonDecoder ?: throw SerializationException("FeeFailure crosses as JSON only")
            return when (val element = json.decodeJsonElement()) {
                is JsonPrimitive -> PLAIN.firstOrNull { element.isString && it.name == element.content }
                    ?: throw SerializationException("unknown FeeFailure: $element")
                is JsonObject -> {
                    val body = (element[CHAIN_READ] as? JsonObject)
                        ?.takeIf { element.size == 1 }
                        ?: throw SerializationException("unknown FeeFailure: $element")
                    val rateLimited = (body["rate_limited"] as? JsonPrimitive)?.booleanOrNull
                        ?: throw SerializationException("FeeFailure.chain_read without rate_limited: $element")
                    ChainRead(rateLimited)
                }
                else -> throw SerializationException("unknown FeeFailure: $element")
            }
        }

        override fun serialize(encoder: Encoder, value: FeeFailure) {
            val json = encoder as? JsonEncoder ?: throw SerializationException("FeeFailure crosses as JSON only")
            json.encodeJsonElement(
                when (value) {
                    is ChainRead -> JsonObject(mapOf(CHAIN_READ to JsonObject(mapOf("rate_limited" to JsonPrimitive(value.rate_limited)))))
                    else -> JsonPrimitive(value.name)
                },
            )
        }
    }

    companion object {
        private const val CHAIN_READ = "chain_read"

        /** Every word that crosses as a plain string — what the drift test holds against the mirror. */
        val PLAIN: List<FeeFailure> = listOf(
            MissingPublicKey, FeeTokenUnavailable, QuoteUnavailable, CalculationFailed, EstimateFailed, GasQuoteTooHigh,
            WouldFail,
        )
    }
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
    /**
     * Spec 096 F2: the operation itself may spend this coin by an amount no
     * call states, and nothing measured what is left (the core's verdict).
     * The sheet warns while it is the coin paying.
     */
    val spent_by_operation: Boolean = false,
    /**
     * Issue #408: why this coin cannot be chosen, when the numbers prove it —
     * the fee in it and what it has to pay from, each written by the core
     * with its unit. `null` when it can pay, or cannot be weighed (no price).
     */
    val short: FeeShortfall? = null,
)

/** A coin's shortfall against the fee, both sides written by the core (`3.58361 USDT`). */
@Serializable
data class FeeShortfall(
    val need: String,
    val have: String,
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
    /**
     * Issue #408: not one coin on offer can pay this fee (every option has a
     * [FeeOptionView.short]). The sheet says so instead of naming the coin in
     * force as though another could stand in.
     */
    val no_coin_pays: Boolean = false,
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

    /** The core's bound on a whole quote (spec 094 S9): answer [FeeShellResult.DeadlineElapsed] after `ms`. */
    @Serializable
    @SerialName("start_deadline")
    data class StartDeadline(val ms: Int) : FeeOperation()
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

    @Serializable
    @SerialName("deadline_elapsed")
    data object DeadlineElapsed : FeeShellResult()
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
        /**
         * Nobody has chosen a fee coin: the machine pays in one that can
         * (covers the fee on top of what the operation itself moves of it;
         * a coin not being sent first, then a stablecoin, then the larger
         * balance). `fee_token` is then only the fallback. `false` = the
         * person's own pick, priced exactly as asked. The view's `fee_token`
         * and `options[].selected` say which coin was taken.
         */
        val auto_fee_token: Boolean = false,
        /**
         * The resolved number preset (`comma_dot`, …) the core writes the
         * amounts it states in — a coin's shortfall (issue #408).
         */
        val number: String = "comma_dot",
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
