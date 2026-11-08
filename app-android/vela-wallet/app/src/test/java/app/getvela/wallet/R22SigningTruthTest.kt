package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.Preferences
import app.getvela.wallet.core.data.SigningPrefs
import app.getvela.wallet.core.designsystem.components.HoldGate
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MultisigExecutor
import app.getvela.wallet.feature.multisig.core.MultisigOperation
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.multisig.core.MultisigShellResult
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.core.SignShellResult
import app.getvela.wallet.feature.signing.core.SafeOwnerAnswer
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.wallet.core.RpcEvent
import app.getvela.wallet.feature.wallet.core.RpcCallVerdict
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import uniffi.vela_core_uniffi.UserOpCall

/**
 * R22 "signing truth" (docs/multisig-machines.md §36) on Android: a plain
 * signing verdict stands on two independent sources, the confirm's friction
 * is the core's, and what is signed is exactly what the sheet showed. Every
 * rule is the core's; this pins the shell's half — that it asks the pool for
 * a corroborated read, hands both answers to the core in the core's envelope,
 * passes the person's pref, and never hashes past the core's attestation.
 */
class R22SigningTruthTest {
    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stop() {
        scopes.forEach { it.cancel() }
        scopes.clear()
        SigningPrefs.refuseBlind.value = false
    }

    // -- §8: the cross-shell vector ------------------------------------------------

    @Test
    fun `the signing-truth sim vectors read the same through this binding`() {
        val vectors = Wire.json.parseToJsonElement(File(repoRoot, "rust/crates/vela-core/tests/vectors/rules/signing-truth.json").readText()).jsonObject
        val user = vectors["user"]!!.jsonPrimitive.content
        val cases = vectors["sim"]!!.jsonArray
        assertTrue("the vector carries its sim cases", cases.size >= 10)
        for (case in cases) {
            val name = case.jsonObject["name"]!!.jsonPrimitive.content
            val expect = case.jsonObject["expect"]!!.jsonObject
            val record = uniffi.vela_core_uniffi.simOutcome(user, case.jsonObject["envelope"].toString())
            assertEquals(name, expect["kind"]!!.jsonPrimitive.content, record.kind)
            expect["source"]?.let { assertEquals("$name: source", (it as? JsonPrimitive)?.takeIf { p -> p.isString }?.content, record.source) }
            expect["notice_key"]?.let { assertEquals("$name: notice", (it as? JsonPrimitive)?.takeIf { p -> p.isString }?.content, record.noticeKey) }
            expect["source_key"]?.let { assertEquals("$name: source line", (it as? JsonPrimitive)?.takeIf { p -> p.isString }?.content, record.sourceKey) }
            // The vector names the moves' amounts, in order.
            expect["deltas"]?.let { want ->
                val got = Wire.json.parseToJsonElement(record.deltasJson).jsonArray.map { it.jsonObject["delta"]!!.jsonPrimitive.content }
                assertEquals("$name: deltas", want.jsonArray.map { it.jsonPrimitive.content }, got)
            }
            // The sheet's verdict JSON (what the gate reads back) carries the same.
            val verdict = Wire.json.parseToJsonElement(SimDeltas.verdictJson(record)).jsonObject
            assertEquals(name, record.kind, verdict["kind"]!!.jsonPrimitive.content)
            assertEquals(name, Wire.json.parseToJsonElement(record.deltasJson), verdict["deltas"])
        }
    }

    @Test
    fun `the envelope is the core's - the pool giving up is unreachable, a witness rides beside`() {
        assertEquals("""{"unreachable":true}""", JSONObject(RpcResult.corroboratedEnvelope(null)).toString())
        assertEquals("""{"unreachable":true}""", JSONObject(RpcResult.corroboratedEnvelope(RpcResult.Failed(rateLimited = true))).toString())
        val body = JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", "0x1")
        val alone = JSONObject(RpcResult.corroboratedEnvelope(RpcResult.Body(body)))
        assertEquals("0x1", alone.getString("result"))
        assertFalse("one node: no witness", alone.has("witness"))
        val both = JSONObject(RpcResult.corroboratedEnvelope(RpcResult.Body(body, witness = body)))
        assertEquals("0x1", both.getJSONObject("witness").getString("result"))
        val own = JSONObject(RpcResult.corroboratedEnvelope(RpcResult.Body(body, ownNode = true)))
        assertTrue(own.getBoolean("ownNode"))
    }

