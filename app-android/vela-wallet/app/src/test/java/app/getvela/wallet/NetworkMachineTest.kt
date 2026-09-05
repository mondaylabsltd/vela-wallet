package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetEvent
import app.getvela.wallet.feature.settings.core.NetOperation
import app.getvela.wallet.feature.settings.core.NetShellResult
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetworkAdminExecutor
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The `network_admin` machine end to end, on the real Rust.
 *
 * The interesting cases here are the ones a shell can get wrong without any
 * test failing: an endpoint saved under a key the other clients do not read, a
 * network list that shows the defaults when storage holds customs, an override
 * that a person types and the core never hears about because nothing reported
 * the blur.
 */
class NetworkMachineTest {
    /**
     * Cancelled after every test.
     *
     * Each host keeps a driver alive for the life of its scope. A suite that
     * leaves a dozen of them running competes with itself for
     * `Dispatchers.Default`, and the symptom lands somewhere else entirely — a
     * different test timing out on a budget it had never come close to. A
     * leaked scope is not untidiness; it is a flake with somebody else's name
     * on it.
     */
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEveryMachine() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }


    private fun host(store: KeyValueStore): CoreHost<NetView> {
        val executor = NetworkAdminExecutor(store)
        return CoreHost(
            bridge = uniffi.vela_core_uniffi.NetworkAdminCore().asBridge(),
            scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it },
            initial = NetView(),
            serializer = NetView.serializer(),
            perform = JsonShell.perform(
                NetOperation.serializer(),
                NetShellResult.serializer(),
                executor::perform,
            ),
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

    /**
     * Wait for the STORE, not for the view.
     *
     * A view can be right while the bytes are not there yet: the core updates
     * its model and emits the write as an effect, so "the row is gone" arrives
     * strictly before "the file says so". Asserting on the view and then
     * reading the store is a race that passes on a fast machine and fails on a
     * loaded one — the kind of flake that gets a test deleted rather than
     * understood.
     */
    private fun FakeStore.await(key: String, predicate: (String?) -> Boolean): String? =
        runBlocking {
            withTimeout(TIMEOUT_MS) {
                while (!predicate(values[key])) kotlinx.coroutines.delay(10)
                values[key]
            }
        }

    @Test
    fun theBuiltInNetworksArriveFromTheCore() {
        val host = host(FakeStore())
        host.send(NetEvent.Started)

        val view = host.settle { it.loaded }
        assertTrue("the core ships a network list", view.networks.isNotEmpty())
        // Nothing has been probed, so nothing claims to be healthy. This is
        // FR-013 at the wire: the shell renders what is here, and there is no
        // health here to render.
        assertTrue(view.networks.all { it.rpc_health == null })
    }

    @Test
    fun aStoredCustomNetworkIsInTheList() {
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.CUSTOM_NETWORKS to
                    """
                    [{"id":"custom-7777","displayName":"Seven","chainId":7777,
                      "iconLabel":"S","iconColor":"#111","iconBg":"#222","logoURL":"",
                      "isL2":false,"rpcURL":"https://rpc.example",
                      "explorerURL":"https://scan.example","bundlerURL":"",
                      "nativeSymbol":"SVN","addedAt":"2026-09-01T00:00:00Z"}]
                    """.trimIndent(),
            ),
        )
        val host = host(store)
        host.send(NetEvent.Started)

        val view = host.settle { it.loaded }
        val custom = view.networks.firstOrNull { it.is_custom }
        assertEquals("Seven", custom?.display_name)
        assertEquals(7777L, custom?.chain_id)
    }

    @Test
    fun deletingACustomNetworkRewritesTheStore() {
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.CUSTOM_NETWORKS to
                    """
                    [{"id":"custom-7777","displayName":"Seven","chainId":7777,
                      "iconLabel":"S","iconColor":"#111","iconBg":"#222","logoURL":"",
                      "isL2":false,"rpcURL":"https://rpc.example",
                      "explorerURL":"","bundlerURL":"","nativeSymbol":"SVN",
                      "addedAt":"2026-09-01T00:00:00Z"}]
                    """.trimIndent(),
            ),
        )
        val host = host(store)
        host.send(NetEvent.Started)
        host.settle { it.loaded && it.networks.any { row -> row.is_custom } }

        host.send(NetEvent.DeleteConfirmed("custom-7777"))
        host.settle { view -> view.networks.none { it.is_custom } }

        assertEquals(
            "[]",
            store.await(KeyValueStore.Keys.CUSTOM_NETWORKS) { it == "[]" },
        )
    }

    @Test
    fun anEndpointIsSavedWhereTheOtherClientsReadIt() {
        // The whole edit → commit sequence a person performs, and the assertion
        // is on the STORED BYTES rather than on the view: a view can be right
        // while the key is wrong, and only the key is shared with onboarding
        // and with the other two clients.
        val store = FakeStore()
        val host = host(store)
        host.send(NetEvent.Started)
        host.settle { it.loaded }

        host.send(NetEvent.EndpointsOpened)
        host.send(
            NetEvent.EndpointEdited(NetEndpointField.BundlerService, "https://bundler.example"),
        )
        host.send(NetEvent.EndpointBlurred(NetEndpointField.BundlerService))

        host.settle { view ->
            view.endpoints.any {
                it.field == NetEndpointField.BundlerService && it.value == "https://bundler.example"
            }
        }

        val raw = store.await(KeyValueStore.Keys.SERVICE_ENDPOINTS) {
            it != null && it.contains("bundler.example")
        }
        val stored = JSONObject(raw!!)
        assertEquals("https://bundler.example", stored.getString("bundlerServiceURL"))
    }

    @Test
    fun nothingIsProbedAndNothingClaimsToBe() {
        // Ten operations are fail-closed until 041. What must NOT happen is the
        // core reporting a green endpoint anyway — the shell answering
        // "unreachable" is what keeps the settings page honest.
        val host = host(FakeStore())
        host.send(NetEvent.Started)
        host.send(NetEvent.EndpointsOpened)
        host.send(NetEvent.EndpointsRefreshRequested)

        val view = host.settle { it.loaded && it.endpoints.isNotEmpty() }
        assertTrue(
            "no endpoint may report itself healthy without a probe",
            view.endpoints.none { it.health is app.getvela.wallet.feature.settings.core.NetServiceHealth.Ok },
        )
    }

    @Test
    fun aSearchFindsNothingWithoutTheIndex() {
        // `fetch_search_index` is fail-closed, so the wizard can offer no
        // suggestions. It must also not claim the chain does not exist — an
        // empty index is ignorance, not a verdict, and the difference shows up
        // as which message the person is given.
        val host = host(FakeStore())
        host.send(NetEvent.Started)
        host.settle { it.loaded }

        host.send(NetEvent.SearchInput("8453"))
        val view = host.settle { it.wizard.query == "8453" }

        assertTrue(view.wizard.suggestions.isEmpty())
        assertFalse("an unfetched index must not enable adding", view.wizard.can_add)
        assertNull("nothing was resolved, so there is no chain info", view.wizard.chain_info)
    }

    private companion object {
        const val TIMEOUT_MS = 10_000L
    }
}
