package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import uniffi.vela_core_uniffi.RelayRejection
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.UserOpFeeMode
import uniffi.vela_core_uniffi.WebAuthnAssertion
import uniffi.vela_core_uniffi.classifyRelayRejection
import uniffi.vela_core_uniffi.isChainWithoutNativeCoin
import uniffi.vela_core_uniffi.parseExistingUserOpHash
import uniffi.vela_core_uniffi.quotedFeeUsable
import uniffi.vela_core_uniffi.relayErrorMessage
import uniffi.vela_core_uniffi.userOpApplyEstimate
import uniffi.vela_core_uniffi.userOpDraft
import uniffi.vela_core_uniffi.userOpFloors
import uniffi.vela_core_uniffi.userOpHasContractCall
import uniffi.vela_core_uniffi.userOpRelayJson
import uniffi.vela_core_uniffi.userOpSafeOpHash
import uniffi.vela_core_uniffi.userOpSign
import uniffi.vela_core_uniffi.userOpWithCalls

/**
 * The submit spine — ONE implementation for a person's own transfer and a
 * dApp's transaction (spec 044 T028, research D7; the desktop's
 * `executor::user_op::submit`).
 *
 * The order is `sendUserOpInBand`'s: isDeployed → nonce → floors → the
 * placeholder fee leg → estimate → apply → the settled fee leg → the SafeOp
 * hash is the challenge → the assertion → the envelope → the relay. The
 * assembly is the core's (`user_op_*` through uniffi); the transports are
 * [RelayClient]'s; the one seam the core cannot have is [UserOpSigner].
 * Nothing here prices, validates or classifies beyond what the core exports.
 *
 * Extracted from `SendExecutor` so the sheet that shows what will happen
 * and the code that makes it happen cannot become two opinions.
 */
