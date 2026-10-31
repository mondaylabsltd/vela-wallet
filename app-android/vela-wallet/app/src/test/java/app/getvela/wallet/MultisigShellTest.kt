package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.multisig.core.HttpAnswer
import app.getvela.wallet.feature.multisig.core.MultisigExecutor
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.feature.multisig.core.MultisigOperation
import app.getvela.wallet.feature.multisig.core.MultisigPeerTransport
import app.getvela.wallet.feature.multisig.core.MultisigPorts
import app.getvela.wallet.feature.multisig.core.MultisigShellPorts
import app.getvela.wallet.feature.multisig.core.MultisigRpcReply
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.multisig.core.MultisigShellResult
import app.getvela.wallet.feature.multisig.core.MultisigWalletRefusal
import app.getvela.wallet.feature.multisig.core.NoPeer
import app.getvela.wallet.feature.multisig.core.PeerArrival
import app.getvela.wallet.feature.multisig.core.PeerOpened
import app.getvela.wallet.feature.multisig.core.StoreRead
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.multisig.core.jo
import app.getvela.wallet.feature.multisig.core.MultisigLive
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.jsonOf
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.long
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The multisig shell (native multisig M1–M6): the executor answers every
 * operation the five machines ask with exactly ONE result the core reads (the
 * web's `shell/handler.test.ts`), and the hub boots the real machines over it
 * (the host dylib, through uniffi — the same Rust the phone runs).
 */
class MultisigShellTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stop() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    /** Ports answered from a simulated world: a store, scripted HTTP and RPC, recorded calls. */
    class FakePorts : MultisigShellPorts {
        override var onNotice: (suspend (JsonObject) -> Unit)? = null
        override var onAskRisk: (suspend (MultisigOperation.AskRisk) -> Boolean)? = null
        val store = mutableMapOf<String, String>()
        var unreadable = false
        var refuseWrites = false
        val http = mutableListOf<Triple<String, String, String?>>()
        var httpAnswer: (String) -> HttpAnswer = { HttpAnswer(200, null, "{}") }
        var rpcAnswer: suspend (Int, String, JsonElement) -> MultisigRpcReply = { _, _, _ -> MultisigRpcReply.Result(JsonPrimitive("0x1")) }
        val notices = mutableListOf<JsonObject>()
        val passkeys = mutableListOf<MultisigOperation.Passkey>()
        var passkeyAnswer: MultisigShellResult = MultisigShellResult.PasskeyCancelled
        var risk = false
        var throwOnSend: Throwable? = null

        override suspend fun http(method: String, url: String, body: String?, timeoutMs: Long): HttpAnswer {
            http += Triple(method, url, body)
            return httpAnswer(url)
        }
        override suspend fun rpc(chainId: Int, method: String, params: JsonElement) = rpcAnswer(chainId, method, params)
        override suspend fun storeRead(key: String): StoreRead = if (unreadable) StoreRead.Unreadable else StoreRead.Value(store[key])
        override suspend fun storeWrite(key: String, value: String?): Boolean {
            if (refuseWrites) return false
            if (value == null) store.remove(key) else store[key] = value
            return true
        }
        override fun nowMs(): Double = 1_700_000_000_000.0
        override fun random(bytes: Int): ByteArray = ByteArray(bytes) { it.toByte() }
        override suspend fun passkey(op: MultisigOperation.Passkey): MultisigShellResult {
            passkeys += op
            return passkeyAnswer
        }
        override suspend fun askRisk(op: MultisigOperation.AskRisk): Boolean = risk
        override suspend fun sendCalls(op: MultisigOperation.SendCalls): MultisigShellResult {
            throwOnSend?.let { throw it }
            return MultisigShellResult.CallsSent("0xop", null)
        }
        override suspend fun quoteCalls(op: MultisigOperation.QuoteCalls): MultisigShellResult = MultisigShellResult.Quoted(jo("kind" to "failed"))
        override suspend fun simulate(op: MultisigOperation.Simulate): String? = null
        override suspend fun recipientFacts(op: MultisigOperation.RecipientRisk): Pair<Boolean?, Boolean?> = false to true
        override suspend fun userOpStatus(op: MultisigOperation.UserOpStatus): Pair<String?, String?>? = "included" to "0xtx"
        override val peer: MultisigPeerTransport = object : MultisigPeerTransport {
            override suspend fun open(relay: String?, invite: String?, pairing: String?) = PeerOpened("room-1", "invite-1", null)
            override suspend fun send(room: String, message: JsonElement) = Unit
            override suspend fun receive(room: String): PeerArrival = PeerArrival.Message(jo("hello" to "there"))
            override suspend fun close(room: String, cancel: Boolean, end: Boolean) = Unit
        }
        override suspend fun wallet(method: String, params: JsonElement): JsonElement = throw MultisigWalletRefusal(4900, "no wallet connected")
        override suspend fun notice(notice: JsonObject) {
            notices += notice
            onNotice?.invoke(notice)
        }
    }

    private fun op(json: String): MultisigOperation = Wire.json.decodeFromString(MultisigOperation.serializer(), json)

    private fun answer(ports: MultisigPorts, json: String): MultisigShellResult = runBlocking { MultisigExecutor(ports).answer(op(json)) }

    private fun encoded(result: MultisigShellResult): JsonObject =
        Wire.json.parseToJsonElement(Wire.json.encodeToString(MultisigShellResult.serializer(), result)) as JsonObject

    // -- the executor ---------------------------------------------------------

    @Test
    fun http_is_answered_as_it_came_and_an_unreachable_host_is_said() {
        val ports = FakePorts().apply { httpAnswer = { HttpAnswer(429, "3", "busy") } }
        val answered = answer(ports, """{"type":"http","method":"POST","url":"http://s/api","body":"{}","timeout_ms":5000}""")
        assertEquals(MultisigShellResult.HttpAnswered(429, "3", "busy"), answered)
        assertEquals(Triple("POST", "http://s/api", "{}"), ports.http.single())
        ports.httpAnswer = { throw java.io.IOException("refused") }
        val down = answer(ports, """{"type":"http","method":"GET","url":"http://s/x","body":null,"timeout_ms":5000}""")
        assertTrue(down is MultisigShellResult.HttpUnreachable)
    }

    @Test
    fun an_rpc_past_its_bound_is_unanswered_never_held() {
        val ports = FakePorts().apply {
            rpcAnswer = { _, _, _ -> delay(5_000); MultisigRpcReply.Result(JsonPrimitive("late")) }
        }
        val started = System.nanoTime()
        val bounded = answer(ports, """{"type":"rpc","chain_id":100,"method":"eth_call","params":[],"bound_ms":50}""")
        assertTrue(bounded is MultisigShellResult.RpcUnanswered)
        assertTrue("answered within the bound", (System.nanoTime() - started) / 1_000_000 < 2_000)
        ports.rpcAnswer = { _, _, _ -> MultisigRpcReply.Error(-32000, "execution reverted") }
        assertEquals(
            MultisigShellResult.RpcError(-32000, "execution reverted"),
            answer(ports, """{"type":"rpc","chain_id":100,"method":"eth_call","params":[],"bound_ms":null}"""),
        )
    }

    @Test
    fun the_store_answers_every_key_or_says_it_failed() {
        val ports = FakePorts().apply { store["vela.multisigAccounts"] = "[]" }
        val loaded = answer(ports, """{"type":"load_store","keys":["vela.multisigAccounts","vela.activeMultisig"]}""")
        assertEquals(MultisigShellResult.StoreLoaded(mapOf("vela.multisigAccounts" to "[]", "vela.activeMultisig" to null)), loaded)
        // A key never written travels as an explicit null — serde reads the entry.
        assertTrue(encoded(loaded)["entries"]!!.let { it as JsonObject }["vela.activeMultisig"] is JsonNull)
        assertEquals(MultisigShellResult.StoreWritten, answer(ports, """{"type":"write_store","key":"k","value":"v"}"""))
        assertEquals("v", ports.store["k"])
        assertEquals(MultisigShellResult.StoreWritten, answer(ports, """{"type":"write_store","key":"k","value":null}"""))
        assertFalse(ports.store.containsKey("k"))
        ports.refuseWrites = true
        assertTrue(answer(ports, """{"type":"write_store","key":"k","value":"v"}""") is MultisigShellResult.StoreFailed)
        ports.unreadable = true
        assertTrue(answer(ports, """{"type":"load_store","keys":["a"]}""") is MultisigShellResult.StoreFailed)
    }

    @Test
    fun the_clock_random_bytes_and_a_wait_answer_in_the_vocabulary() {
        val ports = FakePorts()
        assertEquals(MultisigShellResult.Now(1_700_000_000_000.0), answer(ports, """{"type":"now"}"""))
        assertEquals(MultisigShellResult.Random("0x00010203"), answer(ports, """{"type":"random","bytes":4}"""))
        assertEquals(MultisigShellResult.Waited, answer(ports, """{"type":"wait","ms":1}"""))
    }

    @Test
    fun the_product_pipelines_answer_through_their_ports() {
        val ports = FakePorts().apply { risk = true }
        assertEquals(
            MultisigShellResult.RiskAnswered(true),
            answer(ports, """{"type":"ask_risk","chain_id":100,"safe":"0xS","findings":[]}"""),
        )
        assertEquals(
            MultisigShellResult.RecipientFacts(false, true),
            answer(ports, """{"type":"recipient_risk","chain_id":100,"address":"0xA"}"""),
        )
        assertEquals(
            MultisigShellResult.UserOpStatus("included", "0xtx"),
            answer(ports, """{"type":"user_op_status","chain_id":100,"user_op_hash":"0xop"}"""),
        )
        assertEquals(
            MultisigShellResult.PeerOpened("room-1", "invite-1", null),
            answer(ports, """{"type":"peer_open","relay":null,"invite":null,"pairing":null}"""),
        )
        assertEquals(
            MultisigShellResult.WalletRefused(4900, "no wallet connected"),
            answer(ports, """{"type":"wallet_request","method":"eth_accounts","params":[]}"""),
        )
        val notice = answer(ports, """{"type":"notice","notice":{"kind":"mailbox_took","safe":"0xS"}}""")
        assertEquals(MultisigShellResult.Ack, notice)
        assertEquals("mailbox_took", ports.notices.single()["kind"].str)
    }

    @Test
    fun a_port_that_throws_is_answered_with_the_operations_own_failure() {
        val ports = FakePorts().apply { throwOnSend = IllegalStateException("boom") }
        val failed = answer(ports, """{"type":"send_calls","chain_id":100,"from":"0xA","calls":[],"fee":null,"purpose":null}""")
        assertTrue(failed is MultisigShellResult.CallsFailed)
        assertFalse((failed as MultisigShellResult.CallsFailed).maybe_sent)
    }

    @Test
    fun every_operation_has_a_failure_that_is_never_a_hang() {
        val ops = listOf(
            """{"type":"http","method":"GET","url":"u","body":null,"timeout_ms":1}""" to MultisigShellResult.HttpUnreachable::class,
            """{"type":"rpc","chain_id":1,"method":"m","params":[],"bound_ms":null}""" to MultisigShellResult.RpcUnanswered::class,
            """{"type":"load_store","keys":[]}""" to MultisigShellResult.StoreFailed::class,
            """{"type":"write_store","key":"k","value":null}""" to MultisigShellResult.StoreFailed::class,
            """{"type":"passkey","owner":"0xA","chain_id":1,"challenge":"0x00","purpose":{"kind":"record","multisig":"0xS"}}""" to MultisigShellResult.PasskeyFailed::class,
            """{"type":"ask_risk","chain_id":1,"safe":"0xS","findings":[]}""" to MultisigShellResult.RiskAnswered::class,
            """{"type":"quote_calls","chain_id":1,"from":"0xA","calls":[]}""" to MultisigShellResult.QuoteFailed::class,
            """{"type":"peer_receive","room":"r"}""" to MultisigShellResult.PeerClosed::class,
            """{"type":"wallet_request","method":"m","params":null}""" to MultisigShellResult.WalletRefused::class,
            """{"type":"notice","notice":{"kind":"activated","address":null}}""" to MultisigShellResult.Ack::class,
        )
        for ((json, kind) in ops) {
            val failure = MultisigExecutor.failure(op(json), "down")
            assertEquals(json, kind, failure::class)
            // Every failure encodes as the core reads it.
            assertTrue(encoded(failure)["type"].str!!.isNotEmpty())
        }
        // The risk question nobody could ask is "do not sign".
        assertEquals(
            MultisigShellResult.RiskAnswered(false),
            MultisigExecutor.failure(op("""{"type":"ask_risk","chain_id":1,"safe":"0xS","findings":[]}"""), "x"),
        )
    }

    @Test
    fun the_shells_failure_words_round_trip() {
        val detail = MultisigShellFailure.CannotPay(need = "10", have = "3").detail
        val read = MultisigShellFailure.read(detail)!!
        assertEquals("cannot_pay", read["kind"].str)
        assertEquals("10", read["need"].str)
        // The core's own words are not the shell's.
        assertEquals(null, MultisigShellFailure.read("the mailbox refused it"))
    }

    @Test
    fun a_call_value_travels_as_decimal() {
        assertEquals("255", MultisigLive.decimal(JsonPrimitive("0xff")))
        assertEquals("1000", MultisigLive.decimal(JsonPrimitive("1000")))
        assertEquals("0", MultisigLive.decimal(JsonNull))
        val params = MultisigLive.toOrg(JsonArray(listOf(JsonPrimitive("0xA"), JsonPrimitive(true), JsonNull)))
        assertEquals("[\"0xA\",true,null]", params.toString())
    }

    // -- the hub over the real machines --------------------------------------

    private fun hub(ports: FakePorts, accounts: List<Map<String, Any?>> = emptyList()): MultisigHub {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        return MultisigHub(
            live = ports,
            facts = { MultisigHub.Facts("http://127.0.0.1:1", accounts, listOf(100, 8453), listOf(100), emptyList()) },
            scope = scope,
        )
    }

    @Test
    fun the_accounts_machine_boots_over_this_shell_and_says_loaded() = runBlocking {
        val ports = FakePorts()
        val hub = hub(ports)
        withTimeout(20_000) { hub.boot() }
        assertTrue(hub.loaded)
        assertTrue(hub.accountsView.value.loaded)
        assertEquals(emptyList<Any>(), hub.accountsView.value.accounts)
        // Nothing is shown: the proposals machine follows nothing.
        assertEquals(null, hub.proposalsView.value.followed)
    }

    @Test
    fun a_stored_multisig_is_listed_and_shown_on_its_word() = runBlocking {
        val ports = FakePorts()
        val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        ports.store["vela.multisigAccounts"] = """[{"address":"$safe","owners":["0xD400866e00B055B20752a826CD5C89b811de130b","0x031d7D57c99CAF891e1C250554691Fd12D84772b"],"threshold":2,"chainIds":[100],"recorded":true,"addedMs":1,"genesis":{"factory":"0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67","singleton":"0x41675C099F32341bf84BFc5382aF534df5C7461a","saltNonce":"0x1","initializer":"0x","name":"Treasury"}}]"""
        val hub = hub(ports)
        withTimeout(20_000) { hub.boot() }
        val row = hub.accountsView.value.accounts.single()
        run {
            assertEquals(safe.lowercase(), row.account.address.lowercase())
            assertEquals(2, row.account.threshold)
            hub.activate(row.account.address)
            val shown = withTimeout(10_000) { hub.accounts.until { v -> v["active"].str } }
            assertEquals(safe.lowercase(), shown!!.lowercase())
            withTimeout(10_000) { while (hub.proposalsView.value.followed == null) delay(20) }
            assertEquals(safe.lowercase(), hub.proposalsView.value.followed!!.lowercase())
        }
    }

    @Test
    fun a_notice_to_route_reaches_the_router_and_a_link_is_read_by_the_core() = runBlocking {
        val ports = FakePorts()
        val hub = hub(ports)
        withTimeout(20_000) { hub.boot() }
        // A damaged link is the accounts machine's to read — the shell only hands it the fragment.
        hub.openLink("#ms=not-a-link")
        val stand = withTimeout(10_000) { hub.accounts.until { v -> v["link"]?.let { it["kind"].str } } }
        assertEquals("broken", stand)
    }

    @Test
    fun the_create_sheets_threshold_is_the_machines_and_steps_on_its_word() = runBlocking {
        val ports = FakePorts()
        val hub = hub(ports)
        withTimeout(20_000) { hub.boot() }
        val owners = listOf(
            "0xD400866e00B055B20752a826CD5C89b811de130b",
            "0x031d7D57c99CAF891e1C250554691Fd12D84772b",
            "0x70997970c51812dc3a010c7d01b50e0d17dc79c8",
        )
        hub.send(ev("new_draft"))
        // core-ms-2: the sensible start for three members is all but one.
        val start = withTimeout(10_000) {
            hub.accounts.request(ev("draft", "owners" to owners, "threshold" to 1, "name" to "Team")) { v -> v["create"]["threshold"]?.takeIf { it["members"].int == 3 } }
        }
        assertEquals(2, start["value"].int)
        assertEquals("spare", start["note"].kind)
        assertEquals(true, start["can_raise"].bool)
        val raised = withTimeout(10_000) {
            hub.accounts.request(ev("step_threshold", "up" to true)) { v -> v["create"]["threshold"]?.takeIf { it["value"].int == 3 } }
        }
        assertEquals("all", raised["note"].kind)
        assertEquals(true, raised["risky"].bool)
        assertEquals(false, raised["can_raise"].bool)
        // The typed view reads it as the sheet does.
        withTimeout(10_000) { while (hub.accountsView.value.create.threshold["value"].int != 3) delay(20) }
        assertEquals(emptyList<Int>(), hub.accountsView.value.recentNetworks)
    }

    @Test
    fun a_sites_message_to_a_multisig_is_refused_by_the_machine_and_answered_once() = runBlocking {
        val ports = FakePorts()
        val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        ports.store["vela.multisigAccounts"] = """[{"address":"$safe","owners":["0xD400866e00B055B20752a826CD5C89b811de130b","0x031d7D57c99CAF891e1C250554691Fd12D84772b"],"threshold":2,"chainIds":[100],"recorded":true,"addedMs":1,"genesis":{"factory":"0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67","singleton":"0x41675C099F32341bf84BFc5382aF534df5C7461a","saltNonce":"0x1","initializer":"0x","name":"Treasury"}}]"""
        val hub = hub(ports)
        withTimeout(20_000) { hub.boot() }
        assertTrue(hub.isMultisig(safe.lowercase()))
        val answers = java.util.concurrent.CopyOnWriteArrayList<JsonElement>()
        hub.dappRequest("tab-1:7", safe, "personal_sign", jsonOf(listOf("0x68656c6c6f", safe)), "https://app.example", 100) { answers += it }
        assertEquals(MultisigSheet.Dapp("tab-1:7"), hub.sheet.value)
        // dapp-7: a multisig signs no message — the machine's refusal, held for the person to read.
        withTimeout(10_000) { while (hub.proposalsView.value.dapp["tab-1:7"]["stage"].str != "refused") delay(20) }
        hub.dappLeave("tab-1:7")
        withTimeout(10_000) { while (answers.isEmpty()) delay(20) }
        delay(200)
        assertEquals(1, answers.size)
        val error = answers.single()["error"]
        assertTrue("an error the site reads: $error", (error["code"].long ?: 0) != 0L && error["message"].str != null)
        // A request for an account that is not a multisig here is never held.
        val other = java.util.concurrent.CopyOnWriteArrayList<JsonElement>()
        hub.dappRequest("tab-1:8", "0x0000000000000000000000000000000000000001", "eth_sendTransaction", jsonOf(emptyList<Any>()), "https://app.example", 100) { other += it }
        assertEquals(4100L, other.single()["error"]["code"].long)
    }

    @Test
    fun a_batch_status_is_the_machines_body_and_is_then_let_go() = runBlocking {
        val ports = FakePorts()
        val hub = hub(ports)
        withTimeout(20_000) { hub.boot() }
        // A batch nobody here proposed: the machine says so as a JSON-RPC body.
        val body = withTimeout(20_000) { hub.callsStatus(100, "0x" + "cd".repeat(32)) }
        assertTrue("a JSON-RPC body: $body", body?.get("result") != null || body?.get("error") != null)
        // Taken: not kept in the view.
        withTimeout(10_000) { while (hub.proposalsView.value.callsStatus.isNotEmpty()) delay(20) }
    }

    @Test
    fun events_are_written_as_the_core_reads_them() {
        assertEquals("""{"type":"open_link","hash":"#x","take_kept":false}""", ev("open_link", "hash" to "#x", "take_kept" to false).toString())
        assertEquals("""{"type":"activate","address":null}""", ev("activate", "address" to null).toString())
        assertTrue(NoPeer.toString().isNotEmpty())
    }
}
