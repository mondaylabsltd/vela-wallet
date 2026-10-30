package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetEvent
import app.getvela.wallet.feature.settings.core.NetOperation
import app.getvela.wallet.feature.settings.core.NetServiceEndpoints
import app.getvela.wallet.feature.settings.core.NetShellResult
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetWizardPhase
import app.getvela.wallet.feature.settings.core.NetworkAdminExecutor
import app.getvela.wallet.feature.settings.core.VelaNetworkNames
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Phase 2 settings parity on the real `network_admin` machine: the Vela
 * networks shown on the person's word (taken out of the list, shown again
 * through Add network), "Only use my node", and the multisig service as an
 * endpoint — each asserted on the STORED bytes the web reads, not only on the
 * view.
 */
class NetworkLedgerTest {
    private val scopes = mutableListOf<CoroutineScope>()
    private val invalidated = java.util.Collections.synchronizedList(mutableListOf<Long?>())

    @After
    fun stopEveryMachine() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private fun host(store: KeyValueStore): CoreHost<NetView> {
        val executor = NetworkAdminExecutor(store, invalidatePools = { invalidated += it })
        return CoreHost(
            bridge = uniffi.vela_core_uniffi.NetworkAdminCore().asBridge(),
            scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it },
            initial = NetView(),
            serializer = NetView.serializer(),
            perform = JsonShell.perform(NetOperation.serializer(), NetShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(
                NetOperation.serializer(),
                NetShellResult.serializer(),
                fallback = NetShellResult.StoreLoaded(),
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
    }

    private fun CoreHost<NetView>.settle(predicate: (NetView) -> Boolean): NetView =
        runBlocking { withTimeout(TIMEOUT_MS) { view.first(predicate) } }

    private fun CoreHost<NetView>.send(event: NetEvent) = dispatch(event, NetEvent.serializer())

    private fun FakeStore.await(key: String, predicate: (String?) -> Boolean): String? = runBlocking {
        withTimeout(TIMEOUT_MS) {
            while (!predicate(values[key])) kotlinx.coroutines.delay(10)
            values[key]
        }
    }

    private fun shown(raw: String?): List<Long> =
        raw?.let { JSONObject(it).getJSONArray("shown") }?.let { a -> (0 until a.length()).map { a.getLong(it) } }.orEmpty()

    // -- a Vela network shown on the person's word ---------------------------

    @Test
    fun aHiddenVelaNetworkIsNotListedAndIsNamedAsHidden() {
        val store = FakeStore(mapOf(KeyValueStore.Keys.SHOWN_NETWORKS to """{"version":1,"shown":[$STABLE]}"""))
        val host = host(store)
        host.send(NetEvent.Started)
        val view = host.settle { it.loaded }
        val stable = view.networks.single { it.chain_id == STABLE }
        assertTrue("shown on the person's word: it can be taken out", stable.optional)
        assertFalse(stable.is_custom)
        assertTrue("the other opt-ins are out of the list", view.hidden_vela_chain_ids.isNotEmpty())
        assertTrue(view.hidden_vela_chain_ids.none { id -> view.networks.any { it.chain_id == id } })
        assertFalse(STABLE in view.hidden_vela_chain_ids)
    }

    @Test
    fun removingAShownVelaNetworkHidesItAndKeepsItsSettings() {
        val config = """[{"chainId":$STABLE,"rpcURL":"https://stable.mine.example","explorerURL":"","bundlerURL":""}]"""
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.SHOWN_NETWORKS to """{"version":1,"shown":[$STABLE]}""",
                KeyValueStore.Keys.NETWORK_CONFIG to config,
            ),
        )
        val host = host(store)
        host.send(NetEvent.Started)
        host.settle { it.loaded }

        host.send(NetEvent.DeleteConfirmed("stable"))
        val view = host.settle { v -> v.networks.none { it.chain_id == STABLE } }
        assertTrue(STABLE in view.hidden_vela_chain_ids)
        assertFalse(STABLE in shown(store.await(KeyValueStore.Keys.SHOWN_NETWORKS) { !shown(it).contains(STABLE) }))
        assertEquals("its settings stay, for the day it is shown again", config, store.values[KeyValueStore.Keys.NETWORK_CONFIG])
    }

    @Test
    fun aDefaultNetworkCannotBeHidden() {
        val store = FakeStore()
        val host = host(store)
        host.send(NetEvent.Started)
        val before = host.settle { it.loaded }
        val ethereum = before.networks.single { it.chain_id == 1L }
        assertFalse("shown by default: no remove", ethereum.optional)
        host.send(NetEvent.DeleteConfirmed(ethereum.id))
        host.send(NetEvent.SearchInput("x")) // a later event, so the delete has been applied
        val after = host.settle { it.wizard.query == "x" }
        assertTrue(after.networks.any { it.chain_id == 1L })
        assertFalse(store.values.containsKey(KeyValueStore.Keys.SHOWN_NETWORKS))
    }

