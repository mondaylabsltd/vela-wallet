package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.net.NetHealth
import app.getvela.wallet.core.net.VelaHttp
import java.io.IOException
import java.util.concurrent.TimeUnit
import kotlin.random.Random
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject

/**
 * The only place the `rpc_pool` core touches the outside world — and, through
 * it, the only place this app touches a chain.
 *
 * Seven operations and **not one routing decision**. Which endpoint to try,
 * when to ban it, whether a 429 means "broken" or "later", when to sweep the
 * pool again — all of that is `rpc_pool.rs`. This class posts, probes, draws a
 * random number, sleeps, and writes a ban list.
 *
 * Port source: `app-web/vela-wallet/src/lib/wallet/core/rpc-pool-executor.ts`.
 *
 * **The registry is the shell's half of a routed call.** The core carries no
 * params and no response bodies, so what must survive between "a call was
 * requested" and "here is the verdict" is held by whoever started the call —
 * reachable here, but owned by the facade. That keeps this class free of
 * module state and drivable from a test with its own registry.
 */
class RpcPoolExecutor(
    private val store: KeyValueStore,
    private val registry: RpcPoolCallRegistry,
    private val endpoints: RpcEndpointSource,
    /** Seam for tests: the transport, and the clock. */
    private val transport: RpcTransport = OkHttpTransport(),
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
    private val random: Random = Random.Default,
) {

    suspend fun perform(operation: RpcOperation): RpcShellResult = when (operation) {

        is RpcOperation.LoadPoolConfig -> {
            val seeds = endpoints.forChain(operation.chain_id)
            RpcShellResult.PoolConfig(
                chain_id = operation.chain_id,
                rpc_endpoints = seeds.rpc,
                bundler_endpoints = seeds.bundler,
                now_ms = now(),
            )
        }

        is RpcOperation.JsonRpcPost -> {
            val payload = registry.payload(operation.call_id)
            val started = now()
            if (payload == null) {
                // The caller is already gone — a cancelled screen, a superseded
                // refresh. Report the endpoint as unreachable rather than
                // leaving the core waiting on a call nobody wants; it will route
                // on and conclude.
                RpcShellResult.PostOutcome(
                    call_id = operation.call_id,
                    url = operation.url,
                    outcome = RpcTransportOutcome.Network,
                    latency_ms = 0.0,
                    now_ms = started,
                )
            } else {
                val result = transport.post(
                    url = operation.url,
                    method = operation.method,
                    params = payload.params,
                    xRpcUrl = operation.x_rpc_url,
                    timeoutMs = operation.timeout_ms,
                )
                result.body?.let { registry.keepBody(operation.call_id, operation.url, it) }
                // Debug trace (spec 043 phase 4, device-found): which host,
                // which method, what came back. The pool's verdicts are the
                // core's; this is the only place the raw outcome is visible.
                VelaLog.event(
                    "rpc.post", operation.method,
                    "host" to runCatching { java.net.URI(operation.url).host }.getOrNull(),
                    "outcome" to when (val o = result.outcome) {
                        is RpcTransportOutcome.HttpError -> "http ${o.status}"
                        is RpcTransportOutcome.Response -> if (o.error == null) "ok" else "rpc error ${o.error.code}"
                        else -> o::class.simpleName
                    },
                    // How long the WIRE took. Without it a slow call cannot be
                    // told from one that waited its turn somewhere else.
                    "ms" to (now() - started).toLong(),
                )
                RpcShellResult.PostOutcome(
                    call_id = operation.call_id,
                    url = operation.url,
                    outcome = result.outcome,
                    latency_ms = now() - started,
                    now_ms = now(),
                )
            }
        }

        is RpcOperation.ProbeChainId -> {
            val started = now()
            val result = transport.post(
                url = operation.url,
                method = "eth_chainId",
                params = emptyList(),
                xRpcUrl = null,
                timeoutMs = operation.timeout_ms,
            )
            RpcShellResult.ChainIdProbed(
                chain_id = operation.chain_id,
                url = operation.url,
                // A hex quantity, or nothing. Parsing it is mechanical; what an
                // unexpected id MEANS is the core's (a mismatch is its verdict,
                // not this class's).
                reported = result.body?.optString("result")?.let(::parseChainId),
                latency_ms = now() - started,
                now_ms = now(),
            )
        }

        // The core owns the backoff curve and the cap; the shell owns only the
        // unpredictability, so two devices retrying the same dead endpoint do
        // not march in step.
        is RpcOperation.DrawJitter ->
            RpcShellResult.Jitter(call_id = operation.call_id, value = random.nextDouble())

        is RpcOperation.StartBackoff -> {
            delay(operation.delay_ms.toLong())
            RpcShellResult.BackoffElapsed(call_id = operation.call_id, now_ms = now())
        }

        // Best effort, and shared with the other clients: a device that learned
        // an endpoint is dead should agree with its siblings.
        is RpcOperation.PersistBans -> {
            store.write(
                KeyValueStore.Keys.RPC_BANNED,
                JSONArray().apply {
                    operation.entries.forEach { entry ->
                        put(
                            JSONObject()
                                .put("url", entry.url)
                                .put("bannedAt", entry.banned_at_ms.toLong())
                                .put("permanent", entry.permanent),
                        )
                    }
                }.toString(),
            )
            RpcShellResult.Persisted
        }

        is RpcOperation.Conclude -> {
            registry.settle(operation.call_id, operation.verdict)
            RpcShellResult.Concluded
        }
    }

    /** What to answer when the shell itself threw. Exhaustive by construction. */
    fun neutralAnswer(operation: RpcOperation): RpcShellResult = when (operation) {
        is RpcOperation.LoadPoolConfig ->
            RpcShellResult.PoolConfig(operation.chain_id, emptyList(), emptyList(), now())
        // An escaped exception is a dead endpoint as far as the core is
        // concerned: report it and let the routing continue.
        is RpcOperation.JsonRpcPost -> RpcShellResult.PostOutcome(
            operation.call_id,
            operation.url,
            RpcTransportOutcome.Network,
            0.0,
            now(),
        )
        is RpcOperation.ProbeChainId ->
            RpcShellResult.ChainIdProbed(operation.chain_id, operation.url, null, 0.0, now())
        is RpcOperation.DrawJitter -> RpcShellResult.Jitter(operation.call_id, 0.0)
        is RpcOperation.StartBackoff -> RpcShellResult.BackoffElapsed(operation.call_id, now())
        is RpcOperation.PersistBans -> RpcShellResult.Persisted
        is RpcOperation.Conclude -> {
            registry.settle(operation.call_id, operation.verdict)
            RpcShellResult.Concluded
        }
    }

    /** The bans this device remembers, for the machine's boot event. */
    suspend fun storedBans(): List<RpcBanEntry> {
        val raw = store.read(KeyValueStore.Keys.RPC_BANNED) ?: return emptyList()
        val array = runCatching { JSONArray(raw) }.getOrNull() ?: return emptyList()
        return (0 until array.length()).mapNotNull { index ->
            val record = array.optJSONObject(index) ?: return@mapNotNull null
            val url = record.optString("url").ifEmpty { return@mapNotNull null }
            RpcBanEntry(
                url = url,
                banned_at_ms = record.optDouble("bannedAt", 0.0),
                permanent = record.optBoolean("permanent", false),
            )
        }
    }

    private fun parseChainId(hex: String): Int? {
        val body = hex.removePrefix("0x").ifEmpty { return null }
        return body.toLongOrNull(16)?.takeIf { it in 0..Int.MAX_VALUE.toLong() }?.toInt()
    }
}