    // -- §1: the pool --------------------------------------------------------------

    private fun pool(answer: (String, String) -> app.getvela.wallet.feature.wallet.core.RpcPostResult): Pair<RpcPool, CopyOnWriteArrayList<Pair<String, String>>> {
        val asked = CopyOnWriteArrayList<Pair<String, String>>()
        val transport = FakeRpcTransport { url, method -> asked += url to method; answer(url, method) }
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        return RpcPool(FakeStore(), FakeEndpointSource(listOf(NODE_A, NODE_B)), scope, transport) to asked
    }

    private fun chainAnswers(url: String, method: String) =
        if (method == "eth_chainId") FakeRpcTransport.body("0x64") else FakeRpcTransport.body(if (url == NODE_A) "0xa" else "0xb")

    @Test
    fun `a corroborated call resolves with both bodies, each node having said its chain first`() = runBlocking {
        val (pool, asked) = pool(::chainAnswers)
        pool.start()
        val body = withTimeout(TIMEOUT) { pool.call(CHAIN, "eth_call", listOf("x"), corroborate = true) } as RpcResult.Body
        assertNotNull("a second, independent node answered", body.witness)
        assertEquals(setOf("0xa", "0xb"), setOf(body.json.optString("result"), body.witness!!.optString("result")))
        assertFalse(body.ownNode)
        // The chain-id probes outside any race were answered (the read waited on them).
        assertTrue("each source said its chain first: $asked", asked.count { it.second == "eth_chainId" } >= 2)
        assertEquals("both nodes were asked the read", setOf(NODE_A, NODE_B), asked.filter { it.second == "eth_call" }.map { it.first }.toSet())
    }

    @Test
    fun `an ordinary call is answered as before - one body, no witness`() = runBlocking {
        val (pool, asked) = pool(::chainAnswers)
        pool.start()
        val body = withTimeout(TIMEOUT) { pool.call(CHAIN, "eth_call", listOf("x")) } as RpcResult.Body
        assertNull(body.witness)
        assertEquals("one node asked the read", 1, asked.count { it.second == "eth_call" })
    }

    @Test
    fun `the wire carries corroborate, witness_url and own_node in the core's spelling`() {
        val event = Wire.json.encodeToString(RpcEvent.serializer(), RpcEvent.CallRequested("c1", 100, RpcKind.Rpc, "eth_call", 1.0, corroborate = true))
        assertTrue(event, event.contains("\"corroborate\":true"))
        val verdict = Wire.json.decodeFromString(RpcCallVerdict.serializer(), """{"type":"respond","url":"a","witness_url":"b","own_node":true}""") as RpcCallVerdict.Respond
        assertEquals("b", verdict.witness_url)
        assertTrue(verdict.own_node)
        val old = Wire.json.decodeFromString(RpcCallVerdict.serializer(), """{"type":"respond","url":"a"}""") as RpcCallVerdict.Respond
        assertNull(old.witness_url)
        assertFalse(old.own_node)
    }

    // -- §2b/c: the multisig shell ---------------------------------------------------

    @Test
    fun `a corroborated multisig read answers rpc_corroborated with the envelope as an object`() = runBlocking {
        val ports = MultisigShellTest.FakePorts()
        val corroborated = MultisigExecutor(ports).answer(
            Wire.json.decodeFromString(MultisigOperation.serializer(), """{"type":"rpc","chain_id":100,"method":"eth_getCode","params":["0x1","latest"],"corroborate":true}"""),
        )
        val json = Wire.json.encodeToJsonElement(MultisigShellResult.serializer(), corroborated).jsonObject
        assertEquals("rpc_corroborated", json["type"]!!.jsonPrimitive.content)
        assertTrue("the envelope is a JSON object", json["envelope"] is JsonObject)
        val plain = MultisigExecutor(ports).answer(
            Wire.json.decodeFromString(MultisigOperation.serializer(), """{"type":"rpc","chain_id":100,"method":"eth_getCode","params":["0x1","latest"]}"""),
        )
        assertTrue("corroborate false: exactly as before", plain is MultisigShellResult.RpcResult)
    }

