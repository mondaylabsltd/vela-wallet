package app.getvela.wallet.core.net

import app.getvela.wallet.core.diagnostics.VelaLog
import java.io.IOException
import java.io.InterruptedIOException
import java.util.concurrent.TimeUnit
import okhttp3.Call
import okhttp3.Connection
import okhttp3.ConnectionPool
import okhttp3.EventListener
import okhttp3.Interceptor
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Request
import okhttp3.Response
import okhttp3.ResponseBody.Companion.asResponseBody
import okio.Buffer
import okio.ForwardingSource
import okio.buffer

/**
 * The one HTTP client the read path uses.
 *
 * **One, deliberately.** Every chain read in this app goes through the
 * `rpc_pool` machine, and the pool's whole job is to decide *which endpoint
 * next and why* — scoring, cooldowns, bans, the rate-limit verdict. A second
 * client anywhere is a second component choosing where to send a request, and
 * the routing rules stop being true. `NoStrayHttpClientTest` is what keeps that
 * from happening quietly (spec 041 SC-107).
 *
 * `RegistryClient` keeps its own `HttpURLConnection` and is not in scope here:
 * it predates the pool, talks to one service, and its header explains the
 * choice. This client is for chains.
 *
 * The settings below are transport hygiene, not policy:
 *
 * - **Per-call timeouts are the CORE's** (`timeout_ms` on every operation), so
 *   the client's own read timeout is only a backstop well above them.
 * - **No redirect following.** An RPC endpoint that answers 302 is not an RPC
 *   endpoint, and following it would send a wallet's address to a host the
 *   person never configured.
 * - **No cookies, no cache.** A JSON-RPC POST is not cacheable and carries no
 *   session.
 * - **Connection reuse is the reason this is a client at all**: the pool races
 *   endpoints and re-reads the same hosts constantly.
 * - **No silent retry** (`retryOnConnectionFailure(false)`): okhttp's own
 *   retry would send a POST again — a submit, an approval to the mailbox —
 *   after it may have been acted on. A read whose pooled connection was found
 *   dead is asked again on purpose ([execute], `RpcPoolExecutor`), a write
 *   goes on a connection of its own ([fresh]).
 *
 * r12-5 / r12-6 — a pooled connection is not kept past the point a server
 * may have closed it (the churn of "unexpected end of stream" on reused
 * connections: 169 gas-price and 58 estimate calls in round 12, and a
 * mailbox read on a dead socket counted as the multisig service away):
 *
 * - **An HTTP/1.0 answer closes its connection** ([Http10Close]): without
 *   `Connection: keep-alive` an HTTP/1.0 server closes after the answer (RFC
 *   9112 §9.3) — the fork's relay (Python's `http.server`), a proxy — but
 *   okhttp pooled it, and the next request on it met a closed socket.
 * - **Idle connections leave the pool after [POOL_KEEP_ALIVE_SECONDS]**, under
 *   the shortest idle timeout servers commonly keep (node's 5 s — the dev
 *   fault proxy's; nginx 75 s; load balancers 60 s). okhttp checks a pooled
 *   GET's socket only for being closed on THIS side, so a socket the server
 *   closed while it sat idle was reused, and failed, on every 12-second poll.
 */
object VelaHttp {

