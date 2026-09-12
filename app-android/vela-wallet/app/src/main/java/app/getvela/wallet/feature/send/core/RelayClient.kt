package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.net.VelaHttp
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.TrustReceiptLog
import java.math.BigInteger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.entryPointAddress
import uniffi.vela_core_uniffi.functionSelector

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
 * the settings machine's `clear_bundler_cache` means on this client.
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
        return port.restGet("$base$path", port.bestRpcUrl(chainId))
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

    private suspend fun bundlerCall(chainId: Int, method: String, params: List<Any?>): JSONObject? =
        (port.call(chainId, method, params, RpcKind.Bundler) as? RpcResult.Body)?.json

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

    private fun quoteRow(row: JSONObject): FeeAssetQuote? {
        val recipient = row.optString("recipient").takeIf(::isAddress) ?: return null
        val kind = when (row.optString("asset")) {
            "native" -> FeeAssetKind.Native
            "erc20" -> FeeAssetKind.Erc20
            else -> return null
        }
        val decimals = row.optInt("decimals", -1).takeIf { it >= 0 } ?: return null
        return FeeAssetQuote(
            recipient = recipient,
            asset = kind,
            fee_token = row.optString("feeToken").takeIf(::isAddress),
            balance = decimalText(row.opt("balance")) ?: return null,
            decimals = decimals,
            symbol = row.optString("symbol").ifBlank { return null },
            usd_balance = decimalText(row.opt("usdBalance")) ?: "0",
            usd_price = decimalText(row.opt("usdPrice")),
        )
    }

    /** `pimlico_getUserOperationGasPrice`, one tier. */
    suspend fun bundlerQuote(chainId: Int, tier: FeeTier): FeeBundlerQuote? {
        val body = bundlerCall(chainId, "pimlico_getUserOperationGasPrice", emptyList()) ?: return null
        if (body.has("error")) return null
        val row = body.optJSONObject("result")?.optJSONObject(tierKey(tier)) ?: return null
        return FeeBundlerQuote(
            max_fee_per_gas = decimalOfHex(row.opt("maxFeePerGas")) ?: return null,
            network_fee_per_gas = decimalOfHex(row.opt("networkFeePerGas")),
            relayer_fee_per_gas = decimalOfHex(row.opt("relayerFeePerGas")),
        )
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
    suspend fun sendUserOp(chainId: Int, opJson: String): SubmitAnswer {
        val op = JSONObject(opJson)
        VelaLog.event("relay.submit", "sending", "sender" to op.optString("sender"), "nonce" to op.optString("nonce"))
        var attempt = 0
        while (true) {
            val body = bundlerCall(chainId, "eth_sendUserOperation", listOf(op, entryPointAddress()))
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

    /** The three raw gas signals, each `null` when unreadable; the core prices with what it has. */
    suspend fun gasSignals(chainId: Int, wantTip: Boolean): GasSignals {
        val gasPrice = chainCall(chainId, "eth_gasPrice", emptyList())?.let { decimalOfHex(it.opt("result")) }
        val baseFee = chainCall(chainId, "eth_getBlockByNumber", listOf("latest", false))
            ?.optJSONObject("result")?.let { decimalOfHex(it.opt("baseFeePerGas")) }
        val tip = if (wantTip) {
            chainCall(chainId, "eth_maxPriorityFeePerGas", emptyList())?.let { decimalOfHex(it.opt("result")) }
        } else {
            null
        }
        return GasSignals(gasPrice, baseFee, tip)
    }

    /** `EntryPoint.getNonce(sender, 0)` as a hex QUANTITY; `null` when unreadable. */
    suspend fun nonce(chainId: Int, sender: String): String? {
        val data = "0x" + getNonceSelector + addressWord(sender) + "0".repeat(64)
        val body = chainCall(chainId, "eth_call", listOf(JSONObject().put("to", entryPointAddress()).put("data", data), "latest"))
            ?: return null
        val hex = body.optString("result").takeIf { it.startsWith("0x") && it.length > 2 } ?: return null
        return "0x" + BigInteger(hex.removePrefix("0x"), 16).toString(16)
    }

    /** `eth_getCode` != `0x`; `null` when the chain could not be asked. */
    suspend fun isDeployed(chainId: Int, address: String): Boolean? {
        val body = chainCall(chainId, "eth_getCode", listOf(address, "latest")) ?: return null
        val code = body.optString("result").takeIf { it.startsWith("0x") } ?: return null
        return code.length > 2
    }

    /** The settings machine's `clear_bundler_cache`, on this client. */
    fun clearCaches() {
        synchronized(quoteCache) { quoteCache.clear() }
        synchronized(infoCache) { infoCache.clear() }
    }

    private val getNonceSelector: String by lazy {
        functionSelector("getNonce(address,uint192)").joinToString("") { "%02x".format(it) }
    }

    private companion object {
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

        /** A decimal the relay wrote as a string or a number, as a decimal string. */
        fun decimalText(value: Any?): String? = when (value) {
            is String -> value.trim().takeIf { it.isNotEmpty() && it.all { c -> c.isDigit() || c == '.' } }
            is Number -> value.toString()
            else -> null
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
            .apply { if (!xRpcUrl.isNullOrBlank()) header("X-Rpc-Url", xRpcUrl) }
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