    // -- §2d: the unlisted Safe's isOwner ------------------------------------------

    @Test
    fun `a safe-owner read carries the witness and own node in the core's shape`() {
        val read = SignShellResult.SafeOwnerRead(
            id = "r1", chain_id = 100, safe = "0x1",
            answer = SafeOwnerAnswer.Returned("0x0"),
            witness = SafeOwnerAnswer.Returned("0x0"),
            own_node = null,
        )
        val json = Wire.json.encodeToJsonElement(SignShellResult.serializer(), read).jsonObject
        assertEquals("returned", json["witness"]!!.jsonObject["type"]!!.jsonPrimitive.content)
    }

    // -- §3: the confirm's posture ----------------------------------------------------

    @Test
    fun `a transaction with no corroborated verdict signs only anyway, held, in the danger style`() {
        val hold = uniffi.vela_core_uniffi.holdToConfirmMs().toInt()
        assertEquals(1_500, hold)
        assertEquals(5_000L, uniffi.vela_core_uniffi.confirmAgainWindowMs().toLong())
        assertEquals(hold, HoldGate.HOLD_MS)
        assertEquals(5_000L, HoldGate.DEFAULT_WINDOW_MS)
    }

    @Test
    fun `the core's verdict rides on the outcome the sheet holds`() {
        val unconfirmed = SimDeltas.oneNodeOnly() as SigningController.SimOutcome.Notice
        assertTrue("one node's word: the own-node hint", unconfirmed.ownNodeHint)
        assertTrue("and Check again", unconfirmed.recheck)
        assertFalse("but the node that answered is not watched", unconfirmed.watchNode)
        assertEquals("componentsUi.signing.simUnconfirmed", unconfirmed.key)
        assertEquals("unconfirmed", Wire.json.parseToJsonElement(unconfirmed.verdictJson!!).jsonObject["kind"]!!.jsonPrimitive.content)
        val checked = SimDeltas.nothingMoves() as SigningController.SimOutcome.Ready
        assertEquals("deltas", Wire.json.parseToJsonElement(checked.verdictJson!!).jsonObject["kind"]!!.jsonPrimitive.content)
        assertNull(SigningController.SimOutcome.Pending.verdictJson)
    }

    // -- §4: the multisig door -----------------------------------------------------

    @Test
    fun `refuse blind signing shuts a door that would sign only anyway, with its reason`() {
        val review = Wire.json.parseToJsonElement("""{"sign":{"allowed":true,"anyway":true}}""")
        val ready = Wire.json.parseToJsonElement("""{"kind":"ready"}""")
        val open = MsRules.reviewDoor(ready, review, refuseBlind = false)
        assertTrue(open.enabled && open.anyway)
        assertEquals("an enabled anyway is held", 1_500, open.holdMs)
        val refused = MsRules.reviewDoor(ready, review, refuseBlind = true)
        assertFalse(refused.enabled)
        assertEquals("home.multisig.review.blindRefused", refused.reasonKey)
        assertTrue("said in the reason place above the review", refused.unchecked)
        val plain = MsRules.reviewDoor(ready, Wire.json.parseToJsonElement("""{"sign":{"allowed":true,"anyway":false}}"""), refuseBlind = true)
        assertTrue("a plain review is not blind", plain.enabled && plain.holdMs == 0)
    }

    // -- §5: the pref ----------------------------------------------------------------

