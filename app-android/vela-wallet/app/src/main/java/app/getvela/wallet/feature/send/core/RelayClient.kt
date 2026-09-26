package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.net.VelaHttp
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.TrustReceiptLog
import java.math.BigInteger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.supervisorScope
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.bundlerQuoteCacheable
import uniffi.vela_core_uniffi.entryPointAddress
import uniffi.vela_core_uniffi.feeSignalsCacheTtlMs
import uniffi.vela_core_uniffi.functionSelector
import uniffi.vela_core_uniffi.gasSignalsCacheable
import uniffi.vela_core_uniffi.isChainWithoutNativeCoin

/**
 * The relay (bundler) and the chain, as the send path talks to them —
 * transport only (spec 043, research D8).
 *
 * Every JSON-RPC call goes through the pool, so bans and cooldowns apply to
 * the relay as they do to a chain endpoint; every REST call goes to the
 * bundler base THE POOL names for that chain, with the pool's best RPC URL
 * riding `X-Rpc-Url` so the relay reads the chain through the endpoint this
 * wallet trusts. No number is computed here: quantities cross as decimal
 * strings for the core to price, and the relay's sentences cross for the
 * core to classify.
 *
 * Two caches, both the web's: the in-band quotes for 8 s (a confirm screen
 * re-quotes on every fee-token tap and the relay would rather not be asked
 * four times a second), and the account info for 30 s. `clearCaches` is what
 * the settings machine's `clear_bundler_cache` means on this client. Beside
 * them the fee's own inputs (below), a deployed Safe (for good — code does not
 * go away), and the relay's simulation of one exact operation.
 *
 * Every read a fee quote makes is SINGLE-FLIGHT ([SingleFlight]): the session
 * for the speed in force and the previews of the other speeds ask the same
 * reads at the same instant, and they now share one request and settle
 * together instead of one after another (founder, 2026-09-26).
 */
