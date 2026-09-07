package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.settings.core.NetView
import java.math.BigDecimal
import java.math.BigInteger
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.StateFlow
import org.json.JSONObject
import uniffi.vela_core_uniffi.isChainWithoutNativeCoin

/**
 * The only place the `balance_dashboard` core touches the outside world.
 *
 * Every judgement about money stays in `balance_dashboard.rs`: what the total
 * means, when a figure is stale, what "partial" is, which notice to show,
 * whether a holding can be counted at all. This class reads chains and reports
 * what it read.
 *
 * **Spec 041 phase 4b reads NATIVE COINS only.** ERC-20 holdings and prices
 * need a Multicall3 batch and DEX quotes in the same call — one request per
 * chain instead of one per token — and that is phase 4c. Until then every
 * token crosses with `price_usd = null`, the core reports the total as unknown
 * rather than as a number, and the screen says so. A wrong total would be worse
 * than the fixture it replaced; an honest "not yet" is not.
 */
class BalanceExecutor(
    private val pool: RpcPool,
    private val networks: StateFlow<NetView>,
    private val store: KeyValueStore,
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
) {

    /**
     * Where a chain's holdings go the moment they land, rather than when the
     * slowest chain finishes. The core folds them in; the screen fills in.
     *
     * Settable rather than a constructor argument because the cycle is real:
     * this executor emits events that the machine consumes, and the machine
     * cannot exist until its executor does. Assigned once, by the controller,
     * immediately after both halves are built.
     */
    var stream: (BalanceEvent) -> Unit = {}

    suspend fun perform(operation: BalanceOperation): BalanceShellResult = when (operation) {

        is BalanceOperation.FetchTokens -> fetchTokens(operation)

        // The switcher's rows: no streaming, no force. Same read, quieter.
        is BalanceOperation.FetchAccountAssets -> BalanceShellResult.AccountAssetsFetched(
            address = operation.address,
            tokens = runCatching { nativeHoldings(operation.address).tokens }.getOrNull(),
        )

        is BalanceOperation.ReadBalanceCache -> BalanceShellResult.CachedTotalLoaded(
            address = operation.address,
            usd = readCache()[operation.address.lowercase()],
        )

        is BalanceOperation.ReadBalanceCacheMany -> {
            val cache = readCache()
            BalanceShellResult.CachedBalancesLoaded(
                balances = operation.addresses.mapNotNull { address ->
                    cache[address.lowercase()]?.let { BalanceCacheEntry(address, it) }
                },
            )
        }

        is BalanceOperation.WriteBalanceCache -> {
            writeCache(operation.address, operation.usd)
            BalanceShellResult.BalanceCacheWritten
        }

        is BalanceOperation.StartRetryTimer -> {
            delay(operation.ms.toLong())
            BalanceShellResult.RetryElapsed(timer_id = operation.timer_id)
        }

        is BalanceOperation.WritePrivacy -> {
            store.write(KeyValueStore.Keys.BALANCE_HIDDEN, operation.hidden.toString())
            BalanceShellResult.PrivacyWritten
        }
    }

    /** What to answer when the shell itself threw. Exhaustive by construction. */
    fun neutralAnswer(operation: BalanceOperation): BalanceShellResult = when (operation) {
        // NOT "settled with nothing": the core keeps the last-known tokens and
        // total on an error, so a person watching the screen loses a skeleton
        // rather than their balances.
        is BalanceOperation.FetchTokens ->
            BalanceShellResult.FetchErrored(operation.address, operation.pull)
        is BalanceOperation.FetchAccountAssets ->
            BalanceShellResult.AccountAssetsFetched(operation.address, null)
        is BalanceOperation.ReadBalanceCache ->
            BalanceShellResult.CachedTotalLoaded(operation.address, null)
        is BalanceOperation.ReadBalanceCacheMany ->
            BalanceShellResult.CachedBalancesLoaded(emptyList())
        is BalanceOperation.WriteBalanceCache -> BalanceShellResult.BalanceCacheWritten
        is BalanceOperation.StartRetryTimer ->
            BalanceShellResult.RetryElapsed(operation.timer_id)
        is BalanceOperation.WritePrivacy -> BalanceShellResult.PrivacyWritten
    }

    /** The privacy choice this device remembers, for the machine's hydrate event. */
    suspend fun storedPrivacy(): Boolean =
        store.read(KeyValueStore.Keys.BALANCE_HIDDEN)?.toBooleanStrictOrNull() ?: false

    // -- reading chains ------------------------------------------------------

    private suspend fun fetchTokens(
        operation: BalanceOperation.FetchTokens,
    ): BalanceShellResult {
        val holdings = nativeHoldings(operation.address, streaming = true)
        // Every other machine in this app logs what it did; this one did not,
        // and the first device run could not tell a genuinely empty wallet from
        // twelve failed reads — both render as a total of zero from outside.
        VelaLog.event(
            "balance.fetch",
            "settled",
            "chains" to networks.value.networks.size,
            "held" to holdings.tokens.size,
            "failed" to holdings.failed.size,
            "rateLimited" to pool.view.value.rate_limited_chains.size,
        )
        return BalanceShellResult.FetchSettled(
            address = operation.address,
            pull = operation.pull,
            tokens = holdings.tokens,
            failed_chain_ids = holdings.failed,
            // The pool's own verdict, not a guess: a busy chain and a broken
            // one look identical from here without it.
            rate_limited_chain_ids = pool.view.value.rate_limited_chains,
            now_ms = now(),
        )
    }

    private class Holdings(
        val tokens: List<BalanceToken>,
        val failed: List<Int>,
    )

    /**
     * One chain's answer.
     *
     * `answered` is deliberately separate from whether a token came out: a
     * chain holding nothing answers perfectly well and produces no token. The
     * first version of this file conflated the two, so an empty chain was
     * reported as a FAILED chain — which puts a "this network is down" banner
     * in front of somebody whose only crime is having no funds there. Caught by
     * `aChainThatDidNotAnswerIsReportedAsFailedNotAsZero`.
     */
    private class ChainAnswer(
        val chainId: Int,
        val answered: Boolean,
        val token: BalanceToken?,
    )

    /**
     * One `eth_getBalance` per chain, concurrently.
     *
     * Chains are independent and slow; asking them one at a time would make the
     * home screen as slow as the sum of every endpoint rather than the slowest
     * one. The pool serialises nothing here — it routes each call on its own.
     */
    private suspend fun nativeHoldings(
        address: String,
        streaming: Boolean = false,
    ): Holdings = coroutineScope {
        val rows = networks.value.networks.filter { row ->
            // Tempo has no native coin — its gas is a TIP-20 stablecoin — and
            // its RPC answers the SAME constant for every address while calling
            // it `USD`. Querying it and letting a peg price that constant puts
            // ~4×10^57 dollars into somebody's total. The predicate is the
            // core's; this is not a "that looks too big" threshold.
            !isChainWithoutNativeCoin(row.chain_id.toUInt())
        }

        val results = rows.map { row ->
            async {
                val chainId = row.chain_id.toInt()
                when (val answer = pool.call(chainId, "eth_getBalance", listOf(address, "latest"))) {
                    is RpcResult.Body -> {
                        val token = nativeToken(
                            chainId,
                            row.native_symbol,
                            row.display_name,
                            answer.json,
                        )
                        if (token != null && streaming) {
                            stream(BalanceEvent.ChainAssetsArrived(address, listOf(token)))
                        }
                        ChainAnswer(chainId, answered = true, token = token)
                    }
                    // Both a failure and a rate limit mean "this chain did not
                    // answer". Which of the two it was is the pool's verdict,
                    // read once at settle rather than guessed per call.
                    is RpcResult.Failed, is RpcResult.RangeCapped ->
                        ChainAnswer(chainId, answered = false, token = null)
                }
            }
        }.awaitAll()

        Holdings(
            tokens = results.mapNotNull { it.token },
            failed = results.filterNot { it.answered }.map { it.chainId },
        )
    }

    /**
     * A native balance as the core wants it: a **human decimal string**.
     *
     * Not raw units. The core parses this straight into a float and multiplies
     * it by a price, so `"1500000000000000000"` where `"1.5"` belongs is a
     * total 10^18 times too large — and invisible until prices exist.
     */
    private fun nativeToken(
        chainId: Int,
        symbol: String,
        name: String,
        body: JSONObject,
    ): BalanceToken? {
        val hex = body.optString("result").takeIf { it.startsWith("0x") } ?: return null
        val raw = runCatching { BigInteger(hex.removePrefix("0x").ifEmpty { "0" }, 16) }
            .getOrNull() ?: return null
        if (raw.signum() == 0) return null

        val human = BigDecimal(raw).movePointLeft(NATIVE_DECIMALS).stripTrailingZeros()
        return BalanceToken(
            chain_id = chainId,
            symbol = symbol,
            name = name,
            balance = human.toPlainString(),
            decimals = NATIVE_DECIMALS,
            // `null` = this chain's native coin, which is what this whole
            // function reads.
            token_address = null,
            // live in 041 phase 4c — no price source until the multicall lands,
            // and the core renders an unpriced holding as unpriced rather than
            // as zero.
            price_usd = null,
            spam = false,
        )
    }

    // -- the caches ----------------------------------------------------------

    /** `vela.balanceCache`: `address → {usd, at}`, the Expo bytes. */
    private suspend fun readCache(): Map<String, Double> {
        val raw = store.read(KeyValueStore.Keys.BALANCE_CACHE) ?: return emptyMap()
        val record = runCatching { JSONObject(raw) }.getOrNull() ?: return emptyMap()
        return record.keys().asSequence().mapNotNull { address ->
            val entry = record.optJSONObject(address) ?: return@mapNotNull null
            val usd = entry.optDouble("usd", Double.NaN)
            if (usd.isNaN()) null else address.lowercase() to usd
        }.toMap()
    }

    private suspend fun writeCache(address: String, usd: Double) {
        val raw = store.read(KeyValueStore.Keys.BALANCE_CACHE)
        val record = raw?.let { runCatching { JSONObject(it) }.getOrNull() } ?: JSONObject()
        record.put(
            address.lowercase(),
            JSONObject().put("usd", usd).put("at", now().toLong()),
        )
        store.write(KeyValueStore.Keys.BALANCE_CACHE, record.toString())
    }

    private companion object {
        /**
         * Every EVM chain this wallet supports denominates its native coin in
         * 18 decimals. The one that does not have a native coin at all is
         * filtered out above by the core's own predicate.
         */
        const val NATIVE_DECIMALS = 18
    }
}
