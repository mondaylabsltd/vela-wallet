package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.net.VelaHttp
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject

/** One stablecoin on a chain: `type` is `native` or `bridge`. */
data class ChainStable(val symbol: String, val type: String, val contract: String)

data class ChainDex(val protocol: String, val quoterV2: String?, val router: String?)

data class ChainNative(val name: String, val symbol: String, val decimals: Int)

/** One chain's document, as this app uses it. */
data class ChainInfo(
    val chainId: Int,
    val native: ChainNative,
    val stables: List<ChainStable>,
    val wrappedNative: String?,
    val dex: ChainDex?,
)

/**
 * A chain's registry document, as one read of it came back — three outcomes,
 * never two (PR 2 polish). "No such document" and "no answer" are different
 * facts: a chain with no native coin (Tempo) whose document could not be read
 * was NOT read, and saying it "answered, holds nothing" put $0.00 on a home
 * over money nobody could see.
 */
sealed interface ChainDoc {
    /** A 2xx, parsed. */
    data class Doc(val info: ChainInfo) : ChainDoc

    /** HTTP 404 from a server that answered: definitively no such document. */
    data object Absent : ChainDoc

    /**
     * No answer to go on: the network failed, the read timed out, the server
     * said 5xx, 429 or any other non-2xx, the body could not be read — or
     * there was no endpoint to ask yet. Never cached.
     */
    data object Unread : ChainDoc
}

/**
 * What a chain holds, fetched.
 *
 * Port of `app-web/vela-wallet/src/lib/services/chain-tokens.ts`.
 *
 * This is master data, not a chain read — it comes from the ethereum-data
 * service over plain HTTPS rather than through the RPC pool, exactly as the web
 * does. The endpoint is whatever the `network_admin` machine says it is, so a
 * person who points the app at their own service is honoured here too.
 *
 * **A missing document degrades; an unread one is said as unread.** A chain
 * whose document does not exist simply has no stables and no DEX: its native
 * balance still reads and still shows, unpriced. Nothing invents a token
 * list. A document that could not be READ ([ChainDoc.Unread]) is not cached,
 * so the next round asks again, and the balance read decides what it means
 * for a chain that has nothing else to read ([read]).
 */
