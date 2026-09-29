package app.getvela.wallet

// Spec 084 CHECK PASS - 083 parity probes for Android (S3b, S2, S3, A-6, A-R3, A-H4a).
// TEST SOURCE ONLY. No production file is touched. Each probe RECORDS what the
// code does today (to evidence/android/jvm-probe-observations.txt) and then
// asserts the hand-off's PREDICTED (BAD) behaviour, so a red probe means the
// hand-off's prediction was WRONG and a green probe means it was CONFIRMED.
//
// It needs the debug fixture keys (uniffi.vela_dev_fixtures), so it compiles
// only for the debug variant: run it with :app:testDebugUnitTest.

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.TrackEntryView
import app.getvela.wallet.feature.send.core.TrackOutcome
import app.getvela.wallet.feature.send.core.TrackStatus
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.signing.SigningAftercare
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignErrorNotice
import app.getvela.wallet.feature.signing.core.SignExecutor
import app.getvela.wallet.feature.signing.core.SignMethodKind
import app.getvela.wallet.feature.signing.core.SignOperation
import app.getvela.wallet.feature.signing.core.SignQuotedFee
import app.getvela.wallet.feature.signing.core.SignRequestView
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.SignShellResult
import app.getvela.wallet.feature.signing.core.SignSubmitOutcome
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import androidx.compose.ui.graphics.Color
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
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

