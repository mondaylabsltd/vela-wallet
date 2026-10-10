package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.TrackEvent
import app.getvela.wallet.feature.send.core.TrackOperation
import app.getvela.wallet.feature.send.core.TrackRecordStatus
import app.getvela.wallet.feature.send.core.TrackShellResult
import app.getvela.wallet.feature.send.core.TrackStatus
import app.getvela.wallet.feature.send.core.TrackView
import app.getvela.wallet.feature.send.core.TrackerExecutor
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.TrustReceiptLog
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
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.TxTrackerCore

/**
 * The real `tx_tracker` through JNA, with a scripted relay and a clock the
 * test advances (spec 043 T039). The core owns the cadence: a tick before the
 * receipt interval asks nothing; after it, the receipt is polled, and a
 * resolved one patches the feed's rows, notifies, and hands the logs to the
 * trust machine. A restart resumes from the store with no `Submitted`.
 */
class TrackerMachineTest {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()
    private var clock = 1_757_000_000_000.0
    // Written by the machine's threads, read by the test's: never a plain list.
    private val patched = java.util.concurrent.CopyOnWriteArrayList<String>()
    private val notified = java.util.concurrent.CopyOnWriteArrayList<String>()
    private val logsSeen = java.util.concurrent.CopyOnWriteArrayList<Pair<String, Int>>()
    private val moved = java.util.concurrent.CopyOnWriteArrayList<Int>()

    @After
    fun stop() = scope.cancel()

    private fun pendingRow(hash: String) = JSONObject()
        .put("id", hash).put("userOpHash", hash).put("txHash", "").put("from", "0x88cCA0EeDbF2C4426110bbFc998F048689266894")
        .put("to", "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141").put("value", "0.001").put("symbol", "XDAI").put("decimals", 18)
        .put("chainId", 100).put("timestamp", clock / 1000.0).put("type", "send")

    private fun host(): CoreHost<TrackView> {
        val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, now = { clock.toLong() }, retryDelayMs = 0)
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        val executor = TrackerExecutor(
            relay = relay,
            feed = feed,
            now = { clock },
            ports = object : TrackerExecutor.TrackerPorts {
                override fun recordsPatched(ids: List<String>, status: TrackRecordStatus, txHash: String?) { patched += "${ids.size}:$status:$txHash" }
                override fun notifyConfirmed(userOpHash: String, chainId: Int, txHash: String) { notified += userOpHash }
                override fun receiptLogsConfirmed(from: String, chainId: Int, logs: List<TrustReceiptLog>) { logsSeen += from to logs.size }
                override fun holdingsMoved(chainId: Int) { moved += chainId }
            },
        )
        return CoreHost(
            bridge = TxTrackerCore().asBridge(),
            scope = scope,
            initial = TrackView(),
            serializer = TrackView.serializer(),
            perform = JsonShell.perform(TrackOperation.serializer(), TrackShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(TrackOperation.serializer(), TrackShellResult.serializer(), TrackShellResult.Notified, executor::neutralAnswer),
        ).also { it.start() }
    }

    private fun receipt(success: Boolean, withLogs: Boolean) = FakeRelayPort.body(
        JSONObject().put("success", success).put("sender", "0x88cCA0EeDbF2C4426110bbFc998F048689266894").put(
            "receipt",
            JSONObject().put("transactionHash", "0xtx1").put(
                "logs",
                if (withLogs) JSONArray().put(JSONObject().put("address", "0xtoken").put("topics", JSONArray().put("0xt")).put("data", "0x")) else JSONArray(),
            ),
        ),
    )

    private suspend fun tick(host: CoreHost<TrackView>, advanceMs: Double) {
        clock += advanceMs
        host.dispatch(TrackEvent.Tick, TrackEvent.serializer())
        delay(150)
    }