    @Test
    fun aHiddenVelaNetworkIsShownAgainThroughAddNetwork() {
        val store = FakeStore(mapOf(KeyValueStore.Keys.SHOWN_NETWORKS to """{"version":1,"shown":[]}"""))
        val host = host(store)
        host.send(NetEvent.Started)
        val loaded = host.settle { it.loaded }
        assertTrue(STABLE in loaded.hidden_vela_chain_ids)

        // The search finds it whether or not the chain index answers (it
        // does not, here) — as Vela's own.
        host.send(NetEvent.SearchInput("Stable"))
        val found = host.settle { v -> v.wizard.suggestions.any { it.chain_id == STABLE } }
        assertTrue(found.wizard.suggestions.single { it.chain_id == STABLE }.vela)

        host.send(NetEvent.ChainSelected(chain_id = STABLE, keep_custom_rpc = false))
        val picked = host.settle { it.wizard.phase == NetWizardPhase.Checked }
        assertTrue(picked.wizard.vela_network)
        assertTrue(picked.wizard.can_add)

        host.send(NetEvent.AddConfirmed("2026-10-30T00:00:00Z"))
        val back = host.settle { v -> v.networks.any { it.chain_id == STABLE } }
        assertFalse("shown again as the Vela network it is", back.networks.single { it.chain_id == STABLE }.is_custom)
        assertTrue(STABLE in shown(store.await(KeyValueStore.Keys.SHOWN_NETWORKS) { shown(it).contains(STABLE) }))
        assertFalse("not saved as a custom network", store.values[KeyValueStore.Keys.CUSTOM_NETWORKS].orEmpty().contains("$STABLE"))
    }

    @Test
    fun theCoreNamesEveryHiddenVelaNetwork() {
        val hidden = uniffi.vela_core_uniffi.shownNetworksHidden("{\"version\":1,\"shown\":[]}").map { it.toLong() }
        val names = VelaNetworkNames.of(hidden)
        assertEquals("every opt-in is named", hidden.toSet(), names.keys)
        assertEquals("Stable", names[STABLE])
        assertEquals("MegaETH", names[MEGAETH])
        assertTrue(names.values.all { it.isNotBlank() })
        assertTrue("an id the core does not name is left out", VelaNetworkNames.of(listOf(7_777L)).isEmpty())
    }

    // -- "Only use my node" ----------------------------------------------------

    @Test
    fun onlyMyNodeIsStoredAndTheChainsPoolIsReRead() {
        val store = FakeStore(
            mapOf(KeyValueStore.Keys.NETWORK_CONFIG to """[{"chainId":100,"rpcURL":"https://gnosis.mine.example","explorerURL":"","bundlerURL":""}]"""),
        )
        val host = host(store)
        host.send(NetEvent.Started)
        val loaded = host.settle { it.loaded }
        val gnosis = loaded.networks.single { it.chain_id == 100L }
        assertTrue("an RPC of the person's own", gnosis.own_rpc)
        assertFalse(gnosis.only_mine)

        host.send(NetEvent.OnlyMineChanged(100, true))
        host.settle { v -> v.networks.single { it.chain_id == 100L }.only_mine }
        val raw = store.await(KeyValueStore.Keys.NETWORK_CONFIG) { it?.contains("onlyMine") == true }
        assertTrue(JSONArray(raw).getJSONObject(0).getBoolean("onlyMine"))
        runBlocking { withTimeout(TIMEOUT_MS) { while (100L !in invalidated) kotlinx.coroutines.delay(10) } }

        host.send(NetEvent.OnlyMineChanged(100, false))
        host.settle { v -> !v.networks.single { it.chain_id == 100L }.only_mine }
        store.await(KeyValueStore.Keys.NETWORK_CONFIG) { it?.contains("onlyMine") == false }
    }

    @Test
    fun onlyMyNodeIsNotOfferedOnTheShippedEndpoint() {
        val store = FakeStore()
        val host = host(store)
        host.send(NetEvent.Started)
        val loaded = host.settle { it.loaded }
        assertFalse(loaded.networks.single { it.chain_id == 100L }.own_rpc)
        host.send(NetEvent.OnlyMineChanged(100, true))
        host.send(NetEvent.SearchInput("y"))
        val after = host.settle { it.wizard.query == "y" }
        assertFalse(after.networks.single { it.chain_id == 100L }.only_mine)
        assertFalse(store.values.containsKey(KeyValueStore.Keys.NETWORK_CONFIG))
        assertFalse(100L in invalidated)
    }

    // -- the multisig service is an endpoint -----------------------------------