class Probe083ReceiptsTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()
    private val safe = fixtureMultiAddress()
    private val founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val origin = "http://127.0.0.1:8137"

    /** This op's hash as the relay would answer it on a clean accept; and ANOTHER op's. */
    private val ownOp = "0x" + "11".repeat(32)
    private val otherOp = "0x" + "ab".repeat(32)
    private val otherTx = "0x" + "cd".repeat(32)
    private val revertedTx = "0x" + "ef".repeat(32)

    @After
    fun stop() = scope.cancel()

    private fun observe(id: String, line: String) {
        println("PROBE083 [$id] $line")
        val root = System.getProperty("vela.repo.root") ?: return
        runCatching {
            val file = File(root, "specs/084-mobile-web-083-parity/evidence/android/jvm-probe-observations.txt")
            file.parentFile.mkdirs()
            file.appendText("[$id] $line\n")
        }
    }

    // ---------------------------------------------------------------- scaffolding

    private val signs = java.util.concurrent.atomic.AtomicInteger(0)
    private val fixtureSigner = object : UserOpSigner {
        override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
            signs.incrementAndGet()
            val signed = fixtureAssert(challenge, listOfNotNull(credentialIdHex), 0u)
            return Assertion(signed.credentialIdHex, signed.signatureDerHex, signed.authenticatorDataHex, signed.clientDataJsonHex, null, "platform")
        }
    }

    private fun seedAccount() {
        val keyset = fixtureAccounts()
        val keys = JSONArray()
        keyset.forEach { keys.put(JSONObject().put("credential_id", it.credentialIdHex).put("public_key_hex", it.publicKeyHex).put("name", it.name).put("transports", "internal")) }
        val account = JSONObject().put("id", keyset.first().credentialIdHex).put("name", "Parallel space").put("address", safe)
            .put("public_key_hex", keyset.first().publicKeyHex).put("created_at_iso", "2026-09-12T00:00:00Z").put("keys", keys)
        store.values["vela.accounts"] = JSONArray().put(account).toString()
        store.values["vela.activeAccountIndex"] = "0"
    }

    private fun errorBody(code: Int, message: String) =
        app.getvela.wallet.feature.wallet.core.RpcResult.Body(JSONObject().put("error", JSONObject().put("code", code).put("message", message)))

    private fun receiptBody(success: Boolean, tx: String) = FakeRelayPort.body(
        JSONObject().put("success", success).put("sender", safe).put("receipt", JSONObject().put("transactionHash", tx).put("logs", JSONArray())),
    )

    /** The direct-executor script (hand-off 7.1): an UNDEPLOYED Safe, so no nonce read; the relay's estimate; submit and receipt as each probe says. */
    private fun scriptDirect() {
        port.always("eth_getCode") { FakeRelayPort.body("0x") }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x30d40").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
    }

    private class Seen {
        val opSubmitted = CopyOnWriteArrayList<Pair<String, String>>()
        val responds = CopyOnWriteArrayList<SignResponsePayload>()
        val persisted = CopyOnWriteArrayList<String>()
    }

    private fun executor(seen: Seen, receiptWaitMs: Long, receiptPollMs: Long): SignExecutor {
        val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        val accounts = StoreAccountPort(AccountStore(store))
        val spine = UserOpSpine(relay, accounts, { fixtureSigner })
        return SignExecutor(
            spine = spine, relay = relay, feed = feed,
            ports = object : SignExecutor.Ports {
                override fun respond(transportId: String, id: String, payload: SignResponsePayload) { seen.responds += payload }
                override fun opSubmitted(id: String, userOpHash: String) { seen.opSubmitted += id to userOpHash }
                override fun signingStarted() = Unit
                override fun recordsPersisted() = Unit
                override fun recordPersisted(recordId: String) { seen.persisted += recordId }
                override suspend fun switchAccount(address: String) = true
                override fun nativeSymbol(chainId: Int) = "XDAI"
            },
            receiptWaitMs = receiptWaitMs, receiptPollMs = receiptPollMs,
            origin = { origin },
        )
    }

    private fun sendDust() = SignOperation.SignAndSubmit(
        id = "probe-1", method = "eth_sendTransaction",
        params_json = JSONArray().put(JSONObject().put("from", safe).put("to", founder).put("value", "0x38d7ea4c68000")).toString(),
        chain_id = 100, address = safe, credential_id = fixtureAccounts().first().credentialIdHex,
        quoted_fee = SignQuotedFee(amount = "1000", recipient = safe),
    )

    private fun submitted(): Boolean = port.calls.any { it.endsWith("eth_sendUserOperation") }

    // ---------------------------------------------------------------- A-S3b (direct executor)

    @Test
    fun `A-S3b direct - an already-pending marker answers THIS request with the OTHER op's transaction`() = runBlocking<Unit> {
        seedAccount(); scriptDirect()
        port.always("eth_sendUserOperation") { errorBody(-32602, "already pending [existingHash:$otherOp]") }
        port.always("eth_getUserOperationReceipt") { receiptBody(true, otherTx) }
        val seen = Seen()
        val result = executor(seen, receiptWaitMs = 2_000, receiptPollMs = 50).perform(sendDust())
        val outcome = (result as SignShellResult.Submit).outcome
        observe("A-S3b/direct", "calls=${port.calls} signs=${signs.get()} outcome=$outcome opSubmitted=${seen.opSubmitted}")
        assertTrue("eth_sendUserOperation was never called - the run measured nothing: ${port.calls}", submitted())
        assertEquals("PREDICTED BAD: Succeeded(OTHER's tx)", SignSubmitOutcome.Succeeded(otherTx), outcome)
        assertEquals("PREDICTED BAD: Ports.opSubmitted receives OTHER", otherOp, seen.opSubmitted.single().second)
    }

    /** relay_error_message returns early for AA25 / 'invalid account nonce' (hand-off: the marker is lost). */
    @Test
    fun `A-S3b direct - the same marker inside an AA25 invalid-nonce wording`() = runBlocking<Unit> {
        seedAccount(); scriptDirect()
        port.always("eth_sendUserOperation") { errorBody(-32602, "AA25 invalid account nonce [existingHash:$otherOp]") }
        port.always("eth_getUserOperationReceipt") { receiptBody(true, otherTx) }
        val seen = Seen()
        val result = executor(seen, receiptWaitMs = 2_000, receiptPollMs = 50).perform(sendDust())
        val outcome = (result as SignShellResult.Submit).outcome
        observe("A-S3b/AA25", "calls=${port.calls} outcome=$outcome opSubmitted=${seen.opSubmitted}")
        assertTrue("eth_sendUserOperation was never called: ${port.calls}", submitted())
        // No prediction beyond 'not OTHER': the hand-off says the marker is lost before parsing.
        assertTrue("an AA25 wording never answers OTHER's hash: $outcome", seen.opSubmitted.none { it.second == otherOp })
    }

    // ---------------------------------------------------------------- A-S2 (direct executor)

    @Test
    fun `A-S2 direct - a receipt with success=false is answered as Succeeded(bundle tx)`() = runBlocking<Unit> {
        seedAccount(); scriptDirect()
        port.always("eth_sendUserOperation") { FakeRelayPort.body(ownOp) }
        port.always("eth_getUserOperationReceipt") { receiptBody(false, revertedTx) }
        val seen = Seen()
        val result = executor(seen, receiptWaitMs = 2_000, receiptPollMs = 50).perform(sendDust())
        val outcome = (result as SignShellResult.Submit).outcome
        observe("A-S2/direct", "calls=${port.calls} outcome=$outcome opSubmitted=${seen.opSubmitted}")
        assertTrue("eth_sendUserOperation was never called: ${port.calls}", submitted())
        assertEquals("PREDICTED BAD: Succeeded('0xabc...') for a reverted op", SignSubmitOutcome.Succeeded(revertedTx), outcome)
    }

    // ---------------------------------------------------------------- A-S3 (direct executor)

    @Test
    fun `A-S3 direct - a receipt that never arrives is answered ReceiptPending(op hash)`() = runBlocking<Unit> {
        seedAccount(); scriptDirect()
        port.always("eth_sendUserOperation") { FakeRelayPort.body(ownOp) }
        port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
        val seen = Seen()
        val result = executor(seen, receiptWaitMs = 300, receiptPollMs = 50).perform(sendDust())
        val outcome = (result as SignShellResult.Submit).outcome
        observe("A-S3/direct", "calls=${port.calls} outcome=$outcome opSubmitted=${seen.opSubmitted}")
        assertTrue("eth_sendUserOperation was never called: ${port.calls}", submitted())
        assertEquals("PREDICTED BAD: ReceiptPending(op)", SignSubmitOutcome.ReceiptPending(ownOp), outcome)
    }

    // ---------------------------------------------------------------- the same three through the REAL SigningController (the live path)

    private val answers = java.util.Collections.synchronizedList(ArrayList<Pair<String, JSONObject>>())
    private val handoffs = java.util.Collections.synchronizedList(ArrayList<Pair<String, List<String>>>())
    private val opSubmittedIds = java.util.Collections.synchronizedList(ArrayList<String>())

    private fun scriptLive(send: () -> app.getvela.wallet.feature.wallet.core.RpcResult, receipt: () -> app.getvela.wallet.feature.wallet.core.RpcResult) {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body("0x3b9aca00") }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", "0x3b9aca00")) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x5f5e100") }
        port.always("pimlico_getUserOperationGasPrice") {
            fun row(max: String) = JSONObject().put("maxFeePerGas", max).put("networkFeePerGas", "0x3b9aca00").put("relayerFeePerGas", "0x3b9aca00")
            FakeRelayPort.body(JSONObject().put("fast", row("0x77359400")).put("standard", row("0x59682f00")).put("slow", row("0x4a817c80")))
        }
        port.always("vela_getInBandGasQuote") {
            FakeRelayPort.body(JSONArray().put(JSONObject().put("recipient", "0x2222222222222222222222222222222222222222").put("asset", "native").put("balance", "0x9f3306a949ca000").put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1")))
        }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        port.always("eth_sendUserOperation") { send() }
        port.always("eth_getUserOperationReceipt") { receipt() }
        port.rest["https://relay.test/v1/treasury/100"] = RestAnswer.Ok(JSONObject().put("address", "0x1111111111111111111111111111111111111111").put("bootstrapNeeded", false))
        port.rest["https://relay.test/v1/account/100/${safe.lowercase()}"] = RestAnswer.Ok(JSONObject().put("activeDepositAddress", "0x2222222222222222222222222222222222222222").put("status", "ACTIVE"))
    }

    private fun controller(receiptWaitMs: Long): SigningController {
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
                override fun respond(transportId: String, id: String, payload: SignResponsePayload) {
                    val json = when (payload) {
                        is SignResponsePayload.Ok -> JSONObject().put("result", payload.result ?: JSONObject.NULL)
                        is SignResponsePayload.Err -> JSONObject().put("error", JSONObject().put("code", payload.code).put("message", payload.message.orEmpty()))
                    }
                    answers += "$transportId/$id" to json
                }
                override fun opSubmitted(id: String, userOpHash: String) { opSubmittedIds += userOpHash }
                override fun signingStarted() = Unit
                override fun recordsPersisted() = Unit
                override fun recordPersisted(recordId: String) = Unit
                override suspend fun switchAccount(address: String) = true
                override fun nativeSymbol(chainId: Int) = "XDAI"
                override fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int) { handoffs += userOpHash to recordIds }
                override fun dataBase() = ""
                override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
            },
        )
    }

    private fun dust() = IncomingRequest(
        id = "r1", method = "eth_sendTransaction",
        paramsJson = JSONArray().put(JSONObject().put("from", safe).put("to", founder).put("value", "0x38d7ea4c68000")).toString(),
        origin = origin, transportId = "tab-1", chainId = 100,
    )

    /** Slide (approve) and return the page's answer, waiting up to [waitMs]. */
    private suspend fun runLive(receiptWaitMs: Long, waitMs: Long = 40_000): JSONObject? {
        val c = controller(receiptWaitMs)
        c.open(dust())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        val until = System.currentTimeMillis() + waitMs
        while (answers.none { it.first == "tab-1/r1" } && System.currentTimeMillis() < until) delay(50)
        return answers.firstOrNull { it.first == "tab-1/r1" }?.second
    }

    private fun rows(): List<JSONObject> {
        val raw = store.values[KeyValueStore.Keys.TRANSACTIONS] ?: return emptyList()
        val array = JSONArray(raw)
        return (0 until array.length()).map { array.getJSONObject(it) }
    }

    @Test
    fun `A-S3b live - the page's answer and the record when a marker names another op`() = runBlocking<Unit> {
        seedAccount()
        scriptLive({ errorBody(-32602, "already pending [existingHash:$otherOp]") }, { receiptBody(true, otherTx) })
        val answer = runLive(receiptWaitMs = 2_000)
        delay(500)
        val row = rows().firstOrNull()
        observe("A-S3b/live", "calls=${port.calls.filter { it.contains("Send") || it.contains("Receipt") }} pageAnswer=$answer opSubmitted=$opSubmittedIds record=userOpHash:${row?.optString("userOpHash")} status:${row?.optString("status")} txHash:${row?.optString("txHash")} handoffs=$handoffs")
        assertTrue("eth_sendUserOperation was never called: ${port.calls}", submitted())
        assertEquals("PREDICTED BAD: the page got OTHER's tx hash", otherTx, answer?.optString("result"))
        assertEquals("PREDICTED BAD: the record is keyed to OTHER's op hash", otherOp, row?.optString("userOpHash"))
    }

    @Test
    fun `A-S2 live - the page's answer and the record for a reverted op`() = runBlocking<Unit> {
        seedAccount()
        scriptLive({ FakeRelayPort.body(ownOp) }, { receiptBody(false, revertedTx) })
        val answer = runLive(receiptWaitMs = 10_000)
        withTimeout(10_000) { while (rows().firstOrNull()?.optString("status") != "confirmed" && rows().firstOrNull()?.optString("status") != "failed") delay(50) }
        val row = rows().first()
        observe("A-S2/live", "pageAnswer=$answer record=status:${row.optString("status")} txHash:${row.optString("txHash")} handoffs=$handoffs")
        assertTrue("eth_sendUserOperation was never called: ${port.calls}", submitted())
        assertEquals("PREDICTED BAD: the page got the bundle tx as a success", revertedTx, answer?.optString("result"))
        assertEquals("PREDICTED BAD: the record closes confirmed", "confirmed", row.optString("status"))
    }

    @Test
    fun `A-S3 live - the page's answer and the record when the receipt never comes`() = runBlocking<Unit> {
        seedAccount()
        scriptLive({ FakeRelayPort.body(ownOp) }, { FakeRelayPort.body(JSONObject.NULL) })
        val answer = runLive(receiptWaitMs = 600)
        delay(500)
        val row = rows().first()
        observe("A-S3/live", "pageAnswer=$answer record=status:${row.optString("status")} txHash:${row.optString("txHash")} handoffs=$handoffs")
        assertTrue("eth_sendUserOperation was never called: ${port.calls}", submitted())
        assertEquals("PREDICTED BAD: the page got the op hash", ownOp, answer?.optString("result"))
        assertEquals("pending", row.optString("status"))
    }

    // ---------------------------------------------------------------- A-6: the page's op-hash receipt translation ignores `confirmed`

    @Test
    fun `A-6 - the relay client reports confirmed=false with the bundle tx, and BrowserController drops it`() = runBlocking<Unit> {
        port.always("eth_getUserOperationReceipt") { receiptBody(false, revertedTx) }
        val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)
        val answer = relay.userOpReceipt(100, ownOp)
        observe("A-6/relay", "userOpReceipt(success=false) -> $answer  (BrowserController.userOpTxHash reads only .txHash, see source line)")
        assertTrue(answer is RelayClient.ReceiptAnswer.Resolved)
        answer as RelayClient.ReceiptAnswer.Resolved
        assertFalse(answer.confirmed)
        assertEquals(revertedTx, answer.txHash)
    }

    // ---------------------------------------------------------------- A-R3 / A-H4a / S2-aftercare: the sheet's words

    private fun strings(tag: String): VelaStrings {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set - run via Gradle")
        return I18nRuntime { t -> File(root, "assets/i18n/$t.json").readBytes() }.apply { initialize(tag) }
    }

    private val op = "0xace642c724834405c56919fb198d6a6389cc2867ff3f3e3585b59d1921fe681d"
    private val tx = "0x09c35478682ddce770c02e4aed27c9f0260f7b2f54ae6a7a194f50b70d604c9f"

    private fun ctx(s: VelaStrings, track: TrackEntryView? = null) =
        SigningLive.Context(s, "Gnosis", Color.Red, "XDAI", "Me", safe, typicalS = 5, track = track, explorerUrl = "https://gnosisscan.io")

    @Test
    fun `A-R3 - an included-but-reverted op that the tracker reports Dropped reads the funds-are-safe sentence`() {
        for (tag in listOf("zh", "en")) {
            val s = strings(tag)
            val dropped = TrackEntryView(user_op_hash = op, chain_id = 100, status = TrackStatus.Dropped, tx_hash = tx, submitted_at_ms = 1.0, outcome = TrackOutcome.Final)
            val model = SigningLive.aftercareReceipt(SigningAftercare.StillConfirming(100, op), "Send", ctx(s, dropped))
            observe("A-R3/$tag/aftercare-Dropped", "stage=${model.stage} title=${model.title} captions=${model.captions} hash=${model.hash?.copyValue} viewOnExplorer=${model.viewOnExplorer} cta=${model.cta}")
            assertTrue("PREDICTED BAD ($tag): captions carry send.txErrorGeneric", model.captions.contains(s.t("send.txErrorGeneric")))
            assertTrue("hint key exists: ${s.t("componentsTx.receipt.failedHint")}", s.t("componentsTx.receipt.failedHint").isNotBlank())
            assertTrue("no failedHint on the Android receipt", model.captions.none { it == s.t("componentsTx.receipt.failedHint") })
            assertTrue("no tx hash on the Android receipt", model.hash == null)
        }
        // And the in-sheet path: an error after approval with an op pending.
        val s = strings("zh")
        val failed = SigningLive.receipt(
            SignView(surface = SignSurface.Sheet, pending_op_hash = op, error = SignErrorNotice(kind = SignErrorKind.SubmitFailed, detail = "The transaction was included but reverted ($tx)")),
            emptyList(), ctx(s),
        )
        observe("A-R3/zh/sheet-SubmitFailed-with-revert-detail", "stage=${failed?.stage} title=${failed?.title} captions=${failed?.captions} hash=${failed?.hash?.copyValue} viewOnExplorer=${failed?.viewOnExplorer}")
        assertTrue(failed!!.captions.contains(s.t("send.txErrorGeneric")))
    }

    @Test
    fun `A-H4a - a failed MESSAGE signature shows the transaction-failure sentence`() {
        for (tag in listOf("zh", "en")) {
            val s = strings(tag)
            val request = SignRequestView(id = "r1", method = "personal_sign", kind = SignMethodKind.PersonalSign, params_json = "[\"\",\"$safe\"]", origin = origin, chain_id = 100)
            val failed = SigningLive.receipt(SignView(surface = SignSurface.Sheet, request = request, error = SignErrorNotice(kind = SignErrorKind.SubmitFailed, detail = "personal_sign carried nothing this wallet could sign")), emptyList(), ctx(s))
            observe("A-H4a/$tag", "stage=${failed?.stage} title=${failed?.title} captions=${failed?.captions} cta=${failed?.cta}  offChainNote=${s.t("connect.detail.offChainNote")}")
            assertTrue("PREDICTED BAD ($tag): txErrorGeneric on a message", failed!!.captions.contains(s.t("send.txErrorGeneric")))
        }
    }

    @Test
    fun `A-S2 aftercare - a tx-hash result is drawn Landed with a confirmed tick, whatever the outcome was`() {
        val landed = SigningAftercare.of("eth_sendTransaction", 100, SignResponsePayload.Ok(revertedTx), ownOp)
        observe("A-S2/aftercare", "of(eth_sendTransaction, Ok(bundle tx of a reverted op), pending op) -> $landed")
        assertEquals(SigningAftercare.Landed(100, revertedTx), landed)
    }
}