    @Test
    fun `refuse blind signing is off by default, stored in the core's spelling`() = runBlocking {
        assertEquals("vela.refuseBlindSigning", uniffi.vela_core_uniffi.refuseBlindSigningKey())
        assertFalse(uniffi.vela_core_uniffi.refuseBlindSigning(null))
        assertFalse(uniffi.vela_core_uniffi.refuseBlindSigning("off"))
        assertTrue(uniffi.vela_core_uniffi.refuseBlindSigning("on"))
        val store = FakeStore()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        val prefs = Preferences(store, scope, onFormats = {}, developerBuild = true)
        prefs.load()
        withTimeout(TIMEOUT) { prefs.view.first { it.loaded } }
        assertFalse("default off", prefs.view.value.refuseBlind)
        prefs.setRefuseBlind(true)
        assertTrue(SigningPrefs.refuseBlind.value)
        withTimeout(TIMEOUT) { while (store.read(Preferences.KEY_REFUSE_BLIND) != "on") kotlinx.coroutines.delay(10) }
        // Read back by a fresh launch, through the core's rule.
        val again = Preferences(store, scope, onFormats = {}, developerBuild = true)
        again.load()
        assertTrue(withTimeout(TIMEOUT) { again.view.first { it.loaded } }.refuseBlind)
    }

    // -- the hold's accessible path --------------------------------------------------

    @Test
    fun `the accessible path - first activation arms, a second within the window confirms, after it disarms`() {
        val gate = HoldGate(windowMs = 5_000)
        assertEquals(HoldGate.Step.Armed, gate.activate(1_000))
        assertTrue(gate.armed)
        assertEquals(HoldGate.Step.Confirmed, gate.activate(5_999))
        assertFalse("confirmed: disarmed", gate.armed)
        assertEquals(HoldGate.Step.Armed, gate.activate(10_000))
        assertEquals("past the window: armed again, never confirmed", HoldGate.Step.Armed, gate.activate(15_001))
        gate.expire()
        assertEquals("a touch (or the window) disarms", HoldGate.Step.Armed, gate.activate(15_002))
    }

    // -- §7: WYSIWYS -------------------------------------------------------------------

    private var challenge: ByteArray? = null