class UserOpSpine(
    private val relay: RelayClient,
    private val accounts: SendExecutor.AccountPort,
    private val signer: () -> UserOpSigner,
) {
    /** The displayed fee, signed verbatim. */
    data class Quoted(val amount: String, val recipient: String)

    sealed class Failure {
        data object PasskeyCancelled : Failure()
        data object RelayerUnavailable : Failure()
        data object BundlerUnderfunded : Failure()
        data class Other(val message: String?) : Failure()
    }

    class Refused(val failure: Failure) : Exception()

    private fun other(message: String): Nothing = throw Refused(Failure.Other(message))

    /**
     * Assembles, signs once and submits; returns the accepted user-operation
     * hash (or the one already pending for this nonce). Throws [Refused].
     */
    suspend fun submit(
        chainId: Int,
        account: String,
        calls: List<UserOpCall>,
        gasFeeToken: String?,
        quotedFee: Quoted?,
        signingStarted: () -> Unit = {},
    ): String {
        val keys = accounts.keysOf(account)
        if (keys.isEmpty()) other("No passkey credential for the active account")
        val pinned = keys.first()
        val tempo = isChainWithoutNativeCoin(chainId.toUInt())
        val deployed = relay.isDeployed(chainId, account)
            ?: other("The network could not be reached. Please try again.")
        val nonce = if (deployed) {
            relay.nonce(chainId, account) ?: other("The account's nonce could not be read. Please try again.")
        } else {
            "0x0"
        }
        val floors = userOpFloors(chainId.toUInt(), deployed, (calls.size + 1).toUInt())
        val quoted = quotedFee
            ?.takeIf { quotedFeeUsable(it.amount, it.recipient) }
            ?: other("The fee quote has expired. Please review the updated fee and try again.")
        val feeToken = if (tempo) gasFeeToken ?: SendExecutor.TEMPO_DEFAULT_FEE_TOKEN else gasFeeToken
        val settled: UserOpFeeMode
        val placeholder: UserOpFeeMode
        if (tempo) {
            val collector = relay.accountInfo(chainId, account)?.feeRecipient()
                ?: other("The Tempo gas relayer is unavailable right now. Please try again.")
            if (!quoted.recipient.equals(collector, ignoreCase = true)) {
                other("The gas quote has expired. Please review the updated fee and try again.")
            }
            settled = UserOpFeeMode.Tempo(feeToken = feeToken!!, collector = collector, reimbursement = quoted.amount)
            placeholder = UserOpFeeMode.Tempo(feeToken = feeToken, collector = collector, reimbursement = "1")
        } else {
            settled = UserOpFeeMode.InBand(gasFeeToken = feeToken, amount = quoted.amount, recipient = quoted.recipient)
            placeholder = UserOpFeeMode.InBand(gasFeeToken = feeToken, amount = "1", recipient = account)
        }
        var draft = runCatching {
            userOpDraft(
                sender = account,
                nonce = nonce,
                deployed = deployed,
                keyHexes = keys.map { it.publicKeyHex },
                calls = calls,
                fee = placeholder,
                floors = floors,
            )
        }.getOrElse { other(it.message ?: "The operation could not be assembled.") }
        val hasContractCall = userOpHasContractCall(calls)
        when (val estimate = relay.estimateUserOpGas(chainId, userOpRelayJson(draft, feeToken.takeIf { tempo }))) {
            is RelayClient.EstimateAnswer.Estimated -> draft = userOpApplyEstimate(
                draft,
                estimate.verificationGasLimit,
                estimate.callGasLimit,
                estimate.preVerificationGas,
                floors,
            )
            is RelayClient.EstimateAnswer.Refused, RelayClient.EstimateAnswer.Unreachable -> {
                VelaLog.event("userop.submit", "estimate failed, defaults", "contract" to hasContractCall)
                if (hasContractCall) {
                    other("Could not estimate gas for this transaction. The network may be busy — please try again.")
                }
            }
        }
        draft = userOpWithCalls(draft, calls, settled)
        val challenge = userOpSafeOpHash(draft, chainId.toUInt())
        signingStarted()
        val (transports, method) = accounts.routingOf(account)
        val assertion: Assertion = try {
            signer().sign(challenge, pinned.credentialId, transports, method)
        } catch (failure: PasskeyFailure) {
            if (failure.kind == FailureKind.Cancelled) throw Refused(Failure.PasskeyCancelled)
            other(failure.message ?: "the passkey ceremony failed")
        }
        val signed = runCatching {
            userOpSign(
                draft,
                WebAuthnAssertion(
                    authenticatorData = SendExecutor.unhex(assertion.authenticatorDataHex),
                    clientDataJson = SendExecutor.unhex(assertion.clientDataJsonHex),
                    signatureDer = SendExecutor.unhex(assertion.signatureDerHex),
                ),
                assertion.credentialIdHex,
                keys,
            )
        }.getOrElse { other(it.message ?: "Failed to create signature") }
        return when (val answer = relay.sendUserOp(chainId, userOpRelayJson(signed, feeToken.takeIf { tempo }))) {
            is RelayClient.SubmitAnswer.Accepted -> answer.userOpHash
            is RelayClient.SubmitAnswer.Rejected -> {
                val message = relayErrorMessage(answer.errorJson)
                parseExistingUserOpHash(message)?.let { existing ->
                    VelaLog.event("userop.submit", "previous op pending", "hash" to existing.take(12))
                    return existing
                }
                throw Refused(
                    when (val rejection = classifyRelayRejection(message)) {
                        RelayRejection.RelayerUnavailable -> Failure.RelayerUnavailable
                        RelayRejection.BundlerUnderfunded -> Failure.BundlerUnderfunded
                        is RelayRejection.Other -> Failure.Other(rejection.message.ifBlank { null })
                    },
                )
            }
            RelayClient.SubmitAnswer.Unreachable -> other("The gas relayer could not be reached. Please try again.")
        }
    }
}
