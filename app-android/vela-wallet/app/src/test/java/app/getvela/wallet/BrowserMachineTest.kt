package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DbrEvent
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.browser.core.DbrShellResult
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignResponsePayload
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
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.DappBrowserCore
import uniffi.vela_core_uniffi.dappBrowserInput
import uniffi.vela_core_uniffi.dappProviderScript

/**
 * Spec 070: the in-app browser through the REAL `dapp_browser` machine and
 * this shell's executor — the store keys every client shares, answers that go
 * to the tab that asked and nowhere else, the signing hand-off carrying the
 * granted address and the site's chain, and the script the page gets.
 * (The machine's own rules are the core's 59 conformance tests.)
 */
class BrowserMachineTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val origin = "http://127.0.0.1:8137"

    /** Everything the executor was asked to do, in order. */
    private val delivered = java.util.Collections.synchronizedList(ArrayList<Pair<String, JSONObject>>())
    private val forwarded = java.util.Collections.synchronizedList(ArrayList<DbrOperation.ForwardToSigning>())
    private val cancelled = java.util.Collections.synchronizedList(ArrayList<String>())
    private val records = java.util.Collections.synchronizedList(ArrayList<JSONObject>())
    private val reads = java.util.Collections.synchronizedList(ArrayList<String>())

    @After
    fun stop() = scope.cancel()

    private fun host(store: FakeStore): CoreHost<DbrView> {
        val executor = BrowserExecutor(
            store,
            object : BrowserExecutor.Ports {
                override fun deliver(tab: String, messageJson: String) { delivered += tab to JSONObject(messageJson) }
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): JSONObject? {
                    reads += "$chainId:$method:$bundler"
                    return JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", "0x10")
                }
                override suspend fun userOpTxHash(chainId: Int, userOpHash: String): String? = null
                override fun forwardToSigning(operation: DbrOperation.ForwardToSigning) { forwarded += operation }
                override fun cancelSigning(tab: String, id: String) { cancelled += "$tab/$id" }
                override fun saveConnectionRecord(row: JSONObject) { records += row }
            },
        )
        return CoreHost(
            bridge = DappBrowserCore().asBridge(), scope = scope, initial = DbrView(), serializer = DbrView.serializer(),
            perform = JsonShell.perform(DbrOperation.serializer(), DbrShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(DbrOperation.serializer(), DbrShellResult.serializer(), fallback = DbrShellResult.Ack, answer = executor::neutralAnswer),
        ).also { h ->
            h.dispatch(DbrEvent.Start, DbrEvent.serializer())
            h.dispatch(DbrEvent.NetworksChanged(listOf(1, 100, 8453)), DbrEvent.serializer())
            h.dispatch(DbrEvent.AccountsUpdated(listOf(safe)), DbrEvent.serializer())
            h.dispatch(DbrEvent.AccountSwitched(safe, 1.0e12), DbrEvent.serializer())
        }
    }

    private fun page(h: CoreHost<DbrView>, tab: String, message: JSONObject, from: String = origin, mainFrame: Boolean = true) =
        h.dispatch(DbrEvent.PageMessage(tab = tab, frame_origin = from, is_main_frame = mainFrame, message_json = message.toString()), DbrEvent.serializer())

    private fun hello(h: CoreHost<DbrView>, tab: String, doc: String) = page(h, tab, JSONObject().put("t", "hello").put("doc", doc))

    private fun ask(h: CoreHost<DbrView>, tab: String, doc: String, id: String, method: String, params: JSONArray = JSONArray()) =
        page(h, tab, JSONObject().put("t", "req").put("doc", doc).put("id", id).put("method", method).put("params", params))

    private suspend fun answerFor(id: String): Pair<String, JSONObject> {
        withTimeout(10_000) { while (delivered.none { it.second.optString("id") == id }) delay(10) }
        return delivered.first { it.second.optString("id") == id }
    }

    @Test
    fun `the page gets the core's script, one per host, with the bridge for Android`() {
        val script = dappProviderScript("android")
        assertTrue(script.startsWith("(function () {"))
        assertTrue("the Android bridge posts through the message listener", script.contains("VelaHost.postMessage(s)"))
        assertTrue("the provider itself is in it", script.contains("eip6963:announceProvider"))
        assertFalse("no module syntax survives", script.lines().any { it.trimStart().startsWith("import ") || it.trimStart().startsWith("export ") })
    }

    @Test
    fun `the address bar searches words and opens hosts`() {
        assertEquals("https://app.uniswap.org", dappBrowserInput("app.uniswap.org"))
        assertEquals("https://duckduckgo.com/?q=uniswap", dappBrowserInput("uniswap"))
        assertEquals("http://127.0.0.1:8137/", dappBrowserInput("127.0.0.1:8137/"))
        assertNull(dappBrowserInput("   "))
    }

    @Test
    fun `connect writes the grant under the shared key and answers the tab that asked`() = runBlocking<Unit> {
        val store = FakeStore()
        val h = host(store)
        hello(h, "tab-1", "d1")
        hello(h, "tab-2", "d2")
        ask(h, "tab-2", "d2", "c1", "eth_requestAccounts")
        val consent = withTimeout(10_000) { h.view.first { it.consent != null } }.consent!!
        assertEquals(origin, consent.origin)
        assertEquals(safe, consent.address)
        h.dispatch(DbrEvent.ConsentApproved(1.0e12 + 5), DbrEvent.serializer())
        val (tab, answer) = answerFor("c1")
        assertEquals("the answer goes to the tab that asked", "tab-2", tab)
        assertEquals(safe, answer.getJSONArray("result").getString(0))
        assertEquals("d2", answer.getString("doc"))
        withTimeout(10_000) { while (store.values[BrowserExecutor.grantKey(origin)].isNullOrBlank()) delay(20) }
        val grant = JSONObject(store.values.getValue(BrowserExecutor.grantKey(origin)))
        assertEquals(safe, grant.getString("address"))
        withTimeout(10_000) { while (records.isEmpty()) delay(20) }
        assertEquals("connect", records.single().getString("type"))
        assertEquals(origin, records.single().getString("dappOrigin"))
        assertEquals(1, withTimeout(10_000) { h.view.first { it.sites.isNotEmpty() } }.sites.size)
    }

    @Test
    fun `stored grants and chains are listed at start and a site keeps its chain`() = runBlocking<Unit> {
        val store = FakeStore(
            mapOf(
                BrowserExecutor.grantKey(origin) to """{"origin":"$origin","address":"$safe","chain_id":100,"granted_at_ms":1.0e12}""",
                BrowserExecutor.chainKey(origin) to "8453",
                BrowserExecutor.grantKey("https://broken.example") to "{not json",
            ),
        )
        val h = host(store)
        val view = withTimeout(10_000) { h.view.first { it.ready } }
        assertEquals("an unreadable grant is no grant", listOf(origin), view.sites.map { it.origin })
        assertEquals(8453, view.sites.single().chain_id)
        hello(h, "tab-1", "d1")
        ask(h, "tab-1", "d1", "q1", "eth_chainId")
        assertEquals("0x2105", answerFor("q1").second.getString("result"))
    }

    @Test
    fun `a signature is handed to the sheet with the granted address and the site's chain, and answered once`() = runBlocking<Unit> {
        val store = FakeStore(mapOf(BrowserExecutor.grantKey(origin) to """{"origin":"$origin","address":"$safe","chain_id":100,"granted_at_ms":1.0e12}"""))
        val h = host(store)
        withTimeout(10_000) { h.view.first { it.ready } }
        hello(h, "tab-1", "d1")
        ask(h, "tab-1", "d1", "s1", "personal_sign", JSONArray().put("0x68656c6c6f").put(safe))
        withTimeout(10_000) { while (forwarded.isEmpty()) delay(10) }
        val op = forwarded.single()
        assertEquals("tab-1", op.tab)
        assertEquals(safe, op.granted_address)
        assertEquals(100, op.chain_id)
        h.dispatch(DbrEvent.SigningAnswered("tab-1", "s1", SignResponsePayload.Err(4001, SignErrorKind.UserRejected, null), null), DbrEvent.serializer())
        val answer = answerFor("s1").second
        assertEquals(4001, answer.getJSONObject("error").getInt("code"))
        assertEquals("the core's words", "User rejected the request", answer.getJSONObject("error").getString("message"))
    }

    @Test
    fun `a navigation mid-signature settles the page and closes the sheet`() = runBlocking<Unit> {
        val store = FakeStore(mapOf(BrowserExecutor.grantKey(origin) to """{"origin":"$origin","address":"$safe","chain_id":100,"granted_at_ms":1.0e12}"""))
        val h = host(store)
        withTimeout(10_000) { h.view.first { it.ready } }
        hello(h, "tab-1", "d1")
        ask(h, "tab-1", "d1", "s1", "eth_sendTransaction", JSONArray().put(JSONObject().put("to", safe)))
        withTimeout(10_000) { while (forwarded.isEmpty()) delay(10) }
        hello(h, "tab-1", "d2")
        assertEquals(4900, answerFor("s1").second.getJSONObject("error").getInt("code"))
        withTimeout(10_000) { while (cancelled.isEmpty()) delay(10) }
        assertEquals(listOf("tab-1/s1"), cancelled.toList())
    }

    @Test
    fun `reads go through the pool on the site's chain, and a subframe is never heard`() = runBlocking<Unit> {
        val h = host(FakeStore())
        withTimeout(10_000) { h.view.first { it.ready } }
        hello(h, "tab-1", "d1")
        ask(h, "tab-1", "d1", "r1", "eth_blockNumber")
        assertEquals("0x10", answerFor("r1").second.getString("result"))
        assertEquals(listOf("1:eth_blockNumber:false"), reads.toList())
        page(h, "tab-1", JSONObject().put("t", "req").put("doc", "d1").put("id", "x").put("method", "eth_requestAccounts"), from = "https://ads.example", mainFrame = false)
        delay(200)
        assertNull(h.view.value.consent)
        assertTrue(delivered.none { it.second.optString("id") == "x" })
    }

    @Test
    fun `revoking removes the key and tells the open page`() = runBlocking<Unit> {
        val store = FakeStore(mapOf(BrowserExecutor.grantKey(origin) to """{"origin":"$origin","address":"$safe","chain_id":100,"granted_at_ms":1.0e12}"""))
        val h = host(store)
        withTimeout(10_000) { h.view.first { it.ready } }
        hello(h, "tab-1", "d1")
        h.dispatch(DbrEvent.RevokeRequested(origin), DbrEvent.serializer())
        withTimeout(10_000) { while (store.values.containsKey(BrowserExecutor.grantKey(origin))) delay(10) }
        withTimeout(10_000) { while (delivered.none { it.second.optString("event") == "disconnect" }) delay(10) }
        val events = delivered.filter { it.second.optString("dir") == "evt" }.map { it.second.getString("event") }
        assertEquals(listOf("accountsChanged", "disconnect"), events)
        assertNotNull(h.view.value)
        assertTrue(h.view.value.sites.isEmpty())
    }
}
