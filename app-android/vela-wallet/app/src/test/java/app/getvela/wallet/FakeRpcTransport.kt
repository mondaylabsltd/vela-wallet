package app.getvela.wallet

import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPostResult
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.feature.wallet.core.RpcTransport
import app.getvela.wallet.feature.wallet.core.RpcTransportOutcome
import java.util.concurrent.CopyOnWriteArrayList
import org.json.JSONObject

/**
 * A transport that answers however a test says, and remembers who it was asked.
 *
 * The point of the pool is what it does when an endpoint misbehaves, and the
 * only way to test that is to have an endpoint misbehave on demand. A real one
 * cannot be asked to return 429.
 */
class FakeRpcTransport(
    private val answer: (url: String, method: String) -> RpcPostResult,
) : RpcTransport {

    /** Every URL asked, in order — the evidence for "it routed around". */
    val asked = CopyOnWriteArrayList<String>()

    override suspend fun post(
        url: String,
        method: String,
        params: List<Any?>,
        xRpcUrl: String?,
        timeoutMs: Int,
    ): RpcPostResult {
        asked += url
        return answer(url, method)
    }

    companion object {
        fun body(result: Any?): RpcPostResult = RpcPostResult(
            outcome = RpcTransportOutcome.Response(error = null),
            body = JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", result),
        )

        fun httpError(status: Int): RpcPostResult =
            RpcPostResult(RpcTransportOutcome.HttpError(status))

        fun network(): RpcPostResult = RpcPostResult(RpcTransportOutcome.Network)
    }
}

/** A fixed candidate list, so a test can give the pool something to choose between. */
class FakeEndpointSource(private val urls: List<String>) : RpcEndpointSource {
    override suspend fun forChain(chainId: Int) = RpcSeeds(
        rpc = urls.map { RpcEndpointSeed(url = it, source = RpcSource.Default) },
    )
}
