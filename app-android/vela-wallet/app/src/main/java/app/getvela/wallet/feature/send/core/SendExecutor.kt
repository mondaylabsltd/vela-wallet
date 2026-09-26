package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.wallet.core.Abi
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.HoldingsFeed
import app.getvela.wallet.feature.wallet.core.HoldingsRound
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.settings.core.NetView
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
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
    /** Spec 071: the Trusted Signer, for an account that signed in through it. */
    private val trustedSigner: () -> TrustedSigner? = { null },
    /**
     * Spec 078: the asset list's holdings — `fetch_tokens` is answered from
     * the round the balance machine settled, never a second walk of every
     * chain. `null` (tests) answers from [balances] as it stands.
     */
    private val holdings: HoldingsFeed? = null,
    /**
     * Spec 078: read ahead what a quote on these chains will need, in the
     * background, into the relay client's caches. Fire-and-forget.
     */
    private val prewarm: (account: String, chainIds: List<Int>) -> Unit = { _, _ -> },
) {

    /** The account store, as the send path reads it. */
    interface AccountPort {
        /** The wallet's founding keys for `address`, pinned key first; empty when unknown. */
        suspend fun keysOf(address: String): List<WalletKeyRecord>

        /** The pinned key's stored transports and method, for the ceremony's routing. */
        suspend fun routingOf(address: String): Pair<String, KeyMethod>

        suspend fun publicKeyOf(accountId: String): String?

        /**
         * Every founding key's credential id and stored transports, as JSON for
         * the core's `sign_route` — `[{credential_id, transports}]`. The default
         * is "nothing known", which routes as it always did.
         */
        suspend fun keyRoutesJson(address: String): String = "[]"

        /**
         * The stored account record for `address`, whole — what the core's
         * `signInRoute` reads the account's sign-in key from. `null` when
         * unknown, which signs as a record without one does.
         */
        suspend fun accountJson(address: String): String? = null
    }

    /** The live fee session, as `estimate_fee` needs it (research D7). */
    interface FeeQuoter {
        suspend fun quote(
            chainId: Int,
            account: String,
            calls: List<FeeCall>,
            gasFeeToken: String?,
            publicKeyAvailable: Boolean,
            /** Nobody chose the coin: the fee machine pays in one that can (`estimate_fee.auto_fee_token`). */
            autoFeeToken: Boolean,
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

        /** A first round is still streaming in: what has arrived so far, display-only (`TokensPartial`). */
        fun tokensPartial(tokens: List<SendToken>) {}

        /** `fetch_tokens` is being answered from THIS settled round — later rounds follow as `HoldingsUpdated`. */
        fun holdingsHanded(round: HoldingsRound?) {}
    }

    @Volatile
    private var signing: Job? = null

    suspend fun perform(operation: SendOperation): SendShellResult = when (operation) {
        is SendOperation.FetchTokens -> fetchTokens(operation.address)
        // The asset list reads again; its new round reaches this screen as
        // `HoldingsUpdated` (the `fetch_tokens` beside this answers from the
        // round in hand, so the picker never blanks).
        is SendOperation.ClearTokenCache -> {
            ports.refreshBalances()
            SendShellResult.TokenCacheCleared
        }
        is SendOperation.PrewarmFees -> {
            prewarm(operation.account, operation.chain_ids)
            SendShellResult.FeesPrewarmed
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
                    autoFeeToken = operation.auto_fee_token,
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
        // `first_time` from this device's own send history (the web's
        // `resolveRecipientRisk`): an address it never sent to wears the
        // "first time" tag, the poisoning defence. Not an address → `false`.
        is SendOperation.ResolveRisk -> SendShellResult.RiskResolved(
            SendRecipientRisk(
                is_contract = relay.isContract(operation.chain_id, operation.address),
                first_time = firstTime(operation.address),
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

    /** `true` = never sent to this address from this device; a non-address is never "first". */
    private suspend fun firstTime(address: String): Boolean =
        ADDRESS.matches(address) && !feed.hasSentTo(address)

    // -- tokens -----------------------------------------------------------------

    /**
     * The holdings the balance machine already read — no second walk, and the
     * SAME numbers the asset list shows (spec 078). Answered from the round
     * the dashboard settled for this account; when none has settled yet, the
     * answer waits for it, showing what has streamed in meanwhile
     * (`TokensPartial`) rather than answering "nothing held" for a wallet
     * nobody has finished reading. A round older than [HOLDINGS_FRESH_MS] is
     * shown at once and read again behind it; the new round follows as
     * `HoldingsUpdated`.
     */
    private suspend fun fetchTokens(address: String): SendShellResult {
        val feed = holdings ?: return tokensLoaded(balances())
        val ready = feed.settled.value?.takeIf { it.address.equals(address, ignoreCase = true) }
        var wait = if (ready != null) HoldingsWait.Settled(ready) else awaitRound(feed, address, after = null, acceptUnreachable = true)
        // One source has one failure mode: a round in which no chain answered
        // — a proxy blip at launch — used to become "could not load tokens"
        // at once, and the picker stayed empty until the next ten-minute poll.
        // Ask the dashboard to read again first (the asset list recovers with
        // it) and answer whatever the NEXT round holds; only a second round
        // that reached nothing is the refusal (the desktop's rule).
        if (wait != null && reachedNothing(wait)) {
            val met = (wait as? HoldingsWait.Settled)?.round?.atMs
            VelaLog.event("send.tokens", "round reached nothing, reading again once", "met" to (met != null))
            ports.refreshBalances()
            wait = awaitRound(feed, address, after = met, acceptUnreachable = false)
        }
        return when (wait) {
            is HoldingsWait.Settled -> {
                // Recorded BEFORE the answer leaves, so a round settling
                // after this one is handed over and this one is not twice.
                ports.holdingsHanded(wait.round)
                if (now() - wait.round.atMs > HOLDINGS_FRESH_MS) ports.refreshBalances()
                tokensLoaded(wait.round.view)
            }
            HoldingsWait.Unreachable -> SendShellResult.TokensLoaded(tokens = null, chains = chains())
            null -> {
                // Timed out: what has arrived for this account, or the failure.
                val view = feed.view.value.takeIf { it.address.equals(address, ignoreCase = true) && it.tokens.isNotEmpty() }
                VelaLog.event("send.tokens", "no settled round in time", "partial" to (view?.tokens?.size ?: 0))
                ports.holdingsHanded(null)
                view?.let(::tokensLoaded) ?: SendShellResult.TokensLoaded(tokens = null, chains = chains())
            }
        }
    }

    private sealed class HoldingsWait {
        data class Settled(val round: HoldingsRound) : HoldingsWait()
        data object Unreachable : HoldingsWait()
    }

    /** Nothing held AND something failed, or nothing could be read at all — what [tokensLoaded] would refuse. */
    private fun reachedNothing(wait: HoldingsWait): Boolean = when (wait) {
        is HoldingsWait.Settled -> sendTokens(wait.round.view).isEmpty() && wait.round.view.failed_chain_ids.isNotEmpty()
        HoldingsWait.Unreachable -> true
    }

    /**
     * A settled round for `address` — newer than [after] when given — or the
     * machine's "nothing could be read" (only when [acceptUnreachable]), or
     * `null` on timeout. While the first round is out, what has streamed in
     * shows as `TokensPartial`.
     */
    private suspend fun awaitRound(
        feed: HoldingsFeed,
        address: String,
        after: Double?,
        acceptUnreachable: Boolean,
    ): HoldingsWait? = withTimeoutOrNull(HOLDINGS_WAIT_MS) {
        coroutineScope {
            val streaming = launch {
                var last: List<SendToken>? = null
                feed.view.collect { view ->
                    if (!view.address.equals(address, ignoreCase = true) || view.tokens.isEmpty()) return@collect
                    val tokens = sendTokens(view)
                    if (tokens != last) {
                        last = tokens
                        ports.tokensPartial(tokens)
                    }
                }
            }
            try {
                combine(feed.settled, feed.view) { round, view ->
                    when {
                        round != null && round.address.equals(address, ignoreCase = true) && round.atMs != after ->
                            HoldingsWait.Settled(round)
                        acceptUnreachable && view.address.equals(address, ignoreCase = true) && view.unreachable ->
                            HoldingsWait.Unreachable
                        else -> null
                    }
                }.filterNotNull().first()
            } finally {
                streaming.cancel()
            }
        }
    }

    /**
     * One round as the picker's answer. Nothing came back and something
     * failed: the load failed — an empty wallet on reachable chains is an
     * empty list, not an error (the desktop's `tokens_loaded`).
     */
    private fun tokensLoaded(view: BalanceView): SendShellResult {
        val tokens = sendTokens(view)
        return SendShellResult.TokensLoaded(
            tokens = if (tokens.isEmpty() && view.failed_chain_ids.isNotEmpty()) null else tokens,
            chains = chains(),
        )
    }

    private fun chains(): List<SendChainInfo> = networks().networks.map { row ->
        SendChainInfo(chain_id = row.chain_id.toInt(), network = network(row.chain_id.toInt()), native_symbol = row.native_symbol)
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

    private val spine = UserOpSpine(relay, accounts, signer, measureCall = { chainId, from, to, valueHex, data ->
        (pool.call(chainId, "eth_estimateGas", listOf(JSONObject().put("from", from).put("to", to).put("value", valueHex).put("data", data))) as? RpcResult.Body)
            ?.json?.takeIf { it.has("result") && !it.isNull("result") }?.optString("result")?.takeIf { it.startsWith("0x") }
    }, trustedSigner = trustedSigner)

    /** The spine (spec 044 T028): one implementation for a person's transfer and a dApp's transaction. */
    private suspend fun submitInner(op: SendOperation.SubmitUserOp): String = try {
        spine.submit(
            chainId = op.chain_id,
            account = op.account,
            calls = op.calls.map { UserOpCall(to = it.to, value = it.value, data = it.data) },
            gasFeeToken = op.gas_fee_token,
            quotedFee = op.quoted_fee?.let { UserOpSpine.Quoted(it.amount, it.recipient, it.tier) },
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
        is SendOperation.PrewarmFees -> SendShellResult.FeesPrewarmed
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

        /** How long `fetch_tokens` waits for the dashboard's first round of an account. */
        const val HOLDINGS_WAIT_MS = 30_000L

        /** A settled round older than this is shown at once and read again behind (the desktop's window). */
        const val HOLDINGS_FRESH_MS = 5.0 * 60.0 * 1000.0

        /**
         * The asset list's holdings as Send's tokens — ONE mapping for the
         * `fetch_tokens` answer and every `HoldingsUpdated`. The dashboard's
         * `tokens` already holds its unpriced rows (`unpriced_tokens` is a
         * subset of them, for the detail sheet), so the two are joined by
         * holding and a row is never listed twice. No `logo_urls`: the asset
         * list has none either, and both draw the same mark from the chain,
         * symbol and contract (`Marks.tokenMark`).
         */
        fun sendTokens(view: BalanceView): List<SendToken> =
            (view.tokens + view.unpriced_tokens)
                .distinctBy { "${it.chain_id}:${it.token_address?.lowercase() ?: "native"}" }
                .map { token ->
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

        /** `fee_policy::TEMPO_DEFAULT_FEE_TOKEN` — pathUSD. */
        internal const val TEMPO_DEFAULT_FEE_TOKEN = "0x20c0000000000000000000000000000000000000"

        /** `^0x[0-9a-fA-F]{40}$` — the web's recipient-risk address test. */
        private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

        internal fun unhex(text: String): ByteArray = text.removePrefix("0x").chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
}
