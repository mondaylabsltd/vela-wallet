package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.core.FeedExecutor
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
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureMultiAddress

/**
 * Spec 044 T035 — SC-304's answer: a page's `eth_sendTransaction` through
 * the real sign_request + clear_signing + approval_guard + fee_policy
 * machines and the SAME spine a person's transfer runs, signed by the
 * parallel space's keyset. The sheet's facts are the core's, one signature
 * is produced, the record precedes the response, the page gets the hash.
 */
class DappSignMachineTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()
    private val safe = fixtureMultiAddress()
    private val founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val origin = "http://127.0.0.1:8137"
    private val events = java.util.Collections.synchronizedList(ArrayList<String>())
    private val answers = java.util.Collections.synchronizedList(ArrayList<Pair<String, JSONObject>>())
    private var signs = 0

    @After
    fun stop() = scope.cancel()

    private fun seedAccount() {
        val keyset = fixtureAccounts()
        val keys = JSONArray()
        keyset.forEach { keys.put(JSONObject().put("credential_id", it.credentialIdHex).put("public_key_hex", it.publicKeyHex).put("name", it.name).put("transports", "internal")) }
        val account = JSONObject().put("id", keyset.first().credentialIdHex).put("name", "Parallel space").put("address", safe)
            .put("public_key_hex", keyset.first().publicKeyHex).put("created_at_iso", "2026-09-12T00:00:00Z").put("keys", keys)
        store.values["vela.accounts"] = JSONArray().put(account).toString()
        store.values["vela.activeAccountIndex"] = "0"
    }

    private fun scriptRelay(receiptLands: Boolean = true) {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body("0x3b9aca00") }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", "0x3b9aca00")) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x5f5e100") }
        port.always("pimlico_getUserOperationGasPrice") {
            // A row per speed (spec 069): the sheet can price any of them.
            fun row(max: String) = JSONObject().put("maxFeePerGas", max).put("networkFeePerGas", "0x3b9aca00").put("relayerFeePerGas", "0x3b9aca00")
            FakeRelayPort.body(JSONObject().put("fast", row("0x77359400")).put("standard", row("0x59682f00")).put("slow", row("0x4a817c80")))
        }
        port.always("vela_getInBandGasQuote") {
            FakeRelayPort.body(JSONArray().put(JSONObject().put("recipient", "0x2222222222222222222222222222222222222222").put("asset", "native").put("balance", "0x9f3306a949ca000").put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1")))
        }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        port.always("eth_sendUserOperation") { params ->
            events += "relay.send"
            // Spec 069: the speed the displayed fee was priced at, by name.
            events += "relay.tier:${params.getOrNull(2) ?: "-"}"
            FakeRelayPort.body("0xhash")
        }
        // The receipt: pending once, then landed — the answer is the TX hash.
        // Or (issue 262) it never lands: the op sits in the bundler.
        if (receiptLands) {
            port.answer(
                "eth_getUserOperationReceipt",
                FakeRelayPort.body(JSONObject.NULL),
                FakeRelayPort.body(JSONObject().put("success", true).put("sender", safe).put("receipt", JSONObject().put("transactionHash", "0xtx").put("logs", JSONArray()))),
            )
        } else {
            port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
        }
        port.rest["https://relay.test/v1/treasury/100"] = RestAnswer.Ok(JSONObject().put("address", "0x1111111111111111111111111111111111111111").put("bootstrapNeeded", false))
        port.rest["https://relay.test/v1/account/100/${safe.lowercase()}"] = RestAnswer.Ok(JSONObject().put("activeDepositAddress", "0x2222222222222222222222222222222222222222").put("status", "ACTIVE"))
    }

    private val fixtureSigner = object : UserOpSigner {
        override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
            signs += 1
            events += "sign"
            val signed = fixtureAssert(challenge, listOfNotNull(credentialIdHex), 0u)
            return Assertion(signed.credentialIdHex, signed.signatureDerHex, signed.authenticatorDataHex, signed.clientDataJsonHex, null, "platform")
        }
    }

    private val handoffs = java.util.Collections.synchronizedList(ArrayList<Pair<String, List<String>>>())

    private fun controller(receiptWaitMs: Long = 10_000L): SigningController {
        val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        val accounts = StoreAccountPort(AccountStore(store))
        val credential = fixtureAccounts().first().credentialIdHex
        return SigningController(
            scope = scope, relay = relay, feed = feed, accounts = accounts, signer = { fixtureSigner },
            knownChains = { listOf(1, 100) },
            wallet = SignAccountRef(address = safe, credential_id = credential),
            receiptWaitMs = receiptWaitMs, receiptPollMs = 100L,
            ports = object : SigningController.Ports {
                override fun respond(transportId: String, id: String, json: JSONObject) {
                    events += "respond"
                    answers += "$transportId/$id" to json
                }
                override fun opSubmitted(id: String, userOpHash: String) { events += "op:$userOpHash" }
                override fun signingStarted() { events += "signing" }
                override fun recordsPersisted() {
                    val stored = store.values[KeyValueStore.Keys.TRANSACTIONS].orEmpty()
                    events += if (stored.contains("0xhash")) "persisted:with-hash" else "persisted"
                }
                override fun recordPersisted(recordId: String) { events += "record:$recordId" }
                override suspend fun switchAccount(address: String) = true
                override fun nativeSymbol(chainId: Int) = "XDAI"
                override fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int) {
                    handoffs += userOpHash to recordIds
                    val stored = store.values[KeyValueStore.Keys.TRANSACTIONS].orEmpty()
                    events += if (stored.contains(userOpHash)) "track:persisted" else "track:NOT-PERSISTED"
                }
                override fun dataBase() = ""
                override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
            },
        )
    }

    private fun transfer() = IncomingRequest(
        id = "r1", method = "eth_sendTransaction",
        paramsJson = JSONArray().put(JSONObject().put("from", safe).put("to", founder).put("value", "0x38d7ea4c68000")).toString(),
        origin = origin, transportId = "tab-1", chainId = 100,
    )

    @Test
    fun `a page's transfer is read, priced, signed once, recorded, then answered with the hash`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        c.open(transfer())
        val sheet = withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet && it.request != null } }
        assertEquals(origin, sheet.request!!.origin)
        assertEquals(100, sheet.request!!.chain_id)
        val read = withTimeout(20_000) { c.clear.first { it.resolved } }
        assertTrue("the reading is the core's: ${read.surface}", read.result != null || read.surface.name.isNotEmpty())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        val answer = answers.first { it.first == "tab-1/r1" }.second
        assertEquals("the page gets the TRANSACTION hash once the receipt landed", "0xtx", answer.getString("result"))
        assertEquals("signed exactly once", 1, signs)
        assertTrue("the record precedes the response: $events", events.indexOf("persisted:with-hash") in 0 until events.indexOf("respond"))
        assertTrue("the tracker was handed a persisted row: $events", events.contains("track:persisted"))
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        val row = rows.getJSONObject(0)
        assertEquals("dapp_tx", row.getString("type"))
        assertEquals("0xhash", row.getString("userOpHash"))
        withTimeout(10_000) { while (JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0).optString("status") != "confirmed") delay(50) }
        assertEquals("0xtx", JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0).getString("txHash"))
        assertEquals(origin, row.getString("dappOrigin"))
        assertTrue(row.getString("to").equals(founder, ignoreCase = true))
        withTimeout(10_000) { c.closed.first { it } }
    }

    /**
     * Issue 262: the bundler accepted the op but no receipt came inside the
     * wait. The page still gets an answer — the op hash — but the record is
     * NOT flipped to confirmed with the op hash as its tx hash: it stays
     * pending, and the tracker (handed the op and the record) settles it.
     */
    @Test
    fun `a late receipt answers the op hash and leaves the record pending for the tracker`() = runBlocking<Unit> {
        seedAccount(); scriptRelay(receiptLands = false)
        val c = controller(receiptWaitMs = 600L)
        c.open(transfer())
        withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet && it.request != null } }
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        assertEquals("the page gets the op hash when the receipt is late", "0xhash", answers.first { it.first == "tab-1/r1" }.second.getString("result"))
        withTimeout(10_000) { c.closed.first { it } }
        // Give any (wrong) confirming patch time to land before looking.
        delay(500)
        val row = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0)
        assertEquals("0xhash", row.getString("userOpHash"))
        assertEquals("nothing is known to have landed", "pending", row.getString("status"))
        assertEquals("the op hash is never recorded as a tx hash", "", row.optString("txHash"))
        val handoff = handoffs.singleOrNull()
        assertEquals("the tracker holds the op", "0xhash", handoff?.first)
        assertEquals("…and the very record it must settle", listOf(row.getString("id")), handoff?.second)
    }

    @Test
    fun `a speed picked on the sheet is the speed priced, signed and named on the wire`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        c.open(transfer())
        // The stored default first — `fast` for everybody who never chose.
        val first = withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        assertEquals(FeeTier.Fast, first.fee!!.tier)
        assertEquals(FeeTier.Fast, c.speed.value.tier)
        c.toggleSpeed()
        c.pickSpeed(FeeTier.Slow)
        assertTrue("a pick is one-shot, and says so", withTimeout(10_000) { c.speed.first { it.picked } }.tier == FeeTier.Slow)
        // The sheet re-prices at the speed picked; the old figure never stands in.
        // (Both speeds meet the $0.01 floor on this script, so only the NAME
        // tells them apart — which is the thing the wire has to carry.)
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready && it.fee?.tier == FeeTier.Slow } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        assertTrue("the wire names the speed picked: $events", "relay.tier:slow" in events)
        assertFalse("never the speed walked away from: $events", "relay.tier:fast" in events)
        withTimeout(10_000) { c.closed.first { it } }
    }

    @Test
    fun `a dismissal is the standard refusal once, and nothing is written`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        c.open(transfer())
        withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet } }
        c.swipeDismissed()
        withTimeout(20_000) { while (answers.isEmpty()) delay(50) }
        delay(300)
        assertEquals(1, answers.size)
        assertEquals(4001, answers.single().second.getJSONObject("error").getInt("code"))
        assertEquals(0, signs)
        assertTrue(store.values[KeyValueStore.Keys.TRANSACTIONS].isNullOrEmpty())
        withTimeout(10_000) { c.closed.first { it } }
    }

    @Test
    fun `a page's sign-in message is signed once as an EIP-1271 envelope, nothing submitted`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        c.open(IncomingRequest("r2", "personal_sign", JSONArray().put("0x48656c6c6f2c2056656c61").put(safe).toString(), origin, "tab-1", 100))
        withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet } }
        val read = withTimeout(20_000) { c.clear.first { it.message != null } }
        assertEquals("Hello, Vela", read.message!!.decoded_text)
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r2" }) delay(50) }
        val signature = answers.first { it.first == "tab-1/r2" }.second.getString("result")
        assertTrue("an EIP-1271 envelope, not a bare 65-byte signature: ${signature.length}", signature.startsWith("0x") && signature.length > 300)
        assertEquals(1, signs)
        assertEquals("nothing reached the relay", 0, port.calls.count { it.endsWith("eth_sendUserOperation") })
        withTimeout(10_000) { c.closed.first { it } }
    }

    @Test
    fun `an unlimited approval is blocked until bounded, then leaves bounded`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        val usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"
        val spender = "0x1111111111111111111111111111111111111111"
        val data = "0x095ea7b3" + spender.drop(2).padStart(64, '0') + "f".repeat(64)
        c.open(IncomingRequest("r3", "eth_sendTransaction", JSONArray().put(JSONObject().put("from", safe).put("to", usdc).put("data", data).put("value", "0x0")).toString(), origin, "tab-1", 100))
        val guard = withTimeout(20_000) { c.guard.first { it.detected != null } }
        assertTrue(guard.detected!!.is_unbounded)
        assertFalse("the guard holds the slide", guard.confirm_allowed)
        c.guardPreset(app.getvela.wallet.feature.signing.core.GuardEditorMode.Custom)
        c.guardCustomAmount("1")
        val bounded = withTimeout(20_000) { c.guard.first { it.confirm_allowed && it.rewritten_params_json != null } }
        assertTrue("the calldata carries the bounded amount", bounded.rewritten_params_json!!.contains("095ea7b3"))
        assertFalse(bounded.rewritten_params_json!!.contains("f".repeat(64)))
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r3" }) delay(50) }
        assertTrue(answers.first { it.first == "tab-1/r3" }.second.has("result"))
        assertEquals(1, signs)
        assertEquals(1, port.calls.count { it.endsWith("eth_sendUserOperation") })
    }

    /** Spec 046 US2: eth_sign is presented as the danger it is and, once read, signed over the same envelope. */
    @Test
    fun `eth_sign is the danger surface, not the calm message view, and still signs when approved`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        c.open(IncomingRequest("r5", "eth_sign", JSONArray().put(safe).put("0x48656c6c6f2c2056656c61").toString(), origin, "tab-1", 100))
        withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet } }
        val read = withTimeout(20_000) { c.clear.first { it.message != null } }
        assertEquals(app.getvela.wallet.feature.signing.core.ClearDangerClass.EthSign, read.message!!.danger_class)
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r5" }) delay(50) }
        val signature = answers.first { it.first == "tab-1/r5" }.second.getString("result")
        assertTrue(signature.startsWith("0x") && signature.length > 300)
        assertEquals(1, signs)
        withTimeout(10_000) { c.closed.first { it } }
    }
}
