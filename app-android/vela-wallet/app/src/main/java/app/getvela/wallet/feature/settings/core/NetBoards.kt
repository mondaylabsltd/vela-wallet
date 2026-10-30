package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import uniffi.vela_core_uniffi.NetworkAdminCore

/**
 * The add-network wizard's stops and verdicts, each a view the REAL `network_admin`
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

        // -- the wizard's own path: a chain picked from the search, checked, and
        //    left on its verdict for the person to act on (never saved here) --

        /** Picked and checked: the verifier and every contract are there — it can be added. */
        WizardCompatible,

        /** Picked; its probes did not answer: "unable to verify" — a verdict nobody reached. */
        WizardUnverified,

        /** Picked and refused: no P-256 verifier. */
        WizardNoP256,

        /** Picked and refused: the verifier is there, Vela's contracts are not. */
        WizardMissingContracts,
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
                    reported_chain_id = CHAIN_ID.takeIf { stop != Stop.ScanCheckFailed && stop != Stop.WizardUnverified },
                    latency_ms = 20,
                )
                // The verifier is there and the contracts are not — or
                // neither is; the core tells the two apart. One board has
                // them all.
                "rpc_get_code" -> NetShellResult.Code(
                    operation.optString("url"),
                    operation.optString("address"),
                    if (stop == Stop.WizardCompatible) "0x6080" else "0x",
                )
                "rpc_call_p256" -> NetShellResult.P256Call(
                    operation.optString("url"),
                    if (stop == Stop.ScanNoP256 || stop == Stop.WizardNoP256) "0x" else "0x" + "0".repeat(63) + "1",
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
            Stop.NoRpcEndpoint, Stop.WizardCompatible, Stop.WizardUnverified, Stop.WizardNoP256, Stop.WizardMissingContracts ->
                send(NetEvent.ChainSelected(chain_id = CHAIN_ID, keep_custom_rpc = false))
        }
        return read(script)
    }

    // -- the network ledger: shown networks, own nodes, the endpoints --------

    /** The ledger boards' own-node networks: Gnosis with "Only use my node" on, Arbitrum with it off. */
    const val OWN_ONLY_CHAIN = 100L
    const val OWN_CHAIN = 42_161L
    private const val OWN_ONLY_RPC = "https://gnosis.mynode.example"
    private const val OWN_RPC = "https://arbitrum.mynode.example"

    /** The opt-in Vela network the board's person showed (Stable); the others stay out of the list. */
    const val SHOWN_OPT_IN = 988L

    /** What the ledger board's device holds, as `read_store` answers it. */
    private fun ledgerStore() = NetShellResult.StoreLoaded(
        network_configs = listOf(
            NetNetworkConfig(OWN_ONLY_CHAIN, OWN_ONLY_RPC, "", "", only_mine = true),
            NetNetworkConfig(OWN_CHAIN, OWN_RPC, "", "", only_mine = false),
        ),
        multisig_service = true,
        shown_networks = NetShownNetworks(version = 1, shown = listOf(SHOWN_OPT_IN)),
    )

    /**
     * The network list, a network's detail or the endpoints page as the REAL
     * machine writes them for a device that showed one opt-in Vela network,
     * runs two networks on nodes of its own, and keeps the multisig service
     * among its endpoints. [expand]: the network whose detail is open (its
     * own node does not answer the probe). [endpoints]: the endpoints page
     * opened, every service answering as a board shows them — one fast, one
     * that is not the service it claims, one refused for its address, the
     * rates slow, the multisig service fine.
     */
    fun ledger(expand: Long? = null, endpoints: Boolean = false): NetView {
        val script = CoreScript(NetworkAdminCore().asBridge()) { operation ->
            val result: NetShellResult? = when (operation.optString("type")) {
                "read_store" -> ledgerStore()
                // The person's node does not answer: no chain id came back.
                "probe_rpc" -> NetShellResult.Probed(operation.optString("url"), reported_chain_id = null, latency_ms = 0)
                "probe_reachable" -> NetShellResult.Reachable(operation.optString("url"), ok = true, latency_ms = 90)
                "fetch_service_health" -> {
                    val field = Wire.json.decodeFromString(NetEndpointField.serializer(), "\"${operation.optString("field")}\"")
                    when (field) {
                        NetEndpointField.EthereumData -> NetShellResult.ServiceHealth(field, NetHealthBody.Identity("ethereum-data", "ok"), 62)
                        // Answers, as something else: not the passkey index it claims to be.
                        NetEndpointField.PasskeyIndex -> NetShellResult.ServiceHealth(field, NetHealthBody.Identity("some-other-service", "ok"), 88)
                        NetEndpointField.BundlerService -> NetShellResult.ServiceHealth(field, NetHealthBody.HttpError(503), 120)
                        NetEndpointField.GenesisService -> NetShellResult.ServiceHealth(field, NetHealthBody.Identity("safe-genesis-registry", null), 140)
                        NetEndpointField.FiatRates -> null
                    }
                }
                "fetch_fiat_rates" -> NetShellResult.FiatRates(NetHealthBody.Rates(160), 1_200)
                "write_network_configs", "write_service_endpoints", "write_shown_networks", "write_custom_networks" -> NetShellResult.Written
                "invalidate_pools" -> NetShellResult.Invalidated
                else -> null
            }
            result?.let { Wire.json.encodeToString(NetShellResult.serializer(), it) }
        }
        fun send(event: NetEvent) = script.dispatch(Wire.json.encodeToString(NetEvent.serializer(), event))
        send(NetEvent.Started)
        expand?.let { send(NetEvent.OverrideExpanded(it)) }
        if (endpoints) {
            send(NetEvent.EndpointsOpened)
            // A plain-http relay is refused: nothing is saved, the field says why.
            send(NetEvent.EndpointEdited(NetEndpointField.BundlerService, "http://relay.example.com"))
            send(NetEvent.EndpointBlurred(NetEndpointField.BundlerService))
        }
        return read(script)
    }

    /**
     * Add network on the same device: [query] typed (a hidden Vela network's
     * name finds it, tagged as Vela's own), and — with [pick] — that network
     * picked: shown again as the Vela network it is, nothing to check.
     */
    fun velaWizard(query: String, pick: Long? = null): NetView {
        val script = CoreScript(NetworkAdminCore().asBridge()) { operation ->
            val result: NetShellResult? = when (operation.optString("type")) {
                "read_store" -> ledgerStore()
                "start_search_debounce" -> NetShellResult.DebounceElapsed
                "fetch_search_index" -> NetShellResult.SearchIndex(emptyList())
                "fetch_chain_info" -> NetShellResult.ChainInfo(operation.optLong("chain_id"), null)
                else -> null
            }
            result?.let { Wire.json.encodeToString(NetShellResult.serializer(), it) }
        }
        fun send(event: NetEvent) = script.dispatch(Wire.json.encodeToString(NetEvent.serializer(), event))
        send(NetEvent.Started)
        send(NetEvent.SearchInput(query))
        pick?.let { send(NetEvent.ChainSelected(chain_id = it, keep_custom_rpc = false)) }
        return read(script)
    }

    /**
     * Read the way the live host reads it (`CoreHost.commit`): through
     * `JSONObject`, which writes the core's whole `f64`s ("20.0", a latency)
     * as the integers this mirror declares them.
     */
    private fun read(script: CoreScript): NetView =
        Wire.json.decodeFromString(NetView.serializer(), org.json.JSONObject(script.viewJson()).toString())
}
