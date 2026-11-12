package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DbrEvent
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.browser.core.DbrShellResult
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.signing.core.ConfirmBlock
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.ParentReason
import app.getvela.wallet.feature.signing.core.SafeOwnerAnswer
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignExecutor
import app.getvela.wallet.feature.signing.core.SignOperation
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.SignShellResult
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcResult
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CompletableDeferred
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
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.DappBrowserCore

/**
 * VELA-2026-002 (the 0.9.9 hotfix) on Android — every dApp entry point routes
 * through the core's parent-Safe guard.
 *
 * This shell has one way in for a site: the in-app browser (`dapp_browser`).
 * Its forwarded request goes to the Vela account's signing machines
 * (`SigningController` → `sign_request`), as the app opens them
 * (`AppContainer.openSigning`). Driven here, REAL machines over uniffi, with
 * the vector's 1-of-N takeover — `parent.execTransaction(swapOwner)` with the
 * "approved by msg.sender" signature: every Safe it would act for is read
 * (`read_safe_owner` → this shell's `eth_call`) before anything can be
 * signed; owned → refused, nothing signed, and the page answered `-32603`
 * `parent_safe_blocked` once the person closes it; unreadable → refused
 * "couldn't check"; not owned → the request goes on as before.
 */
class ParentSafeEntryPointsTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle"))
    private val vector = JSONObject(File(repoRoot, "rust/crates/vela-core/tests/vectors/rules/parent-safe-guard.json").readText())
    private val signer: String = vector.getString("signer")
    private val parent: String = vector.getString("parent")
    private val unknown: String = vector.getString("unknown")
    private val chain: Int = vector.getInt("chainId")
    private val origin = "http://127.0.0.1:8137"
    private val strings: VelaStrings =
        I18nRuntime { tag -> File(repoRoot, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }

    @After
    fun stop() = scope.cancel()

    private fun case(name: String): JSONObject {
        val cases = vector.getJSONArray("cases")
        return (0 until cases.length()).map(cases::getJSONObject).first { it.getString("name") == name }.getJSONObject("input")
    }

    private val takeover by lazy { case("a: execTransaction(swapOwner) on a Safe this account owns, v=1") }

    /** The browser's forward, as `AppContainer.openSigning` opens the signing machines for it. */
    private fun incoming(operation: DbrOperation.ForwardToSigning) = IncomingRequest(
        id = operation.id,
        method = operation.method,
        paramsJson = operation.params_json,
        origin = operation.origin,
        transportId = operation.tab,
        chainId = operation.chain_id,
        grantedAddress = operation.granted_address,
    )

    private fun word(b: Int) = "0x" + "0".repeat(63) + b.toString()

    private fun returned(value: String) = RpcResult.Body(JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", value))

    // -- the in-app browser ------------------------------------------------------

    private val delivered = CopyOnWriteArrayList<Pair<String, JSONObject>>()
    private val forwarded = CopyOnWriteArrayList<DbrOperation.ForwardToSigning>()

    /** The REAL browser machine, with [account] granted to [origin] on [grantChain]. */
    private fun browser(account: String, grantChain: Int, alsoAccounts: List<String> = emptyList()): CoreHost<DbrView> {
        val store = FakeStore(
            mapOf(BrowserExecutor.grantKey(origin) to """{"origin":"$origin","address":"$account","chain_id":$grantChain,"granted_at_ms":1.0e12}"""),
        )
        val executor = BrowserExecutor(
            store,
            object : BrowserExecutor.Ports {
                override fun deliver(tab: String, messageJson: String) { delivered += tab to JSONObject(messageJson) }
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): BrowserExecutor.ReadAnswer =
                    BrowserExecutor.ReadAnswer.Body(JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", "0x10"))
                override suspend fun userOpTxHash(chainId: Int, userOpHash: String): String? = null
                override fun forwardToSigning(operation: DbrOperation.ForwardToSigning) { forwarded += operation }
                override fun cancelSigning(tab: String, id: String) = Unit
                override fun saveConnectionRecord(row: JSONObject) = Unit
            },
        )
        return CoreHost(
            bridge = DappBrowserCore().asBridge(), scope = scope, initial = DbrView(), serializer = DbrView.serializer(),
            perform = JsonShell.perform(DbrOperation.serializer(), DbrShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(DbrOperation.serializer(), DbrShellResult.serializer(), fallback = DbrShellResult.Ack, answer = executor::neutralAnswer),
        ).also { h ->
            h.dispatch(DbrEvent.Start, DbrEvent.serializer())
            h.dispatch(DbrEvent.NetworksChanged(listOf(1, 100, chain)), DbrEvent.serializer())
            h.dispatch(DbrEvent.AccountsUpdated(listOf(account) + alsoAccounts), DbrEvent.serializer())
            h.dispatch(DbrEvent.AccountSwitched(account, 1.0e12), DbrEvent.serializer())
        }
    }

    private fun page(h: CoreHost<DbrView>, message: JSONObject) =
        h.dispatch(DbrEvent.PageMessage(tab = "tab-1", frame_origin = origin, is_main_frame = true, message_json = message.toString()), DbrEvent.serializer())

    /** A page in tab-1 asks [method] with [params]; the browser machine forwards it to signing. */
    private suspend fun pageAsks(h: CoreHost<DbrView>, id: String, method: String, params: JSONArray): DbrOperation.ForwardToSigning {
        withTimeout(10_000) { h.view.first { it.ready } }
        page(h, JSONObject().put("t", "hello").put("doc", "d1"))
        page(h, JSONObject().put("t", "req").put("doc", "d1").put("id", id).put("method", method).put("params", params))
        withTimeout(10_000) { while (forwarded.none { it.id == id }) delay(10) }
        return forwarded.first { it.id == id }
    }

    private suspend fun pageAnswer(id: String): JSONObject {
        withTimeout(10_000) { while (delivered.none { it.second.optString("id") == id }) delay(10) }
        return delivered.first { it.second.optString("id") == id }.second
    }

    // -- the Vela account's signing machines ---------------------------------------

    private val responses = CopyOnWriteArrayList<SignResponsePayload>()
    private val reads = CopyOnWriteArrayList<Triple<Int, String, String>>()
    private val signs = AtomicInteger()

    private fun controller(
        safeOwner: suspend (chainId: Int, safe: String, data: String) -> RpcResult = { _, _, _ -> RpcResult.Failed(rateLimited = false) },
    ): SigningController = SigningController(
        scope = scope,
        relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
        feed = FeedExecutor(store = FakeStore(), ownAccounts = { emptyList() }),
        accounts = StoreAccountPort(AccountStore(FakeStore())),
        signer = {
            object : UserOpSigner {
                override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
                    signs.incrementAndGet()
                    error("a refused request is never signed")
                }
            }
        },
        knownChains = { listOf(1, 100, chain) },
        wallet = SignAccountRef(address = signer, credential_id = "00"),
        ports = object : SigningController.Ports {
            override fun respond(transportId: String, id: String, payload: SignResponsePayload) { responses += payload }
            override fun opSubmitted(id: String, submitted: app.getvela.wallet.feature.send.core.UserOpSpine.Submitted) { signs.incrementAndGet() }
            override fun recordsPersisted() = Unit
            override fun recordPersisted(recordId: String) = Unit
            override suspend fun switchAccount(address: String) = true
            override fun nativeSymbol(chainId: Int) = "ETH"
            override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) = Unit
            override fun dataBase() = ""
            override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
            override suspend fun safeOwnerCall(chainId: Int, safe: String, data: String): RpcResult {
                reads += Triple(chainId, safe, data)
                return safeOwner(chainId, safe, data)
            }
        },
    )

    private suspend fun refused(c: SigningController): SignView =
        withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet && it.blocked != null } }

    /** However it is asked, nothing is signed; closing answers the page "the wallet refused", never 4001. */
    private suspend fun closeRefused(c: SigningController): SignResponsePayload.Err {
        c.approve()
        delay(300)
        assertEquals("nothing signed or sent", 0, signs.get())
        c.reject()
        withTimeout(10_000) { while (responses.isEmpty()) delay(10) }
        assertEquals("answered once", 1, responses.size)
        val answer = responses.single() as SignResponsePayload.Err
        assertEquals(-32603, answer.code)
        assertEquals(SignErrorKind.ParentSafeBlocked, answer.kind)
        return answer
    }

    private fun sheetWords(view: SignView): String =
        SigningLive.statusBlocks(view, strings).filterIsInstance<SigningBlock.Warning>().single().text

    @Test
    fun `the browser's takeover of a Safe this account owns is refused after the read, never signed, answered -32603`() = runBlocking<Unit> {
        val h = browser(signer, chain)
        val op = pageAsks(h, "s1", takeover.getString("method"), takeover.getJSONArray("params"))
        assertTrue("the granted account, as the browser says it: ${op.granted_address}", op.granted_address.equals(signer, ignoreCase = true))
        val c = controller { _, _, _ -> returned(word(1)) }
        c.open(incoming(op))
        val view = refused(c)
        val blocked = view.blocked!!
        assertEquals(parent, blocked.parent)
        assertEquals(ParentReason.Owner, blocked.reason)
        assertEquals("execTransaction", blocked.function)
        assertFalse(view.confirm_gate_open)
        assertEquals(ConfirmBlock.Refused, view.confirm_block)
        assertEquals(SignErrorKind.ParentSafeBlocked, view.error?.kind)
        assertEquals("one isOwner read of the Safe: $reads", listOf(parent), reads.map { it.second })
        // The sheet says it in the catalog's words, by that Safe.
        assertEquals(
            strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to "execTransaction", "safe" to ExploreLive.shortAddress(parent))),
            sheetWords(view),
        )
        val answer = closeRefused(c)
        // The page hears it from the browser machine, as the app hands it back.
        h.dispatch(DbrEvent.SigningAnswered("tab-1", "s1", answer, null), DbrEvent.serializer())
        val error = pageAnswer("s1").getJSONObject("error")
        assertEquals(-32603, error.getInt("code"))
        assertEquals("parent_safe_blocked", error.getString("kind"))
    }

    @Test
    fun `every named hotfix shape a page sends is refused once read owned`() = runBlocking<Unit> {
        for ((n, short) in listOf("exec-swap", "exec-add", "exec-thr", "approveHash", "MultiSend-nested").withIndex()) {
            responses.clear()
            reads.clear()
            val input = case("hotfix $short: isOwner true, refused")
            val h = browser(signer, chain)
            val op = pageAsks(h, "h$n", input.getString("method"), input.getJSONArray("params"))
            val c = controller { _, _, _ -> returned(word(1)) }
            c.open(incoming(op))
            val view = refused(c)
            assertEquals(short, unknown, view.blocked!!.parent)
            assertEquals(short, ParentReason.Owner, view.blocked!!.reason)
            closeRefused(c)
        }
    }

    @Test
    fun `the takeover of a Safe this account does not own goes on as before`() = runBlocking<Unit> {
        val input = case("hotfix exec-swap: unknown Safe, not owner, allowed")
        val h = browser(signer, chain)
        val op = pageAsks(h, "s2", input.getString("method"), input.getJSONArray("params"))
        val c = controller { _, _, _ -> returned(word(0)) }
        c.open(incoming(op))
        withTimeout(10_000) { while (reads.isEmpty()) delay(10) }
        val view = withTimeout(20_000) {
            c.sign.first { it.surface == SignSurface.Sheet && it.confirm_block != ConfirmBlock.Reading }
        }
        assertNull("not this account's Safe: no refusal", view.blocked)
        assertTrue("and nothing answered for it", responses.isEmpty())
    }

    @Test
    fun `an unknown Safe is read through this shell's eth_call, and one this account owns is refused`() = runBlocking<Unit> {
        val input = case("unknown Safe, isOwner true: refused")
        val owned = vector.getJSONArray("cases").let { cases -> (0 until cases.length()).map(cases::getJSONObject) }
            .first { it.getString("name") == "unknown Safe, isOwner true: refused" }
            .getJSONObject("input").getJSONArray("answers").getJSONObject(0).getJSONObject("answer").getString("data")
        val h = browser(signer, chain)
        val op = pageAsks(h, "s3", input.getString("method"), input.getJSONArray("params"))
        val gate = CompletableDeferred<Unit>()
        val c = controller { _, _, _ ->
            gate.await()
            RpcResult.Body(JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", owned))
        }
        c.open(incoming(op))
        // The read is out: nothing can be confirmed while it is.
        val reading = withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet && it.confirm_block == ConfirmBlock.Reading } }
        assertFalse(reading.confirm_gate_open)
        assertNull(reading.blocked)
        withTimeout(10_000) { while (reads.isEmpty()) delay(10) }
        val (readChain, readSafe, data) = reads.single()
        assertEquals(chain, readChain)
        assertEquals(unknown, readSafe)
        assertEquals("isOwner(the signer), built by the core", vector.getString("isOwnerSelector") + "0".repeat(24) + signer.removePrefix("0x"), data)
        gate.complete(Unit)
        val view = refused(c)
        assertEquals(unknown, view.blocked!!.parent)
        assertEquals(ParentReason.Owner, view.blocked!!.reason)
        assertEquals(
            strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to "execTransaction", "safe" to ExploreLive.shortAddress(unknown))),
            sheetWords(view),
        )
        val answer = closeRefused(c)
        h.dispatch(DbrEvent.SigningAnswered("tab-1", "s3", answer, null), DbrEvent.serializer())
        assertEquals("parent_safe_blocked", pageAnswer("s3").getJSONObject("error").getString("kind"))
    }

    @Test
    fun `an owner read nobody could answer refuses the request as couldn't check`() = runBlocking<Unit> {
        val input = case("isOwner unreadable (no node answered): refused, couldn't check")
        val h = browser(signer, chain)
        val op = pageAsks(h, "s4", input.getString("method"), input.getJSONArray("params"))
        val c = controller { _, _, _ -> RpcResult.Failed(rateLimited = false) }
        c.open(incoming(op))
        val view = refused(c)
        assertEquals(unknown, view.blocked!!.parent)
        assertEquals(ParentReason.Unchecked, view.blocked!!.reason)
        assertEquals("approveHash", view.blocked!!.function)
        assertEquals(1, reads.size)
        assertEquals(
            strings.t("componentsUi.signing.parentSafeUnchecked", mapOf("function" to "approveHash", "safe" to ExploreLive.shortAddress(unknown))),
            sheetWords(view),
        )
        closeRefused(c)
    }

    /**
     * 0.9.10: a page writes the Safe's address without its `0x` (or `0X`, or
     * under-width) — the encoders read it as the Safe, 0.9.9's guard did not.
     * Every arrival case of the vector is answered -32602 before any read or
     * sheet, nothing signed, and the page hears it.
     */
    @Test
    fun `a non-canonical address from a page is answered -32602 before any read, never signed`() = runBlocking<Unit> {
        val cases = vector.getJSONArray("cases").let { all -> (0 until all.length()).map(all::getJSONObject) }
            .filter { it.getJSONObject("expect").optString("verdict") == "invalid" }
        assertTrue("the arrival cases: ${cases.size}", cases.size >= 7)
        for ((n, c) in cases.withIndex()) {
            val name = c.getString("name")
            responses.clear()
            reads.clear()
            val input = c.getJSONObject("input")
            val h = browser(signer, chain)
            val op = pageAsks(h, "b$n", input.getString("method"), input.getJSONArray("params"))
            val ctl = controller { _, _, _ -> returned(word(1)) }
            ctl.open(incoming(op))
            withTimeout(10_000) { while (responses.isEmpty()) delay(10) }
            val answer = responses.single() as SignResponsePayload.Err
            assertEquals(name, -32602, answer.code)
            assertEquals(name, SignErrorKind.InvalidParams, answer.kind)
            assertTrue("$name: never read: $reads", reads.isEmpty())
            assertEquals(name, 0, signs.get())
            h.dispatch(DbrEvent.SigningAnswered("tab-1", "b$n", answer, null), DbrEvent.serializer())
            assertEquals(name, -32602, pageAnswer("b$n").getJSONObject("error").getInt("code"))
        }
    }

    /** 0.9.10: a page's SafeMessage / SafeOp for a Safe v1.5.0 this account owns is its approval — read, refused. */
    @Test
    fun `a SafeMessage or SafeOp for an owned Safe v1_5_0 is refused once read owned`() = runBlocking<Unit> {
        for ((n, pair) in listOf(
            "0.9.10 v1.5.0: SafeMessage under an owned Safe v1.5.0's domain, isOwner true: refused" to "SafeMessage",
            "0.9.10 v1.5.0: SafeOp for an owned Safe v1.5.0 (its `safe`), under the 4337 module's domain: refused" to "SafeOp",
        ).withIndex()) {
            val (name, function) = pair
            responses.clear()
            reads.clear()
            val input = case(name)
            val h = browser(signer, chain)
            val op = pageAsks(h, "v$n", input.getString("method"), input.getJSONArray("params"))
            val ctl = controller { _, _, _ -> returned(word(1)) }
            ctl.open(incoming(op))
            val view = refused(ctl)
            assertEquals(name, parent, view.blocked!!.parent)
            assertEquals(name, ParentReason.Owner, view.blocked!!.reason)
            assertEquals(name, function, view.blocked!!.function)
            assertEquals(name, listOf(parent), reads.map { it.second })
            assertEquals(
                strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to function, "safe" to ExploreLive.shortAddress(parent))),
                sheetWords(view),
            )
            closeRefused(ctl)
        }
    }

    @Test
    fun `the read is answered as the node said - result verbatim, its error, or unreachable`() {
        fun body(json: JSONObject) = RpcResult.Body(json)
        val one = "0x" + "0".repeat(63) + "1"
        assertEquals(SafeOwnerAnswer.Returned(one), SignExecutor.safeOwnerAnswer(8453, body(JSONObject().put("result", one))))
        assertEquals(SafeOwnerAnswer.Returned("0x"), SignExecutor.safeOwnerAnswer(8453, body(JSONObject().put("result", "0x"))))
        assertEquals(
            SafeOwnerAnswer.RpcError(3, "execution reverted"),
            SignExecutor.safeOwnerAnswer(8453, body(JSONObject().put("error", JSONObject().put("code", 3).put("message", "execution reverted")))),
        )
        assertEquals(
            SafeOwnerAnswer.RpcError(null, "odd"),
            SignExecutor.safeOwnerAnswer(8453, body(JSONObject().put("error", JSONObject().put("message", "odd")))),
        )
        assertTrue(SignExecutor.safeOwnerAnswer(8453, RpcResult.Failed(rateLimited = true)) is SafeOwnerAnswer.Unreachable)
        assertTrue(SignExecutor.safeOwnerAnswer(8453, RpcResult.Failed(rateLimited = false)) is SafeOwnerAnswer.Unreachable)
        assertTrue("the backstop ran out", SignExecutor.safeOwnerAnswer(8453, null) is SafeOwnerAnswer.Unreachable)
        assertTrue("no result is no answer", SignExecutor.safeOwnerAnswer(8453, body(JSONObject().put("jsonrpc", "2.0"))) is SafeOwnerAnswer.Unreachable)
        // The wire, both ways: the core's operation decodes, and the answer echoes it.
        val op = Wire.json.decodeFromString(
            SignOperation.serializer(),
            """{"type":"read_safe_owner","id":"r1","chain_id":8453,"safe":"$unknown","account":"$signer","data":"0x2f54bf6e"}""",
        ) as SignOperation.ReadSafeOwner
        assertEquals(unknown, op.safe)
        val result = SignShellResult.SafeOwnerRead(op.id, op.chain_id, op.safe, SafeOwnerAnswer.RpcError(-32005, "rate limited"))
        assertEquals(
            """{"type":"safe_owner_read","id":"r1","chain_id":8453,"safe":"$unknown","answer":{"type":"rpc_error","code":-32005,"message":"rate limited"}}""",
            Wire.json.encodeToString(SignShellResult.serializer(), result),
        )
    }
}
