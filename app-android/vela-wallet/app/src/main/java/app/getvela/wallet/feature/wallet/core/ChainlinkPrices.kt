package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.diagnostics.VelaLog
import org.json.JSONObject

/**
 * The last rung of the price ladder: Chainlink's feeds on Ethereum mainnet.
 *
 * Port of `app-web/vela-wallet/src/lib/services/price-service.ts`. One
 * Multicall3 batch on chain 1, cached for three minutes, read only when a
 * chain's own DEX and its own local feed have not answered — `choose_native_price`
 * decides that, not this class.
 *
 * Every feed here reports USD with 8 decimals.
 */
class ChainlinkPrices(private val pool: RpcPool) {

    @Volatile
    private var cache: Pair<Map<String, Double>, Long>? = null
    private val lock = Any()

    /** Symbol → USD. Empty when the batch could not be had; never partial-invented. */
    suspend fun prices(): Map<String, Double> {
        cache?.let { (prices, at) ->
            if (System.currentTimeMillis() - at < CACHE_TTL_MS) return prices
        }

        val symbols = FEEDS.keys.toList()
        val calls = symbols.map { symbol -> Abi.Call(FEEDS.getValue(symbol), Abi.encodeLatestRound()) }
        val request = JSONObject()
            .put("to", Abi.MULTICALL3)
            .put("data", Abi.encodeAggregate3(calls))

        val answer = pool.call(ETHEREUM, "eth_call", listOf(request, "latest"))
        if (answer !is RpcResult.Body) {
            // Keep whatever was last known rather than dropping every price:
            // a stale mainnet feed is a better answer than none, and the core
            // still prefers a chain's own sources over this one.
            return cache?.first ?: emptyMap()
        }

        val hex = answer.json.optString("result").takeIf { it.startsWith("0x") }
            ?: return cache?.first ?: emptyMap()
        val results = Abi.decodeAggregate3(hex)

        val prices = HashMap<String, Double>()
        symbols.forEachIndexed { index, symbol ->
            val result = results.getOrNull(index) ?: return@forEachIndexed
            if (!result.success) return@forEachIndexed
            Abi.decodeChainlinkUsd(result.data)?.let { prices[symbol] = it }
        }

        VelaLog.event("balance.chainlink", "read", "feeds" to "${prices.size}/${symbols.size}")
        if (prices.isEmpty()) return cache?.first ?: emptyMap()
        synchronized(lock) { cache = prices to System.currentTimeMillis() }
        return prices
    }

    companion object {
        private const val CACHE_TTL_MS = 3L * 60 * 1000
        private const val ETHEREUM = 1

        /**
         * Immutable proxy addresses. The aggregator behind each one changes;
         * the proxy does not.
         */
        private val FEEDS: Map<String, String> = linkedMapOf(
            "ETH" to "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419",
            "BNB" to "0x14e613AC691a42F21B17a6Dc7232f070FF175d25",
            "MATIC" to "0x7bAC85A8a13A4BcD8abb3eB7d6b4d632c5a57676",
            "AVAX" to "0xFF3EEb22B5E3dE6e705b44749C2559d704923FD7",
            "DAI" to "0xAed0c38402a5d19df6E4c03F4E2DceD6e29c1ee9",
        )

        /** What this wallet calls a coin, versus what Chainlink calls its feed. */
        private val ALIASES = mapOf("POL" to "MATIC", "XDAI" to "DAI")

        /**
         * A native symbol's mainnet price, or `null`.
         *
         * A chain whose gas coin IS a stablecoin (Tempo's `USD`) is pegged at a
         * dollar — the same peg the stablecoin rows use, and the reason that
         * chain is never asked for a balance in the first place.
         */
        fun resolve(nativeSymbol: String, prices: Map<String, Double>): Double? {
            val upper = nativeSymbol.uppercase()
            if (upper == "USD") return 1.0
            return prices[upper] ?: ALIASES[upper]?.let { prices[it] }
        }
    }
}