/** What the shell must hold between a call starting and its verdict. */
interface RpcPoolCallRegistry {
    /** The payload to POST for `callId`; `null` once the call concluded. */
    fun payload(callId: String): RpcPayload?

    /** Remember the body `url` returned — verdicts name a URL, not a body. */
    fun keepBody(callId: String, url: String, body: JSONObject)

    /** A verdict arrived: settle whoever is waiting. */
    fun settle(callId: String, verdict: RpcCallVerdict)
}

data class RpcPayload(val method: String, val params: List<Any?>)

/** Where a chain's candidate endpoints come from (research D4). */
interface RpcEndpointSource {
    suspend fun forChain(chainId: Int): RpcSeeds
}

data class RpcSeeds(
    val rpc: List<RpcEndpointSeed> = emptyList(),
    val bundler: List<RpcEndpointSeed> = emptyList(),
)

/** The POST, and nothing about what it means. */
interface RpcTransport {
    suspend fun post(
        url: String,
        method: String,
        params: List<Any?>,
        xRpcUrl: String?,
        timeoutMs: Int,
    ): RpcPostResult
}

data class RpcPostResult(val outcome: RpcTransportOutcome, val body: JSONObject? = null)

/**
 * The shipping transport.
 *
 * It classifies **mechanically** — a status, a shape, a timeout, a socket
 * failure — and never interprets. Whether HTTP 429 means "back off" or "ban" is
 * the core's call, and it can only make it if this reports what actually
 * happened.
 */