class ChainData(
    private val endpoint: () -> String,
    /** The HTTP client; a test hands one whose answers it chooses. */
    private val http: () -> OkHttpClient = { VelaHttp.client },
) {

    private val cache = HashMap<Int, Pair<ChainDoc, Long>>()
    private val lock = Any()

    /**
     * The chain's document, classified: [ChainDoc.Doc] and [ChainDoc.Absent]
     * are cached for 30 minutes, [ChainDoc.Unread] never — an answer that
     * never came is asked for again next time.
     */
    suspend fun read(chainId: Int): ChainDoc {
        synchronized(lock) {
            cache[chainId]?.let { (doc, at) ->
                if (System.currentTimeMillis() - at < CACHE_TTL_MS) return doc
            }
        }

        val base = endpoint().trimEnd('/')
        // No endpoint yet (the network list not loaded): nothing was asked.
        if (base.isEmpty()) return ChainDoc.Unread
        val doc = fetch("$base/chains/eip155-$chainId.json", chainId)
        if (doc != ChainDoc.Unread) {
            synchronized(lock) { cache[chainId] = doc to System.currentTimeMillis() }
        }
        return doc
    }

    /**
     * The chain's document, or `null` when there is none to be had — for the
     * callers to whom "absent" and "unread" both mean "no facts" (the trust
     * machine's registry snapshot).
     */
    suspend fun forChain(chainId: Int): ChainInfo? = (read(chainId) as? ChainDoc.Doc)?.info

    /** Forget everything — for a person who has just changed the endpoint. */
    fun clear() {
        synchronized(lock) { cache.clear() }
    }

    private suspend fun fetch(url: String, chainId: Int): ChainDoc = withContext(Dispatchers.IO) {
        val request = runCatching { Request.Builder().url(url).get().build() }.getOrNull()
            ?: return@withContext ChainDoc.Unread
        val client = http().newBuilder()
            .callTimeout(TIMEOUT_MS, TimeUnit.MILLISECONDS)
            .build()
        runCatching {
            client.newCall(request).execute().use { response ->
                when {
                    response.isSuccessful -> {
                        val body = response.body?.string() ?: return@use ChainDoc.Unread
                        ChainDoc.Doc(parse(JSONObject(body), chainId))
                    }
                    // The server answered, and there is no such document.
                    response.code == HTTP_NOT_FOUND -> ChainDoc.Absent
                    // 5xx, 429, a redirect this client does not follow, any other answer: not read.
                    else -> ChainDoc.Unread
                }
            }
        }.getOrElse { ChainDoc.Unread }
    }

    /**
     * The registry document, trusted field by field.
     *
     * Anything malformed falls back rather than throwing: this is a remote
     * document, and a wallet that cannot show a balance because a third party
     * shipped a bad `decimals` is worse than one that assumes 18.
     */
    private fun parse(raw: JSONObject, chainId: Int): ChainInfo {
        val nativeObject = raw.optJSONObject("nativeCurrency")
        val decimals = nativeObject?.optInt("decimals", 18)?.takeIf { it in 0..255 } ?: 18
        val native = ChainNative(
            name = nativeObject?.optString("name")?.ifBlank { null } ?: "Ether",
            symbol = nativeObject?.optString("symbol")?.ifBlank { null } ?: "ETH",
            decimals = decimals,
        )

        val stables = ArrayList<ChainStable>()
        raw.optJSONArray("stables")?.let { array ->
            for (index in 0 until array.length()) {
                val entry = array.optJSONObject(index) ?: continue
                val contract = entry.optString("contract")
                val symbol = entry.optString("symbol")
                if (contract.isBlank() || symbol.isBlank()) continue
                stables.add(ChainStable(symbol, entry.optString("type"), contract))
            }
        }

        return ChainInfo(
            chainId = chainId,
            native = native,
            stables = stables,
            wrappedNative = raw.optString("wrappedNativeToken").ifBlank { null },
            // The built-in table wins over the document: these are the DEXes
            // this wallet has actually been shown to quote against, and a
            // remote change to them would change what a coin appears to be
            // worth.
            dex = BUILTIN_DEX[chainId] ?: parseDex(raw.optJSONObject("dex")),
        )
    }

    private fun parseDex(raw: JSONObject?): ChainDex? {
        if (raw == null) return null
        val protocol = raw.optString("protocol").ifBlank { return null }
        val contracts = raw.optJSONObject("contracts") ?: return null
        return ChainDex(
            protocol = protocol,
            quoterV2 = contracts.optString("quoterV2").ifBlank { null },
            router = contracts.optString("router").ifBlank { null },
        )
    }

    companion object {
        private const val CACHE_TTL_MS = 30L * 60 * 1000
        private const val TIMEOUT_MS = 8_000L
        private const val HTTP_NOT_FOUND = 404

        /**
         * The preferred quote token: native USDC, then any USDC, then USDT,
         * then whatever is first. It orders the price attempts; it is not a
         * source of decimals for anyone else's quote.
         */
        fun pickQuoteToken(stables: List<ChainStable>): ChainStable? =
            stables.firstOrNull { it.symbol == "USDC" && it.type == "native" }
                ?: stables.firstOrNull { it.symbol == "USDC" }
                ?: stables.firstOrNull { it.symbol == "USDT" }
                ?: stables.firstOrNull()

        /**
         * The DEX per chain, built in rather than fetched.
         *
         * Verbatim from the web's `BUILTIN_DEX`. Each is the most mainstream
         * venue on that chain, and they are pinned here because a wrong quoter
         * address does not fail loudly — it fails as a missing price, or worse,
         * a wrong one.
         */
        private val BUILTIN_DEX: Map<Int, ChainDex> = mapOf(
            // Ethereum — Uniswap V3
            1 to ChainDex("uniswap-v3", "0x61fFE014bA17989E743c5F6cB21bF9697530B21e", null),
            // Optimism — Uniswap V3
            10 to ChainDex("uniswap-v3", "0x61fFE014bA17989E743c5F6cB21bF9697530B21e", null),
            // BSC — PancakeSwap V3
            56 to ChainDex("uniswap-v3", "0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997", null),
            // Gnosis — SushiSwap V3
            100 to ChainDex("uniswap-v3", "0xb1E835Dc2785b52265711e17fCCb0fd018226a6e", null),
            // Unichain — Uniswap V3
            130 to ChainDex("uniswap-v3", "0x61fFE014bA17989E743c5F6cB21bF9697530B21e", null),
            // Polygon — Uniswap V3
            137 to ChainDex("uniswap-v3", "0x61fFE014bA17989E743c5F6cB21bF9697530B21e", null),
            // Monad — Uniswap V3
            143 to ChainDex("uniswap-v3", "0x61fFE014bA17989E743c5F6cB21bF9697530B21e", null),
            // World Chain — Uniswap V3
            480 to ChainDex("uniswap-v3", "0x61fFE014bA17989E743c5F6cB21bF9697530B21e", null),
            // Base — Aerodrome
            8453 to ChainDex("solidly", null, "0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43"),
            // Avalanche — Uniswap V3
            43114 to ChainDex("uniswap-v3", "0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F", null),
            // Arbitrum — Uniswap V3
            42161 to ChainDex("uniswap-v3", "0x61fFE014bA17989E743c5F6cB21bF9697530B21e", null),
        )
    }
}
