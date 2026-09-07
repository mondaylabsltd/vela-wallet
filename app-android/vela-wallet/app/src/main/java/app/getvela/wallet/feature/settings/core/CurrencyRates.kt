package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.net.VelaHttp
import app.getvela.wallet.feature.wallet.core.Abi
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONObject

/**
 * What one US dollar is worth in somebody's own currency.
 *
 * Port of `currency-rate.ts` + `fiat-rates.ts` + `fiat-fx.ts`
 * (`app-web/vela-wallet/src/lib/services/`). The waterfall, USD → code:
 *
 *  1. the Chainlink fiat feed on Ethereum mainnet, if that currency has one
 *  2. the configurable FX endpoint
 *  3. **`null`** — and null is not 1.
 *
 * That last line is the whole point. The core refuses to convert on a null
 * rate and keeps showing the USD figure; a shell that defaulted to 1 would
 * tell somebody in Tokyo that their ¥150,000 is $150,000, and in Jakarta that
 * a million rupiah is a million dollars.
 */
class CurrencyRates(
    private val pool: RpcPool,
    private val store: KeyValueStore,
    /** The FX endpoint the `network_admin` machine points at. */
    private val endpoint: () -> String,
) {

    private val lock = Mutex()

    @Volatile
    private var chainlinkCache: Pair<Map<String, Double>, Long>? = null

    @Volatile
    private var fxCache: Triple<String, Map<String, Double>, Long>? = null

    /** USD → [code], or `null` when nothing can price it right now. */
    suspend fun resolve(code: String): Double? {
        val upper = code.uppercase()
        if (upper == "USD") return 1.0

        if (upper in FIAT_FEEDS) {
            chainlinkRates()[upper]?.takeIf { it > 0.0 }?.let { return it }
        }
        return fxRates()[upper]?.takeIf { it > 0.0 }
    }

    // -- the Chainlink feeds -------------------------------------------------

    /**
     * Every supported fiat feed in one batch on Ethereum mainnet.
     *
     * **The inversion is the dangerous line.** A `<CCY>/USD` feed answers what
     * one unit of that currency is worth in dollars — 0.0068 for JPY. The rate
     * this returns is the other direction, USD → currency, so it is `1 /` that.
     * Getting it backwards is not a rounding error: it shows a Japanese person
     * a balance 22,000 times wrong, in the confident direction.
     *
     * The feeds do not share a decimals: most are 8, PHP is 18. Each one's is
     * read in the same batch rather than assumed.
     */
    private suspend fun chainlinkRates(): Map<String, Double> = lock.withLock {
        chainlinkCache?.let { (rates, at) ->
            if (System.currentTimeMillis() - at < RATE_TTL_MS) return rates
        }

        val codes = FIAT_FEEDS.keys.toList()
        val calls = codes.flatMap { code ->
            val feed = FIAT_FEEDS.getValue(code)
            listOf(
                Abi.Call(feed, Abi.encodeLatestRound()),
                Abi.Call(feed, Abi.encodeDecimals()),
            )
        }
        val request = JSONObject()
            .put("to", Abi.MULTICALL3)
            .put("data", Abi.encodeAggregate3(calls))
        val answer = pool.call(ETHEREUM, "eth_call", listOf(request, "latest"))
        val hex = (answer as? RpcResult.Body)?.json?.optString("result")
            ?.takeIf { it.startsWith("0x") }
            ?: return persistedChainlink()
        val results = Abi.decodeAggregate3(hex)

        val rates = HashMap<String, Double>()
        codes.forEachIndexed { index, code ->
            val round = results.getOrNull(index * 2)?.takeIf { it.success } ?: return@forEachIndexed
            val decimalsResult = results.getOrNull(index * 2 + 1)?.takeIf { it.success }
            val decimals = decimalsResult?.let { Abi.decodeUint8(it.data) } ?: 8
            val usdPerUnit = Abi.decodeChainlinkAnswer(round.data, decimals) ?: return@forEachIndexed
            if (usdPerUnit > 0.0) rates[code] = 1.0 / usdPerUnit
        }

        if (rates.isEmpty()) return persistedChainlink()
        chainlinkCache = rates to System.currentTimeMillis()
        store.write(KeyValueStore.Keys.FIAT_RATES, JSONObject(rates as Map<*, *>).toString())
        VelaLog.event("currency.chainlink", "read", "feeds" to "${rates.size}/${codes.size}")
        rates
    }

    /** The last good answer, for offline and first paint. Stale beats absent. */
    private suspend fun persistedChainlink(): Map<String, Double> {
        chainlinkCache?.let { return it.first }
        val raw = store.read(KeyValueStore.Keys.FIAT_RATES) ?: return emptyMap()
        return raw.toRateMap()
    }

    // -- the configurable endpoint -------------------------------------------

    /**
     * The FX endpoint's whole table.
     *
     * Two shapes are accepted, so the provider is swappable without a release:
     * Frankfurter's array of `{quote, rate}`, and the `{rates: {...}}` object
     * every other provider uses.
     *
     * The cache is keyed by URL, so a person who changes the endpoint gets the
     * new provider's numbers rather than the old one's.
     */
    private suspend fun fxRates(): Map<String, Double> {
        val url = endpoint().trim()
        if (url.isEmpty()) return persistedFx(url)

        lock.withLock {
            fxCache?.let { (cachedUrl, rates, at) ->
                if (cachedUrl == url && System.currentTimeMillis() - at < FX_TTL_MS) return rates
            }
        }

        val body = withContext(Dispatchers.IO) {
            runCatching {
                val client = VelaHttp.client.newBuilder()
                    .callTimeout(FX_TIMEOUT_MS, TimeUnit.MILLISECONDS)
                    .build()
                client.newCall(Request.Builder().url(url).get().build()).execute().use { response ->
                    if (response.isSuccessful) response.body?.string() else null
                }
            }.getOrNull()
        } ?: return persistedFx(url)

        val rates = normalize(body) ?: return persistedFx(url)
        lock.withLock { fxCache = Triple(url, rates, System.currentTimeMillis()) }
        store.write(
            KeyValueStore.Keys.FX_RATES,
            JSONObject().put("url", url).put("rates", JSONObject(rates as Map<*, *>)).toString(),
        )
        return rates
    }

    private suspend fun persistedFx(url: String): Map<String, Double> {
        fxCache?.let { return it.second }
        val raw = store.read(KeyValueStore.Keys.FX_RATES) ?: return emptyMap()
        val record = runCatching { JSONObject(raw) }.getOrNull() ?: return emptyMap()
        // A table fetched from a DIFFERENT endpoint is not this endpoint's
        // answer. Serving it would quietly keep the old provider's numbers
        // after somebody switched away from it.
        if (url.isNotEmpty() && record.optString("url") != url) return emptyMap()
        return record.optJSONObject("rates")?.toString()?.toRateMap() ?: emptyMap()
    }

    /** A provider's response as `{USD: 1, EUR: 0.92, …}`, or `null` if it is neither shape. */
    internal fun normalize(body: String): Map<String, Double>? {
        val rates = HashMap<String, Double>()
        rates["USD"] = 1.0

        val array = runCatching { JSONArray(body) }.getOrNull()
        if (array != null) {
            for (index in 0 until array.length()) {
                val row = array.optJSONObject(index) ?: continue
                val code = row.optString("quote").uppercase().ifBlank { continue }
                val rate = row.optDouble("rate", Double.NaN)
                if (rate.isFinite() && rate > 0) rates[code] = rate
            }
        } else {
            val record = runCatching { JSONObject(body) }.getOrNull() ?: return null
            val table = record.optJSONObject("rates") ?: return null
            table.keys().forEach { key ->
                val rate = table.optDouble(key, Double.NaN)
                if (rate.isFinite() && rate > 0) rates[key.uppercase()] = rate
            }
        }
        // Only `USD: 1`, which this put there — the provider priced nothing.
        return rates.takeIf { it.size > 1 }
    }

    private fun String.toRateMap(): Map<String, Double> {
        val record = runCatching { JSONObject(this) }.getOrNull() ?: return emptyMap()
        val out = HashMap<String, Double>()
        record.keys().forEach { key ->
            val rate = record.optDouble(key, Double.NaN)
            if (rate.isFinite() && rate > 0) out[key.uppercase()] = rate
        }
        return out
    }

    companion object {
        private const val ETHEREUM = 1
        private const val RATE_TTL_MS = 5L * 60 * 1000
        private const val FX_TTL_MS = 6L * 60 * 60 * 1000
        private const val FX_TIMEOUT_MS = 8_000L

        /**
         * Chainlink's `<CCY>/USD` proxies on Ethereum mainnet.
         *
         * The web resolves these through ENS (`eur-usd.data.eth`) and falls
         * back to this table; Android reads the table directly. These are
         * immutable proxy addresses — the aggregator behind one changes, the
         * proxy does not — and a wrong one fails its call, which produces no
         * rate rather than a wrong one.
         */
        internal val FIAT_FEEDS: Map<String, String> = linkedMapOf(
            "EUR" to "0xb49f677943BC038e9857d61E7d053CaA2C1734C1",
            "GBP" to "0x5c0Ab2d9b5a7ed9f470386e82BB36A3613cDd4b5",
            "JPY" to "0xBcE206caE7f0ec07b545EddE332A47C2F75bbeb3",
            "CNY" to "0xeF8A4aF35cd47424672E3C590aBD37FBB7A7759a",
            "AUD" to "0x77F9710E7d0A19669A13c055F62cd80d313dF022",
            "CAD" to "0xa34317DB73e77d453b1B8d04550c44D10e981C8e",
            "CHF" to "0x449d117117838fFA61263B61dA6301AA2a88B13A",
            "KRW" to "0x01435677FB11763550905594A16B645847C1d0F3",
            "BRL" to "0x3126E7F38D5f60f4E2B6ec3511C7bdbD79317Df1",
            "MXN" to "0xdb4881Ab0ad6b8423f76dd8C9d65542749a1dB77",
            "PHP" to "0x3C7dB4D25deAb7c89660512C5494Dc9A3FC40f78",
            "IDR" to "0x91b99C9b75aF469a71eE1AB528e8da994A5D7030",
            "ARS" to "0xE41cD2DcC63EB63A9D9e62f2a3D9b49e6d0C0A1d",
        )
    }
}