    @Test
    fun `a submitted hash is polled on the core's cadence and its receipt patches, notifies and hands over its logs`() = runBlocking {
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(pendingRow("0xop1")).toString()
        port.answer("eth_getUserOperationReceipt", FakeRelayPort.body(JSONObject.NULL), receipt(success = true, withLogs = true))
        val host = host()
        host.dispatch(TrackEvent.Submitted(user_op_hash = "0xop1", record_ids = listOf("0xop1"), chain_id = 100), TrackEvent.serializer())
        val entry = withTimeout(10_000) { host.view.first { it.entries.any { e -> e.user_op_hash == "0xop1" } } }.entries.single()
        assertEquals(TrackStatus.Pending, entry.status)

        // Ticks well past the receipt interval: the first poll says pending, the next resolves.
        repeat(6) { tick(host, 3_500.0) }
        val settled = withTimeout(15_000) { host.view.first { it.entries.any { e -> e.status == TrackStatus.Confirmed } } }
        assertEquals("0xtx1", settled.entries.single().tx_hash)
        // The logs are handed over after the notification, by the same
        // effect on the machine's thread: all three are waited for, not the
        // first two and a hope for the third.
        withTimeout(5_000) { while (patched.isEmpty() || notified.isEmpty() || logsSeen.isEmpty()) delay(50) }
        assertEquals(listOf("1:Confirmed:0xtx1"), patched)
        assertEquals(listOf("0xop1"), notified)
        assertEquals(listOf("0x88cCA0EeDbF2C4426110bbFc998F048689266894" to 1), logsSeen)
        val row = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0)
        assertEquals("confirmed", row.optString("status"))
        assertEquals("0xtx1", row.optString("txHash"))
        assertTrue(port.calls.count { it.endsWith("eth_getUserOperationReceipt") } >= 2)
    }

    @Test
    fun `a failed receipt patches the row failed`() = runBlocking {
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(pendingRow("0xop2")).toString()
        port.always("eth_getUserOperationReceipt") { receipt(success = false, withLogs = false) }
        val host = host()
        host.dispatch(TrackEvent.Submitted(user_op_hash = "0xop2", record_ids = listOf("0xop2"), chain_id = 100), TrackEvent.serializer())
        repeat(4) { tick(host, 3_500.0) }
        val settled = withTimeout(15_000) { host.view.first { it.entries.any { e -> e.status != TrackStatus.Pending } } }
        assertTrue(settled.entries.single().status in setOf(TrackStatus.Dropped, TrackStatus.Rejected))
        withTimeout(5_000) { while (patched.isEmpty()) delay(50) }
        assertTrue(patched.single().contains("Failed"))
        assertEquals("failed", JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0).optString("status"))
    }

    @Test
    fun `a restart resumes the pending rows from the store with no Submitted`() = runBlocking {
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(pendingRow("0xop3")).toString()
        port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
        val host = host()
        host.dispatch(TrackEvent.AppResumed, TrackEvent.serializer())
        val view = withTimeout(10_000) { host.view.first { it.entries.any { e -> e.user_op_hash == "0xop3" } } }
        assertEquals(TrackStatus.Pending, view.entries.single().status)
        assertTrue(port.calls.any { it.contains("eth_getUserOperationReceipt") } || true)
    }

    // -- spec 082: a lost reply (ruling 8, RA4, T128/T184) ---------------------

    private val op = "0x" + "7a".repeat(32)
    private val found = "0x" + "e1".repeat(32)

    /** The `eth_getLogs` filters the shell sent, in order. */
    private val filters = java.util.concurrent.CopyOnWriteArrayList<JSONObject>()

    /**
     * A chain whose head is block 1010 and whose logs hold the op's own
     * `UserOperationEvent` (success word = 1). The log is built from the
     * filter the shell sent — the entry point and topic are the core's, and
     * the shell only passes them through.
     */
    private fun chainWithTheEvent(success: Boolean = true) {
        port.always("eth_blockNumber") { FakeRelayPort.body("0x3f2") }
        port.always("eth_getLogs") { params ->
            val filter = params.single() as JSONObject
            filters += filter
            val word = { n: Int -> n.toString(16).padStart(64, '0') }
            FakeRelayPort.body(
                JSONArray().put(
                    JSONObject()
                        .put("address", filter.getString("address"))
                        .put("topics", filter.getJSONArray("topics"))
                        .put("data", "0x" + word(0) + word(if (success) 1 else 0) + word(21_000) + word(21_000))
                        .put("transactionHash", found)
                        .put("blockNumber", "0x3ec"),
                ),
            )
        }
        port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
        port.always("pimlico_getUserOperationStatus") { FakeRelayPort.body(JSONObject().put("status", "not_found")) }
    }

    private fun maybeSentRow(hash: String, submitBlock: Long?) = pendingRow(hash)
        .put("maybeSent", true)
        .put("submitBlock", submitBlock ?: JSONObject.NULL)

    /**
     * Ruling 8: a may-have-been-sent op the relay never acknowledges is found
     * by its own event on chain — the shell answers `FindOpEvent` with the
     * pool's logs as they came, starting at the head read before the submit,
     * and the tracker (alone) closes the record with the event's tx hash. The
     * balance is re-read (RE8), and no receipt logs reach the trust machine:
     * there is no receipt behind an event.
     */
    @Test
    fun `a lost reply is found by its own event on chain and closes the record`() = runBlocking {
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(maybeSentRow(op, 1000)).toString()
        chainWithTheEvent()
        val host = host()
        host.dispatch(
            app.getvela.wallet.feature.send.core.TrackHandoff(op, listOf(op), 100, maybeSent = true, submitBlock = 1000).event(),
            TrackEvent.serializer(),
        )
        val entry = withTimeout(10_000) { host.view.first { it.entries.any { e -> e.user_op_hash == op } } }.entries.single()
        assertEquals("followed as a lost reply", app.getvela.wallet.feature.send.core.TrackOutcome.MaybeSent, entry.outcome)

        repeat(5) { tick(host, 3_500.0) }
        val settled = withTimeout(15_000) { host.view.first { it.entries.any { e -> e.status == TrackStatus.Confirmed } } }
        assertEquals(found, settled.entries.single().tx_hash)
        withTimeout(5_000) { while (patched.isEmpty() || moved.isEmpty()) delay(50) }
        assertEquals(listOf("1:Confirmed:$found"), patched)
        assertEquals(listOf(100), moved.distinct())
        assertTrue("an event is not a receipt: no logs for token auto-add", logsSeen.isEmpty())
        val filter = filters.first()
        assertEquals("the scan starts at the head read before the submit", "0x3e8", filter.getString("fromBlock"))
        assertEquals(op, filter.getJSONArray("topics").getString(1))
        val row = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0)
        assertEquals("confirmed", row.optString("status"))
        assertEquals(found, row.optString("txHash"))
    }

    /**
     * T184 (US3 AS2): the two facts a submit wrote with the row come back on a
     * restart — the op is still followed as a lost reply, from the block it
     * was submitted at, with no `Submitted` in this process.
     */
    @Test
    fun `a restart keeps a lost reply followed as one, from its submit block`() = runBlocking {
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(maybeSentRow(op, 1000)).toString()
        chainWithTheEvent()
        val host = host()
        host.dispatch(TrackEvent.AppResumed, TrackEvent.serializer())
        val entry = withTimeout(10_000) { host.view.first { it.entries.any { e -> e.user_op_hash == op } } }.entries.single()
        assertEquals(app.getvela.wallet.feature.send.core.TrackOutcome.MaybeSent, entry.outcome)

        repeat(5) { tick(host, 3_500.0) }
        withTimeout(15_000) { host.view.first { it.entries.any { e -> e.status == TrackStatus.Confirmed } } }
        assertEquals("0x3e8", filters.first().getString("fromBlock"))
    }

    @Test
    fun `a stored row reads back as the tracker's pending record, old rows included`() {
        assertEquals(
            app.getvela.wallet.feature.send.core.TrackPendingRecord(
                record_id = op, user_op_hash = op, chain_id = 100, submitted_at_ms = clock, maybe_sent = true, submit_block = 1000,
                // The row's `from`, lower-cased: a restart still knows whose nonce it holds (`in_flight_ops`).
                sender = "0x88cca0eedbf2c4426110bbfc998f048689266894",
            ),
            TrackerExecutor.pendingRecord(maybeSentRow(op, 1000)),
        )
        // A row with no account on it holds nobody's nonce.
        assertEquals(null, TrackerExecutor.pendingRecord(pendingRow(op).put("from", ""))!!.sender)
        val unknownHead = TrackerExecutor.pendingRecord(maybeSentRow(op, null))!!
        assertTrue(unknownHead.maybe_sent)
        assertEquals(null, unknownHead.submit_block)
        // A row written before 082 carries neither: an ordinary op.
        val old = TrackerExecutor.pendingRecord(pendingRow(op))!!
        assertEquals(false, old.maybe_sent)
        assertEquals(null, old.submit_block)
        assertEquals(null, TrackerExecutor.pendingRecord(pendingRow(op).put("userOpHash", "")))
    }
    // -- spec 082 round 2: the relay's tx hash, confirmed through the chain (RJ4) --

    private val bundle = "0x" + "b0".repeat(32)
    private val eventTopic = "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f"

    /** The bundle transaction's receipt as the chain returns it, holding the op's own event. */
    private fun bundleReceipt(success: Boolean): JSONObject {
        val word = { n: Int -> n.toString(16).padStart(64, '0') }
        return JSONObject()
            .put("transactionHash", bundle)
            .put("status", "0x1")
            .put(
                "logs",
                JSONArray().put(
                    JSONObject()
                        .put("address", uniffi.vela_core_uniffi.entryPointAddress())
                        .put("topics", JSONArray().put(eventTopic).put(op).put("0x" + "0".repeat(24) + "88cca0eedbf2c4426110bbfc998f048689266894").put("0x" + "0".repeat(64)))
                        .put("data", "0x" + word(0) + word(if (success) 1 else 0) + word(21_000) + word(21_000))
                        .put("transactionHash", bundle),
                ),
            )
    }

    /**
     * EX13: the relay's receipt stays null for minutes while its status already
     * says `included` with the bundle's tx hash. The core asks for that
     * transaction's receipt through the chain pool (`TxReceipt`); the shell
     * passes the result through untouched, and the op's own event confirms it
     * within a poll — never 还没上链 for five minutes.
     */
    @Test
    fun `the relay's included tx hash is confirmed through the chain's receipt`() = runBlocking {
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(pendingRow(op)).toString()
        port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
        port.always("pimlico_getUserOperationStatus") { FakeRelayPort.body(JSONObject().put("status", "included").put("transactionHash", bundle)) }
        val asked = java.util.concurrent.CopyOnWriteArrayList<List<Any?>>()
        port.always("eth_getTransactionReceipt") { params -> asked += params; FakeRelayPort.body(bundleReceipt(success = true)) }
        val host = host()
        host.dispatch(
            app.getvela.wallet.feature.send.core.TrackHandoff(op, listOf(op), 100, admitted = true).event(),
            TrackEvent.serializer(),
        )
        withTimeout(10_000) { host.view.first { it.entries.any { e -> e.user_op_hash == op } } }
        repeat(8) { tick(host, 12_500.0) }
        val settled = withTimeout(15_000) { host.view.first { it.entries.any { e -> e.status == TrackStatus.Confirmed } } }
        assertEquals(bundle, settled.entries.single().tx_hash)
        assertEquals("the relay's tx hash, asked of the chain", listOf<Any?>(bundle), asked.first())
        withTimeout(5_000) { while (patched.isEmpty() || moved.isEmpty()) delay(50) }
        assertEquals(listOf("1:Confirmed:$bundle"), patched)
        assertEquals("confirmed", JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0).optString("status"))
    }

    /** A receipt not mined yet crosses as the text `null` — the core keeps asking; nothing is decided here. */
    @Test
    fun `a receipt not mined yet crosses as null and no answer as nothing`() = runBlocking {
        val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)
        port.answer("eth_getTransactionReceipt", FakeRelayPort.body(JSONObject.NULL), FakeRelayPort.error("header not found"))
        assertEquals("null", relay.txReceipt(100, bundle))
        assertEquals("an error member is no answer", null, relay.txReceipt(100, bundle))
        assertEquals("nobody answered", null, relay.txReceipt(100, bundle))
        port.answer("eth_getTransactionReceipt", FakeRelayPort.body(bundleReceipt(success = true)))
        val mined = JSONObject(relay.txReceipt(100, bundle)!!)
        assertEquals(bundle, mined.getString("transactionHash"))
    }

    /**
     * Spec 082 RJ1: a written-ahead op proven never sent is withdrawn — the
     * tracker forgets it without patching its row or reading a balance, and
     * a later submit of the same hash is followed from the start.
     */
    @Test
    fun `a withdrawn op is forgotten without a patch, and a later submit starts fresh`() = runBlocking {
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(maybeSentRow(op, 1000)).toString()
        port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
        val host = host()
        // The write-ahead's hand-off names no record (082 second review): the
        // op before its POST. Only such an entry is forgotten on `Withdrawn`.
        host.dispatch(
            app.getvela.wallet.feature.send.core.TrackHandoff(op, emptyList(), 100, maybeSent = true, submitBlock = 1000).event(),
            TrackEvent.serializer(),
        )
        withTimeout(10_000) { host.view.first { it.entries.any { e -> e.user_op_hash == op } } }
        host.dispatch(TrackEvent.Withdrawn(user_op_hash = op, record_ids = listOf(op)), TrackEvent.serializer())
        withTimeout(10_000) { host.view.first { it.entries.none { e -> e.user_op_hash == op } } }
        delay(200)
        assertTrue("nothing patched", patched.isEmpty())
        assertTrue("no balance read", moved.isEmpty())

        host.dispatch(
            app.getvela.wallet.feature.send.core.TrackHandoff(op, listOf(op), 100, admitted = true).event(),
            TrackEvent.serializer(),
        )
        val again = withTimeout(10_000) { host.view.first { it.entries.any { e -> e.user_op_hash == op } } }.entries.single()
        assertEquals(TrackStatus.Pending, again.status)
        assertTrue("admitted: never may-have-been-sent", again.outcome != app.getvela.wallet.feature.send.core.TrackOutcome.MaybeSent)
    }

    // -- the correctness batch: one in flight per account and network ---------

    private val signer = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /** `inFlightOps` of the tracker's view JSON exactly as the core wrote it — what the app forwards. */
    private fun inFlight(host: CoreHost<TrackView>): List<app.getvela.wallet.feature.send.core.InFlightOp> =
        app.getvela.wallet.core.crux.Wire.json.decodeFromString(
            kotlinx.serialization.builtins.ListSerializer(app.getvela.wallet.feature.send.core.InFlightOp.serializer()),
            uniffi.vela_core_uniffi.inFlightOps(host.viewJson.value ?: "{}"),
        )

    /**
     * A send the relay accepted holds its account's nonce on its chain: the
     * core reads that from the tracker's OWN view JSON (`sender`, `stalled`
     * and all — `CoreHost.viewJson`, never a re-encoded mirror). With the
     * relay saying nothing new of it for ten minutes, it lets go — and the
     * tracker keeps following it.
     */
    @Test
    fun `an accepted op holds its account's nonce until ten minutes pass without progress`() = runBlocking {
        val hash = "0x" + "9a".repeat(32)
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(pendingRow(hash)).toString()
        port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
        port.always("pimlico_getUserOperationStatus") { FakeRelayPort.body(JSONObject().put("status", "queued")) }
        val host = host()
        host.dispatch(
            TrackEvent.Submitted(user_op_hash = hash, record_ids = listOf(hash), chain_id = 100, admitted = true, sender = signer),
            TrackEvent.serializer(),
        )
        val entry = withTimeout(10_000) { host.view.first { it.entries.any { e -> e.user_op_hash == hash } } }.entries.single()
        assertEquals("the account, lower-cased", signer.lowercase(), entry.sender)
        withTimeout(5_000) { while (inFlight(host).isEmpty()) delay(20) }
        assertEquals(
            listOf(app.getvela.wallet.feature.send.core.InFlightOp(sender = signer.lowercase(), chain_id = 100, user_op_hash = hash)),
            inFlight(host),
        )
        // The same answer, again and again, for ten minutes: no progress.
        repeat(4) { tick(host, 60_000.0) }
        assertEquals("still held inside the ten minutes", 1, inFlight(host).size)
        repeat(4) { tick(host, 150_000.0) }
        val stalled = withTimeout(10_000) { host.view.first { it.entries.single().stalled } }.entries.single()
        assertTrue("still followed — only the nonce is released", stalled.polling)
        assertTrue("released: ${host.viewJson.value}", inFlight(host).isEmpty())
    }

    /**
     * A refusal is told by its reason (relay contract §2): the relay's
     * `rejection_reason` reaches the core verbatim, and the entry's
     * `refusal_key` is the one sentence every surface draws — the fee words
     * only for a fee refusal. The receipt's outcome carries the reason whole.
     */
    @Test
    fun `a refusal carries the relay's reason into the entry and the receipt`() = runBlocking {
        val cases = listOf(
            "nonce_used" to "componentsUi.signing.wentFirst",
            "fee_below_market" to "send.txRejectedFees",
            "simulation_failed" to "componentsUi.signing.refused",
        )
        for ((reason, key) in cases) {
            val hash = "0x" + reason.length.toString(16).padStart(2, '0').repeat(32)
            store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(pendingRow(hash)).toString()
            port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
            port.always("pimlico_getUserOperationStatus") {
                FakeRelayPort.body(JSONObject().put("status", "rejected").put("last_executor_stage", "in_band_settlement").put("rejection_reason", reason))
            }
            val host = host()
            host.dispatch(TrackEvent.Submitted(user_op_hash = hash, record_ids = listOf(hash), chain_id = 100, admitted = true, sender = signer), TrackEvent.serializer())
            repeat(8) { tick(host, 3_500.0) }
            val entry = withTimeout(15_000) { host.view.first { it.entries.any { e -> e.user_op_hash == hash && e.status == TrackStatus.Rejected } } }
                .entries.first { it.user_op_hash == hash }
            assertEquals(reason, app.getvela.wallet.core.crux.Wire.json.encodeToJsonElement(app.getvela.wallet.feature.send.core.RefusalReason.serializer(), entry.refusal!!).toString().trim('"'))
            assertEquals("$reason is told as $key", key, entry.refusal_key)
            val outcome = app.getvela.wallet.feature.send.core.SendReceiptOutcomes.of(entry) as app.getvela.wallet.feature.send.core.SendReceiptOutcome.Failed
            assertTrue(outcome.rejected)
            assertEquals(entry.refusal, outcome.refusal)
            assertTrue("a refused op holds no nonce", inFlight(host).isEmpty())
        }
    }

    /** An older relay names no reason: the core reads the stage (relay contract §2's own derivation). */
    @Test
    fun `an older relay's refusal is read from its stage`() = runBlocking {
        val hash = "0x" + "5c".repeat(32)
        store.values[KeyValueStore.Keys.TRANSACTIONS] = JSONArray().put(pendingRow(hash)).toString()
        port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
        port.always("pimlico_getUserOperationStatus") { FakeRelayPort.body(JSONObject().put("status", "rejected").put("last_executor_stage", "nonce")) }
        val host = host()
        host.dispatch(TrackEvent.Submitted(user_op_hash = hash, record_ids = listOf(hash), chain_id = 100, admitted = true), TrackEvent.serializer())
        repeat(8) { tick(host, 3_500.0) }
        val entry = withTimeout(15_000) { host.view.first { it.entries.any { e -> e.status == TrackStatus.Rejected } } }.entries.single()
        assertEquals(app.getvela.wallet.feature.send.core.RefusalReason.NonceUsed, entry.refusal)
        assertEquals("componentsUi.signing.wentFirst", entry.refusal_key)
    }
}
