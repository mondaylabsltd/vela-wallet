package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import java.math.BigDecimal
import java.math.BigInteger
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.StateFlow
import org.json.JSONObject
import uniffi.vela_core_uniffi.NativeQuoteGroup
import uniffi.vela_core_uniffi.bestNativeDexPrice
import uniffi.vela_core_uniffi.chooseNativePrice
import uniffi.vela_core_uniffi.isChainWithoutNativeCoin

/**
 * The only place the `balance_dashboard` core touches the outside world.
 *
 * Every judgement about money stays in `balance_dashboard.rs`: what the total
 * means, when a figure is stale, what "partial" is, which notice to show,
 * whether a holding can be counted at all. This class reads chains and reports
 * what it read.
 *
 * **One request per chain.** Native balance, every stablecoin balance, the
 * wrapped native token, the DEX quotes that price the coin and the chain's own
 * Chainlink feed all ride in a single `aggregate3`. Twelve chains cost twelve
 * requests, not a hundred and twenty, and the home screen is as slow as the
 * slowest chain rather than the sum of them.
 *
 * **Where the price rules are.** Not here. Which pool wins inside a quote
 * token, which stable wins across them, and whether a DEX quote is trustworthy
 * enough to beat Chainlink are `vela_core::app::balance_dashboard`, called over
 * the bridge. This class decodes words and asks.
 *
 * Custom ERC-20s are not priced yet: their rule (`firstGroupedQuotePrice`)
 * exists only in the web's TypeScript and has no owner in the core. It gets one
 * before Android reads it. // live in 041 phase 4d
 */