    val client: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .connectTimeout(CONNECT_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            // A backstop only: the core sends a per-call timeout and the
            // executor applies it to the call itself.
            .readTimeout(BACKSTOP_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .writeTimeout(BACKSTOP_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .followRedirects(false)
            .followSslRedirects(false)
            .retryOnConnectionFailure(false)
            .connectionPool(ConnectionPool(POOL_MAX_IDLE, POOL_KEEP_ALIVE_SECONDS, TimeUnit.SECONDS))
            .addNetworkInterceptor(Http10Close)
            .build()
    }

    /**
     * r11-9: the same client with a pool that keeps nothing — every call a
     * connection of its own. For a write (a submit), whose failure on a
     * pooled connection a server or a tunnel had already closed read as
     * "it may have been sent" though nothing reached anyone (round 11: the
     * relay behind `adb reverse` answered HTTP/1.0 and closed; okhttp kept
     * the connection; the next POST met "unexpected end of stream"); and for
     * a read asked again after meeting such a connection.
     */
    val fresh: OkHttpClient by lazy {
        client.newBuilder()
            .connectionPool(ConnectionPool(0, 1, TimeUnit.SECONDS))
            .build()
    }

    /**
     * r12-5: [request] on [client] (a [VelaHttp] derivative: its timeouts),
     * the way a browser treats a reused keep-alive socket — a GET or HEAD
     * whose POOLED connection turned out closed before any answer came is
     * asked once more on a connection of its own. A write is never sent
     * twice: it goes on [fresh] the first time, so a pooled socket's death
     * is never its failure. Throws what the last attempt threw. [track] is
     * handed each call (to cancel it).
     */
    fun execute(client: OkHttpClient, request: Request, track: (Call) -> Unit = {}): Response {
        if (!idempotent(request)) {
            return fresh.newBuilder().callTimeout(client.callTimeoutMillis.toLong(), TimeUnit.MILLISECONDS).build()
                .newCall(request).also(track).execute()
        }
        val use = PoolUse()
        val first = client.newBuilder().eventListener(use).build().newCall(request).also(track)
        return try {
            first.execute()
        } catch (failure: IOException) {
            // A time-out, or a cancel, is the call's own end — never asked again.
            if (!use.reused || failure is InterruptedIOException || first.isCanceled()) throw failure
            VelaLog.event(
                "http.transport", "stale connection: asked again on a new one",
                "host" to request.url.host, "method" to request.method, "error" to failure.javaClass.simpleName,
            )
            fresh.newBuilder().callTimeout(client.callTimeoutMillis.toLong(), TimeUnit.MILLISECONDS).build()
                .newCall(request).also(track).execute()
        }
    }

    /** A request that may be asked twice: it only reads. */
    fun idempotent(request: Request): Boolean = request.method == "GET" || request.method == "HEAD"

    /** Whether a call ran on a connection the pool held (read after it failed). */
    internal class PoolUse : EventListener() {
        @Volatile private var connecting = false
        @Volatile var reused = false
            private set

        override fun connectStart(call: Call, inetSocketAddress: java.net.InetSocketAddress, proxy: java.net.Proxy) {
            connecting = true
        }

        override fun connectionAcquired(call: Call, connection: Connection) {
            if (!connecting) reused = true
        }
    }

    /**
     * An HTTP/1.0 answer that does not say `Connection: keep-alive` ends its
     * connection (RFC 9112 §9.3): once its body is read or closed, the socket
     * is closed here, so the pool's next health check drops it instead of
     * handing a closed connection to the next request.
     */
    internal object Http10Close : Interceptor {
        override fun intercept(chain: Interceptor.Chain): Response {
            val response = chain.proceed(chain.request())
            if (!closesAfter(response)) return response
            val socket = chain.connection()?.socket() ?: return response
            val body = response.body ?: return response.also { runCatching { socket.close() } }
            val closing = object : ForwardingSource(body.source()) {
                override fun read(sink: Buffer, byteCount: Long): Long {
                    val read = super.read(sink, byteCount)
                    if (read == -1L) runCatching { socket.close() }
                    return read
                }

                override fun close() {
                    try {
                        super.close()
                    } finally {
                        runCatching { socket.close() }
                    }
                }
            }
            return response.newBuilder().body(closing.buffer().asResponseBody(body.contentType(), body.contentLength())).build()
        }

        /** HTTP/1.0 without keep-alive (one saying `Connection: close` okhttp retires itself). */
        fun closesAfter(response: Response): Boolean {
            val connection = response.header("Connection").orEmpty()
            if (connection.contains("close", ignoreCase = true)) return false // okhttp already retires it
            return response.protocol == Protocol.HTTP_1_0 && !connection.contains("keep-alive", ignoreCase = true)
        }
    }

    private const val CONNECT_TIMEOUT_SECONDS = 10L
    private const val BACKSTOP_TIMEOUT_SECONDS = 60L

    /** Idle connections kept per address — okhttp's default. */
    private const val POOL_MAX_IDLE = 5

    /** How long an idle connection may wait for its next request: under node's 5 s server default. */
    const val POOL_KEEP_ALIVE_SECONDS = 4L
}
