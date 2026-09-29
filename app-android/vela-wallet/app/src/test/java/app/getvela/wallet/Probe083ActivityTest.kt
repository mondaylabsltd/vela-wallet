package app.getvela.wallet

// Spec 084 CHECK PASS - 083 parity probes for Android: the ACTIVITY family
// (A-H2, A-F1, A-F3, A-REC, A-4, A-7, A-8) with no money.
//
// It runs real dApp requests through the REAL SigningController + spine (fixture
// keys, a scripted relay), lets the sign path write its records into an in-memory
// store, feeds that store to the REAL activity_feed core (the build's own
// libvela_core_uniffi), and draws the rows and the detail with the app's own
// WalletLive / FlowLive. It also reads the core's RAW feed JSON, to show which
// 083 fields the core sends and the Kotlin wire drops.
//
// TEST SOURCE ONLY - no production file is touched. It needs the debug fixture
// keys (uniffi.vela_dev_fixtures): run with :app:testDebugUnitTest.

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.FeedEvent
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.FeedOperation
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedShellResult
import app.getvela.wallet.feature.wallet.core.FeedView
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.descriptors.elementNames
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.ActivityFeedCore
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureMultiAddress

class Probe083ActivityTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()
    private val safe = fixtureMultiAddress()
    private val founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"
    private val dead = "0x000000000000000000000000000000000000dEaD"
    private val origin = "http://127.0.0.1:8137"

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

    private fun strings(tag: String): VelaStrings {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set - run via Gradle")
        return I18nRuntime { t -> File(root, "assets/i18n/$t.json").readBytes() }.apply { initialize(tag) }
    }

    private val fixtureSigner = object : UserOpSigner {
        override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
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

    private var opCounter = 0
    private val neverLands = java.util.Collections.synchronizedSet(HashSet<String>())
    private val answers = java.util.Collections.synchronizedList(ArrayList<Pair<String, JSONObject>>())

    private fun opHash(n: Int) = "0x" + n.toString(16).padStart(64, '0')
    private fun txOf(op: String) = "0x" + "77".repeat(31) + op.takeLast(2)

    private fun scriptLive() {
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
        port.always("eth_sendUserOperation") { opCounter += 1; FakeRelayPort.body(opHash(opCounter)) }
        port.always("eth_getUserOperationReceipt") { params ->
            val op = params.firstOrNull() as? String ?: ""
            if (op in neverLands) {
                FakeRelayPort.body(JSONObject.NULL)
            } else {
                FakeRelayPort.body(JSONObject().put("success", true).put("sender", safe).put("receipt", JSONObject().put("transactionHash", txOf(op)).put("logs", JSONArray())))
            }
        }
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
                override fun opSubmitted(id: String, userOpHash: String) = Unit
                override fun signingStarted() = Unit
                override fun recordsPersisted() = Unit
                override fun recordPersisted(recordId: String) = Unit
                override suspend fun switchAccount(address: String) = true
                override fun nativeSymbol(chainId: Int) = "XDAI"
                override fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int) = Unit
                override fun dataBase() = ""
                override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
            },
        )
    }

    /** Open [request], slide, and wait for the page's answer and the sheet's close. */
    private suspend fun slide(request: IncomingRequest, receiptWaitMs: Long = 10_000): JSONObject? {
        val c = controller(receiptWaitMs)
        c.open(request)
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        withTimeout(20_000) { c.sign.first { it.confirm_gate_open } }
        c.approve()
        val until = System.currentTimeMillis() + 40_000
        while (answers.none { it.first == "${request.transportId}/${request.id}" } && System.currentTimeMillis() < until) delay(50)
        val answer = answers.firstOrNull { it.first == "${request.transportId}/${request.id}" }?.second
        delay(400)
        return answer
    }

    private fun approveData(spender: String, amountHex64: String) =
        "0x095ea7b3" + spender.removePrefix("0x").padStart(64, '0') + amountHex64

    private fun request(id: String, method: String, params: JSONObject) = IncomingRequest(
        id = id, method = method, paramsJson = JSONArray().put(params).toString(), origin = origin, transportId = "tab-$id", chainId = 100,
    )

    /** The real activity_feed core over [store], read RAW (JsonObject) and decoded the way the app decodes it. */
    private suspend fun feed(): Pair<JsonObject, FeedView> {
        val executor = FeedExecutor(store = store, ownAccounts = { emptyList() })
        val host = CoreHost(
            bridge = ActivityFeedCore().asBridge(), scope = scope, initial = JsonObject(emptyMap()), serializer = JsonObject.serializer(),
            perform = JsonShell.perform(FeedOperation.serializer(), FeedShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(FeedOperation.serializer(), FeedShellResult.serializer(), fallback = FeedShellResult.HapticPlayed, answer = executor::neutralAnswer),
            onFault = { println("PROBE083 core fault: $it") },
        )
        host.dispatch(FeedEvent.AccountSwitched(safe), FeedEvent.serializer())
        val raw = withTimeout(20_000) { host.view.first { view -> (view["rows"] as? JsonArray)?.count { it.jsonObject["type"]?.toString() == "\"item\"" } ?: 0 >= 1 } }
        delay(500)
        val settled = host.view.value
        return settled to Wire.json.decodeFromJsonElement(FeedView.serializer(), settled)
    }

    private fun txFixture(s: VelaStrings) = (FlowFixtures.build(FlowState.A2, s).sheet as FlowSheet.TxDetail).model

    private fun describe(tag: String, feed: FeedView) {
        val s = strings(tag)
        val groups = WalletLive.activity(feed, s)
        for (row in groups.flatMap { it.rows }) {
            observe("ACT/$tag/row ${row.id}", "title='${row.title}' subtitle='${row.subtitle}' amount='${row.amount}' unit='${row.unit}' positive=${row.positive}")
            val detail = FlowLive.txDetail(txFixture(s), feed, row.id, s, chainNames = mapOf(100 to "Gnosis"), explorers = mapOf(100 to "https://gnosisscan.io"))
            observe(
                "ACT/$tag/detail ${row.id}",
                "title='${detail?.title}' status=${detail?.status} amount='${detail?.amount}' fiat='${detail?.fiat}' " +
                    "facts=${detail?.facts?.map { "${it.label}=${it.value}" }} explorerUrl=${detail?.explorerUrl}",
            )
        }
    }

    // ---------------------------------------------------------------- the rows

    @Test
    fun `ACTIVITY - what 活动 shows for a landed send, a capped approve, a batch with page-written to-value, and a pending send`() = runBlocking<Unit> {
        seedAccount(); scriptLive()

        // R1: Send dust (0.001 xDAI to the founder). R2: a CAPPED approve (1 base unit) to the Gnosis USDC.
        slide(request("d1", "eth_sendTransaction", JSONObject().put("from", safe).put("to", founder).put("value", "0x38d7ea4c68000")))
        slide(request("a1", "eth_sendTransaction", JSONObject().put("from", safe).put("to", usdc).put("value", "0x0").put("data", approveData("0x1111111111111111111111111111111111111111", "0".repeat(63) + "1"))))
        // R3: the REC probe - a wallet_sendCalls whose top-level to/value are page-written and never shown on the sheet.
        val calls = JSONArray().put(JSONObject().put("to", usdc).put("data", approveData("0x1111111111111111111111111111111111111111", "0".repeat(64))).put("value", "0x0"))
        slide(request("b1", "wallet_sendCalls", JSONObject().put("version", "2.0.0").put("from", safe).put("chainId", "0x64").put("to", dead).put("value", "0xffffffffffffffffffff").put("calls", calls)))
        // R4: a send that never lands inside the wait (pending).
        neverLands += opHash(4)
        slide(request("p1", "eth_sendTransaction", JSONObject().put("from", safe).put("to", founder).put("value", "0x38d7ea4c68000")), receiptWaitMs = 600)

        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        for (i in 0 until rows.length()) {
            val r = rows.getJSONObject(i)
            observe("ACT/store", "id=${r.optString("id")} type=${r.optString("type")} status=${r.optString("status")} to=${r.optString("to")} value=${r.optString("value")} symbol=${r.optString("symbol")} txHash=${r.optString("txHash")} keys=${r.keys().asSequence().toList()}")
        }

        val (raw, decoded) = feed()
        // Which 083 fields does the core's RAW feed send per item, and does the Kotlin FeedItem keep them?
        val items = raw["rows"]!!.jsonArray.map { it.jsonObject }.filter { it["type"].toString() == "\"item\"" }.map { it["item"]!!.jsonObject }
        for (item in items) {
            observe("ACT/raw-core-item ${item["id"]}", "keys=${item.keys} dapp=${item["dapp"]}")
        }
        val kotlinItemFields = app.getvela.wallet.feature.wallet.core.FeedItem.serializer().descriptor.elementNames.toList()
        observe("ACT/kotlin-FeedItem-fields", "$kotlinItemFields")
        observe("ACT/kotlin-FeedTxRecord-fields", "${app.getvela.wallet.feature.wallet.core.FeedTxRecord.serializer().descriptor.elementNames.toList()}")

        describe("zh", decoded)
        describe("en", decoded)

        // REC #2: does the page-written `to` switch off the first-time-recipient tag?
        val executor = FeedExecutor(store = store, ownAccounts = { emptyList() })
        val sentToDead = executor.hasSentTo(dead)
        observe("REC/hasSentTo", "hasSentTo(0x...dEaD) after ONLY a wallet_sendCalls(to=dEaD, calls->USDC approve) = $sentToDead  (BAD = true: the tag is switched off by a page-authored to)")
        assertTrue("the run produced rows", items.isNotEmpty())
        assertEquals("PREDICTED BAD (REC): a page-written batch 'to' counts as 'sent to'", true, sentToDead)
    }

    /** A-4 and the sign-path / tracker race (hand-off 10): the detail's status comes from tx_hash, and patchRecords is last-write-wins. */
    @Test
    fun `A-4 - the detail chip follows the tx hash, and patchRecords lets the last writer win`() = runBlocking<Unit> {
        fun row(id: String, status: String, txHash: String, ts: Long) = JSONObject().put("id", id).put("userOpHash", opHash(ts.toInt() % 200 + 1)).put("txHash", txHash)
            .put("from", safe).put("to", founder).put("value", "0x38d7ea4c68000").put("symbol", "XDAI").put("decimals", 18).put("chainId", 100)
            .put("timestamp", 1_790_000_000L + ts).put("status", status).put("type", "dapp_tx").put("dappOrigin", origin)
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray()
            .put(row("failed-no-hash", "failed", "", 1))
            .put(row("failed-after-confirmed", "failed", txOf(opHash(2)), 2))
            .put(row("pending-no-hash", "pending", "", 3))
            .toString()
        val executor = FeedExecutor(store = store, ownAccounts = { emptyList() })
        // the race: the tracker closes it failed, then the sign path (or the reverse) closes it confirmed.
        executor.patchRecords(listOf("failed-no-hash"), "confirmed", txOf(opHash(9)))
        val afterConfirmedLast = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0).optString("status")
        executor.patchRecords(listOf("failed-no-hash"), "failed", null)
        val afterFailedLast = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0).let { "${it.optString("status")} txHash=${it.optString("txHash")}" }
        observe("A-4/patchRecords-race", "confirmed-then-failed -> ${afterFailedLast}; (failed-then-confirmed left ${afterConfirmedLast} after the first patch on a fresh copy)")
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray()
            .put(row("failed-no-hash", "failed", "", 1))
            .put(row("failed-after-confirmed", "failed", txOf(opHash(2)), 2))
            .put(row("pending-no-hash", "pending", "", 3))
            .toString()
        val (_, decoded) = feed()
        val s = strings("zh")
        for (id in listOf("failed-no-hash", "failed-after-confirmed", "pending-no-hash")) {
            val detail = FlowLive.txDetail(txFixture(s), decoded, id, s, chainNames = mapOf(100 to "Gnosis"))
            observe("A-4/$id", "store status=${JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).let { a -> (0 until a.length()).map { a.getJSONObject(it) }.first { it.optString("id") == id }.optString("status") }} -> detail chip=${detail?.status?.text} hash=${detail?.facts?.firstOrNull { f -> f.label == s.t("componentsTx.detail.hash") || f.mono }?.value}")
        }
        val chipOf = { id: String -> FlowLive.txDetail(txFixture(s), decoded, id, s)?.status?.text }
        assertEquals("PREDICTED BAD: a FAILED record with no hash reads 处理中", "处理中", chipOf("failed-no-hash"))
        assertEquals("PREDICTED BAD: a FAILED record that kept a tx hash reads 已确认", "已确认", chipOf("failed-after-confirmed"))
    }

    /** The core alone, no signing: a stored dApp row whose `to` is the hand-off's 日本語 probe. Isolated: if the core's shortener panics it can take the worker down. */
    @Test
    fun `REC - a stored dapp_tx whose to is not an address (日本語日本語) does not crash the feed core`() = runBlocking<Unit> {
        val row = JSONObject().put("id", "dapp-1-tx").put("userOpHash", opHash(9)).put("txHash", txOf(opHash(9))).put("from", safe).put("to", "日本語日本語")
            .put("value", "0x0").put("symbol", "XDAI").put("decimals", 18).put("chainId", 100).put("timestamp", 1_790_000_000L).put("status", "confirmed")
            .put("type", "dapp_tx").put("dappOrigin", origin)
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(row).toString()
        val (raw, decoded) = feed()
        observe("REC/日本語", "core survived; rawItems=${raw["rows"]}")
        describe("zh", decoded)
        assertTrue(decoded.rows.any { it is FeedRow.Item })
    }
}