    private suspend fun dappSubmit(calls: List<UserOpCall>, review: String?): UserOpSpine.Refused {
        val port = FakeRelayPort()
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_blockNumber") { FakeRelayPort.body("0x10") }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        val accounts = AccountStore(FakeStore())
        accounts.saveAccount(
            JSONObject().put("id", CREDENTIAL).put("address", SAFE).put("public_key_hex", KEY).put("created_at_iso", "2026-01-01T00:00:00Z").put(
                "keys",
                JSONArray().put(JSONObject().put("credential_id", CREDENTIAL).put("public_key_hex", KEY).put("name", "Key 1").put("transports", "internal")),
            ),
        )
        val spine = UserOpSpine(
            relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            accounts = StoreAccountPort(accounts),
            signer = {
                object : UserOpSigner {
                    override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
                        this@R22SigningTruthTest.challenge = challenge
                        throw PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")
                    }
                }
            },
            measureCall = { _, _, _, _, _ -> "0x30d40" },
        )
        return try {
            spine.submit(100, SAFE, calls, null, UserOpSpine.Quoted(FEE, RECIPIENT), dapp = UserOpSpine.DappAttest(review))
            fail("nothing may be sent here")
            throw IllegalStateException()
        } catch (refused: UserOpSpine.Refused) {
            refused
        }
    }

    private fun review(calls: List<UserOpCall>): String = JSONObject()
        .put("kind", "calls").put("chain_id", 100).put("account", SAFE)
        .put("calls", JSONArray(calls.map { JSONObject().put("to", it.to).put("value", it.value).put("data", it.data) }))
        .put("fee", JSONObject().put("recipient", RECIPIENT))
        .toString()

    @Test
    fun `the reviewed request is signed - the passkey is asked for exactly it`() = runBlocking {
        val calls = listOf(UserOpCall(to = TOKEN, value = "0", data = TRANSFER))
        val refused = dappSubmit(calls, review(calls))
        assertEquals("it reached the passkey", UserOpSpine.Failure.PasskeyCancelled, refused.failure)
        assertNotNull(challenge)
    }

    @Test
    fun `a drifted request is refused before the passkey - nothing signed`() = runBlocking {
        val shown = listOf(UserOpCall(to = TOKEN, value = "0", data = TRANSFER))
        val drifted = listOf(UserOpCall(to = TOKEN, value = "0", data = TRANSFER.dropLast(1) + "2"))
        val refused = dappSubmit(drifted, review(shown))
        assertTrue(refused.failure is UserOpSpine.Failure.Other)
        assertNull("no passkey was asked", challenge)
    }

    @Test
    fun `a request with no frozen review is signed by no one`() = runBlocking {
        val calls = listOf(UserOpCall(to = TOKEN, value = "0", data = TRANSFER))
        val refused = dappSubmit(calls, null)
        assertTrue(refused.failure is UserOpSpine.Failure.Other)
        assertNull(challenge)
    }

    @Test
    fun `a message digest goes on only when it is the reviewed one`() {
        val digest = ByteArray(32) { 7 }
        val review = JSONObject().put("kind", "message").put("chain_id", 100).put("account", SAFE)
            .put("digest", "0x" + digest.joinToString("") { "%02x".format(it) }).toString()
        assertTrue(uniffi.vela_core_uniffi.dappMessageHash(digest, SAFE, review).contentEquals(digest))
        val other = ByteArray(32) { 8 }
        assertTrue("a drifted digest is refused", runCatching { uniffi.vela_core_uniffi.dappMessageHash(other, SAFE, review) }.isFailure)
    }

    // -- 7b r21-1: expired says expired ------------------------------------------------

    @Test
    fun `an execute refused with AA22 says the Send's expired words, calmly`() {
        val assets = File(repoRoot, "assets/i18n")
        val strings = app.getvela.wallet.core.i18n.I18nRuntime(
            readOnDemand = { namespace, tag ->
                File(assets, app.getvela.wallet.core.i18n.I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes()
            },
        ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
            initialize("en")
            include(app.getvela.wallet.core.i18n.I18nCatalogs.MULTISIG)
        }
        val words = MultisigWords(
            strings = strings,
            accounts = app.getvela.wallet.feature.multisig.core.MsAccountsView(),
            localNames = emptyMap(),
            networkName = { if (it == 100) "Gnosis" else null },
            nativeSymbol = { if (it == 100) "XDAI" else null },
        )
        val expired = MultisigShellFailure.of(UserOpSpine.Failure.Expired).detail
        assertEquals("""{"kind":"expired"}""", JSONObject(expired).toString())
        assertEquals("expired", MsRules.failureSaid(expired).kind)
        assertEquals(strings.t("componentsUi.signing.expired"), words.failureText(expired))
        // The relay's raw AA22 carried in an unknown detail is read the same.
        val raw = MultisigShellFailure.of(UserOpSpine.Failure.Rejected("SUBMIT REFUSED by simulation: AA22 expired or not due")).detail
        assertEquals(strings.t("componentsUi.signing.expired"), words.failureText(raw))
        val state = Wire.json.parseToJsonElement(JSONObject().put("kind", "failed").put("detail", expired).toString())
        assertEquals("calm: nothing went out", app.getvela.wallet.feature.multisig.MsTone.Warning, words.actionFailureTone(state))
    }

    // -- 7c r21-2: a fee never reads below its charge ---------------------------------

    @Test
    fun `fee figures are the core's - rounded up at the shown places`() {
        assertEquals("0.000411", SendLive.feeFromBase("410400290875302", 18))
        assertEquals("0.00000000013", SendLive.feeFromBase("123456789", 18))
        assertEquals("1.27", SendLive.feeFromBase("1270000", 6, SendLive.TOKEN_FEE_PLACES))
        assertEquals("a token fee rounds up at four places", "1.2346", SendLive.feeFromBase("1234501", 6, SendLive.TOKEN_FEE_PLACES))
        assertEquals("0.000412", MsRules.feeFigure("411000000000001"))
    }

    private companion object {
        const val CHAIN = 100
        const val NODE_A = "https://rpc.alpha-node.org/rpc"
        const val NODE_B = "https://rpc.beta-node.net/rpc"
        const val TIMEOUT = 20_000L
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val TOKEN = "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d"
        const val RECIPIENT = "0x2222222222222222222222222222222222222222"
        const val FEE = "1000000000000000"
        const val CREDENTIAL = "aabbcc"
        val KEY = "04" + "11".repeat(64)
        val TRANSFER = "0xdeadbeef" + "0".repeat(63) + "1"
    }
}
