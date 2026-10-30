package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import uniffi.vela_core_uniffi.NetworkAdminCore

/**
 * The names of Vela's own networks that are out of the person's lists
 * (`NetView.hidden_vela_chain_ids`), as the core names them — for the network
 * list's "Also supported, not in your list: Stable, MegaETH…" (the web's
 * `moreNetworks`).
 *
 * The view carries those networks by chain id only, and no export names a
 * built-in network yet, so the names are asked of the core's own table the
 * one way it answers today: a throwaway `network_admin` machine, every
 * Vela network out of its lists, searched by chain id — the search's
 * `vela` rows are the core's (`vela_suggestions`), never the chain index's.
 * No table of names is kept here to drift from the core's. Pure and
 * synchronous (no effect leaves the machine), and remembered per chain for
 * the process.
 *
 * Retire this for a direct export once the core has one (named in the
 * Android phase-2 settings report).
 */
object VelaNetworkNames {
    private val known = java.util.concurrent.ConcurrentHashMap<Long, String>()

    /** Each of [chainIds] the core names, in their order; a chain it does not name is left out. */
    fun of(chainIds: List<Long>): Map<Long, String> {
        val missing = chainIds.filter { !known.containsKey(it) }
        if (missing.isNotEmpty()) runCatching { ask(missing) }
        return chainIds.mapNotNull { id -> known[id]?.let { id to it } }.toMap()
    }

    private fun ask(chainIds: List<Long>) {
        val script = CoreScript(NetworkAdminCore().asBridge()) { operation ->
            val result: NetShellResult? = when (operation.optString("type")) {
                // Every Vela network shown on the person's word is out of
                // this machine's lists, so a search for any of them finds it.
                "read_store" -> NetShellResult.StoreLoaded(shown_networks = NetShownNetworks(version = 1, shown = emptyList()))
                "start_search_debounce" -> NetShellResult.DebounceElapsed
                // Nothing from the chain index: the rows left are the core's own.
                "fetch_search_index" -> NetShellResult.SearchIndex(emptyList())
                else -> null
            }
            result?.let { Wire.json.encodeToString(NetShellResult.serializer(), it) }
        }
        fun send(event: NetEvent) = script.dispatch(Wire.json.encodeToString(NetEvent.serializer(), event))
        send(NetEvent.Started)
        for (id in chainIds) {
            send(NetEvent.SearchInput(id.toString()))
            val view = Wire.json.decodeFromString(NetView.serializer(), org.json.JSONObject(script.viewJson()).toString())
            view.wizard.suggestions
                .firstOrNull { it.vela && it.chain_id == id && it.name.isNotBlank() }
                ?.let { known[id] = it.name }
        }
    }
}
