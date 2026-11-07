package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.format.ShortAddress
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DbrEvent
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.browser.core.DbrShellResult
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigRpcReply
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.long
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningLive
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
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
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

/**
 * §31 on Android — every dApp entry point routes through the core's
 * parent-Safe guard.
 *
 * This shell has one way in for a site: the in-app browser (`dapp_browser`).
 * Its forwarded request goes to the Vela account's signing machines
 * (`SigningController` → `sign_request`) or, for a multisig the site was
 * shown, to the proposals machine (`MultisigHub.dappRequest`). (WalletConnect
 * is not a way in here — a `wc:` link is said "unsupported"; PeerConnect is
 * the multisig's outside-owner session, where Vela asks a wallet, not a site
 * asking Vela.) Both are driven here, REAL machines over uniffi, with the
 * vector's 1-of-N takeover — `parent.execTransaction(swapOwner)` with the
 * "approved by msg.sender" signature — and the device's multisig list naming
 * the vector's `parent`: refused, nothing signed, and the page answered
 * `-32603` `parent_safe_blocked` once the person closes it. And a Safe not on
 * the list is read (`read_safe_owner` → this shell's `eth_call`) before
 * anything can be signed: owned → refused; unreadable → refused "couldn't check".
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

    private val takeover by lazy { case("a: execTransaction(swapOwner) on a multisig on the list, v=1") }

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
        multisigs: kotlinx.coroutines.flow.StateFlow<List<String>>?,
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
            override fun multisigs() = multisigs
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
    fun `the browser's takeover of a multisig on the list is refused at once, never signed, answered -32603`() = runBlocking<Unit> {
        val h = browser(signer, chain)
        val op = pageAsks(h, "s1", takeover.getString("method"), takeover.getJSONArray("params"))
        assertTrue("the granted account, as the browser says it: ${op.granted_address}", op.granted_address.equals(signer, ignoreCase = true))
        val c = controller(multisigs = MutableStateFlow(listOf(parent)))
        c.open(IncomingRequest.fromBrowser(op))
        val view = refused(c)
        val blocked = view.blocked!!
        assertEquals(parent, blocked.parent)
        assertEquals(ParentReason.Member, blocked.reason)
        assertEquals("execTransaction", blocked.function)
        assertFalse(view.confirm_gate_open)
        assertEquals(ConfirmBlock.Refused, view.confirm_block)
        assertEquals(SignErrorKind.ParentSafeBlocked, view.error?.kind)
        assertTrue("a multisig on the list is refused without a read: $reads", reads.isEmpty())
        // The sheet says it in the catalog's words, by that Safe.
        assertEquals(
            strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to "execTransaction", "safe" to ShortAddress.of(parent))),
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
    fun `a multisig list that changes while the sheet is up refuses the request on it`() = runBlocking<Unit> {
        val list = MutableStateFlow(emptyList<String>())
        val h = browser(signer, chain)
        val op = pageAsks(h, "s2", takeover.getString("method"), takeover.getJSONArray("params"))
        val gate = CompletableDeferred<RpcResult>()
        val c = controller(multisigs = list) { _, _, _ -> gate.await() }
        c.open(IncomingRequest.fromBrowser(op))
        // Not on the list yet: read first — the confirm stays shut while it is out.
        val reading = withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet && it.confirm_block == ConfirmBlock.Reading } }
        assertFalse(reading.confirm_gate_open)
        list.value = listOf(parent)
        val view = refused(c)
        assertEquals(parent, view.blocked!!.parent)
        assertEquals(ParentReason.Member, view.blocked!!.reason)
        gate.complete(RpcResult.Failed(rateLimited = false))
        closeRefused(c)
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
        val c = controller(multisigs = MutableStateFlow(listOf(parent))) { _, _, _ ->
            gate.await()
            RpcResult.Body(JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", owned))
        }
        c.open(IncomingRequest.fromBrowser(op))
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
            strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to "execTransaction", "safe" to ShortAddress.of(unknown))),
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
        val c = controller(multisigs = MutableStateFlow(listOf(parent))) { _, _, _ -> RpcResult.Failed(rateLimited = false) }
        c.open(IncomingRequest.fromBrowser(op))
        val view = refused(c)
        assertEquals(unknown, view.blocked!!.parent)
        assertEquals(ParentReason.Unchecked, view.blocked!!.reason)
        assertEquals("approveHash", view.blocked!!.function)
        assertEquals(1, reads.size)
        assertEquals(
            strings.t("componentsUi.signing.parentSafeUnchecked", mapOf("function" to "approveHash", "safe" to ShortAddress.of(unknown))),
            sheetWords(view),
        )
        closeRefused(c)
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

    // -- a multisig the site was shown (the proposals machine) -----------------------

    private val hubScopes = mutableListOf<CoroutineScope>()

    @After
    fun stopHubs() = hubScopes.forEach { it.cancel() }

    private val child = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private fun row(address: String, salt: String) =
        """{"address":"$address","owners":["0xD400866e00B055B20752a826CD5C89b811de130b","0x031d7D57c99CAF891e1C250554691Fd12D84772b"],"threshold":2,"chainIds":[100],"recorded":true,"addedMs":1,"genesis":{"factory":"0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67","singleton":"0x41675C099F32341bf84BFc5382aF534df5C7461a","saltNonce":"$salt","initializer":"0x","name":"Treasury"}}"""

    private suspend fun hub(ports: MultisigShellTest.FakePorts, rows: List<String>): MultisigHub {
        ports.store["vela.multisigAccounts"] = rows.joinToString(",", "[", "]")
        val hubScope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { hubScopes += it }
        val hub = MultisigHub(
            live = ports,
            facts = { MultisigHub.Facts("http://127.0.0.1:1", emptyList(), listOf(100, chain), listOf(100), emptyList()) },
            scope = hubScope,
        )
        withTimeout(20_000) { hub.boot() }
        return hub
    }

    /** The browser's forward, routed as the app routes it: a multisig's request is the proposals machine's. */
    private suspend fun multisigAsks(hub: MultisigHub, h: CoreHost<DbrView>, id: String, to: String, data: String): Pair<String, CopyOnWriteArrayList<JsonElement>> {
        val params = JSONArray().put(JSONObject().put("from", child).put("to", to).put("value", "0x0").put("data", data))
        val op = pageAsks(h, id, "eth_sendTransaction", params)
        assertTrue("the app's routing: a multisig on this device", hub.isMultisig(op.granted_address))
        val answers = CopyOnWriteArrayList<JsonElement>()
        val heldId = "${op.tab}:${op.id}"
        hub.dappRequest(heldId, op.granted_address, op.method, Wire.json.parseToJsonElement(op.params_json), op.origin, op.chain_id) { answers += it }
        return heldId to answers
    }

    private suspend fun refusalOf(hub: MultisigHub, id: String): JsonElement {
        withTimeout(20_000) { while (hub.proposalsView.value.dapp[id]["stage"].str != "refused") delay(20) }
        return hub.proposalsView.value.dapp[id]["refusal"]!!
    }

    private suspend fun leaveAndAnswer(hub: MultisigHub, h: CoreHost<DbrView>, id: String, heldId: String, answers: List<JsonElement>) {
        hub.dappLeave(heldId)
        withTimeout(10_000) { while (answers.isEmpty()) delay(20) }
        delay(200)
        assertEquals("answered once", 1, answers.size)
        assertEquals(-32603L, answers.single()["error"]["code"].long)
        h.dispatch(DbrEvent.MultisigAnswered("tab-1", id, answers.single()), DbrEvent.serializer())
        assertEquals(-32603, pageAnswer(id).getJSONObject("error").getInt("code"))
    }

    @Test
    fun `a site asking a multisig to take over a multisig on the list is refused in the catalog's words`() = runBlocking<Unit> {
        val ports = MultisigShellTest.FakePorts()
        val hub = hub(ports, listOf(row(child, "0x1"), row(parent, "0x2")))
        assertTrue(hub.isMultisig(parent))
        val h = browser(child, 100, alsoAccounts = listOf(signer))
        val data = takeover.getJSONArray("params").getJSONObject(0).getString("data")
        val (heldId, answers) = multisigAsks(hub, h, "m1", parent, data)
        val refusal = refusalOf(hub, heldId)
        assertEquals("parent_safe", refusal["data"]["reason"].str)
        assertEquals("member", refusal["data"]["why"].str)
        assertEquals("execTransaction", refusal["data"]["function"].str)
        assertEquals("componentsUi.signing.parentSafeBlocked", refusal["data"]["key"].str)
        assertNull("nothing drafted", hub.proposalsView.value.draft)
        assertTrue("nothing signed", ports.passkeys.isEmpty())
        assertEquals(
            strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to "execTransaction", "safe" to ShortAddress.of(parent))),
            MultisigWords(strings).dappRefusal(refusal),
        )
        leaveAndAnswer(hub, h, "m1", heldId, answers)
    }

    @Test
    fun `a site asking a multisig to act for a Safe the chain says it owns is refused after the read`() = runBlocking<Unit> {
        val ports = MultisigShellTest.FakePorts()
        val isOwnerReads = AtomicInteger()
        val hub = hub(ports, listOf(row(child, "0x1")))
        ports.rpcAnswer = { _, method, params ->
            val call = (params as? JsonArray)?.firstOrNull()
            if (method == "eth_call" && call["to"].str.equals(unknown, ignoreCase = true) && call["data"].str.orEmpty().startsWith("0x2f54bf6e")) {
                isOwnerReads.incrementAndGet()
                MultisigRpcReply.Result(JsonPrimitive("0x" + "0".repeat(63) + "1"))
            } else {
                MultisigRpcReply.Result(JsonPrimitive("0x1"))
            }
        }
        val h = browser(child, 100, alsoAccounts = listOf(signer))
        val (heldId, answers) = multisigAsks(hub, h, "m2", unknown, "0xd4d9bdcd" + "cd".repeat(32))
        val refusal = refusalOf(hub, heldId)
        assertTrue("isOwner was read through the machine's own rpc", isOwnerReads.get() > 0)
        assertEquals("owner", refusal["data"]["why"].str)
        assertEquals(
            strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to "approveHash", "safe" to ShortAddress.of(unknown))),
            MultisigWords(strings).dappRefusal(refusal),
        )
        leaveAndAnswer(hub, h, "m2", heldId, answers)
    }

    @Test
    fun `a site asking a multisig to act for a Safe nobody could read is refused couldn't check`() = runBlocking<Unit> {
        val ports = MultisigShellTest.FakePorts()
        val hub = hub(ports, listOf(row(child, "0x1")))
        ports.rpcAnswer = { _, method, params ->
            val call = (params as? JsonArray)?.firstOrNull()
            if (method == "eth_call" && call["to"].str.equals(unknown, ignoreCase = true)) MultisigRpcReply.Unanswered("offline")
            else MultisigRpcReply.Result(JsonPrimitive("0x1"))
        }
        val h = browser(child, 100, alsoAccounts = listOf(signer))
        val (heldId, answers) = multisigAsks(hub, h, "m3", unknown, "0xd4d9bdcd" + "cd".repeat(32))
        val refusal = refusalOf(hub, heldId)
        assertEquals("unchecked", refusal["data"]["why"].str)
        assertEquals("componentsUi.signing.parentSafeUnchecked", refusal["data"]["key"].str)
        assertEquals(
            strings.t("componentsUi.signing.parentSafeUnchecked", mapOf("function" to "approveHash", "safe" to ShortAddress.of(unknown))),
            MultisigWords(strings).dappRefusal(refusal),
        )
        assertNotNull(refusal["message"].str)
        leaveAndAnswer(hub, h, "m3", heldId, answers)
    }

    @Test
    fun `any other multisig refusal keeps its own words`() {
        val words = MultisigWords(strings)
        assertNull(words.dappRefusal(Wire.json.parseToJsonElement("""{"code":4200,"message":"a multisig signs no messages"}""")))
        assertNull(words.dappRefusal(null))
    }
}