    @Test
    fun theMultisigServiceIsListedSavedAndReset() {
        val store = FakeStore()
        val host = host(store)
        host.send(NetEvent.Started)
        val loaded = host.settle { it.loaded }
        assertEquals(NetEndpointField.entries.toList(), loaded.endpoints.map { it.field })
        val genesis = loaded.endpoints.single { it.field == NetEndpointField.GenesisService }
        assertEquals(NetServiceEndpoints("", "", "", "").genesis_service_url, genesis.default_value)

        host.send(NetEvent.EndpointsOpened)
        host.send(NetEvent.EndpointEdited(NetEndpointField.GenesisService, "https://genesis.mine.example"))
        host.send(NetEvent.EndpointBlurred(NetEndpointField.GenesisService))
        val saved = store.await(KeyValueStore.Keys.SERVICE_ENDPOINTS) { it?.contains("genesis.mine.example") == true }
        assertEquals("https://genesis.mine.example", JSONObject(saved!!).getString("genesisServiceURL"))

        host.send(NetEvent.ResetEndpointsToDefaults)
        val reset = store.await(KeyValueStore.Keys.SERVICE_ENDPOINTS) { it?.contains("genesis.mine.example") == false }
        assertEquals(genesis.default_value, JSONObject(reset!!).getString("genesisServiceURL"))
    }

    @Test
    fun aPlainHttpEndpointIsRefusedAndNotSaved() {
        val store = FakeStore()
        val host = host(store)
        host.send(NetEvent.Started)
        host.settle { it.loaded }
        host.send(NetEvent.EndpointsOpened)
        host.send(NetEvent.EndpointEdited(NetEndpointField.GenesisService, "http://genesis.example.com"))
        host.send(NetEvent.EndpointBlurred(NetEndpointField.GenesisService))
        host.settle { v -> v.endpoints.single { it.field == NetEndpointField.GenesisService }.refused }
        assertFalse(store.values[KeyValueStore.Keys.SERVICE_ENDPOINTS].orEmpty().contains("genesis.example.com"))
    }

    // -- the executor: the web's bytes ----------------------------------------

    @Test
    fun theExecutorKeepsTheMultisigServiceAsTheWebDoes() = runBlocking {
        val store = FakeStore(mapOf(KeyValueStore.Keys.SERVICE_ENDPOINTS to """{"genesisServiceURL":"https://g.example"}"""))
        val executor = NetworkAdminExecutor(store)
        val loaded = executor.perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded
        assertTrue("the multisig service is one of this shell's endpoints", loaded.multisig_service)
        assertEquals("https://g.example", loaded.endpoints.genesis_service_url)
        assertTrue((executor.neutralAnswer(NetOperation.ReadStore) as NetShellResult.StoreLoaded).multisig_service)

        executor.perform(NetOperation.WriteServiceEndpoints(NetServiceEndpoints("https://a", "https://b", "https://c", "https://d", "https://e")))
        val written = JSONObject(store.values[KeyValueStore.Keys.SERVICE_ENDPOINTS]!!)
        assertEquals("https://e", written.getString("genesisServiceURL"))
        assertEquals("https://d", written.getString("fiatRatesURL"))
    }

    // -- the pool's whole view --------------------------------------------------

    @Test
    fun thePoolViewDecodesWhereEachCallGoesNext() {
        val json = """{"failed_chains":[],"rate_limited_chains":[],"banned":[],"unreached_chains":[],
            "fallback_chains":[42161],"own_only_chains":[100],"own_node_unreached_chains":[100],
            "pending_urls":{"c1":["https://b.example","https://c.example"]}}"""
        val view = Wire.json.decodeFromString(app.getvela.wallet.feature.wallet.core.RpcPoolView.serializer(), json)
        assertEquals(listOf("https://b.example", "https://c.example"), view.pending_urls["c1"])
        assertEquals(listOf(100), view.own_node_unreached_chains)
    }

    @Test
    fun theSigningPagesViewDecodesEveryField() {
        val json = """{"pages":[{"url":"https://sign.getvela.app/","name":"","domain":"getvela.app","official":true,"trusted":[]}],
            "saved":[{"url":"https://sign.example.com/","name":"Mine","trusted":[]}],"add_error":"duplicate","loaded":true}"""
        val view = Wire.json.decodeFromString(app.getvela.wallet.feature.settings.core.SigningPagesView.serializer(), json)
        assertEquals("Mine", view.saved.single().name)
        assertEquals("duplicate", view.add_error)
        assertTrue(view.loaded)
        assertEquals(app.getvela.wallet.feature.settings.core.OFFICIAL_SIGNING_PAGE_URL, view.pages.single().url)
        // Before the list is read: the official page alone, edits not offered.
        val initial = app.getvela.wallet.feature.settings.core.SigningPagesView()
        assertEquals(app.getvela.wallet.feature.settings.core.OFFICIAL_SIGNING_PAGE_URL, initial.pages.single().url)
        assertFalse(initial.loaded)
    }

    private companion object {
        const val TIMEOUT_MS = 10_000L
        const val STABLE = 988L
        const val MEGAETH = 4_326L
    }
}