class RelayClient(
    private val port: RelayPort,
    /** The configured relay host when the pool names no base for a chain. */
    private val builtinBase: () -> String,
    private val now: () -> Long = System::currentTimeMillis,
    /** The submit retry's pause; tests set zero. */
    private val retryDelayMs: Long = SUBMIT_RETRY_DELAY_MS,
) {

    // -- REST ------------------------------------------------------------------

    private suspend fun restGet(chainId: Int, path: String): RestAnswer {
        val base = (port.bundlerBase(chainId) ?: builtinBase()).trimEnd('/')
        val answer = port.restGet("$base$path", port.bestRpcUrl(chainId))
        VelaLog.event("relay.rest", path, "base" to base, "outcome" to answer::class.simpleName, "status" to (answer as? RestAnswer.Status)?.code)
        return answer
    }

    /** `GET /v1/treasury/{chain}` — 404 is "this chain is not covered", never "down". */
    suspend fun probeTreasury(chainId: Int): SendTreasuryProbe {
        val data = when (val answer = restGet(chainId, "/v1/treasury/$chainId")) {
            is RestAnswer.Ok -> answer.json
            is RestAnswer.Status -> return if (answer.code == 404) SendTreasuryProbe.Uncovered else SendTreasuryProbe.Unknown
            RestAnswer.Failed -> return SendTreasuryProbe.Unknown
        }
        val address = data.optString("address").takeIf(::isAddress) ?: return SendTreasuryProbe.Unknown
        val status = SendTreasuryStatus(
            chain_id = chainId,
            address = address,
            asset = if (data.optString("asset") == "pathUSD") SendTreasuryAsset.PathUsd else SendTreasuryAsset.Native,
            balance = bigHex(data.opt("balance")).toString(),
            floor = bigHex(data.opt("floor")).toString(),
            bootstrap_needed = data.optBoolean("bootstrapNeeded", false),
        )
        return if (status.bootstrap_needed) SendTreasuryProbe.LowFloat(status) else SendTreasuryProbe.Covered
    }

    /** What the relay knows about this account on this chain (`/v1/account`). */
    data class AccountInfo(
        val depositAddress: String,
        val settlementRecipient: String?,
        val status: String,
    ) {
        /** Where an in-band fee leg pays: the settlement recipient, else the deposit address. */
        fun feeRecipient(): String? = settlementRecipient ?: depositAddress.takeIf { it.isNotBlank() }
    }

    private val infoCache = HashMap<String, Pair<AccountInfo, Long>>()

    suspend fun accountInfo(chainId: Int, safe: String): AccountInfo? {
        val key = "$chainId:${safe.lowercase()}"
        synchronized(infoCache) {
            infoCache[key]?.let { (info, at) -> if (now() - at < INFO_TTL_MS) return info }
        }
        return infoFlights.run(key) { readAccountInfo(chainId, safe, key) }
    }

    private val infoFlights = SingleFlight<String, AccountInfo?>()

    private suspend fun readAccountInfo(chainId: Int, safe: String, key: String): AccountInfo? {
        val data = (restGet(chainId, "/v1/account/$chainId/${safe.lowercase()}") as? RestAnswer.Ok)?.json
            ?: return null
        val info = AccountInfo(
            depositAddress = data.optString("activeDepositAddress"),
            // A corrupted field degrades to the deposit-address fallback; it
            // must never poison the fee leg.
            settlementRecipient = data.optString("settlementRecipient").takeIf(::isAddress),
            status = data.optString("status").ifBlank { "UNKNOWN" },
        )
        synchronized(infoCache) { infoCache[key] = info to now() }
        return info
    }

    // -- bundler JSON-RPC -------------------------------------------------------

    private suspend fun bundlerCall(chainId: Int, method: String, params: List<Any?>): JSONObject? {
        val answer = port.call(chainId, method, params, RpcKind.Bundler)
        val body = (answer as? RpcResult.Body)?.json
        VelaLog.event(
            "relay.rpc", method,
            "chain" to chainId,
            "outcome" to answer::class.simpleName,
            "error" to body?.optJSONObject("error")?.optString("message")?.take(80),
        )
        return body
    }

    private val quoteCache = HashMap<String, Pair<List<FeeAssetQuote>, Long>>()

    /**
     * `vela_getInBandGasQuote`: the fee assets the relay accepts for this
     * account, with what it holds of each. `null` = the relay could not be
     * asked or answered nothing usable. Without a native USD price only the
     * payable native row survives, as on the web.
     */
    suspend fun inBandQuotes(chainId: Int, safe: String): List<FeeAssetQuote>? {
        val key = "$chainId:${safe.lowercase()}"
        synchronized(quoteCache) {
            quoteCache[key]?.let { (quotes, at) -> if (now() - at < QUOTE_TTL_MS) return quotes }
        }
        return quoteFlights.run(key) { readInBandQuotes(chainId, safe, key) }
    }

    private val quoteFlights = SingleFlight<String, List<FeeAssetQuote>?>()

    private suspend fun readInBandQuotes(chainId: Int, safe: String, key: String): List<FeeAssetQuote>? {
        val body = bundlerCall(chainId, "vela_getInBandGasQuote", listOf(JSONObject().put("safeAddress", safe)))
            ?: return null
        if (body.has("error")) return null
        val rows = body.optJSONArray("result") ?: return null
        val quotes = (0 until rows.length()).mapNotNull { rows.optJSONObject(it)?.let(::quoteRow) }
        val nativeUnpriced = quotes.firstOrNull { it.asset == FeeAssetKind.Native }?.usd_price == null
        val usable = if (nativeUnpriced) quotes.filter { it.asset == FeeAssetKind.Native } else quotes
        if (usable.isEmpty()) return null
        synchronized(quoteCache) { quoteCache[key] = usable to now() }
        return usable
    }

    /**
     * One `vela_getInBandGasQuote` row, or nothing if it is not well-formed —
     * the desktop's `parse_quote_row`, field for field. The relay writes
     * `balance` as HEX ("0x9f33…"), `feeToken` as JSON null for the native
     * row, the USD figures as decimal strings; the first device run parsed
     * the balance as a decimal and dropped every row.
     */
    private fun quoteRow(row: JSONObject): FeeAssetQuote? {
        val recipient = row.optString("recipient").takeIf(::isAddress) ?: return null
        val kind = when (row.optString("asset")) {
            "native" -> FeeAssetKind.Native
            "erc20" -> FeeAssetKind.Erc20
            else -> return null
        }
        val feeToken = row.optString("feeToken").takeIf(::isAddress)
        val decimals = row.optInt("decimals", -1).takeIf { it >= 0 } ?: return null
        val symbol = row.optString("symbol").trim().ifBlank { return null }
        val usdPrice = decimalText(row.opt("usdPrice"))
        // USD values are conversion metadata: native gas prices from gas units
        // alone, a stablecoin needs its own price or it cannot be converted.
        if (kind == FeeAssetKind.Erc20 && (feeToken == null || usdPrice == null)) return null
        return FeeAssetQuote(
            recipient = recipient,
            asset = kind,
            fee_token = if (kind == FeeAssetKind.Erc20) feeToken else null,
            balance = bigHex(row.opt("balance")).toString(),
            decimals = decimals,
            symbol = symbol,
            usd_balance = decimalText(row.opt("usdBalance")) ?: "0",
            usd_price = usdPrice,
        )
    }

    /**
     * `pimlico_getUserOperationGasPrice`, one tier. The relay answers EVERY
     * tier in one response, so the whole response is read once per chain —
     * for everybody asking at that moment — and each tier's row is held on
     * its own: the speed in force and the two previews price off one call.
     */
    suspend fun bundlerQuote(chainId: Int, tier: FeeTier): FeeBundlerQuote? {
        val key = "$chainId:${tierKey(tier)}"
        synchronized(feeSignalCache) {
            (feeSignalCache[key] as? Pair<*, *>)?.let { (quote, at) ->
                if (quote is FeeBundlerQuote && now() - (at as Long) < feeSignalsTtlMs) return quote
            }
        }
        // The epoch is part of the flight's key: a refresh asked while a read
        // is out starts a NEW read rather than joining the older one.
        val epoch = feeSignalEpoch(chainId)
        val rows = bundlerFlights.run(chainId to epoch) {
            val read = readBundlerQuotes(chainId)
            // Never a missing quote, nor a zero cap the core rejects as
            // degenerate: "the relay did not answer" is not a measurement. The
            // rule is the core's (`fee_policy::bundler_quote_cacheable`), row by row.
            if (read != null && feeSignalEpoch(chainId) == epoch) {
                synchronized(feeSignalCache) {
                    val at = now()
                    for ((name, quote) in read) {
                        if (bundlerQuoteCacheable(quote.max_fee_per_gas)) feeSignalCache["$chainId:$name"] = quote to at
                    }
                }
            }
            read
        }
        return rows?.get(tierKey(tier))
    }

    private val bundlerFlights = SingleFlight<Pair<Int, Long>, Map<String, FeeBundlerQuote>?>()

    /** Every tier row of one `pimlico_getUserOperationGasPrice` answer, by the relay's tier name. */
    private suspend fun readBundlerQuotes(chainId: Int): Map<String, FeeBundlerQuote>? {
        val body = bundlerCall(chainId, "pimlico_getUserOperationGasPrice", emptyList()) ?: return null
        if (body.has("error")) return null
        val result = body.optJSONObject("result") ?: return null
        val rows = LinkedHashMap<String, FeeBundlerQuote>()
        for (name in result.keys()) {
            val row = result.optJSONObject(name) ?: continue
            rows[name] = FeeBundlerQuote(
                max_fee_per_gas = decimalOfHex(row.opt("maxFeePerGas")) ?: continue,
                // The tip this tier is signed with — what the core turns into the
                // gas bid on screen (issue 684). Absent on a generic bundler.
                max_priority_fee_per_gas = decimalOfHex(row.opt("maxPriorityFeePerGas")),
                network_fee_per_gas = decimalOfHex(row.opt("networkFeePerGas")),
                relayer_fee_per_gas = decimalOfHex(row.opt("relayerFeePerGas")),
            )
        }
        return rows
    }

    // -- the fee's inputs, held still (issue 212; Android's since spec 069) --
    //
    // The fee session re-samples on every quote run, and since 069 up to three
    // sessions price one send at once, one per speed. Uncached, a recipient
    // edit re-rolled the gas price, and three tiers priced on three readings
    // could not be compared. A COMPLETE reading is held 15 s per chain — the
    // web's `fetchRawGasSignals` window — and dropped by the refresh control,
    // a failed quote and every submit ([invalidateFeeSignals]). The epoch
    // stops a read that was in flight when somebody asked for a fresh one
    // from landing its older answer in the cache.

    private val feeSignalCache = HashMap<String, Pair<Any, Long>>()
    private val feeSignalEpochs = HashMap<Int, Long>()

    private fun feeSignalEpoch(chainId: Int): Long = synchronized(feeSignalCache) { feeSignalEpochs[chainId] ?: 0L }

    /** Forget this chain's held readings, so the next quote run measures again. */
    fun invalidateFeeSignals(chainId: Int) {
        synchronized(feeSignalCache) {
            feeSignalEpochs[chainId] = (feeSignalEpochs[chainId] ?: 0L) + 1
            feeSignalCache.keys.removeAll { it.startsWith("$chainId:") }
        }
        synchronized(simulations) { simulations.keys.removeAll { it.chainId == chainId } }
    }

    // -- the relay's simulation of one exact operation --------------------------

    /** One exact operation: the chain, the account, whether it is deployed, and every call byte for byte. */
    data class SimulationKey(val chainId: Int, val account: String, val deployed: Boolean, val calls: List<FeeCall>)

    private val simulations = HashMap<SimulationKey, Pair<FeeGasOutcome, Long>>()
    private val simulationFlights = SingleFlight<Pair<SimulationKey, Long>, FeeGasOutcome>()

    /**
     * The relay's simulation of one exact operation, held for the core's
     * fee-signal window (`fee_policy::SIMULATION_CACHE_TTL_MS` is that same
     * window) and read ONCE for every session asking at the same moment.
     * Nothing the simulation measures depends on the speed, so the speed in
     * force and both previews price one operation off one simulation, settled
     * together. Only a real estimate is held — a refusal or a missing context
     * is asked again next time — and it goes with the chain's fee signals
     * ([invalidateFeeSignals]: the refresh control, every submit).
     */
    suspend fun simulation(
        chainId: Int,
        account: String,
        deployed: Boolean,
        calls: List<FeeCall>,
        simulate: suspend () -> FeeGasOutcome,
    ): FeeGasOutcome {
        val key = SimulationKey(
            chainId = chainId,
            account = account.lowercase(),
            deployed = deployed,
            calls = calls.map { FeeCall(to = it.to.lowercase(), value = it.value, data = it.data.lowercase()) },
        )
        synchronized(simulations) {
            simulations[key]?.let { (outcome, at) -> if (now() - at < feeSignalsTtlMs) return outcome }
        }
        val epoch = feeSignalEpoch(chainId)
        return simulationFlights.run(key to epoch) {
            val outcome = simulate()
            if (outcome is FeeGasOutcome.Estimated && feeSignalEpoch(chainId) == epoch) {
                synchronized(simulations) { simulations[key] = outcome to now() }
            }
            outcome
        }
    }

    // -- read ahead (spec 078: the picker is open) -------------------------------

    /**
     * Every read a first quote on these chains makes before its simulation,
     * all at once, each through the cache (and the single flight) the fee
     * session reads: the deployment, the gas signals, the relay's gas quote
     * (one call answers every tier) — on Tempo the fee recipient instead —
     * and the in-band rows. Measured on the live relay those are 3–5 s of a
     * first quote; read while the person is choosing a token, the quote the
     * pick starts is left with its simulation. Errors are nobody's business
     * here: the quote reads for itself.
     */
    suspend fun prewarmFees(account: String, chainIds: List<Int>, tier: FeeTier) {
        val started = now()
        supervisorScope {
            for (chainId in chainIds) {
                val tempo = isChainWithoutNativeCoin(chainId.toUInt())
                launch { runCatching { isDeployed(chainId, account) } }
                launch { runCatching { gasSignals(chainId, wantTip = !tempo) } }
                launch { runCatching { if (tempo) accountInfo(chainId, account) else bundlerQuote(chainId, tier) } }
                launch { runCatching { inBandQuotes(chainId, account) } }
            }
        }
        VelaLog.event("relay.prewarm", "warm", "chains" to chainIds.joinToString(","), "ms" to (now() - started))
    }

    sealed class EstimateAnswer {
        data class Estimated(
            val verificationGasLimit: String,
            val callGasLimit: String,
            val preVerificationGas: String,
        ) : EstimateAnswer()

        /** The relay answered and refused; `message` is its sentence, for the log. */
        data class Refused(val message: String) : EstimateAnswer()

        data object Unreachable : EstimateAnswer()
    }

    /** `eth_estimateUserOperationGas` for a draft's relay JSON. */
    suspend fun estimateUserOpGas(chainId: Int, opJson: String): EstimateAnswer {
        val body = bundlerCall(chainId, "eth_estimateUserOperationGas", listOf(JSONObject(opJson), entryPointAddress()))
            ?: return EstimateAnswer.Unreachable
        body.optJSONObject("error")?.let { error ->
            return EstimateAnswer.Refused(error.optString("message").ifBlank { "Gas estimation failed" })
        }
        val result = body.optJSONObject("result") ?: return EstimateAnswer.Refused("Failed to estimate gas — empty result")
        return EstimateAnswer.Estimated(
            verificationGasLimit = decimalOfHex(result.opt("verificationGasLimit")) ?: return EstimateAnswer.Refused("verificationGasLimit missing"),
            callGasLimit = decimalOfHex(result.opt("callGasLimit")) ?: return EstimateAnswer.Refused("callGasLimit missing"),
            preVerificationGas = decimalOfHex(result.opt("preVerificationGas")) ?: return EstimateAnswer.Refused("preVerificationGas missing"),
        )
    }

    sealed class SubmitAnswer {
        data class Accepted(val userOpHash: String) : SubmitAnswer()

        /** The relay's `error` member as JSON — the core turns it into a sentence and a class. */
        data class Rejected(val errorJson: String) : SubmitAnswer()

        data object Unreachable : SubmitAnswer()
    }

    /**
     * `eth_sendUserOperation`, retried up to three times while the relay says
     * it is busy ("currently processing", "Retry later") — the web's loop.
     */
    /**
     * `[userOperation, entryPoint, tier?]` — the relay's wire since it learned
     * about speed (spec 068; Android's since 069). The tier is a NAME, never a
     * wei figure: the relay resolves it at submit time and clamps it between
     * its inclusion floor and what the signed reimbursement funds. `null`
     * sends the pre-068 two-element params exactly, and the dead `rapid` is
     * never sent — a relay refuses an unknown name with -32602.
     */
    suspend fun sendUserOp(chainId: Int, opJson: String, tier: FeeTier? = null): SubmitAnswer {
        val op = JSONObject(opJson)
        VelaLog.event("relay.submit", "sending", "sender" to op.optString("sender"), "nonce" to op.optString("nonce"), "tier" to (tier?.let(::tierKey) ?: "-"))
        val params = submitParams(op, tier)
        var attempt = 0
        while (true) {
            val body = bundlerCall(chainId, "eth_sendUserOperation", params)
                ?: return SubmitAnswer.Unreachable
            body.optString("result").takeIf { it.startsWith("0x") }?.let { return SubmitAnswer.Accepted(it) }
            val error = body.opt("error")
            val message = (error as? JSONObject)?.optString("message").orEmpty()
            val retryable = message.contains("currently processing") || message.contains("Retry later")
            if (!retryable || attempt == SUBMIT_MAX_RETRIES) {
                return SubmitAnswer.Rejected(error?.toString() ?: "null")
            }
            attempt += 1
            VelaLog.event("relay.submit", "busy", "retry" to attempt)
            delay(retryDelayMs)
        }
    }

    sealed class ReceiptAnswer {
        data object Unreachable : ReceiptAnswer()

        data object Pending : ReceiptAnswer()

        data class Resolved(
            val confirmed: Boolean,
            val txHash: String,
            val sender: String?,
            val logs: List<TrustReceiptLog>,
        ) : ReceiptAnswer()
    }

    /** `eth_getUserOperationReceipt`: unreachable, not yet, or landed (with its logs). */
    suspend fun userOpReceipt(chainId: Int, userOpHash: String): ReceiptAnswer {
        if (userOpHash.isBlank()) return ReceiptAnswer.Unreachable
        val body = bundlerCall(chainId, "eth_getUserOperationReceipt", listOf(userOpHash)) ?: return ReceiptAnswer.Unreachable
        if (body.has("error")) return ReceiptAnswer.Unreachable
        val result = body.optJSONObject("result") ?: return ReceiptAnswer.Pending
        val receipt = result.optJSONObject("receipt") ?: return ReceiptAnswer.Pending
        val txHash = receipt.optString("transactionHash").takeIf { it.startsWith("0x") } ?: return ReceiptAnswer.Pending
        val logs = receipt.optJSONArray("logs")?.let { array ->
            (0 until array.length()).mapNotNull { array.optJSONObject(it)?.let(::trustLog) }
        }.orEmpty()
        return ReceiptAnswer.Resolved(
            confirmed = !(result.has("success") && !result.optBoolean("success", true)),
            txHash = txHash,
            sender = result.optString("sender").takeIf { it.isNotBlank() },
            logs = logs,
        )
    }

    private fun trustLog(log: JSONObject): TrustReceiptLog? {
        val address = log.optString("address").takeIf { it.isNotBlank() } ?: return null
        val topics = log.optJSONArray("topics") ?: return null
        return TrustReceiptLog(
            address = address,
            topics = (0 until topics.length()).mapNotNull { topics.optString(it).ifBlank { null } },
            data = log.optString("data").ifBlank { "0x" },
        )
    }

    /** `eth_getUserOperationStatus` (a Vela extension): `null` for an older relay or a failure. */
    suspend fun userOpStatus(chainId: Int, userOpHash: String): Pair<TrackLifecycle, String?>? {
        if (userOpHash.isBlank()) return null
        val body = bundlerCall(chainId, "eth_getUserOperationStatus", listOf(userOpHash)) ?: return null
        if (body.has("error")) return null
        val result = body.optJSONObject("result") ?: return null
        val status = when (result.optString("status")) {
            "not_found" -> TrackLifecycle.NotFound
            "queued" -> TrackLifecycle.Queued
            "not_submitted" -> TrackLifecycle.NotSubmitted
            "submitted" -> TrackLifecycle.Submitted
            "rejected" -> TrackLifecycle.Rejected
            "included" -> TrackLifecycle.Included
            "failed" -> TrackLifecycle.Failed
            else -> return null
        }
        return status to result.optString("last_executor_stage").takeIf { it.isNotBlank() }
    }

    // -- chain reads the send path needs ------------------------------------------

    private suspend fun chainCall(chainId: Int, method: String, params: List<Any?>): JSONObject? =
        (port.call(chainId, method, params, RpcKind.Rpc) as? RpcResult.Body)?.json

    data class GasSignals(val ethGasPrice: String?, val baseFee: String?, val priorityFee: String?)

    /**
     * The three raw gas signals, each `null` when unreadable; the core prices
     * with what it has. Held 15 s per chain when the reading is COMPLETE —
     * every leg that was asked for answered, and the price is positive — and
     * never otherwise (see [invalidateFeeSignals]).
     */
    suspend fun gasSignals(chainId: Int, wantTip: Boolean): GasSignals {
        val key = "$chainId:gas:$wantTip"
        synchronized(feeSignalCache) {
            (feeSignalCache[key] as? Pair<*, *>)?.let { (signals, at) ->
                if (signals is GasSignals && now() - (at as Long) < feeSignalsTtlMs) return signals
            }
        }
        val epoch = feeSignalEpoch(chainId)
        return gasFlights.run(Triple(chainId, wantTip, epoch)) { readGasSignals(chainId, wantTip, key, epoch) }
    }

    private val gasFlights = SingleFlight<Triple<Int, Boolean, Long>, GasSignals>()

    private suspend fun readGasSignals(chainId: Int, wantTip: Boolean, key: String, epoch: Long): GasSignals = coroutineScope {
        // Three independent reads, side by side: one round trip, not three.
        val gasPriceRead = async { chainCall(chainId, "eth_gasPrice", emptyList())?.let { decimalOfHex(it.opt("result")) } }
        val blockRead = async { chainCall(chainId, "eth_getBlockByNumber", listOf("latest", false))?.optJSONObject("result") }
        val tipRead = async {
            if (wantTip) chainCall(chainId, "eth_maxPriorityFeePerGas", emptyList())?.let { decimalOfHex(it.opt("result")) } else null
        }
        val gasPrice = gasPriceRead.await()
        val block = blockRead.await()
        val baseFee = block?.let { decimalOfHex(it.opt("baseFeePerGas")) }
        val tip = tipRead.await()
        val signals = GasSignals(gasPrice, baseFee, tip)
        // A block that ANSWERED without `baseFeePerGas` is a real pre-London
        // reading; a block that did not answer is a failed leg. What may be
        // held is the core's rule (`fee_policy::gas_signals_cacheable`).
        val complete = gasSignalsCacheable(gasPrice, block != null, wantTip, tip)
        if (complete && feeSignalEpoch(chainId) == epoch) {
            synchronized(feeSignalCache) { feeSignalCache[key] = signals to now() }
        }
        signals
    }

    /** `EntryPoint.getNonce(sender, 0)` as a hex QUANTITY; `null` when unreadable. */
    suspend fun nonce(chainId: Int, sender: String): String? {
        val data = "0x" + getNonceSelector + addressWord(sender) + "0".repeat(64)
        val body = chainCall(chainId, "eth_call", listOf(JSONObject().put("to", entryPointAddress()).put("data", data), "latest"))
            ?: return null
        val hex = body.optString("result").takeIf { it.startsWith("0x") && it.length > 2 } ?: return null
        return "0x" + BigInteger(hex.removePrefix("0x"), 16).toString(16)
    }

    /**
     * `eth_getCode` != `0x`; `null` when the chain could not be asked. A Safe
     * seen deployed stays deployed — code does not go away — so that answer is
     * held for good; "not deployed" and "unknown" never are (the first send
     * deploys it).
     */
    suspend fun isDeployed(chainId: Int, address: String): Boolean? {
        val key = "$chainId:${address.lowercase()}"
        synchronized(deployedSafes) { if (key in deployedSafes) return true }
        return deployFlights.run(key) { readDeployed(chainId, address, key) }
    }

    private val deployedSafes = HashSet<String>()
    private val deployFlights = SingleFlight<String, Boolean?>()

    private suspend fun readDeployed(chainId: Int, address: String, key: String): Boolean? {
        val body = chainCall(chainId, "eth_getCode", listOf(address, "latest")) ?: return null
        val code = body.optString("result").takeIf { it.startsWith("0x") } ?: return null
        val deployed = code.length > 2
        if (deployed) synchronized(deployedSafes) { deployedSafes.add(key) }
        return deployed
    }

    /**
     * The recipient-risk answer's `is_contract` — NOT [isDeployed], which asks
     * whether the SENDER's Safe exists. The web's `isContractAddress`,
     * verbatim: an EIP-7702 delegated EOA carries code `0xef0100 ++ implAddr`
     * (exactly 23 bytes) and is a WALLET with smart-account features — a
     * person's account, never badged "contract". Any other code is a
     * contract; `null` when the chain could not be asked.
     */
    suspend fun isContract(chainId: Int, address: String): Boolean? {
        val code = chainCall(chainId, "eth_getCode", listOf(address, "latest"))
            ?.takeUnless { it.has("error") }
            ?.opt("result") as? String ?: return null
        if (EIP7702_DESIGNATOR.matches(code)) return false
        return code != "0x" && code.length > 2
    }

    /** The settings machine's `clear_bundler_cache`, on this client. */
    fun clearCaches() {
        synchronized(quoteCache) { quoteCache.clear() }
        synchronized(infoCache) { infoCache.clear() }
    }

    /** The core's fee-signal window (`fee_policy::FEE_SIGNALS_CACHE_TTL_MS`). */
    private val feeSignalsTtlMs: Long by lazy { feeSignalsCacheTtlMs().toLong() }

    private val getNonceSelector: String by lazy {
        functionSelector("getNonce(address,uint192)").joinToString("") { "%02x".format(it) }
    }

    private companion object {
        val EIP7702_DESIGNATOR = Regex("^0xef0100[0-9a-f]{40}$", RegexOption.IGNORE_CASE)
        const val QUOTE_TTL_MS = 8_000L
        const val INFO_TTL_MS = 30_000L

        const val SUBMIT_MAX_RETRIES = 3
        const val SUBMIT_RETRY_DELAY_MS = 3_000L

        fun tierKey(tier: FeeTier) = when (tier) {
            FeeTier.Slow -> "slow"
            FeeTier.Standard -> "standard"
            FeeTier.Rapid -> "rapid"
            FeeTier.Fast -> "fast"
        }

        fun isAddress(value: String?): Boolean =
            value != null && value.length == 42 && value.startsWith("0x") && value.drop(2).all { it.isDigit() || it.lowercaseChar() in 'a'..'f' }

        fun addressWord(address: String): String = address.removePrefix("0x").lowercase().padStart(64, '0')

        /** `parseBigIntHex`: a `0x` hex, a bare hex, or a JSON number; anything else is zero. */
        fun bigHex(value: Any?): BigInteger = when (value) {
            is Number -> BigInteger.valueOf(value.toLong()).max(BigInteger.ZERO)
            is String -> runCatching {
                val clean = value.removePrefix("0x").ifEmpty { "0" }
                BigInteger(clean, 16)
            }.getOrDefault(BigInteger.ZERO).max(BigInteger.ZERO)
            else -> BigInteger.ZERO
        }

        /** A hex QUANTITY as a decimal string; `null` when absent or not hex. */
        fun decimalOfHex(value: Any?): String? {
            val text = value as? String ?: return null
            if (!text.startsWith("0x")) return null
            return runCatching { BigInteger(text.removePrefix("0x").ifEmpty { "0" }, 16).toString() }.getOrNull()
        }

        /** A decimal the relay wrote as a string or a number, as a decimal string (`decimal_text`). */
        fun decimalText(value: Any?): String? {
            val raw = when (value) {
                is String -> value.trim()
                is Number -> value.toString()
                else -> return null
            }
            val parsed = raw.toDoubleOrNull() ?: return null
            return raw.takeIf { parsed.isFinite() && parsed >= 0.0 }
        }
    }
}

