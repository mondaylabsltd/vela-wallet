package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import uniffi.vela_core_uniffi.NetworkAdminCore

/**
 * The add-network wizard's stops, each a view the REAL `network_admin`
 * machine wrote — a search typed or a chain handed over, its catalog read and
 * its probes answered as the case says — for the Settings gallery's boards,
 * never a live surface. So a board shows what the core says when the wizard
 * stops (its sentence, `error_key`; the check it kept, `compat`; whether a
 * network is named), not a drawing of it.
 */
object NetBoards {
    enum class Stop {
        /**
         * The scan path (a chain added by id, no confirm step) on a chain
         * that has the P-256 verifier and lacks Vela's contracts: refused,
         * the check kept beside the stop — its reason, and Chain Setup.
         */
        ScanMissingContracts,

        /** The scan path on a chain with no P-256 verifier: refused for that, nothing to deploy. */
        ScanNoP256,

        /** The scan path on a chain whose probes did not answer: "unable to verify" — never a refusal. */
        ScanCheckFailed,

        /** A chain picked from the search that is already in the list. */
        AlreadyAdded,

        /** A chain the catalog has no entry for. */
        NotFound,

        /** A chain the catalog knows and lists no usable RPC for: one must be typed. */
        NoRpcEndpoint,
    }

    /** The boards' refused chain, as the ST10C/ST10D drawings name it. */
    const val CHAIN_ID = 48_900L
    private const val CHAIN_NAME = "Zircuit"
    private const val RPC = "https://rpc.zircuit.example"
    private const val NOW_ISO = "2026-10-10T00:00:00.000Z"

    /** The whole view, as the core writes it, for [stop]. */
    fun view(stop: Stop): NetView {
        val catalog = NetRawChainData(
            chain_id = CHAIN_ID,
            name = CHAIN_NAME,
            short_name = "zircuit",
            native_currency_symbol = "ETH",
            rpc = if (stop == Stop.NoRpcEndpoint) emptyList() else listOf(RPC),
        )
        val script = CoreScript(NetworkAdminCore().asBridge()) { operation ->
            val result: NetShellResult? = when (operation.optString("type")) {
                "read_store" -> NetShellResult.StoreLoaded()
                "start_search_debounce" -> NetShellResult.DebounceElapsed
                "fetch_search_index" -> NetShellResult.SearchIndex(
                    listOf(NetChainIndexEntry(chain_id = 1, name = "Ethereum", short_name = "eth", native_currency_symbol = "ETH", has_logo = true)),
                )
                "fetch_chain_info" -> NetShellResult.ChainInfo(
                    operation.optLong("chain_id"),
                    catalog.takeIf { stop != Stop.NotFound },
                )
                "probe_rpc" -> NetShellResult.Probed(
                    operation.optString("url"),
                    reported_chain_id = CHAIN_ID.takeIf { stop != Stop.ScanCheckFailed },
                    latency_ms = 20,
                )
                // The verifier is there and the contracts are not — or
                // neither is; the core tells the two apart.
                "rpc_get_code" -> NetShellResult.Code(operation.optString("url"), operation.optString("address"), "0x")
                "rpc_call_p256" -> NetShellResult.P256Call(
                    operation.optString("url"),
                    if (stop == Stop.ScanNoP256) "0x" else "0x" + "0".repeat(63) + "1",
                )
                // Anything else (the built-in networks' health probes, the
                // endpoints') stays out: the board is the wizard.
                else -> null
            }
            result?.let { Wire.json.encodeToString(NetShellResult.serializer(), it) }
        }
        fun send(event: NetEvent) = script.dispatch(Wire.json.encodeToString(NetEvent.serializer(), event))
        send(NetEvent.Started)
        when (stop) {
            Stop.ScanMissingContracts, Stop.ScanNoP256, Stop.ScanCheckFailed -> send(NetEvent.AddByChainIdRequested(CHAIN_ID, NOW_ISO))
            Stop.AlreadyAdded -> {
                send(NetEvent.SearchInput("Ethereum"))
                send(NetEvent.ChainSelected(chain_id = 1, keep_custom_rpc = false))
            }
            Stop.NotFound -> {
                send(NetEvent.SearchInput(CHAIN_ID.toString()))
                send(NetEvent.ChainSelected(chain_id = CHAIN_ID, keep_custom_rpc = false))
            }
            Stop.NoRpcEndpoint -> send(NetEvent.ChainSelected(chain_id = CHAIN_ID, keep_custom_rpc = false))
        }
        // Read the way the live host reads it (`CoreHost.commit`): through
        // `JSONObject`, which writes the core's whole `f64`s ("20.0", a
        // latency) as the integers this mirror declares them.
        return Wire.json.decodeFromString(NetView.serializer(), org.json.JSONObject(script.viewJson()).toString())
    }
}
