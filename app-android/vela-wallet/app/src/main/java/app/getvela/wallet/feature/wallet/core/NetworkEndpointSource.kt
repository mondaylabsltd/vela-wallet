package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.settings.core.NetView
import kotlinx.coroutines.flow.StateFlow

/**
 * Where a chain's candidate endpoints come from.
 *
 * **From the `network_admin` machine, not from storage.** Spec 040 already
 * built the reader for custom networks, per-network overrides, provider keys
 * and service endpoints; a second reader of those same keys would be a second
 * opinion about which endpoints exist, and the two would disagree the moment
 * somebody edited one (research D4).
 *
 * The core scores what it is given by `RpcSource` tier, so the *order* here is
 * not a preference — it is a statement about provenance, and the machine
 * decides what that is worth.
 *
 * **What this does not yet collect** (spec 041 phase 2, when the chain index
 * becomes reachable): the index's own RPC list and the provider-key URLs. The
 * pool routes correctly with one candidate per chain; it simply has less to
 * choose from, and every extra tier is a wider net rather than a different
 * behaviour.
 */
class NetworkEndpointSource(
    /**
     * The network machine's view, read at CALL time.
     *
     * A provider rather than the flow itself, because the pool and the machine
     * that feeds it are built in the same breath: the pool needs this list, and
     * the settings machine that produces the list needs the pool for its
     * currency rate. Resolving late breaks that knot without either side
     * knowing about the other.
     */
    private val networks: () -> NetView,
    /**
     * Wait for the list to exist before answering (spec 043 phase 4,
     * device-found). The pool asks for a chain's seeds ONCE and keeps the
     * answer for the process; the tracker's first poll at `open()` asked
     * before the settings machine had its rows, got "no endpoints", and
     * Gnosis was dead for the session — no balance, no send, no receipt.
     */
    private val ready: suspend () -> Unit = {},
) : RpcEndpointSource {

    override suspend fun forChain(chainId: Int): RpcSeeds {
        ready()
        val row = networks().networks.firstOrNull { it.chain_id.toInt() == chainId }
        if (row == null) {
            VelaLog.event("rpc.seeds", "chain $chainId", "rows" to networks().networks.size, "answer" to "no row")
            return RpcSeeds()
        }
        VelaLog.event("rpc.seeds", "chain $chainId", "rpc" to row.rpc_url.takeIf { it.isNotBlank() }?.let { runCatching { java.net.URI(it).host }.getOrNull() }, "bundler" to row.bundler_url.takeIf { it.isNotBlank() })

        return RpcSeeds(
            rpc = listOfNotNull(
                row.rpc_url.takeIf { it.isNotBlank() }?.let { url ->
                    // A custom network's URL is one the person typed, which is
                    // the highest tier the core knows. A built-in's is the
                    // default we ship.
                    RpcEndpointSeed(
                        url = url,
                        source = if (row.is_custom) RpcSource.User else RpcSource.Default,
                    )
                },
            ),
            bundler = listOfNotNull(
                row.bundler_url.takeIf { it.isNotBlank() }
                    ?.let { RpcEndpointSeed(url = it, source = RpcSource.Builtin) },
            ),
        )
    }
}
