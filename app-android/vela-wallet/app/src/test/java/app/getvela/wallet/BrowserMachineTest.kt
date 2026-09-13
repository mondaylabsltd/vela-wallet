package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DpermEvent
import app.getvela.wallet.feature.browser.core.DpermOperation
import app.getvela.wallet.feature.browser.core.DpermShellResult
import app.getvela.wallet.feature.browser.core.DpermView
import app.getvela.wallet.feature.browser.core.RequestRouter
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.DappPermissionsCore

/**
 * Spec 044 T026: connect through the real `dapp_permissions` machine — a
 * never-connected origin gets consent, approval answers one address, the
 * second ask is instant, a dismissal is the standard refusal once, a
 * revoke fires `disconnect` and re-asks; the grant is keyed by origin. And
 * the shell's own routing for what the core forwards.
 */
class BrowserMachineTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val answers = java.util.Collections.synchronizedList(ArrayList<Pair<String, JSONObject>>())
    private val events = java.util.Collections.synchronizedList(ArrayList<JSONObject>())
    private val forwarded = java.util.Collections.synchronizedList(ArrayList<String>())
    private val records = java.util.Collections.synchronizedList(ArrayList<JSONObject>())
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val origin = "http://127.0.0.1:8137"

    @After
    fun stop() = scope.cancel()

    private fun host(): CoreHost<DpermView> {
        val executor = BrowserExecutor(
            store,
            object : BrowserExecutor.Ports {
                override fun respond(id: String, json: JSONObject) { answers += id to json }
                override fun emit(json: JSONObject) { events += json }
                override fun settleForwarded(code: Int, message: String) { answers += "settled" to BrowserExecutor.errorJson("settled", code, message) }
                override fun saveConnectionRecord(row: JSONObject) { records += row }
                override fun forward(id: String, method: String, paramsJson: String, origin: String) { forwarded += "$id:$method" }
            },
        )
        return CoreHost(
            bridge = DappPermissionsCore().asBridge(), scope = scope, initial = DpermView(), serializer = DpermView.serializer(),
            perform = JsonShell.perform(DpermOperation.serializer(), DpermShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(DpermOperation.serializer(), DpermShellResult.serializer(), fallback = DpermShellResult.Ack, answer = executor::neutralAnswer),
            onFault = { error -> throw AssertionError("core fault", error) },
        ).also { h ->
            h.dispatch(DpermEvent.AccountsUpdated(listOf(safe)), DpermEvent.serializer())
            h.dispatch(DpermEvent.AccountSwitched(safe, 1.0e12), DpermEvent.serializer())
            h.dispatch(DpermEvent.ChainChanged(100), DpermEvent.serializer())
            h.dispatch(DpermEvent.NavigationStarted("$origin/?v=3"), DpermEvent.serializer())
        }
    }

    private fun ask(h: CoreHost<DpermView>, id: String, method: String, params: String = "[]") =
        h.dispatch(DpermEvent.ProviderRequest(id = id, method = method, params_json = params, origin = origin, is_main_frame = true), DpermEvent.serializer())

    private suspend fun answered(id: String): JSONObject = withTimeout(10_000) {
        while (answers.none { it.first == id }) delay(20)
        answers.first { it.first == id }.second
    }

    @Test
    fun `consent, then the instant answer, then the standard refusal once`() = runBlocking {
        val h = host()
        withTimeout(10_000) { h.view.first { it.current_origin == origin } }
        ask(h, "r1", "eth_requestAccounts")
        val asking = withTimeout(10_000) { h.view.first { it.consent != null } }
        assertEquals("the consent card carries the CORE's origin", origin, asking.consent!!.origin)
        h.dispatch(DpermEvent.ConsentApproved(1.0e12 + 5), DpermEvent.serializer())
        val a1 = answered("r1")
        assertEquals(safe, a1.getJSONArray("result").getString(0))
        val connected = withTimeout(10_000) { h.view.first { it.connected_address != null } }
        assertEquals(safe, connected.connected_address)
        withTimeout(10_000) { while (records.isEmpty()) delay(20) }
        assertEquals("connect", records.single().getString("type"))
        assertEquals(origin, records.single().getString("dappOrigin"))
        // The grant is keyed by ORIGIN, not URL.
        withTimeout(10_000) { while (store.values[BrowserExecutor.grantKey(origin)].isNullOrBlank()) delay(20) }
        assertNull(store.values["${BrowserExecutor.grantKey(origin)}/?v=3"])
        // The second ask: instant, no consent.
        ask(h, "r2", "eth_accounts")
        val a2 = answered("r2")
        assertEquals(safe, a2.getJSONArray("result").getString(0))
        assertNull(h.view.value.consent)
        // A read is forwarded, never answered here.
        ask(h, "r3", "eth_blockNumber")
        withTimeout(10_000) { while (forwarded.none { it == "r3:eth_blockNumber" }) delay(20) }
        // Revoke: the page hears disconnect, and asks again next time.
        h.dispatch(DpermEvent.RevokeRequested(origin), DpermEvent.serializer())
        withTimeout(10_000) { while (events.none { it.optString("event") == "disconnect" }) delay(20) }
        withTimeout(10_000) { h.view.first { it.connected_address == null } }
        ask(h, "r4", "eth_requestAccounts")
        withTimeout(10_000) { h.view.first { it.consent != null } }
        h.dispatch(DpermEvent.ConsentRejected, DpermEvent.serializer())
        val a4 = answered("r4")
        assertEquals(4001, a4.getJSONObject("error").getInt("code"))
        assertEquals("User rejected the request", a4.getJSONObject("error").getString("message"))
        assertEquals("one refusal, not two", 1, answers.count { it.first == "r4" })
    }

    @Test
    fun `the shell answers what the core forwards - facts, reads, switches, refusals`() = runBlocking {
        var chain = 100
        val switched = ArrayList<Int>()
        val router = RequestRouter(
            object : RequestRouter.Ports {
                override fun browserChain() = chain
                override fun knownChains() = listOf(1, 100)
                override fun switchChain(chainId: Int) { chain = chainId; switched += chainId }
                override suspend fun poolCall(chainId: Int, method: String, params: JSONArray, bundler: Boolean): JSONObject? = when (method) {
                    "eth_blockNumber" -> JSONObject().put("result", "0x2df8cad")
                    "eth_getLogs" -> JSONObject().put("error", JSONObject().put("code", -32005).put("message", "range too wide"))
                    else -> null
                }
                override fun respond(id: String, json: JSONObject) { answers += id to json }
                override fun sign(id: String, method: String, paramsJson: String, origin: String) { forwarded += "$id:$method" }
                override suspend fun receiptFor(userOpHash: String): RequestRouter.Receipt? = null
            },
        )
        router.route("s1", "eth_chainId", "[]", origin)
        assertEquals("0x64", answers.first { it.first == "s1" }.second.getString("result"))
        router.route("s2", "net_version", "[]", origin)
        assertEquals("100", answers.first { it.first == "s2" }.second.getString("result"))
        router.route("s3", "eth_blockNumber", "[]", origin)
        assertEquals("0x2df8cad", answers.first { it.first == "s3" }.second.getString("result"))
        router.route("s4", "eth_getLogs", "[{}]", origin)
        assertEquals(-32005, answers.first { it.first == "s4" }.second.getJSONObject("error").getInt("code"))
        router.route("s5", "eth_gasPrice", "[]", origin)
        assertEquals(-32603, answers.first { it.first == "s5" }.second.getJSONObject("error").getInt("code"))
        router.route("s6", "wallet_switchEthereumChain", """[{"chainId":"0x1"}]""", origin)
        assertEquals(listOf(1), switched)
        assertTrue(answers.first { it.first == "s6" }.second.isNull("result"))
        router.route("s7", "wallet_switchEthereumChain", """[{"chainId":"0x89"}]""", origin)
        assertEquals(4902, answers.first { it.first == "s7" }.second.getJSONObject("error").getInt("code"))
        router.route("s8", "wallet_watchAsset", "[]", origin)
        assertTrue(answers.first { it.first == "s8" }.second.isNull("result"))
        router.route("s9", "eth_signTransaction", "[]", origin)
        assertEquals(4900, answers.first { it.first == "s9" }.second.getJSONObject("error").getInt("code"))
        router.route("s10", "eth_sendTransaction", "[]", origin)
        assertEquals(listOf("s10:eth_sendTransaction"), forwarded)
        assertNotNull(BrowserExecutor.connectionRow(safe, 100, origin, 1000L).getString("id"))
    }
}