class OkHttpTransport : RpcTransport {

    /**
     * **The blocking call confines itself.**
     *
     * `execute()` blocks, so this suspend function is only safe on a dispatcher
     * that tolerates blocking — and nothing in its signature says so. Leaving
     * that to the caller made an unrelated file's dispatcher choice
     * (`AppContainer.wallet`) load-bearing: harmonising that scope with its
     * `Main.immediate` siblings would have crashed the read path with
     * `NetworkOnMainThreadException`. The confinement belongs here, where the
     * blocking is.
     */
    override suspend fun post(
        url: String,
        method: String,
        params: List<Any?>,
        xRpcUrl: String?,
        timeoutMs: Int,
    ): RpcPostResult = withContext(Dispatchers.IO) {
        val body = JSONObject()
            .put("jsonrpc", "2.0")
            .put("id", 1)
            .put("method", method)
            .put("params", JSONArray(params))
            .toString()
            .toRequestBody(JSON)

        val request = Request.Builder()
            .url(url)
            .post(body)
            .apply { if (xRpcUrl != null) header("X-Rpc-Url", xRpcUrl) }
            .build()

        val call = VelaHttp.client.newBuilder()
            .callTimeout(timeoutMs.toLong(), TimeUnit.MILLISECONDS)
            .build()
            .newCall(request)

        try {
            call.await().use { response ->
                NetHealth.reached()
                if (!response.isSuccessful) {
                    return@withContext RpcPostResult(RpcTransportOutcome.HttpError(response.code))
                }
                val text = response.body?.string().orEmpty()
                val json = runCatching { JSONObject(text) }.getOrNull()
                    ?: return@withContext RpcPostResult(RpcTransportOutcome.NonJson)
                val error = json.optJSONObject("error")?.let {
                    RpcErrorInfo(
                        code = if (it.isNull("code")) null else it.optInt("code"),
                        message = if (it.isNull("message")) null else it.optString("message"),
                    )
                }
                RpcPostResult(RpcTransportOutcome.Response(error), json)
            }
        } catch (timeout: java.io.InterruptedIOException) {
            // okhttp reports a call timeout as an InterruptedIOException, which
            // is a subclass of IOException — so it must be caught FIRST or every
            // timeout would be reported as a generic network failure, and the
            // core would ban an endpoint that was merely slow.
            RpcPostResult(RpcTransportOutcome.Timeout)
        } catch (network: IOException) {
            NetHealth.unreached()
            RpcPostResult(RpcTransportOutcome.Network)
        }
    }

    /**
     * The call, SUSPENDED rather than blocked on.
     *
     * `execute()` inside `withContext(Dispatchers.IO)` looks confined and is
     * not: the pool's scope IS `Dispatchers.IO`, and `withContext` to the
     * dispatcher a coroutine is already on does not dispatch — it runs in
     * place. The driver starts every effect UNDISPATCHED on its one consumer,
     * so the blocking call ran ON the consumer, and the whole pool sent one
     * request at a time: ~95 calls queued behind a home-screen scan at half a
     * second each, and a sign-in's registry proof waited 30 s for its turn
     * (device-found 2026-09-19, `rpc.call slow … inFlight=96`). `enqueue`
     * suspends for real, so the consumer moves on and requests overlap.
     */
    private suspend fun okhttp3.Call.await(): okhttp3.Response =
        suspendCancellableCoroutine { continuation ->
            continuation.invokeOnCancellation { cancel() }
            enqueue(object : okhttp3.Callback {
                override fun onResponse(call: okhttp3.Call, response: okhttp3.Response) {
                    continuation.resume(response) { _, _, _ -> response.close() }
                }

                override fun onFailure(call: okhttp3.Call, e: IOException) {
                    continuation.resumeWithException(e)
                }
            })
        }

    private companion object {
        val JSON = "application/json; charset=utf-8".toMediaType()
    }
}
