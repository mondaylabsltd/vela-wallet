package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.net.VelaHttp
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject

/**
 * The network checks behind the settings screens: is this endpoint alive, what
 * chain does it think it is on, does it have the P-256 precompile, is this
 * service healthy, does this chain exist.
 *
 * Port of the network half of
 * `app-web/vela-wallet/src/lib/settings/core/network-admin-executor.ts`.
 *
 * **Why these do not go through the RPC pool.** The pool's job is to pick a
 * good endpoint out of several and route around the bad ones. These calls are
 * the opposite: they test ONE named URL, usually one the person has just typed,
 * and routing away from it would answer a question nobody asked. They still use
 * the app's one HTTP client, so the "no stray clients" rule holds.
 *
 * **Nothing here decides anything.** A latency is measured, a chain id is
 * parsed, a health body is read out. Whether 400ms is slow, whether a mismatched
 * chain id is fatal, whether a service calling itself something unexpected means
 * unhealthy — every one of those is `network_admin.rs`.
 */
class NetworkProbes(
    /** The ethereum-data service, from the machine's own endpoint list. */
    private val chainIndexUrl: () -> String,
) {

    /** What one probe observed. `reportedChainId` is null when nothing answered. */
    data class Probe(val reportedChainId: Long?, val latencyMs: Int)

    /** Whether a URL answered at all, and how fast. */
    data class Reach(val ok: Boolean, val latencyMs: Int)

    /** A health response, unjudged: the fields as sent, or the status that came back. */
    data class Health(
        val httpStatus: Int?,
        val service: String?,
        val status: String?,
        val rateCount: Int?,
        val latencyMs: Int,
    )

    // -- chain reads on ONE named endpoint -----------------------------------

    /** `eth_chainId` against this URL, and what it took. */
    suspend fun probeRpc(url: String): Probe {
        val started = System.currentTimeMillis()
        // A websocket endpoint gets a websocket probe: one `eth_chainId`
        // frame, one reply, and the socket closed (spec 043 T049).
        val result = if (url.startsWith("ws://") || url.startsWith("wss://")) {
            wsRpc(url, "eth_chainId", JSONArray())
        } else {
            jsonRpc(url, "eth_chainId", JSONArray())
        }
        return Probe(
            reportedChainId = (result as? String)?.let(::parseHexChainId),
            latencyMs = elapsed(started),
        )
    }

    /**
     * Whether a URL answers at all.
     *
     * An explorer is a website, not an API. Anything that comes back — a page,
     * a redirect, a 404 — proves the host is there, and that is the whole
     * question. Only a connection that never completes is unreachable.
     */
    suspend fun probeReachable(url: String): Reach {
        val started = System.currentTimeMillis()
        val ok = withContext(Dispatchers.IO) {
            runCatching {
                client(PROBE_TIMEOUT_MS)
                    .newCall(Request.Builder().url(url).get().build())
                    .execute()
                    .use { true }
            }.getOrDefault(false)
        }
        return Reach(ok, elapsed(started))
    }

    /** `eth_getCode` — whether something is deployed at an address. */
    suspend fun getCode(url: String, address: String): String? =
        jsonRpc(url, "eth_getCode", JSONArray().put(address).put("latest")) as? String

    /**
     * Does this chain have the RIP-7212 P-256 precompile?
     *
     * A known-good signature over `sha256("test")`. The precompile answers with
     * a word when the signature verifies and empty when it does not; a chain
     * without it answers empty too, and telling those apart is the core's job.
     *
     * `gas: 0x100000` is not decoration — zkSync-family chains reject the call
     * outright without it.
     */
    suspend fun callP256(url: String): String? {
        val request = JSONObject()
            .put("to", P256_PRECOMPILE)
            .put("data", VALID_P256_CALL)
            .put("gas", "0x100000")
        return jsonRpc(url, "eth_call", JSONArray().put(request).put("latest")) as? String
    }

    // -- service health -------------------------------------------------------

    /** `GET {base}/api/health`, read out rather than judged. */
    suspend fun serviceHealth(baseUrl: String): Health {
        val started = System.currentTimeMillis()
        // A cache-buster: a health check answered from a cache is a health
        // check of the cache.
        val url = "${baseUrl.trimEnd('/')}/api/health?_t=$started"
        val response = get(url) ?: return Health(null, null, null, null, elapsed(started))
        val latency = elapsed(started)
        if (response.second != null) return Health(response.second, null, null, null, latency)

        val json = runCatching { JSONObject(response.first.orEmpty()) }.getOrNull()
        return Health(
            httpStatus = null,
            service = json?.optString("service")?.ifBlank { null },
            status = json?.optString("status")?.ifBlank { null },
            rateCount = null,
            latencyMs = latency,
        )
    }

    /**
     * The fiat endpoint, counted.
     *
     * The two accepted shapes are a shell question — how many rates came back.
     * Whether zero of them is a failure is the core's.
     */
    suspend fun fiatRates(url: String): Health {
        val started = System.currentTimeMillis()
        val response = get(url) ?: return Health(null, null, null, null, elapsed(started))
        val latency = elapsed(started)
        if (response.second != null) return Health(response.second, null, null, null, latency)

        val body = response.first.orEmpty()
        val count = runCatching { JSONArray(body).length() }.getOrNull()
            ?: runCatching { JSONObject(body).optJSONObject("rates")?.length() }.getOrNull()
            ?: 0
        return Health(null, null, null, rateCount = count, latencyMs = latency)
    }

    // -- the chain registry ---------------------------------------------------

    /** Every chain the index knows, for the add-network search. */
    suspend fun searchIndex(): List<NetChainIndexEntry> {
        val base = chainIndexUrl().trimEnd('/')
        if (base.isEmpty()) return emptyList()
        val body = get("$base/index/fuse-chains.json")?.first ?: return emptyList()
        val rows = runCatching { JSONObject(body).optJSONArray("data") }.getOrNull()
            ?: return emptyList()

        val out = ArrayList<NetChainIndexEntry>(rows.length())
        for (index in 0 until rows.length()) {
            val row = rows.optJSONObject(index) ?: continue
            val chainId = row.optLong("chainId", -1)
            // **One unrepresentable chain must not cost the whole index.**
            //
            // The core holds a chain id as `u32`, and the public index carries
            // ids past it — 7,078,815,900 is in there today. Handing one over
            // makes serde reject the ENTIRE `search_index` result, so the
            // fault this produced was not "that chain is missing" but "network
            // search does nothing at all", with a core fault in the log and
            // nothing on screen.
            //
            // Dropping it is honest: a chain this core cannot represent is a
            // chain this wallet cannot add, so it is not offered. Whether the
            // core's own type should widen is a question for every platform —
            // the web guards this no better, and iOS met the same defect in
            // spec 050 — and it is recorded rather than answered here.
            if (chainId < 0 || chainId > U32_MAX) continue
            out.add(
                NetChainIndexEntry(
                    chain_id = chainId,
                    name = row.optString("name"),
                    short_name = row.optString("shortName"),
                    native_currency_symbol = row.optString("nativeCurrencySymbol"),
                    has_logo = row.optBoolean("hasLogo", false),
                ),
            )
        }
        return out
    }

    /** One chain's registry document, for the add-network wizard. */
    suspend fun chainInfo(chainId: Long): NetRawChainData? {
        val base = chainIndexUrl().trimEnd('/')
        if (base.isEmpty()) return null
        val body = get("$base/chains/eip155-$chainId.json")?.first ?: return null
        val raw = runCatching { JSONObject(body) }.getOrNull() ?: return null
        val native = raw.optJSONObject("nativeCurrency")

        return NetRawChainData(
            // Same ceiling as the index: a chain id the core cannot hold is
            // reported as absent rather than as a number it will reject.
            chain_id = raw.optLong("chainId", -1).takeIf { it in 0..U32_MAX },
            name = raw.optString("name").ifBlank { null },
            short_name = raw.optString("shortName").ifBlank { null },
            native_currency_name = native?.optString("name")?.ifBlank { null },
            native_currency_symbol = native?.optString("symbol")?.ifBlank { null },
            native_currency_decimals = native?.takeIf { it.has("decimals") }?.optLong("decimals"),
            rpc = raw.optJSONArray("rpc")?.let { array ->
                (0 until array.length()).mapNotNull { array.optString(it).ifBlank { null } }
            } ?: emptyList(),
            explorers = raw.optJSONArray("explorers")?.let { array ->
                (0 until array.length()).map { array.optJSONObject(it)?.optString("url").orEmpty() }
            } ?: emptyList(),
            testnet = raw.optBoolean("testnet", false),
        )
    }

    // -- plumbing -------------------------------------------------------------

    /** A JSON-RPC call to ONE url. `null` for any failure or any error reply. */
    private suspend fun jsonRpc(url: String, method: String, params: JSONArray): Any? =
        withContext(Dispatchers.IO) {
            val payload = JSONObject()
                .put("jsonrpc", "2.0")
                .put("id", 1)
                .put("method", method)
                .put("params", params)
                .toString()
                .toRequestBody(JSON)

            runCatching {
                client(PROBE_TIMEOUT_MS)
                    .newCall(Request.Builder().url(url).post(payload).build())
                    .execute()
                    .use { response ->
                        if (!response.isSuccessful) return@use null
                        val json = JSONObject(response.body?.string().orEmpty())
                        // An error reply is not an answer. The core reads a null
                        // as "this endpoint did not tell us", which is true.
                        if (json.has("error") && !json.isNull("error")) return@use null
                        if (json.isNull("result")) null else json.get("result")
                    }
            }.getOrNull()
        }

    /** `body to null` on success, `null to status` on an HTTP error, `null` if nothing answered. */
    private suspend fun get(url: String): Pair<String?, Int?>? = withContext(Dispatchers.IO) {
        runCatching {
            client(PROBE_TIMEOUT_MS)
                .newCall(Request.Builder().url(url).get().build())
                .execute()
                .use { response ->
                    if (!response.isSuccessful) null to response.code
                    else response.body?.string() to null
                }
        }.getOrNull()
    }

    /** One JSON-RPC frame over a websocket; the first reply, or `null` when nothing comes back in time. */
    private suspend fun wsRpc(url: String, method: String, params: JSONArray): Any? = withContext(Dispatchers.IO) {
        val payload = JSONObject().put("jsonrpc", "2.0").put("id", 1).put("method", method).put("params", params).toString()
        val reply = kotlinx.coroutines.CompletableDeferred<Any?>()
        val socket = runCatching {
            client(PROBE_TIMEOUT_MS).newWebSocket(
                Request.Builder().url(url).build(),
                object : okhttp3.WebSocketListener() {
                    override fun onOpen(webSocket: okhttp3.WebSocket, response: okhttp3.Response) {
                        webSocket.send(payload)
                    }

                    override fun onMessage(webSocket: okhttp3.WebSocket, text: String) {
                        val json = runCatching { JSONObject(text) }.getOrNull()
                        val result = json?.takeIf { !(it.has("error") && !it.isNull("error")) }?.opt("result")
                        reply.complete(result)
                        webSocket.close(1000, null)
                    }

                    override fun onFailure(webSocket: okhttp3.WebSocket, t: Throwable, response: okhttp3.Response?) {
                        reply.complete(null)
                    }

                    override fun onClosing(webSocket: okhttp3.WebSocket, code: Int, reason: String) {
                        reply.complete(null)
                    }
                },
            )
        }.getOrNull() ?: return@withContext null
        try {
            kotlinx.coroutines.withTimeoutOrNull(PROBE_TIMEOUT_MS) { reply.await() }
        } finally {
            socket.cancel()
        }
    }

    private fun client(timeoutMs: Long) = VelaHttp.client.newBuilder()
        .callTimeout(timeoutMs, TimeUnit.MILLISECONDS)
        .build()

    private fun elapsed(started: Long): Int =
        (System.currentTimeMillis() - started).coerceIn(0, Int.MAX_VALUE.toLong()).toInt()

    private fun parseHexChainId(hex: String): Long? = runCatching {
        hex.removePrefix("0x").ifEmpty { return null }.toLong(16)
    }.getOrNull()

    private companion object {
        /** The same budget the web gives a network check. */
        const val PROBE_TIMEOUT_MS = 8_000L

        /** What the core can hold in a `chain_id`. */
        const val U32_MAX = 4_294_967_295L

        val JSON = "application/json; charset=utf-8".toMediaType()

        /** RIP-7212's precompile address. */
        const val P256_PRECOMPILE = "0x0000000000000000000000000000000000000100"

        /** `sha256("test")` and a valid signature over it, with its public key. */
        const val VALID_P256_CALL = "0x" +
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08" +
            "7bf0e18d07660f15994adce5c3836d7bd6167cdb5726f631098f433ebe0be9c0" +
            "3936edbe5c791477e714e58244afb690b9b88b833ff4acdf0fbd1b28bf0b1182" +
            "3be8cbcb3f590087711ae5ed74b9cd06a88058d0bbe700b5f0ec5a1bfac15592" +
            "f989ef9bfaae0fee03c36625e88eae99806a879d813411f876e7e03a2ffd8314"
    }
}
