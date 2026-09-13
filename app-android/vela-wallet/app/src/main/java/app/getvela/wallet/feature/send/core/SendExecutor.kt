package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.wallet.core.Abi
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.settings.core.NetView
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.job
import org.json.JSONObject
import uniffi.vela_core_uniffi.RelayRejection
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.UserOpFeeMode
import uniffi.vela_core_uniffi.WalletKeyRecord
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
 * The `send` machine's eighteen arms (spec 043 T027).
 *
 * This file is the ORDER — which read happens before which, what is fatal and
 * what falls back — in `sendUserOpInBand`'s order, the same the desktop's
 * `executor/user_op.rs` performs. The assembly is the core's (`user_op_*`
 * through uniffi); the transports are [RelayClient]'s; the one seam the core
 * cannot have is [UserOpSigner]. Nothing here prices, validates or classifies:
 * the relay's words go to the core to be classified, and the screen prints the
 * core's sentence.
 *
 * Two refusals before any signature: a deployed wallet whose nonce could not
 * be read, and a batch carrying a real contract call whose gas could not be
 * estimated — a passkey prompt on an operation the relay must reject is a
 * prompt wasted.
 */
class SendExecutor(
    private val relay: RelayClient,
    private val pool: RpcPool,
    private val feed: FeedExecutor,
    private val accounts: AccountPort,
    private val balances: () -> BalanceView,
    private val networks: () -> NetView,
    private val signer: () -> UserOpSigner,
    private val feeQuoter: FeeQuoter,
    private val ports: SendPorts,
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
    /** Spec 043 T048: the identity waterfall the contacts machine also asks. */
    private val identity: suspend (String) -> SendRecipientIdentity? = { null },
) {

    /** The account store, as the send path reads it. */
    interface AccountPort {
        /** The wallet's founding keys for `address`, pinned key first; empty when unknown. */
        suspend fun keysOf(address: String): List<WalletKeyRecord>

        /** The pinned key's stored transports and method, for the ceremony's routing. */
        suspend fun routingOf(address: String): Pair<String, KeyMethod>

        suspend fun publicKeyOf(accountId: String): String?
    }

    /** The live fee session, as `estimate_fee` needs it (research D7). */
    interface FeeQuoter {
        suspend fun quote(
            chainId: Int,
            account: String,
            calls: List<FeeCall>,
            gasFeeToken: String?,
            publicKeyAvailable: Boolean,
        ): SendFeeOutcome
    }

    /** What the screen owns: the tracker handoff, the alert surface, haptics, close. */
    interface SendPorts {
        fun signingStarted()

        /** Spec 046 US3: a scanned chain the wallet lacks — the settings machine adds it (or does not know it). */
        suspend fun addNetwork(chainId: Long): SendAddNetworkOutcome = SendAddNetworkOutcome.NotFound

        fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int)

        fun haptic(kind: SendHapticKind)

        fun alert(kind: SendAlertKind)

        fun closed()

        fun refreshBalances()

        /** The pending row is on disk: the feed re-reads, so the home shows it at submit (FR-006). */
        fun recordsPersisted()
    }

    @Volatile
    private var signing: Job? = null

    suspend fun perform(operation: SendOperation): SendShellResult = when (operation) {
        is SendOperation.FetchTokens -> fetchTokens()
        is SendOperation.ClearTokenCache -> {
            ports.refreshBalances()
            SendShellResult.TokenCacheCleared
        }
        is SendOperation.ResolveTokenMetadata -> SendShellResult.TokenMetadata(
            tokenMetadata(operation.chain_id, operation.address),
        )
        // Spec 046 US3: the scan path — `AddByChainIdRequested` in the settings machine.
        is SendOperation.AddNetwork -> SendShellResult.NetworkAdded(ports.addNetwork(operation.chain_id.toLong()))
        is SendOperation.EstimateFee -> {
            // A batch takes precedence only when it HAS legs; an empty one
            // would otherwise silence the single call beside it.
            val calls = when {
                !operation.batch.isNullOrEmpty() -> operation.batch
                operation.tx != null -> listOf(operation.tx)
                else -> emptyList()
            }
            SendShellResult.FeeEstimated(
                feeQuoter.quote(
                    chainId = operation.chain_id,
                    account = operation.account,
                    calls = calls,
                    gasFeeToken = operation.gas_fee_token,
                    publicKeyAvailable = operation.public_key_hex != null,
                ),
            )
        }
        is SendOperation.ProbeTreasury -> SendShellResult.TreasuryProbed(relay.probeTreasury(operation.chain_id))
        is SendOperation.LoadAccountCredential ->
            SendShellResult.AccountCredential(accounts.publicKeyOf(operation.account_id))
        is SendOperation.SubmitUserOp -> submit(operation)
        SendOperation.CancelPasskeySign -> {
            signing?.cancel(CancellationException("cancelled by the person"))
            SendShellResult.PasskeyCancelAcknowledged
        }
        is SendOperation.PersistTxRecords -> {
            val ok = feed.writeRecords(operation.records.map(::feedRow))
            if (ok) ports.recordsPersisted() else VelaLog.event("send.persist", "refused", "rows" to operation.records.size)
            SendShellResult.RecordsPersisted
        }
        is SendOperation.TrackSubmitted -> {
            ports.trackSubmitted(operation.user_op_hash, operation.record_ids, operation.chain_id)
            SendShellResult.TrackHandedOff
        }
        is SendOperation.ResolveIdentity -> SendShellResult.IdentityResolved(identity(operation.address))
        // `first_time` needs the local send history by counterparty (the
        // contacts machine's `has_prior_interaction`); it lands with the
        // identity backfill (T048). `null` = not judged, never "yes".
        is SendOperation.ResolveRisk -> SendShellResult.RiskResolved(
            SendRecipientRisk(
                is_contract = relay.isDeployed(operation.chain_id, operation.address),
                first_time = null,
            ),
        )
        // No simulation engine on this base (046).
        is SendOperation.SimulateCalls -> SendShellResult.SimResolved(null)
        is SendOperation.StartTimer -> {
            delay(operation.ms.toLong())
            SendShellResult.TimerElapsed(operation.tag)
        }
        is SendOperation.Haptic -> {
            ports.haptic(operation.kind)
            SendShellResult.HapticPlayed
        }
        is SendOperation.ShowAlert -> {
            ports.alert(operation.kind)
            SendShellResult.AlertAcknowledged
        }
        SendOperation.Close -> {
            ports.closed()
            SendShellResult.Closed
        }
    }

    // -- tokens -----------------------------------------------------------------

    /** The holdings the balance machine already read — no second walk. */
    private fun fetchTokens(): SendShellResult {
        val view = balances()
        val net = networks()
        val chains = net.networks.map { row ->
            SendChainInfo(chain_id = row.chain_id.toInt(), network = network(row.chain_id.toInt()), native_symbol = row.native_symbol)
        }
        val tokens = (view.tokens + view.unpriced_tokens).map { token ->
            SendToken(
                network = network(token.chain_id),
                chain_id = token.chain_id,
                symbol = token.symbol,
                balance = token.balance,
                decimals = token.decimals,
                token_address = token.token_address,
                price_usd = token.price_usd,
                logo_urls = emptyList(),
                spam = token.spam,
            )
        }
        return SendShellResult.TokensLoaded(tokens = tokens, chains = chains)
    }

    private suspend fun tokenMetadata(chainId: Int, address: String): SendTokenMeta? {
        val symbol = pool.call(chainId, "eth_call", listOf(JSONObject().put("to", address).put("data", Abi.encodeSymbol()), "latest"))
        val decimals = pool.call(chainId, "eth_call", listOf(JSONObject().put("to", address).put("data", Abi.encodeDecimals()), "latest"))
        val symbolHex = (symbol as? RpcResult.Body)?.json?.optString("result")?.takeIf { it.startsWith("0x") } ?: return null
        val decimalsHex = (decimals as? RpcResult.Body)?.json?.optString("result")?.takeIf { it.startsWith("0x") } ?: return null
        val text = Abi.decodeString(symbolHex) ?: return null
        return SendTokenMeta(symbol = text, decimals = runCatching { Abi.decodeUint8(decimalsHex) }.getOrNull() ?: return null)
    }

    // -- the submit spine ---------------------------------------------------------

    private suspend fun submit(op: SendOperation.SubmitUserOp): SendShellResult = coroutineScope {
        signing = currentCoroutineContext().job
        try {
            SendShellResult.Submitted(user_op_hash = submitInner(op), now_ms = now())
        } catch (refused: SubmitRefused) {
            VelaLog.event("send.submit", "refused", "why" to refused.failure.toString().take(160))
            SendShellResult.SubmitFailed(refused.failure)
        } catch (cancelled: CancellationException) {
            VelaLog.event("send.submit", "cancelled")
            SendShellResult.SubmitFailed(SendSubmitFailure.PasskeyCancelled)
        } finally {
            signing = null
        }
    }

    private class SubmitRefused(val failure: SendSubmitFailure) : Exception()

    private fun other(message: String): Nothing = throw SubmitRefused(SendSubmitFailure.Other(message))

    private val spine = UserOpSpine(relay, accounts, signer)

    /** The spine (spec 044 T028): one implementation for a person's transfer and a dApp's transaction. */
    private suspend fun submitInner(op: SendOperation.SubmitUserOp): String = try {
        spine.submit(
            chainId = op.chain_id,
            account = op.account,
            calls = op.calls.map { UserOpCall(to = it.to, value = it.value, data = it.data) },
            gasFeeToken = op.gas_fee_token,
            quotedFee = op.quoted_fee?.let { UserOpSpine.Quoted(it.amount, it.recipient) },
            signingStarted = { ports.signingStarted() },
        )
    } catch (refused: UserOpSpine.Refused) {
        throw SubmitRefused(
            when (val failure = refused.failure) {
                UserOpSpine.Failure.PasskeyCancelled -> SendSubmitFailure.PasskeyCancelled
                UserOpSpine.Failure.RelayerUnavailable -> SendSubmitFailure.RelayerUnavailable
                UserOpSpine.Failure.BundlerUnderfunded -> SendSubmitFailure.BundlerUnderfunded
                is UserOpSpine.Failure.Other -> SendSubmitFailure.Other(failure.message)
            },
        )
    }

    /** The feed's own camelCase row for a submitted send (data-model.md). */
    private fun feedRow(record: SendTxRecord): JSONObject = JSONObject()
        .put("id", record.id)
        .put("userOpHash", record.user_op_hash)
        .put("txHash", record.tx_hash)
        .put("from", record.from)
        .put("to", record.to)
        .put("toName", record.to_name ?: JSONObject.NULL)
        .put("value", record.value)
        .put("symbol", record.symbol)
        .put("decimals", record.decimals)
        .put("logoUrls", org.json.JSONArray(record.logo_urls))
        .put("chainId", record.chain_id)
        .put("timestamp", record.timestamp_s)
        .put("usd", record.usd ?: JSONObject.NULL)
        .put("type", "send")

    /** What the core hears when an arm threw: nothing was done. */
    fun neutralAnswer(operation: SendOperation): SendShellResult = when (operation) {
        is SendOperation.FetchTokens -> SendShellResult.TokensLoaded(tokens = null, chains = emptyList())
        is SendOperation.ClearTokenCache -> SendShellResult.TokenCacheCleared
        is SendOperation.ResolveTokenMetadata -> SendShellResult.TokenMetadata(null)
        is SendOperation.AddNetwork -> SendShellResult.NetworkAdded(SendAddNetworkOutcome.Error)
        is SendOperation.EstimateFee -> SendShellResult.FeeEstimated(SendFeeOutcome.Failed(SendEstimateFailure.Other))
        is SendOperation.ProbeTreasury -> SendShellResult.TreasuryProbed(SendTreasuryProbe.Unknown)
        is SendOperation.LoadAccountCredential -> SendShellResult.AccountCredential(null)
        is SendOperation.SubmitUserOp -> SendShellResult.SubmitFailed(SendSubmitFailure.Other(null))
        SendOperation.CancelPasskeySign -> SendShellResult.PasskeyCancelAcknowledged
        is SendOperation.PersistTxRecords -> SendShellResult.RecordsPersisted
        is SendOperation.TrackSubmitted -> SendShellResult.TrackHandedOff
        is SendOperation.ResolveIdentity -> SendShellResult.IdentityResolved(null)
        is SendOperation.ResolveRisk -> SendShellResult.RiskResolved(null)
        is SendOperation.SimulateCalls -> SendShellResult.SimResolved(null)
        is SendOperation.StartTimer -> SendShellResult.TimerElapsed(operation.tag)
        is SendOperation.Haptic -> SendShellResult.HapticPlayed
        is SendOperation.ShowAlert -> SendShellResult.AlertAcknowledged
        SendOperation.Close -> SendShellResult.Closed
    }

    internal companion object {
        /** The id half the core keys tokens on; the chain id is the whole story here. */
        fun network(chainId: Int) = "chain-$chainId"

        /** `fee_policy::TEMPO_DEFAULT_FEE_TOKEN` — pathUSD. */
        internal const val TEMPO_DEFAULT_FEE_TOKEN = "0x20c0000000000000000000000000000000000000"

        internal fun unhex(text: String): ByteArray = text.removePrefix("0x").chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
}