/** The relay client's view of the pool and the network — injected so tests script answers. */
interface RelayPort {
    suspend fun call(chainId: Int, method: String, params: List<Any?>, kind: RpcKind): RpcResult

    suspend fun bundlerBase(chainId: Int): String?

    suspend fun bestRpcUrl(chainId: Int): String?

    suspend fun restGet(url: String, xRpcUrl: String?): RestAnswer
}

sealed class RestAnswer {
    data class Ok(val json: JSONObject) : RestAnswer()

    data class Status(val code: Int) : RestAnswer()

    data object Failed : RestAnswer()
}

/** The real port: the pool for JSON-RPC and routing, the one HTTP client for REST. */
class PoolRelayPort(
    private val pool: RpcPool,
    private val http: OkHttpClient = VelaHttp.client,
) : RelayPort {
    override suspend fun call(chainId: Int, method: String, params: List<Any?>, kind: RpcKind): RpcResult =
        pool.call(chainId, method, params, kind)

    override suspend fun bundlerBase(chainId: Int): String? = pool.bundlerBase(chainId)

    override suspend fun bestRpcUrl(chainId: Int): String? = pool.bestRpcUrl(chainId)

    override suspend fun restGet(url: String, xRpcUrl: String?): RestAnswer = withContext(Dispatchers.IO) {
        val request = Request.Builder().url(url).get().header("accept", "application/json")
            // Spec 081 FR-007: see RpcPoolExecutor — the endpoint stays local.
            .apply { @Suppress("UNUSED_EXPRESSION") xRpcUrl }
            .build()
        runCatching {
            http.newCall(request).execute().use { response ->
                if (!response.isSuccessful) return@use RestAnswer.Status(response.code)
                val text = response.body?.string().orEmpty()
                runCatching { RestAnswer.Ok(JSONObject(text)) }.getOrDefault(RestAnswer.Failed)
            }
        }.getOrDefault(RestAnswer.Failed)
    }
}

/**
 * The name a submission gives its speed, or `null` to name none. Only the
 * three offered names ever reach the relay: `rapid` is dead (spec 068) and a
 * relay refuses an unknown name before any handler runs.
 */
internal fun wireTierName(tier: FeeTier?): String? = when (tier) {
    FeeTier.Fast -> "fast"
    FeeTier.Standard -> "standard"
    FeeTier.Slow -> "slow"
    FeeTier.Rapid, null -> null
}

/** `eth_sendUserOperation`'s params: two elements, or three with a speed. */
internal fun submitParams(op: JSONObject, tier: FeeTier?): List<Any> {
    val name = wireTierName(tier)
    return if (name == null) listOf(op, entryPointAddress()) else listOf(op, entryPointAddress(), name)
}
