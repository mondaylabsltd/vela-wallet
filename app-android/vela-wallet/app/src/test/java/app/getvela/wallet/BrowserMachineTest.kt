package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.browser.core.BrowserDebugMode
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DbrEvent
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.browser.core.DbrShellResult
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.browser.core.ProviderBridge
import app.getvela.wallet.feature.browser.core.ProviderScript
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
    private val delivered = java.util.concurrent.CopyOnWriteArrayList<Pair<String, JSONObject>>()
    private val forwarded = java.util.concurrent.CopyOnWriteArrayList<DbrOperation.ForwardToSigning>()
    private val cancelled = java.util.concurrent.CopyOnWriteArrayList<String>()
    private val records = java.util.concurrent.CopyOnWriteArrayList<JSONObject>()
    private val reads = java.util.concurrent.CopyOnWriteArrayList<String>()

    /** What a read answers, by method — `"0x10"` unless a test says otherwise. */
    @Volatile private var readResult: (String) -> Any = { "0x10" }

    @After
    fun stop() = scope.cancel()

    private fun host(store: FakeStore): CoreHost<DbrView> {
        val executor = BrowserExecutor(
            store,
            object : BrowserExecutor.Ports {
                override fun deliver(tab: String, messageJson: String) { delivered += tab to JSONObject(messageJson) }
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): JSONObject? {
                    reads += "$chainId:$method:$bundler"
                    return JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", readResult(method))
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
    fun `the page gets the core's script for each debug mode, with the bridge for Android`() {
        val ordinary = ProviderBridge.script(debugMode = false)
        val debug = ProviderBridge.script(debugMode = true)
        assertEquals(dappProviderScript("android", false), ordinary)
        assertEquals(dappProviderScript("android", true), debug)
        for (script in listOf(ordinary, debug)) {
            assertTrue(script.startsWith("(function () {"))
            assertTrue("the Android bridge posts through the message listener", script.contains("VelaHost.postMessage(s)"))
            assertTrue("the provider itself is in it", script.contains("eip6963:announceProvider"))
            assertFalse("no module syntax survives", script.lines().any { it.trimStart().startsWith("import ") || it.trimStart().startsWith("export ") })
        }
        // Spec 088 FR-004: debug mode off, only a secure context (https, or http on loopback) is offered the wallet.
        assertTrue("no provider off a secure context", ordinary.contains("(window.top !== window || !window.isSecureContext) return;"))
        assertFalse("and no host test at all", ordinary.contains("location.hostname"))
        // Spec 091: debug mode on, http on this device's own network too — the core's host test, no copy here.
        assertTrue(debug.contains("window.isSecureContext || (location.protocol === 'http:'"))
        assertTrue(debug.contains("(location.hostname)"))
    }

    /** A LAN dApp under development, and what it asks first. */
    private val lan = "http://192.168.1.5:3000"

    private fun lanAsk(h: CoreHost<DbrView>, tab: String, doc: String, id: String, method: String) =
        page(h, tab, JSONObject().put("t", "req").put("doc", doc).put("id", id).put("method", method).put("params", JSONArray()), from = lan)

    /**
     * Spec 091: the controller's debug mode — stated to the core at start and
     * on every change, and every open tab's document-start script swapped for
     * its next document. The engines here are [ProviderScript]s over a fake
     * WebView (the script list `WebViewCompat` keeps); the gate is the REAL core.
     */
    @Test
    fun `debug mode swaps every tab's script and moves the core's gate`() = runBlocking<Unit> {
        val h = host(FakeStore())
        withTimeout(10_000) { h.view.first { it.ready } }
        val webViews = List(2) { mutableListOf<String>() }
        val engines = webViews.map { scripts -> ProviderScript(debugMode = false) { script -> scripts += script; { scripts.remove(script) } } }
        val stated = java.util.concurrent.CopyOnWriteArrayList<DbrEvent>()
        val debug = BrowserDebugMode(initial = false, dispatch = { stated += it; h.dispatch(it, DbrEvent.serializer()) }) { on ->
            engines.forEach { it.swap(on) }
        }
        assertEquals("stated at start", listOf<DbrEvent>(DbrEvent.DebugModeChanged(false)), stated.toList())
        assertEquals(List(2) { listOf(dappProviderScript("android", false)) }, webViews)

        // Off: a LAN page calling the bridge by hand is never answered.
        page(h, "tab-lan", JSONObject().put("t", "hello").put("doc", "d0"), from = lan)
        lanAsk(h, "tab-lan", "d0", "q-off", "eth_chainId")

        debug.set(true)
        assertEquals(DbrEvent.DebugModeChanged(true), stated.last())
        assertEquals("one script per WebView, the debug one", List(2) { listOf(dappProviderScript("android", true)) }, webViews)
        // The same mode again changes nothing.
        debug.set(true)
        assertEquals(2, stated.size)

        // On: the page's next document is offered the wallet and answered.
        page(h, "tab-lan", JSONObject().put("t", "hello").put("doc", "d1"), from = lan)
        lanAsk(h, "tab-lan", "d1", "q-on", "eth_chainId")
        assertEquals("tab-lan", answerFor("q-on").first)
        assertTrue("nothing from before it was on: $delivered", delivered.none { it.second.optString("id") == "q-off" })

        // Off again, with a request open: withdrawn at once — answered 4900 — and the scripts go back.
        lanAsk(h, "tab-lan", "d1", "c-on", "eth_requestAccounts")
        withTimeout(10_000) { h.view.first { it.consent != null } }
        debug.set(false)
        assertEquals(4900, answerFor("c-on").second.getJSONObject("error").getInt("code"))
        withTimeout(10_000) { h.view.first { it.consent == null } }
        assertEquals(List(2) { listOf(dappProviderScript("android", false)) }, webViews)
        lanAsk(h, "tab-lan", "d1", "q-after", "eth_chainId")
        hello(h, "tab-ok", "ok")
        ask(h, "tab-ok", "ok", "q-ok", "eth_chainId")
        answerFor("q-ok")
        assertTrue("the page's later messages are ignored", delivered.none { it.second.optString("id") == "q-after" })
    }

    /** Spec 091: a controller built with debug mode on says so before any page message is read. */
    @Test
    fun `debug mode on at start is the core's before the first page message`() = runBlocking<Unit> {
        val h = host(FakeStore())
        BrowserDebugMode(initial = true, dispatch = { h.dispatch(it, DbrEvent.serializer()) }) {}
        withTimeout(10_000) { h.view.first { it.ready } }
        page(h, "tab-lan", JSONObject().put("t", "hello").put("doc", "d1"), from = lan)
        lanAsk(h, "tab-lan", "d1", "q1", "eth_chainId")
        assertEquals("tab-lan", answerFor("q1").first)
    }

    /**
     * Spec 088 FR-004: the listener cannot be limited to "any https origin"
     * (WebView's origin rules have no such pattern), so the bridge object is in
     * every page — and the core reads nothing a page off a secure context sends.
     */
    @Test
    fun `a page off a secure context is never answered`() = runBlocking<Unit> {
        val h = host(FakeStore())
        withTimeout(10_000) { h.view.first { it.ready } }
        for ((i, insecure) in listOf("http://dapp.example", "http://192.168.1.4:8137", "http://127.0.0.1.evil.com").withIndex()) {
            page(h, "tab-$i", JSONObject().put("t", "hello").put("doc", "d$i"), from = insecure)
            page(h, "tab-$i", JSONObject().put("t", "req").put("doc", "d$i").put("id", "x$i").put("method", "eth_chainId").put("params", JSONArray()), from = insecure)
            page(h, "tab-$i", JSONObject().put("t", "req").put("doc", "d$i").put("id", "c$i").put("method", "eth_requestAccounts").put("params", JSONArray()), from = insecure)
        }
        // The same request from the loopback test page IS answered — so silence above is the rule, not a slow core.
        hello(h, "tab-ok", "ok")
        ask(h, "tab-ok", "ok", "q-ok", "eth_chainId")
        answerFor("q-ok")
        assertTrue("nothing delivered to an insecure page: $delivered", delivered.all { it.first == "tab-ok" })
        assertNull(h.view.value.consent)
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
        assertEquals("user_rejected", answer.getJSONObject("error").getString("kind"))
    }

    /**
     * Spec 081, device-found — through this shell, where the report came from.
     *
     * A refused request was answered with the refused FUNCTION's name and
     * nothing else ("addOwnerWithThreshold"), which reads as a label rather
     * than an answer. The page now gets a sentence that names it, and a
     * machine-readable `kind` beside the code.
     *
     * This lived in `SigningLiveTest` against `SignExecutor.responseJson` while
     * the shell built the page's answer itself. Spec 070 moved that into the
     * core (`dapp_browser` → `dapp_rpc::sign_error_json`), so the assertion
     * moved to where the answer is now made — and it is the REAL machine
     * answering here, not this shell's copy of the rule.
     */
    @Test
    fun `a refused request tells the page what happened and why`() = runBlocking<Unit> {
        val store = FakeStore(mapOf(BrowserExecutor.grantKey(origin) to """{"origin":"$origin","address":"$safe","chain_id":100,"granted_at_ms":1.0e12}"""))
        val h = host(store)
        withTimeout(10_000) { h.view.first { it.ready } }
        hello(h, "tab-1", "d1")
        ask(h, "tab-1", "d1", "s1", "eth_sendTransaction", JSONArray().put(JSONObject().put("to", safe)))
        withTimeout(10_000) { while (forwarded.isEmpty()) delay(10) }
        h.dispatch(
            DbrEvent.SigningAnswered(
                "tab-1",
                "s1",
                SignResponsePayload.Err(-32603, SignErrorKind.SelfCallBlocked, "addOwnerWithThreshold"),
                null,
            ),
            DbrEvent.serializer(),
        )
        val error = answerFor("s1").second.getJSONObject("error")
        assertEquals(-32603, error.getInt("code"))
        assertEquals("self_call_blocked", error.getString("kind"))
        val message = error.getString("message")
        assertTrue("a sentence, not a label: $message", message.contains("refused a call that would change who controls"))
        assertTrue("and it names what was refused: $message", message.contains("addOwnerWithThreshold"))
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

    /**
     * Spec 097 E (S2): a batch answered with its id — the user operation's
     * hash, which the signing core now answers the moment the relay takes it
     * — is a batch this browser knows, with no op named by the sheet (the
     * answer can come before `rememberUserOp`). `wallet_getCallsStatus` of it
     * goes to the relay on the chain the batch went to and reads 200 with the
     * receipt once it landed. Before, the phones answered the TX hash, which
     * no status lookup knows. The pass's PancakeSwap batch on BNB Chain.
     */
    @Test
    fun `a batch answered with its id reads 200 with its receipt once landed`() = runBlocking<Unit> {
        val op = "0x8a74fe42e2eac06e4d9fc59c8be722807eb0de072e4e96b2715385b4dd075cc3"
        val tx = "0x3c8446de48ff3ee143e40c4209068662fe5b5952bcc9dc28049d0b80631fee29"
        val store = FakeStore(mapOf(BrowserExecutor.grantKey(origin) to """{"origin":"$origin","address":"$safe","chain_id":56,"granted_at_ms":1.0e12}"""))
        val h = host(store)
        h.dispatch(DbrEvent.NetworksChanged(listOf(1, 56, 100)), DbrEvent.serializer())
        withTimeout(10_000) { h.view.first { it.ready } }
        hello(h, "tab-1", "d1")
        val calls = JSONArray().put(JSONObject().put("to", "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d").put("data", "0x"))
        ask(h, "tab-1", "d1", "b1", "wallet_sendCalls", JSONArray().put(JSONObject().put("from", safe).put("calls", calls)))
        withTimeout(10_000) { while (forwarded.isEmpty()) delay(10) }
        assertEquals(56, forwarded.single().chain_id)
        h.dispatch(DbrEvent.SigningAnswered("tab-1", "b1", SignResponsePayload.Ok(op), null), DbrEvent.serializer())
        assertEquals(op, answerFor("b1").second.getString("result"))

        readResult = { method ->
            when (method) {
                "eth_getUserOperationReceipt" -> JSONObject()
                    .put("success", true)
                    .put("logs", JSONArray())
                    .put("receipt", JSONObject().put("transactionHash", tx).put("blockNumber", "0x778456d").put("status", "0x1"))
                else -> JSONObject.NULL
            }
        }
        ask(h, "tab-1", "d1", "s1", "wallet_getCallsStatus", JSONArray().put(op))
        val status = answerFor("s1").second.getJSONObject("result")
        assertEquals(200, status.getInt("status"))
        assertEquals(op, status.getString("id"))
        assertEquals("0x38", status.getString("chainId"))
        assertEquals(tx, status.getJSONArray("receipts").getJSONObject(0).getString("transactionHash"))
        assertTrue(reads.toList().toString(), reads.contains("56:eth_getUserOperationReceipt:true"))

        // The tx hash is no batch id.
        ask(h, "tab-1", "d1", "s2", "wallet_getCallsStatus", JSONArray().put(tx))
        assertEquals(5730, answerFor("s2").second.getJSONObject("error").getInt("code"))
    }
}
