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
import org.junit.Assert.assertNull
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

    /** [signedInWith]: the key the account signed in with (2026-09-26); `null` = a record from before. */
    private fun seedAccount(signedInWith: JSONObject? = null) {
        val keyset = fixtureAccounts()
        val keys = JSONArray()
        keyset.forEach { keys.put(JSONObject().put("credential_id", it.credentialIdHex).put("public_key_hex", it.publicKeyHex).put("name", it.name).put("transports", "internal")) }
        val account = JSONObject().put("id", keyset.first().credentialIdHex).put("name", "Parallel space").put("address", safe)
            .put("public_key_hex", keyset.first().publicKeyHex).put("created_at_iso", "2026-09-12T00:00:00Z").put("keys", keys)
        if (signedInWith != null) account.put("signed_in_with", signedInWith)
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
            FakeRelayPort.body("0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1")
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

    /** Where each ceremony went: the pinned credential, its transports, its method. */
    private val routes = java.util.concurrent.CopyOnWriteArrayList<Triple<String?, String, KeyMethod>>()

    private val fixtureSigner = object : UserOpSigner {
        override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
            signs += 1
            events += "sign"
            routes += Triple(credentialIdHex, transports, method)
            val signed = fixtureAssert(challenge, listOfNotNull(credentialIdHex), 0u)
            return Assertion(signed.credentialIdHex, signed.signatureDerHex, signed.authenticatorDataHex, signed.clientDataJsonHex, null, "platform")
        }
    }

    private val handoffs = java.util.Collections.synchronizedList(ArrayList<Pair<String, List<String>>>())

    /** Every hand-off whole, in order: spec 082's flags ride along (the write-ahead's, then the relay's). */
    private val handed = java.util.concurrent.CopyOnWriteArrayList<app.getvela.wallet.feature.send.core.TrackHandoff>()

    /** Spec 082 RJ1: every op the tracker was told to forget — proven never sent. */
    private val withdraws = java.util.concurrent.CopyOnWriteArrayList<Pair<String, List<String>>>()

    /** The hash the executor computed before any POST (`OpSigned`). */
    @Volatile
    private var signedOp: String? = null

    /** The tracker's view, as the app's tracker would publish it (spec 082 RJ4: the answer follows it). */
    private val tracker = kotlinx.coroutines.flow.MutableStateFlow(app.getvela.wallet.feature.send.core.TrackView())

    private fun controller(
        receiptWaitMs: Long = 10_000L,
        signer: UserOpSigner = fixtureSigner,
        now: () -> Double = { System.currentTimeMillis().toDouble() },
    ): SigningController {
        val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        val accounts = StoreAccountPort(AccountStore(store))
        val credential = fixtureAccounts().first().credentialIdHex
        return SigningController(
            scope = scope, relay = relay, feed = feed, accounts = accounts, signer = { signer },
            knownChains = { listOf(1, 100) },
            wallet = SignAccountRef(address = safe, credential_id = credential),
            now = now,
            receiptWaitMs = receiptWaitMs, receiptPollMs = 100L,
            ports = object : SigningController.Ports {
                override fun respond(transportId: String, id: String, payload: app.getvela.wallet.feature.signing.core.SignResponsePayload) {
                    events += "respond"
                    // The page's shape, as the browser core renders it (spec 070).
                    val json = when (payload) {
                        is app.getvela.wallet.feature.signing.core.SignResponsePayload.Ok -> JSONObject().put("result", payload.result?.let { org.json.JSONTokener(it.toString()).nextValue() } ?: JSONObject.NULL)
                        is app.getvela.wallet.feature.signing.core.SignResponsePayload.Err -> JSONObject().put("error", JSONObject().put("code", payload.code).put("message", payload.message.orEmpty()))
                    }
                    answers += "$transportId/$id" to json
                }
                override fun opSubmitted(id: String, submitted: app.getvela.wallet.feature.send.core.UserOpSpine.Submitted) { events += "op:${submitted.userOpHash}" }
                override fun opSigned(id: String, userOpHash: String, submitBlock: Long?) { signedOp = userOpHash; events += "op-signed:$userOpHash" }
                override fun trackWithdrawn(userOpHash: String, recordIds: List<String>) { withdraws += userOpHash to recordIds }
                override fun ceremonyStarted(id: String) { events += "signing" }
                override fun ceremonyDone(id: String) { events += "signed" }
                override fun recordsPersisted() {
                    val stored = store.values[KeyValueStore.Keys.TRANSACTIONS].orEmpty()
                    events += if (stored.contains("0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1")) "persisted:with-hash" else "persisted"
                }
                override fun recordPersisted(recordId: String) { events += "record:$recordId" }
                override suspend fun switchAccount(address: String) = true
                override fun nativeSymbol(chainId: Int) = "XDAI"
                override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) {
                    val userOpHash = handoff.userOpHash
                    handoffs += userOpHash to handoff.recordIds
                    handed += handoff
                    val stored = store.values[KeyValueStore.Keys.TRANSACTIONS].orEmpty()
                    events += if (stored.contains(userOpHash)) "track:persisted" else "track:NOT-PERSISTED"
                }
                override fun dataBase() = ""
                override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
                override fun trackerView() = tracker
            },
        )
    }

    /** The relay accepts the op under the hash the wallet computed — the written-ahead row's. */
    private fun relayTakesTheLocalHash() {
        port.always("eth_sendUserOperation") {
            events += "relay.send"
            val rows = JSONArray(store.values[KeyValueStore.Keys.TRANSACTIONS] ?: "[]")
            FakeRelayPort.body(if (rows.length() > 0) rows.getJSONObject(0).getString("userOpHash") else signedOp)
        }
    }

    private fun trackerSays(status: app.getvela.wallet.feature.send.core.TrackStatus, txHash: String?) {
        tracker.value = app.getvela.wallet.feature.send.core.TrackView(
            listOf(
                app.getvela.wallet.feature.send.core.TrackEntryView(
                    user_op_hash = signedOp!!, chain_id = 100, status = status, tx_hash = txHash, submitted_at_ms = 1.0,
                    outcome = app.getvela.wallet.feature.send.core.TrackOutcome.Final,
                ),
            ),
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
        // A record from before the sign-in key: the first key, over its stored route.
        assertEquals(listOf(Triple(fixtureAccounts().first().credentialIdHex, "internal", KeyMethod.Platform)), routes)
        assertTrue("the record precedes the response: $events", events.indexOf("persisted:with-hash") in 0 until events.indexOf("respond"))
        assertTrue("the tracker was handed a persisted row: $events", events.contains("track:persisted"))
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        val row = rows.getJSONObject(0)
        assertEquals("dapp_tx", row.getString("type"))
        assertEquals("0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1", row.getString("userOpHash"))
        // Spec 082 RA8 (W3): the page has its tx hash, but the record is the
        // tracker's to close — it alone judges a revert. It stays pending here
        // (no tracker runs in this test), handed over with its record id.
        withTimeout(10_000) { c.closed.first { it } }
        delay(300)
        val stored = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0)
        assertEquals("never confirmed by the sheet", "pending", stored.getString("status"))
        assertEquals("the written-ahead row gave way to the relay's: one row", 1, rows.length())
        // Spec 082 RJ1: the record was written ahead under the LOCAL hash before
        // the POST; the relay answered another hash (`userop.hash_mismatch`), so
        // the local one was withdrawn and the relay's recorded and handed over.
        val local = signedOp!!
        assertTrue("the write-ahead came first: $events", events.indexOf("op-signed:$local") in 0 until events.indexOf("relay.send"))
        // The write-ahead's hand-off names no record (082 second review); the
        // withdrawal names the written-ahead row it takes back.
        assertTrue(handed.first().recordIds.isEmpty())
        assertEquals(local, withdraws.single().first)
        assertTrue(withdraws.single().second.isNotEmpty())
        assertEquals(listOf(row.getString("id")), handoffs.last().second)
        assertEquals("0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1", handoffs.last().first)
        assertEquals("the relay answered: sent", false, stored.optBoolean("maybeSent", true))
        assertTrue("the ceremony's two edges reached the core: $events", events.indexOf("signing") in 0 until events.indexOf("signed"))
        assertEquals(origin, row.getString("dappOrigin"))
        assertTrue(row.getString("to").equals(founder, ignoreCase = true))
        withTimeout(10_000) { c.closed.first { it } }
    }

    /**
     * The relay answered another hash within the write-ahead's millisecond (a
     * fast relay; here a clock that stands still), so the relay's row takes
     * the withdrawn row's id (`dapp-<ms>-tx`). That deleted row is not the
     * relay's on disk: its op reaches the tracker only once its own row is
     * written (043's ordering) — the transfer test's other flake, where it
     * arrived first ("track:NOT-PERSISTED") — and after the write-ahead's.
     */
    @Test
    fun `a relay hash in the write-ahead's millisecond waits for its own row`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller(now = { 1_791_000_000_000.0 })
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        withTimeout(10_000) { while (handed.size < 2) delay(20) }
        val record = "dapp-1791000000000-tx"
        assertEquals("both rows took the one id: $events", 2, events.count { it == "record:$record" })
        assertEquals("the write-ahead's, then the relay's", listOf(emptyList(), listOf(record)), handed.map { it.recordIds })
        assertEquals("track:persisted", events.filter { it.startsWith("track:") }.last())
        assertTrue("the relay's row was on disk first: $events", events.indexOf("persisted:with-hash") < events.lastIndexOf("track:persisted"))
        withTimeout(10_000) { c.closed.first { it } }
    }

    /**
     * Founder, 2026-09-26: a page's transaction signs with the key the account
     * signed in with, over the route that reached it — here its SECOND key,
     * on a security key. The sheet offers no other.
     */
    @Test
    fun `a page's transaction signs with the key the account signed in with, over its route`() = runBlocking<Unit> {
        val second = fixtureAccounts()[1].credentialIdHex
        seedAccount(signedInWith = JSONObject().put("credential_id", second).put("method", "security_key")); scriptRelay()
        val c = controller()
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        assertEquals("0xtx", answers.first { it.first == "tab-1/r1" }.second.getString("result"))
        assertEquals(listOf(Triple<String?, String, KeyMethod>(second, "usb,nfc,ble", KeyMethod.SecurityKey)), routes)
        withTimeout(10_000) { c.closed.first { it } }
    }

    /**
     * Spec 082 G21: the relay took the operation and its reply was lost. The
     * page gets exactly one answer — "not confirmed yet", naming the LOCAL op
     * hash, never that hash as a result (083, owner ruling 2026-10-01) — the
     * row is written pending under that hash with `maybeSent`, and the
     * tracker is handed it as a may-have-been-sent op.
     */
    @Test
    fun `a lost reply is answered not confirmed once and hands the tracker a may-have-been-sent op`() = runBlocking<Unit> {
        seedAccount(); scriptRelay(receiptLands = false)
        port.always("eth_sendUserOperation") { events += "relay.send"; app.getvela.wallet.feature.wallet.core.RpcResult.Failed(rateLimited = false, maybeDelivered = true) }
        val c = controller(receiptWaitMs = 600L)
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        val answered = answers.filter { it.first == "tab-1/r1" }
        assertEquals("exactly one answer", 1, answered.size)
        val local = notConfirmedOp(answered.single().second)
        withTimeout(10_000) { c.closed.first { it } }
        delay(300)
        val row = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0)
        assertEquals(local, row.getString("userOpHash"))
        assertEquals("pending", row.getString("status"))
        assertTrue("kept with the row for a restart (T184)", row.getBoolean("maybeSent"))
        // Written ahead (no record named), then the lost reply's verdict names
        // the row: one op, both "may have been sent" (082 second review).
        withTimeout(10_000) { while (handed.size < 2) delay(20) }
        assertEquals(listOf(local, local), handoffs.map { it.first })
        assertEquals(listOf(emptyList(), listOf(row.getString("id"))), handoffs.map { it.second })
        assertTrue(handed.all { it.maybeSent })
        assertEquals("posted once: nobody pays twice", 1, events.count { it == "relay.send" })
    }

    /**
     * Spec 082 RB2 (money rule): the page's tab closed while the passkey was
     * up. Between the passkey and the relay POST the executor asks again —
     * nothing is sent for nobody, nobody is answered, nothing is recorded.
     */
    @Test
    fun `a page that left during the passkey gets nothing sent`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        var sheet: SigningController? = null
        val leavesMidCeremony = object : UserOpSigner {
            override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
                // The browser core's `cancelSigning`: the page is gone.
                sheet!!.cancel()
                return fixtureSigner.sign(challenge, credentialIdHex, transports, method)
            }
        }
        val c = controller(signer = leavesMidCeremony).also { sheet = it }
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(20_000) { while (events.none { it == "sign" }) delay(50) }
        delay(1_500)
        assertEquals("never posted: $events", 0, events.count { it == "relay.send" })
        assertTrue("nobody answered: $answers", answers.none { it.first == "tab-1/r1" })
        assertTrue(store.values[KeyValueStore.Keys.TRANSACTIONS].isNullOrEmpty() || JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).length() == 0)
        assertTrue(handoffs.isEmpty())
    }

    /** Spec 082 RA10: nothing left the device — the page is told so in the core's fixed words, and nothing is recorded. */
    @Test
    fun `a relay that was never reached answers not sent in the core's words and records nothing`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        port.always("eth_sendUserOperation") { app.getvela.wallet.feature.wallet.core.RpcResult.Failed(rateLimited = false, maybeDelivered = false) }
        val c = controller()
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        // Spec 096 F8: the failure is on the sheet, its answer held — nothing
        // was sent, so it may be tried again — until the person closes it.
        val failed = withTimeout(30_000) { c.sign.first { it.error != null } }
        assertTrue("nothing sent, no refusal: Try again", failed.failure_retryable)
        assertTrue("held while it shows", answers.none { it.first == "tab-1/r1" })
        c.swipeDismissed()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        val error = answers.single { it.first == "tab-1/r1" }.second.getJSONObject("error")
        assertEquals(-32603, error.getInt("code"))
        assertTrue("never the pool's text: $error", error.getString("message").contains(uniffi.vela_core_uniffi.userOpNotSentDetail()))
        // Spec 082 RJ1: the record was written ahead, then withdrawn — the
        // op is proven never sent, so nothing is left to follow.
        withTimeout(10_000) { while (withdraws.isEmpty()) delay(20) }
        delay(200)
        assertTrue("no record for an op nobody has", store.values[KeyValueStore.Keys.TRANSACTIONS].isNullOrEmpty() || JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).length() == 0)
        assertEquals("every hand-off taken back", handoffs.map { it.first }.distinct(), withdraws.map { it.first })
        assertTrue("handed over only as may-have-been-sent, before the POST", handed.all { it.maybeSent && !it.admitted })
    }

    /**
     * Spec 096 F8: a failure that sent nothing is tried again from the sheet —
     * the request goes back to review, still unanswered, and the second confirm
     * is answered with the transaction like any other.
     */
    @Test
    fun `try again after a failure that sent nothing submits afresh and answers once`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        port.always("eth_sendUserOperation") { app.getvela.wallet.feature.wallet.core.RpcResult.Failed(rateLimited = false, maybeDelivered = false) }
        val c = controller()
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { c.sign.first { it.error != null && it.failure_retryable } }
        // The relay is back; the person tries again.
        scriptRelay()
        c.retry()
        withTimeout(20_000) { c.sign.first { it.error == null && it.confirm_gate_open } }
        assertTrue("still unanswered", answers.none { it.first == "tab-1/r1" })
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        val answered = answers.filter { it.first == "tab-1/r1" }
        assertEquals("exactly one answer", 1, answered.size)
        assertEquals("0xtx", answered.single().second.getString("result"))
        assertEquals("signed twice: once per confirm", 2, signs)
    }

    /**
     * Issue 262: the bundler accepted the op but no receipt came inside the
     * wait. The page still gets an answer — the op hash — but the record is
     * NOT flipped to confirmed with the op hash as its tx hash: it stays
     * pending, and the tracker (handed the op and the record) settles it.
     */
    @Test
    fun `a late receipt is answered not confirmed and leaves the record pending for the tracker`() = runBlocking<Unit> {
        seedAccount(); scriptRelay(receiptLands = false)
        val c = controller(receiptWaitMs = 600L)
        c.open(transfer())
        withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet && it.request != null } }
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        assertEquals(
            "the page is told it is not confirmed yet when the receipt is late (083)",
            "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1",
            notConfirmedOp(answers.first { it.first == "tab-1/r1" }.second),
        )
        withTimeout(10_000) { c.closed.first { it } }
        // Give any (wrong) confirming patch time to land before looking.
        delay(500)
        val row = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0)
        assertEquals("0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1", row.getString("userOpHash"))
        assertEquals("nothing is known to have landed", "pending", row.getString("status"))
        assertEquals("the op hash is never recorded as a tx hash", "", row.optString("txHash"))
        val handoff = handoffs.lastOrNull()
        assertEquals("the tracker holds the op", "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1", handoff?.first)
        assertEquals("…and the very record it must settle", listOf(row.getString("id")), handoff?.second)
    }

    /**
     * Spec 082 RJ1 (G34, P0): the record exists before the bytes leave. The
     * relay's handler looks at the store at the moment of the POST: one
     * pending "may have been sent" row under the op's own hash is already
     * there. The relay taking that op marks the row sent — still pending, the
     * tracker's to close — and hands the tracker the same op again, admitted.
     */
    @Test
    fun `the record is on disk before the POST, and the relay taking the op marks it sent`() = runBlocking<Unit> {
        seedAccount(); scriptRelay(receiptLands = false)
        port.always("eth_sendUserOperation") {
            events += "relay.send"
            val rows = JSONArray(store.values[KeyValueStore.Keys.TRANSACTIONS] ?: "[]")
            val ahead = (0 until rows.length()).map { rows.getJSONObject(it) }.singleOrNull { it.optBoolean("maybeSent") && it.optString("status") == "pending" }
            events += if (ahead != null) "relay.saw-record" else "relay.NO-RECORD"
            // The relay accepts it under the hash the wallet computed.
            FakeRelayPort.body(ahead?.getString("userOpHash") ?: signedOp)
        }
        val c = controller(receiptWaitMs = 600L)
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        assertTrue("the row was on disk when the POST left: $events", "relay.saw-record" in events)
        val local = signedOp!!
        assertEquals("one answer: not confirmed yet (the receipt is late)", local, notConfirmedOp(answers.single { it.first == "tab-1/r1" }.second))
        withTimeout(10_000) { c.closed.first { it } }
        withTimeout(10_000) { while (handed.size < 2) delay(20) }
        delay(200)
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        assertEquals("one row, never a second for the same op", 1, rows.length())
        val row = rows.getJSONObject(0)
        assertEquals(local, row.getString("userOpHash"))
        assertEquals("pending", row.getString("status"))
        assertEquals("the relay took it: no longer may-have-been-sent", false, row.getBoolean("maybeSent"))
        // The hand-off key includes the flags (the review of RJ1): both are fed.
        assertEquals(listOf(true to false, false to true), handed.map { it.maybeSent to it.admitted })
        assertTrue(handed.all { it.userOpHash == local })
        assertEquals("the write-ahead names no record; the relay's verdict names the row", listOf(emptyList(), listOf(row.getString("id"))), handed.map { it.recordIds })
        assertTrue(withdraws.isEmpty())
        assertEquals("posted once", 1, events.count { it == "relay.send" })
    }

    /**
     * Spec 082 RJ1 (DX9): the app is gone — quit, crashed, the window closed —
     * after the record was written and while the POST is out. What is left on
     * disk is one pending "may have been sent" row under the op's hash, which
     * the tracker's next launch reads back as exactly that and resolves.
     */
    @Test
    fun `a quit while the POST is out leaves one pending may-have-been-sent row for the next launch`() = runBlocking<Unit> {
        seedAccount(); scriptRelay(receiptLands = false)
        val posting = kotlinx.coroutines.CompletableDeferred<Unit>()
        port.before = { method -> if (method == "eth_sendUserOperation") { posting.complete(Unit); kotlinx.coroutines.awaitCancellation() } }
        val c = controller()
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { posting.await() }
        // The process ends here: nothing more runs.
        scope.cancel()
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        assertEquals(1, rows.length())
        val row = rows.getJSONObject(0)
        assertEquals(signedOp, row.getString("userOpHash"))
        assertEquals("pending", row.getString("status"))
        assertTrue("may have been sent", row.getBoolean("maybeSent"))
        assertEquals("dapp_tx", row.getString("type"))
        assertTrue("nobody was answered", answers.isEmpty())
        // The next launch: the tracker's pending records read it back as one.
        val pending = app.getvela.wallet.feature.send.core.TrackerExecutor.pendingRecord(row)!!
        assertEquals(signedOp, pending.user_op_hash)
        assertTrue(pending.maybe_sent)
        assertEquals(handed.single().userOpHash, signedOp)
        assertTrue(handed.single().maybeSent)
    }

    /**
     * Spec 082 RJ1: no clearance, no POST. The store would not take the record,
     * so the core's clearance is not one this shell can stand behind: nothing
     * is posted, and the page is told "not sent" in the core's words.
     */
    @Test
    fun `a record the store would not take is never posted`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        store.refuseWrites = true
        c.approve()
        withTimeout(30_000) { c.sign.first { it.error != null } }
        c.swipeDismissed()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        assertEquals("zero POSTs", 0, events.count { it == "relay.send" })
        val error = answers.single { it.first == "tab-1/r1" }.second.getJSONObject("error")
        assertEquals(-32603, error.getInt("code"))
        assertTrue(error.getString("message").contains(uniffi.vela_core_uniffi.userOpNotSentDetail()))
    }

    /**
     * Spec 082 RJ3: the relay refused the op. The page gets one -32603 in the
     * core's "refused" words — never the relay's text, never "try again" — and
     * the sheet says it was refused.
     */
    @Test
    fun `a relay refusal answers refused once and the sheet never says try again`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        port.always("eth_sendUserOperation") { events += "relay.send"; FakeRelayPort.error("UserOperation reverted during simulation with reason: AA23 reverted") }
        val c = controller()
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        val sheet = withTimeout(30_000) { c.sign.first { it.error != null } }
        assertTrue("the sheet knows it was refused", sheet.failure_refused)
        assertTrue("a refusal is not tried again", !sheet.failure_retryable)
        // Spec 096 F8: answered when the person closes the sheet.
        assertTrue("held while it shows", answers.none { it.first == "tab-1/r1" })
        c.swipeDismissed()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r1" }) delay(50) }
        val answered = answers.filter { it.first == "tab-1/r1" }
        assertEquals("exactly one answer", 1, answered.size)
        val error = answered.single().second.getJSONObject("error")
        assertEquals(-32603, error.getInt("code"))
        assertTrue("the core's words: $error", error.getString("message").contains(uniffi.vela_core_uniffi.userOpRefusedDappDetail()))
        assertEquals("posted once", 1, events.count { it == "relay.send" })
    }

    /**
     * Spec 082 RJ4 (EX13, DX-W1): the page gets its tx hash as soon as the
     * tracker knows it — here the relay's receipt never comes, and the tracker
     * confirms the op through the chain. The core answers from `OpTracked`,
     * once, and the receipt wait ends there instead of polling on.
     */
    @Test
    fun `the page is answered from the tracker's verdict and the receipt wait ends`() = runBlocking<Unit> {
        seedAccount(); scriptRelay(receiptLands = false); relayTakesTheLocalHash()
        val c = controller(receiptWaitMs = 60_000L)
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (events.none { it.startsWith("op:") }) delay(20) }
        delay(300)
        val landed = "0x" + "c4".repeat(32)
        trackerSays(app.getvela.wallet.feature.send.core.TrackStatus.Confirmed, landed)
        withTimeout(10_000) { while (answers.none { it.first == "tab-1/r1" }) delay(20) }
        assertEquals("the tracker's tx hash, long before the 60 s wait", landed, answers.single { it.first == "tab-1/r1" }.second.getString("result"))
        delay(400)
        val polls = port.calls.count { it.endsWith("eth_getUserOperationReceipt") }
        delay(600)
        assertEquals("the receipt wait ended with the answer", polls, port.calls.count { it.endsWith("eth_getUserOperationReceipt") })
        assertEquals("exactly one answer", 1, answers.count { it.first == "tab-1/r1" })
    }

    /**
     * Spec 082 RJ4 (review): the tracker holds the op from the write-ahead on,
     * so it can reach its verdict while the POST is still out — a slow relay,
     * the op already mined. That verdict reaches the core before the core holds
     * the op (`OpSubmitted`), and is dropped there; the tracker's entry does not
     * change again. The page still gets the tracker's tx hash as soon as the
     * relay's answer lands — not the op hash at the end of the window.
     */
    @Test
    fun `a verdict the tracker reached while the POST was out answers the page once the relay's answer lands`() = runBlocking<Unit> {
        seedAccount(); scriptRelay(receiptLands = false)
        port.always("eth_sendUserOperation") { events += "relay.send"; FakeRelayPort.body(signedOp) }
        val posting = kotlinx.coroutines.CompletableDeferred<Unit>()
        val release = kotlinx.coroutines.CompletableDeferred<Unit>()
        port.before = { method -> if (method == "eth_sendUserOperation") { posting.complete(Unit); release.await() } }
        val c = controller(receiptWaitMs = 60_000L)
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { posting.await() }
        val landed = "0x" + "d5".repeat(32)
        trackerSays(app.getvela.wallet.feature.send.core.TrackStatus.Confirmed, landed)
        delay(500)
        assertTrue("nobody is answered while the POST is out: $answers", answers.none { it.first == "tab-1/r1" })
        release.complete(Unit)
        withTimeout(10_000) { while (answers.none { it.first == "tab-1/r1" }) delay(20) }
        assertEquals("the tracker's tx hash, long before the 60 s wait", landed, answers.single { it.first == "tab-1/r1" }.second.getString("result"))
        delay(500)
        assertEquals("exactly one answer", 1, answers.count { it.first == "tab-1/r1" })
        assertEquals("posted once", 1, events.count { it == "relay.send" })
    }

    /**
     * Spec 082 RJ3 (DX-W3): the tracker's `rejected` answers the page -32603
     * "refused", once. Spec 097 N4: after "Submitted" — the relay took the op —
     * the refusal stays on the sheet in its own words, Done and no Try again,
     * and the page hears it when the person closes it, not under the words.
     */
    @Test
    fun `the tracker's rejection answers refused once`() = runBlocking<Unit> {
        seedAccount(); scriptRelay(receiptLands = false); relayTakesTheLocalHash()
        val c = controller(receiptWaitMs = 60_000L)
        c.open(transfer())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (events.none { it.startsWith("op:") }) delay(20) }
        withTimeout(10_000) { c.sign.first { it.pending_op_hash != null } }
        trackerSays(app.getvela.wallet.feature.send.core.TrackStatus.Rejected, null)
        val sheet = withTimeout(10_000) { c.sign.first { it.error != null } }
        assertEquals("the sheet stays", SignSurface.Sheet, sheet.surface)
        assertTrue("it says refused", sheet.failure_refused)
        assertTrue("the rid went out: no Try again", !sheet.failure_retryable)
        assertNull("no longer submitted", sheet.pending_op_hash)
        delay(500)
        assertTrue("held while it shows: $answers", answers.none { it.first == "tab-1/r1" })
        assertFalse("the sheet is not over", c.closed.value)
        c.swipeDismissed()
        withTimeout(10_000) { while (answers.none { it.first == "tab-1/r1" }) delay(20) }
        val error = answers.single { it.first == "tab-1/r1" }.second.getJSONObject("error")
        assertEquals(-32603, error.getInt("code"))
        assertTrue(error.getString("message").contains(uniffi.vela_core_uniffi.userOpRefusedDappDetail()))
        withTimeout(10_000) { c.closed.first { it } }
        delay(500)
        assertEquals("exactly one answer", 1, answers.count { it.first == "tab-1/r1" })
    }

    @Test
    fun `a speed picked on the sheet is the speed priced, signed and named on the wire`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        c.open(transfer())
        // The stored default first — `standard` for everybody who never chose.
        val first = withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        assertEquals(FeeTier.Standard, first.fee!!.tier)
        assertEquals(FeeTier.Standard, c.speed.value.tier)
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
        assertFalse("never the speed walked away from: $events", "relay.tier:standard" in events)
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
    fun `an unlimited approval is kept as asked and goes out as the site built it`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        val usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"
        val spender = "0x1111111111111111111111111111111111111111"
        val data = "0x095ea7b3" + spender.drop(2).padStart(64, '0') + "f".repeat(64)
        c.open(IncomingRequest("r3k", "eth_sendTransaction", JSONArray().put(JSONObject().put("from", safe).put("to", usdc).put("data", data).put("value", "0x0")).toString(), origin, "tab-1", 100))
        val guard = withTimeout(20_000) { c.guard.first { it.detected != null } }
        assertTrue(guard.detected!!.is_unbounded)
        // 2026-09-26: Permit2 bundles revert when the wallet re-encodes the
        // approve, so the site's ask is preselected — seen, said, consented.
        assertTrue("kept as asked needs no extra tap", guard.confirm_allowed)
        assertEquals(app.getvela.wallet.feature.signing.core.GuardChoice.Unlimited, guard.editor!!.choice)
        assertTrue(guard.unlimited_consented)
        assertNull("the site's own bytes", guard.rewritten_params_json)
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        withTimeout(30_000) { while (answers.none { it.first == "tab-1/r3k" }) delay(50) }
        assertTrue("the submit guard let the consented approval through", answers.first { it.first == "tab-1/r3k" }.second.has("result"))
        assertEquals(1, signs)
        assertEquals(1, port.calls.count { it.endsWith("eth_sendUserOperation") })
    }

    @Test
    fun `an unlimited approval leaves bounded once the person names a cap`() = runBlocking<Unit> {
        seedAccount(); scriptRelay()
        val c = controller()
        val usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"
        val spender = "0x1111111111111111111111111111111111111111"
        val data = "0x095ea7b3" + spender.drop(2).padStart(64, '0') + "f".repeat(64)
        c.open(IncomingRequest("r3", "eth_sendTransaction", JSONArray().put(JSONObject().put("from", safe).put("to", usdc).put("data", data).put("value", "0x0")).toString(), origin, "tab-1", 100))
        val guard = withTimeout(20_000) { c.guard.first { it.detected != null } }
        assertTrue(guard.detected!!.is_unbounded)
        c.guardPreset(app.getvela.wallet.feature.signing.core.GuardEditorMode.Custom)
        c.guardCustomAmount("1")
        val bounded = withTimeout(20_000) { c.guard.first { it.confirm_allowed && it.rewritten_params_json != null } }
        assertFalse("a cap withdraws the consent", bounded.unlimited_consented)
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

    /**
     * The op a "not confirmed yet" answer names (083, owner ruling
     * 2026-10-01): `-32603` with the core's sentence — never the op hash as a
     * result.
     */
    private fun notConfirmedOp(answer: org.json.JSONObject): String {
        assertFalse("never the op hash as a result: $answer", answer.has("result"))
        val error = answer.getJSONObject("error")
        assertEquals(-32603, error.getInt("code"))
        val message = error.getString("message")
        assertTrue(message, message.startsWith("The transaction was submitted but is not confirmed yet"))
        return Regex("user operation (0x[0-9a-fA-F]{64})").find(message)?.groupValues?.get(1)
            ?: throw AssertionError("the op is named: $message")
    }
}
