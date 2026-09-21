package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.UserOpFeeMode
import uniffi.vela_core_uniffi.userOpDraft
import uniffi.vela_core_uniffi.userOpFloors
import uniffi.vela_core_uniffi.userOpRelayJson

/**
 * The `fee_policy` machine's seven arms (spec 043 T025; `MeasureInnerCalls`
 * since the issue #262 follow-up).
 *
 * Numbers in, numbers out: the three gas signals, the relay's quote, the
 * in-band fee assets, the fee recipient, and a gas estimate for the batch the
 * core built — drafted by the core with its dummy signature so the estimate is
 * byte-identical in shape to what the submit will send. Every padding rule,
 * every price, every "is this quote too high" is the core's.
 */
class FeeExecutor(
    private val relay: RelayClient,
    /** The wallet's founding keys, for an undeployed account's initCode. */
    private val keyHexes: suspend (account: String) -> List<String>,
    /**
     * `eth_estimateGas({from, to, value, data})` for one call — the gas hex, or
     * `null` when nobody answered. The same read `UserOpSpine.measureCall` makes
     * at submit, so the quote prices the `callGasLimit` the op will carry
     * (issue #262 follow-up). Unwired = nothing measured = the relay's figure.
     */
    private val measureCall: suspend (chainId: Int, from: String, to: String, valueHex: String, data: String) -> String? =
        { _, _, _, _, _ -> null },
) {

    suspend fun perform(operation: FeeOperation): FeeShellResult = arm(operation).also { result ->
        VelaLog.event("send.fee.arm", operation::class.simpleName ?: "?", "answer" to summary(result))
    }

    private fun summary(result: FeeShellResult): String = when (result) {
        is FeeShellResult.GasPrice -> "gas=${result.eth_gas_price ?: "-"} base=${result.base_fee ?: "-"} tip=${result.priority_fee ?: "-"}"
        is FeeShellResult.BundlerQuote -> "quote=${result.quote?.max_fee_per_gas ?: "-"}"
        is FeeShellResult.InBandQuotes -> "quotes=${result.quotes?.size ?: "-"}"
        is FeeShellResult.FeeRecipient -> "recipient=${result.recipient?.take(10) ?: "-"}"
        is FeeShellResult.UserOpGas -> "gas=${result.outcome::class.simpleName}"
        is FeeShellResult.InnerCallsMeasured -> "inner=${result.gas.joinToString(",") { it ?: "-" }}"
        FeeShellResult.TtlElapsed -> "ttl"
    }

    private suspend fun arm(operation: FeeOperation): FeeShellResult = when (operation) {
        is FeeOperation.FetchGasPrice -> {
            val signals = relay.gasSignals(operation.chain_id, operation.want_tip)
            FeeShellResult.GasPrice(
                eth_gas_price = signals.ethGasPrice,
                base_fee = signals.baseFee,
                priority_fee = signals.priorityFee,
            )
        }

        is FeeOperation.FetchBundlerQuote ->
            FeeShellResult.BundlerQuote(relay.bundlerQuote(operation.chain_id, operation.tier))

        is FeeOperation.FetchInBandQuotes ->
            FeeShellResult.InBandQuotes(relay.inBandQuotes(operation.chain_id, operation.account))

        is FeeOperation.FetchFeeRecipient -> FeeShellResult.FeeRecipient(
            relay.accountInfo(operation.chain_id, operation.account)?.feeRecipient(),
        )

        is FeeOperation.EstimateUserOpGas -> FeeShellResult.UserOpGas(estimate(operation))

        is FeeOperation.MeasureInnerCalls -> FeeShellResult.InnerCallsMeasured(
            // Each call on its own, from the Safe. What an unmeasured call means
            // (no floor — the relay's figure) is the core's.
            operation.calls.map { call ->
                runCatching {
                    val valueHex = "0x" + java.math.BigInteger(call.value.ifBlank { "0" }).toString(16)
                    measureCall(operation.chain_id, operation.from, call.to, valueHex, call.data)
                        ?.let { java.math.BigInteger(it.removePrefix("0x"), 16).toString() }
                }.getOrNull()
            },
        )

        is FeeOperation.StartTtl -> {
            delay(operation.ms.toLong())
            FeeShellResult.TtlElapsed
        }
    }

    /**
     * `simulateUserOpGas`: a truthful dummy operation — the real nonce for a
     * deployed account, the real initCode for an undeployed one — to the
     * relay's estimator. `ContextUnavailable` is the shell unable to build a
     * TRUTHFUL draft; `SimulationFailed` is a truthful draft the relay refused.
     */
    private suspend fun estimate(operation: FeeOperation.EstimateUserOpGas): FeeGasOutcome = coroutineScope {
        val nonce = if (operation.deployed) {
            relay.nonce(operation.chain_id, operation.account) ?: return@coroutineScope FeeGasOutcome.ContextUnavailable
        } else {
            "0x0"
        }
        val keys = if (operation.deployed) emptyList() else keyHexes(operation.account)
        if (!operation.deployed && keys.isEmpty()) return@coroutineScope FeeGasOutcome.ContextUnavailable
        val calls = operation.calls.map { UserOpCall(to = it.to, value = it.value, data = it.data) }
        val floors = userOpFloors(
            chainId = operation.chain_id.toUInt(),
            deployed = operation.deployed,
            subCalls = calls.size.toUInt(),
        )
        val draft = runCatching {
            userOpDraft(
                sender = operation.account,
                nonce = nonce,
                deployed = operation.deployed,
                keyHexes = keys,
                calls = calls,
                fee = UserOpFeeMode.EstimateOnly,
                floors = floors,
            )
        }.getOrElse { error ->
            VelaLog.failure("send.fee.draft", "could not draft the estimate op", error)
            return@coroutineScope FeeGasOutcome.ContextUnavailable
        }
        val json = async { userOpRelayJson(draft, null) }
        when (val answer = relay.estimateUserOpGas(operation.chain_id, json.await())) {
            is RelayClient.EstimateAnswer.Estimated -> FeeGasOutcome.Estimated(
                verification_gas_limit = answer.verificationGasLimit,
                call_gas_limit = answer.callGasLimit,
                pre_verification_gas = answer.preVerificationGas,
            )
            is RelayClient.EstimateAnswer.Refused -> {
                VelaLog.event("send.fee", "estimate refused", "why" to answer.message.take(120))
                FeeGasOutcome.SimulationFailed
            }
            RelayClient.EstimateAnswer.Unreachable -> FeeGasOutcome.SimulationFailed
        }
    }

    /** What the core hears when an arm threw: nothing was read. */
    fun neutralAnswer(operation: FeeOperation): FeeShellResult = when (operation) {
        is FeeOperation.FetchGasPrice -> FeeShellResult.GasPrice()
        is FeeOperation.FetchBundlerQuote -> FeeShellResult.BundlerQuote(null)
        is FeeOperation.FetchInBandQuotes -> FeeShellResult.InBandQuotes(null)
        is FeeOperation.FetchFeeRecipient -> FeeShellResult.FeeRecipient(null)
        is FeeOperation.EstimateUserOpGas -> FeeShellResult.UserOpGas(FeeGasOutcome.ContextUnavailable)
        is FeeOperation.MeasureInnerCalls -> FeeShellResult.InnerCallsMeasured(operation.calls.map { null })
        is FeeOperation.StartTtl -> FeeShellResult.TtlElapsed
    }
}