class BalanceExecutor(
    private val pool: RpcPool,
    private val networks: StateFlow<NetView>,
    private val store: KeyValueStore,
    /** What a chain holds and how to price it; `null` = its document is unavailable. */
    private val chainInfo: suspend (Int) -> ChainInfo? = { null },
    /** Chainlink's mainnet feeds, the last rung of the ladder. */
    private val mainnetPrices: suspend () -> Map<String, Double> = { emptyMap() },
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
            tokens = runCatching { holdings(operation.address).tokens }.getOrNull(),
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
        val holdings = holdings(operation.address, streaming = true)
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
        val tokens: List<BalanceToken>,
    )

    /** What a slot in the batch is, which decides how it gets priced. */
    private enum class Kind { Native, Stable, Wrapped }

    private class Slot(
        val kind: Kind,
        val symbol: String,
        val name: String,
        val contract: String?,
        val knownDecimals: Int?,
        val balanceIndex: Int,
        val decimalsIndex: Int?,
    )

    /**
     * Every chain at once.
     *
     * Chains are independent and slow; asking them one at a time would make the
     * home screen as slow as the sum of every endpoint rather than the slowest
     * one. The pool serialises nothing — it routes each call on its own.
     *
     * The mainnet Chainlink batch is fetched once up front, not per chain: it
     * is the same five feeds whoever is asking.
     */
    private suspend fun holdings(
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

        val chainlinkUsd = runCatching { mainnetPrices() }.getOrDefault(emptyMap())

        val results = rows.map { row ->
            async {
                val answer = readChain(address, row.chain_id.toInt(), row, chainlinkUsd)
                if (streaming && answer.tokens.isNotEmpty()) {
                    stream(BalanceEvent.ChainAssetsArrived(address, answer.tokens))
                }
                answer
            }
        }.awaitAll()

        Holdings(
            tokens = results.flatMap { it.tokens },
            failed = results.filterNot { it.answered }.map { it.chainId },
        )
    }

    /**
     * One chain, one round trip.
     *
     * The batch carries: the native balance, each stablecoin's balance and
     * `decimals()`, the wrapped native token's, a DEX quote of one whole coin
     * against every stable, and the chain's own Chainlink feed if it has one.
     * Positions are recorded as the calls are appended, because a result is
     * matched to its call by index and nothing else.
     */
    private suspend fun readChain(
        address: String,
        chainId: Int,
        row: NetNetworkRow,
        mainnetPrices: Map<String, Double>,
    ): ChainAnswer {
        val chain = chainInfo(chainId)
        val nativeSymbol = chain?.native?.symbol ?: row.native_symbol
        val nativeName = chain?.native?.name ?: row.display_name
        val nativeDecimals = chain?.native?.decimals ?: NATIVE_DECIMALS

        val calls = ArrayList<Abi.Call>()
        val slots = ArrayList<Slot>()

        slots.add(
            Slot(
                Kind.Native, nativeSymbol, nativeName, null, nativeDecimals,
                balanceIndex = calls.size, decimalsIndex = null,
            ),
        )
        calls.add(Abi.Call(Abi.MULTICALL3, Abi.encodeGetEthBalance(address)))

        val stables = chain?.stables.orEmpty()
        stables.forEach { stable ->
            val balanceIndex = calls.size
            calls.add(Abi.Call(stable.contract, Abi.encodeBalanceOf(address)))
            val decimalsIndex = calls.size
            calls.add(Abi.Call(stable.contract, Abi.encodeDecimals()))
            slots.add(
                Slot(
                    Kind.Stable, stable.symbol, stable.symbol, stable.contract, null,
                    balanceIndex, decimalsIndex,
                ),
            )
        }

        val wrapped = chain?.wrappedNative
        if (wrapped != null) {
            val balanceIndex = calls.size
            calls.add(Abi.Call(wrapped, Abi.encodeBalanceOf(address)))
            val decimalsIndex = calls.size
            calls.add(Abi.Call(wrapped, Abi.encodeDecimals()))
            slots.add(
                Slot(
                    Kind.Wrapped, "W$nativeSymbol", "Wrapped $nativeName", wrapped, null,
                    balanceIndex, decimalsIndex,
                ),
            )
        }

        // One quote group per stable, never one flat list. The amount a quote
        // returns is denominated in THAT stable's base units, so a group scaled
        // by a neighbour's `decimals()` mis-prices by 10^12 the moment a chain
        // holds both a 6-decimal USDC and an 18-decimal DAI. Each group also
        // carries its own decimals read; the core applies its own default when
        // that read failed, rather than borrowing another group's real value.
        val quoteGroups = ArrayList<Pair<List<Int>, Int?>>()
        val dex = chain?.dex
        if (wrapped != null && dex != null) {
            val amountIn = BigInteger.TEN.pow(nativeDecimals)
            stables.forEachIndexed { index, stable ->
                val indices = quoteCalls(calls, dex, wrapped, stable.contract, amountIn)
                // +1: the native coin is slot 0, so stable `index` is slot 1+index.
                if (indices.isNotEmpty()) quoteGroups.add(indices to slots[1 + index].decimalsIndex)
            }
        }

        var localFeedIndex: Int? = null
        NATIVE_CHAINLINK_FEEDS[chainId]?.let { feed ->
            localFeedIndex = calls.size
            calls.add(Abi.Call(feed, Abi.encodeLatestRound()))
        }

        val results = batch(chainId, calls)
            ?: return nativeOnlyFallback(address, chainId, nativeSymbol, nativeName, nativeDecimals)

        // -- what the chain answered --

        val dexPrice = bestNativeDexPrice(
            quoteGroups.map { (indices, decimalsIndex) ->
                NativeQuoteGroup(
                    amountsOut = quoteAmounts(results, indices, dex?.protocol),
                    quoteDecimals = decimalsIndex
                        ?.let { results.getOrNull(it) }
                        ?.takeIf { it.success }
                        ?.let { Abi.decodeUint8(it.data).toUInt() },
                )
            },
        )
        val localChainlink = localFeedIndex
            ?.let { results.getOrNull(it) }
            ?.takeIf { it.success }
            ?.let { Abi.decodeChainlinkUsd(it.data) }
        val nativePrice = chooseNativePrice(
            dex = dexPrice,
            chainlinkLocal = localChainlink,
            chainlinkEth = ChainlinkPrices.resolve(nativeSymbol, mainnetPrices),
        )

        VelaLog.event(
            "balance.price",
            "resolved",
            "chain" to chainId,
            "symbol" to nativeSymbol,
            "usd" to nativePrice.price,
            "source" to nativePrice.source,
        )

        val tokens = slots.mapNotNull { slot ->
            val balanceResult = results.getOrNull(slot.balanceIndex)?.takeIf { it.success }
                ?: return@mapNotNull null
            val raw = Abi.decodeUint256(balanceResult.data)
            if (raw.signum() == 0) return@mapNotNull null

            val decimals = slot.knownDecimals
                ?: slot.decimalsIndex
                    ?.let { results.getOrNull(it) }
                    ?.takeIf { it.success }
                    ?.let { Abi.decodeUint8(it.data) }
                ?: DEFAULT_DECIMALS

            BalanceToken(
                chain_id = chainId,
                symbol = slot.symbol,
                name = slot.name,
                balance = humanDecimal(raw, decimals),
                decimals = decimals,
                token_address = slot.contract,
                price_usd = when (slot.kind) {
                    Kind.Native, Kind.Wrapped -> nativePrice.price
                    // **$1.00 here is a membership verdict, not a default.**
                    // The token came from this chain's curated stablecoin list,
                    // and ≈$1 is what that list means; the same approximation is
                    // core-owned on the paths that matter for records and
                    // signing. A de-peg gate was considered and rejected on the
                    // web: the only measurement available is the same DEX quote
                    // whose thin pools the price ladder already defends against,
                    // and nulling a stablecoin on its say-so would silently drop
                    // a real holding out of somebody's total.
                    Kind.Stable -> 1.0
                },
                spam = false,
            )
        }

        return ChainAnswer(chainId, answered = true, tokens = tokens)
    }

    /** The DEX calls for one pair, appended in place; the indices are the answer. */
    private fun quoteCalls(
        calls: MutableList<Abi.Call>,
        dex: ChainDex,
        tokenIn: String,
        tokenOut: String,
        amountIn: BigInteger,
    ): List<Int> {
        val indices = ArrayList<Int>()
        when {
            dex.protocol == "uniswap-v3" && dex.quoterV2 != null ->
                // The fee tiers worth trying: 0.05%, 0.3%, 0.25% (PancakeSwap
                // V3) and 1% for exotic pairs.
                listOf(500, 3000, 2500, 10000).forEach { fee ->
                    indices.add(calls.size)
                    calls.add(
                        Abi.Call(dex.quoterV2, Abi.encodeQuoteV3(tokenIn, tokenOut, amountIn, fee)),
                    )
                }

            dex.protocol == "solidly" && dex.router != null ->
                listOf(false, true).forEach { stable ->
                    indices.add(calls.size)
                    calls.add(
                        Abi.Call(
                            dex.router,
                            Abi.encodeGetAmountsOut(amountIn, tokenIn, tokenOut, stable),
                        ),
                    )
                }
            // liquidity-book and curve are not quoted; those chains fall to
            // Chainlink, which is what the ladder is for.
        }
        return indices
    }

    /**
     * The successful quote outputs of one group, in that stable's base units.
     *
     * Decode only — no comparison, no scaling, no "which pool is best". A zero
     * amount is reported as `"0"` rather than dropped, because deciding that a
     * zero quote cannot price is the core's rule.
     */
    private fun quoteAmounts(
        results: List<Abi.CallResult>,
        indices: List<Int>,
        protocol: String?,
    ): List<String> = indices.mapNotNull { index ->
        val result = results.getOrNull(index)?.takeIf { it.success } ?: return@mapNotNull null
        val amount = if (protocol == "solidly") {
            Abi.decodeAmountsOut(result.data)
        } else {
            Abi.decodeUint256(result.data)
        }
        amount.toString()
    }

    /** The batch itself. `null` means this chain did not answer at all. */
    private suspend fun batch(chainId: Int, calls: List<Abi.Call>): List<Abi.CallResult>? {
        if (calls.isEmpty()) return emptyList()
        val request = JSONObject()
            .put("to", Abi.MULTICALL3)
            .put("data", Abi.encodeAggregate3(calls))
        val answer = pool.call(chainId, "eth_call", listOf(request, "latest"))
        if (answer !is RpcResult.Body) return null
        val hex = answer.json.optString("result").takeIf { it.startsWith("0x") } ?: return null
        return Abi.decodeAggregate3(hex).takeIf { it.isNotEmpty() }
    }

    /**
     * When the batch fails, ask for the native balance the plain way.
     *
     * Multicall3 is deployed at the same address on every chain this wallet
     * ships with, but "every chain" includes ones a person added themselves,
     * and a chain without it would otherwise lose a balance that a single
     * `eth_getBalance` can still read. Losing a real holding because a
     * convenience contract is missing is a worse failure than making one extra
     * request.
     */
    private suspend fun nativeOnlyFallback(
        address: String,
        chainId: Int,
        symbol: String,
        name: String,
        decimals: Int,
    ): ChainAnswer {
        val answer = pool.call(chainId, "eth_getBalance", listOf(address, "latest"))
        if (answer !is RpcResult.Body) return ChainAnswer(chainId, answered = false, emptyList())

        val hex = answer.json.optString("result").takeIf { it.startsWith("0x") }
            ?: return ChainAnswer(chainId, answered = true, emptyList())
        val raw = runCatching { BigInteger(hex.removePrefix("0x").ifEmpty { "0" }, 16) }.getOrNull()
            ?: return ChainAnswer(chainId, answered = true, emptyList())
        if (raw.signum() == 0) return ChainAnswer(chainId, answered = true, emptyList())

        return ChainAnswer(
            chainId,
            answered = true,
            tokens = listOf(
                BalanceToken(
                    chain_id = chainId,
                    symbol = symbol,
                    name = name,
                    balance = humanDecimal(raw, decimals),
                    decimals = decimals,
                    token_address = null,
                    // No batch means no quotes and no local feed. An unpriced
                    // holding is shown and not counted; that is the core's rule
                    // and the honest answer here.
                    price_usd = null,
                    spam = false,
                ),
            ),
        )
    }

    /**
     * A balance as the core wants it: a **human decimal string**.
     *
     * Not raw units. The core parses this straight into a float and multiplies
     * it by a price, so `"1500000000000000000"` where `"1.5"` belongs is a
     * total 10^18 times too large — and that was invisible until this phase
     * gave prices a source.
     */
    private fun humanDecimal(raw: BigInteger, decimals: Int): String =
        BigDecimal(raw).movePointLeft(decimals).stripTrailingZeros().toPlainString()

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
         * What a chain's native coin is denominated in when its registry
         * document could not be fetched. Every EVM chain this wallet ships with
         * uses 18; the one that has no native coin at all is filtered out above
         * by the core's own predicate.
         */
        const val NATIVE_DECIMALS = 18

        /** What an ERC-20 is assumed to use when its own `decimals()` failed. */
        const val DEFAULT_DECIMALS = 18

        /**
         * The chain's OWN Chainlink feed for its native coin, read inside the
         * same batch and so costing nothing extra. Polygon is deliberately
         * absent: its feed did not survive the MATIC→POL migration, and the DEX
         * quotes cover it.
         */
        val NATIVE_CHAINLINK_FEEDS: Map<Int, String> = mapOf(
            1 to "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419", // ETH/USD
            10 to "0x13e3Ee699D1909E989722E753853AE30b17e08c5", // ETH/USD on Optimism
            56 to "0x0567F2323251f0Aab15c8dFb1967E4e8A7D42aeE", // BNB/USD
            100 to "0x678df3415fc31947dA4324eC63212874be5a82f8", // DAI/USD on Gnosis
            8453 to "0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70", // ETH/USD on Base
            42161 to "0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612", // ETH/USD on Arbitrum
            43114 to "0x0A77230d17318075983913bC2145DB16C7366156", // AVAX/USD
        )
    }
}
