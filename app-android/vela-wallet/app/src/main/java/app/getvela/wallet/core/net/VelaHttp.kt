package app.getvela.wallet.core.net

import java.util.concurrent.TimeUnit
import okhttp3.OkHttpClient

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
            .build()
    }

    private const val CONNECT_TIMEOUT_SECONDS = 10L
    private const val BACKSTOP_TIMEOUT_SECONDS = 60L
}
