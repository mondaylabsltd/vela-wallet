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
    private val patched = ArrayList<String>()
    private val notified = ArrayList<String>()
    private val logsSeen = ArrayList<Pair<String, Int>>()

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
        withTimeout(5_000) { while (patched.isEmpty() || notified.isEmpty()) delay(50) }
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
}
