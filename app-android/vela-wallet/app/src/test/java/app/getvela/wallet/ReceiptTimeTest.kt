package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.wallet.core.FeedEvent
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.FeedOperation
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedShellResult
import app.getvela.wallet.feature.wallet.core.FeedTxKind
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcPostResult
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.feature.wallet.core.TrustExecutor
import app.getvela.wallet.feature.wallet.core.TrustOperation
import app.getvela.wallet.feature.wallet.core.TrustShellResult
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
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
import uniffi.vela_core_uniffi.ActivityFeedCore

/**
 * PR 3, fix A on Android — **a receipt's time is its block's time, never
 * "now"**.
 *
 * On the owner's iPhone three receipts of 2026-09-29 stood under "Today" on
 * 2026-10-10: the scan could not read their block's time, the core stamped
 * them with the clock the shell handed it beside the answer, and the shell
 * stored that for good. The core no longer takes a clock, withholds a
 * transfer whose block it has not read, and repairs the records stored
 * before: it asks this shell for one record's block time
 * (`read_receive_time`) and to rewrite that one record (`write_receive_time`).
 *
 * What is pinned here is this shell's half: the trust executor's answer
 * carries no clock; the feed executor reads a block's time through the pool
 * and answers `null` — never a clock — when it cannot; the store rewrite
 * touches one record's two fields; and, on the REAL `activity_feed`, an old
 * record stamped "today" ends under its own day.
 */
class ReceiptTimeTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEverything() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private val wallet = "0x1111111111111111111111111111111111111111"
    private val sender = "0x9f3c000000000000000000000000000000021ae0"

    /** 2026-09-29 12:00:00 UTC — the day the money really arrived. */
    private val blockTime = 1_790_683_200L
    private val blockHex = "0x" + blockTime.toString(16)

    /** The device's clock in these tests: 2026-10-10 12:00:00 UTC. No answer may ever be it. */
    private val clockMs = 1_791_633_600_000.0

    private class OneEndpoint : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    /** One call the pool was asked to make: the method and its params, as sent. */
    private data class Asked(val method: String, val params: List<Any?>)

    private class Chain(val pool: RpcPool, val asked: List<Asked>)

    /** A pool over one endpoint that answers as [answer] says — and remembers what it was asked. */
    private fun chain(store: FakeStore = FakeStore(), answer: (method: String, params: List<Any?>) -> RpcPostResult): Chain {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val asked = CopyOnWriteArrayList<Asked>()
        val transport = FakeRpcTransport { _, method, params ->
            asked += Asked(method, params)
            answer(method, params)
        }
        val pool = RpcPool(store = store, endpoints = OneEndpoint(), scope = scope, transport = transport)
        runBlocking { pool.start() }
        return Chain(pool, asked)
    }

    private fun receipt(block: String? = "0x2a4f1c0") = JSONObject()
        .put("transactionHash", "0xfeed01")
        .put("status", "0x1")
        .apply { if (block != null) put("blockNumber", block) }

    private fun header(timestamp: String? = blockHex) = JSONObject()
        .put("number", "0x2a4f1c0")
        .apply { if (timestamp != null) put("timestamp", timestamp) }

    private fun feed(store: FakeStore, pool: RpcPool?) = FeedExecutor(
        store = store,
        ownAccounts = { emptyList() },
        now = { clockMs },
        pool = pool,
    )

    private fun receive(id: String, timestamp: Any, txHash: String = "0xfeed01") = JSONObject()
        .put("id", id)
        .put("userOpHash", "")
        .put("txHash", txHash)
        .put("from", sender)
        .put("to", wallet)
        .put("value", "0.001")
        .put("symbol", "xDAI")
        .put("decimals", 18)
        .put("chainId", 100)
        .put("timestamp", timestamp)
        .put("status", "confirmed")
        .put("type", "receive")

    private suspend fun rows(store: FakeStore): List<JSONObject> {
        val array = JSONArray(store.read(KeyValueStore.Keys.TRANSACTIONS))
        return (0 until array.length()).map { array.getJSONObject(it) }
    }

    /** Every field of a stored row, as the bytes it is stored in. */
    private fun fields(row: JSONObject): Map<String, String> =
        row.keys().asSequence().associateWith { key -> JSONObject().put("v", row.get(key)).toString() }

    // -- (d) the trust executor: no clock beside a block's time ----------------------

    @Test
    fun `the trust executor's block time carries no clock, read or unread`() = runBlocking {
        val readable = chain { method, _ -> if (method == "eth_getBlockByNumber") FakeRpcTransport.body(header()) else FakeRpcTransport.body("0x") }
        val operation = TrustOperation.RpcGetBlockByNumber(address = wallet, chain_id = 100, block = "0x2a4f1c0")

        val read = TrustExecutor(readable.pool, FakeStore()).perform(operation)
        assertEquals(TrustShellResult.BlockTimestamp(wallet, 100, 0x2a4f1c0.toDouble(), blockTime.toDouble()), read)
        val wire = JSONObject(Wire.json.encodeToString(TrustShellResult.serializer(), read))
        assertEquals(setOf("type", "address", "chain_id", "block_number", "timestamp_sec"), wire.keys().asSequence().toSet())
        assertEquals("the header alone: its time is all that is read", listOf<Any?>("0x2a4f1c0", false), readable.asked.single { it.method == "eth_getBlockByNumber" }.params)

        // A header nobody could read: `null`, and still no clock for the core to fall back on.
        val silent = chain { _, _ -> FakeRpcTransport.body(JSONObject.NULL) }
        val trust = TrustExecutor(silent.pool, FakeStore())
        for (unread in listOf(trust.perform(operation), trust.neutralAnswer(operation))) {
            assertEquals(TrustShellResult.BlockTimestamp(wallet, 100, 0x2a4f1c0.toDouble(), null), unread)
            val unreadWire = JSONObject(Wire.json.encodeToString(TrustShellResult.serializer(), unread))
            assertFalse("no clock crosses: $unreadWire", unreadWire.has("now_ms"))
            assertTrue("unread is absent or null, never a number: $unreadWire", !unreadWire.has("timestamp_sec") || unreadWire.isNull("timestamp_sec"))
        }
    }

    // -- (b) read_receive_time, over a stubbed pool -----------------------------------

    @Test
    fun `a receipt's block time is read through the pool — receipt, then its block`() = runBlocking {
        val chain = chain { method, _ ->
            when (method) {
                "eth_getTransactionReceipt" -> FakeRpcTransport.body(receipt())
                "eth_getBlockByNumber" -> FakeRpcTransport.body(header())
                else -> FakeRpcTransport.body("0x")
            }
        }
        val answer = feed(FakeStore(), chain.pool).perform(FeedOperation.ReadReceiveTime(id = "rx-1", chain_id = 100, tx_hash = "0xfeed01"))

        assertEquals(FeedShellResult.ReceiveTimeRead(id = "rx-1", timestamp_sec = blockTime.toDouble()), answer)
        // Two reads, in order, and nothing else: the transaction's receipt
        // for its block, that block's header (never its transactions).
        assertEquals(
            listOf(
                Asked("eth_getTransactionReceipt", listOf("0xfeed01")),
                Asked("eth_getBlockByNumber", listOf("0x2a4f1c0", false)),
            ),
            chain.asked.toList(),
        )
        val wire = JSONObject(Wire.json.encodeToString(FeedShellResult.serializer(), answer))
        assertEquals(setOf("type", "id", "timestamp_sec"), wire.keys().asSequence().toSet())
    }

    @Test
    fun `a block time that cannot be read is answered as not read, never as the clock`() = runBlocking {
        val operation = FeedOperation.ReadReceiveTime(id = "rx-1", chain_id = 100, tx_hash = "0xfeed01")
        val notRead = FeedShellResult.ReceiveTimeRead(id = "rx-1", timestamp_sec = null)
        fun body(method: String, receipt: RpcPostResult, block: RpcPostResult) = when (method) {
            "eth_getTransactionReceipt" -> receipt
            "eth_getBlockByNumber" -> block
            else -> FakeRpcTransport.body("0x")
        }
        val cases = mapOf<String, (String) -> RpcPostResult>(
            "the endpoint does not know the transaction" to { m -> body(m, FakeRpcTransport.body(JSONObject.NULL), FakeRpcTransport.body(header())) },
            "the receipt read fails" to { m -> body(m, FakeRpcTransport.network(), FakeRpcTransport.body(header())) },
            "the receipt names no block" to { m -> body(m, FakeRpcTransport.body(receipt(block = null)), FakeRpcTransport.body(header())) },
            "the receipt's block does not read" to { m -> body(m, FakeRpcTransport.body(receipt(block = "latest")), FakeRpcTransport.body(header())) },
            "the block read fails" to { m -> body(m, FakeRpcTransport.body(receipt()), FakeRpcTransport.network()) },
            "the endpoint does not have the block" to { m -> body(m, FakeRpcTransport.body(receipt()), FakeRpcTransport.body(JSONObject.NULL)) },
            "the header has no time" to { m -> body(m, FakeRpcTransport.body(receipt()), FakeRpcTransport.body(header(timestamp = null))) },
            "the header's time does not read" to { m -> body(m, FakeRpcTransport.body(receipt()), FakeRpcTransport.body(header(timestamp = "0xnope"))) },
        )
        for ((why, answer) in cases) {
            val chain = chain { method, _ -> answer(method) }
            val executor = feed(FakeStore(), chain.pool)
            assertEquals(why, notRead, executor.perform(operation))
            // One attempt: when to ask again is the core's.
            assertTrue("$why: asked ${chain.asked}", chain.asked.count { it.method == "eth_getTransactionReceipt" } >= 1)
            assertTrue("$why: a block read without a block", chain.asked.none { it.method == "eth_getBlockByNumber" && it.params.firstOrNull() !is String })
        }
        // A host with no pool at all, and an executor that threw.
        val alone = feed(FakeStore(), pool = null)
        assertEquals(notRead, alone.perform(operation))
        assertEquals(notRead, alone.neutralAnswer(operation))
        // `null` is absent on the wire — nothing a reader could take for a time, and never this device's clock.
        val wire = Wire.json.encodeToString(FeedShellResult.serializer(), notRead)
        assertEquals("""{"type":"receive_time_read","id":"rx-1"}""", wire)
        assertFalse(wire.contains((clockMs / 1000).toLong().toString()))
    }

    // -- (a) write_receive_time: one record, two fields ------------------------------

    @Test
    fun `the rewrite gives one record its block's time and the mark, and moves nothing else`() = runBlocking {
        val store = FakeStore()
        val wrong = (clockMs / 1000) // stamped with the clock: "today"
        val send = JSONObject().put("id", "send-1").put("userOpHash", "0xop").put("txHash", "0xaaa").put("from", wallet).put("to", sender)
            .put("toName", JSONObject.NULL).put("value", "5").put("symbol", "USDC").put("decimals", 6).put("chainId", 100)
            .put("timestamp", 1_791_000_000.5).put("type", "send").put("maybeSent", false)
        val old = receive("100-0xfeed01-3", wrong).put("tokenAddress", "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83").put("usd", "0.001")
        val marked = receive("100-0xbeef02-0", 1_790_000_000L, txHash = "0xbeef02").put("timeVerified", true)
        store.write(KeyValueStore.Keys.TRANSACTIONS, JSONArray().put(send).put(old).put(marked).toString())
        val before = rows(store)
        val executor = feed(store, pool = null)

        // Read back: the mark as stored — and no mark where none is stored.
        val unrepaired = (executor.perform(FeedOperation.ReadTxStore(wallet, 1)) as FeedShellResult.StoreLoaded).records.associateBy { it.id }
        assertNull("an old record carries no mark: that is what marks it for repair", unrepaired.getValue("100-0xfeed01-3").time_verified)
        assertEquals(true, unrepaired.getValue("100-0xbeef02-0").time_verified)
        assertNull(unrepaired.getValue("send-1").time_verified)
        val unmarkedWire = JSONObject(Wire.json.encodeToString(app.getvela.wallet.feature.wallet.core.FeedTxRecord.serializer(), unrepaired.getValue("100-0xfeed01-3")))
        assertFalse("absent stays absent: $unmarkedWire", unmarkedWire.has("time_verified"))

        val answer = executor.perform(FeedOperation.WriteReceiveTime(id = "100-0xfeed01-3", timestamp_sec = blockTime.toDouble()))
        assertEquals(FeedShellResult.ReceiveTimeWritten(id = "100-0xfeed01-3", ok = true), answer)

        val after = rows(store)
        assertEquals("the same records, in the same order", before.map { it.getString("id") }, after.map { it.getString("id") })
        // The one record: the block's time and the mark…
        val rewritten = after[1]
        assertEquals(blockTime, rewritten.getLong("timestamp"))
        assertEquals(true, rewritten.getBoolean("timeVerified"))
        // …and every other field of it byte for byte.
        assertEquals(fields(before[1]) - "timestamp", fields(rewritten) - "timestamp" - "timeVerified")
        // The other records: untouched, every field.
        assertEquals(fields(before[0]), fields(after[0]))
        assertEquals(fields(before[2]), fields(after[2]))

        // The next read: the block's time, its own day, and the mark.
        val repaired = (executor.perform(FeedOperation.ReadTxStore(wallet, 2)) as FeedShellResult.StoreLoaded).records.associateBy { it.id }
        val record = repaired.getValue("100-0xfeed01-3")
        assertEquals(blockTime.toDouble(), record.timestamp, 0.0)
        assertEquals(true, record.time_verified)
        assertEquals(FeedTxKind.Receive, record.kind)
        assertTrue("filed under its own day, not today's", record.day_start_ms < unrepaired.getValue("100-0xfeed01-3").day_start_ms)
        assertEquals(unrepaired.getValue("send-1"), repaired.getValue("send-1"))
        assertEquals(unrepaired.getValue("100-0xbeef02-0"), repaired.getValue("100-0xbeef02-0"))
    }

    @Test
    fun `a rewrite for a record nobody stored, or one the store refuses, answers not ok and writes nothing`() = runBlocking {
        val store = FakeStore()
        store.write(KeyValueStore.Keys.TRANSACTIONS, JSONArray().put(receive("100-0xfeed01-3", clockMs / 1000)).toString())
        val bytes = store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)
        val executor = feed(store, pool = null)

        val unknown = FeedOperation.WriteReceiveTime(id = "100-0xnobody-0", timestamp_sec = blockTime.toDouble())
        assertEquals(FeedShellResult.ReceiveTimeWritten("100-0xnobody-0", ok = false), executor.perform(unknown))
        assertEquals("nothing was written", bytes, store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))

        store.refuseWrites = true
        val refused = FeedOperation.WriteReceiveTime(id = "100-0xfeed01-3", timestamp_sec = blockTime.toDouble())
        assertEquals(FeedShellResult.ReceiveTimeWritten("100-0xfeed01-3", ok = false), executor.perform(refused))
        assertEquals(bytes, store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        assertEquals(FeedShellResult.ReceiveTimeWritten("100-0xfeed01-3", ok = false), executor.neutralAnswer(refused))

        // An empty store, and a time that is no time.
        store.refuseWrites = false
        assertEquals(FeedShellResult.ReceiveTimeWritten("100-0xfeed01-3", ok = false), executor.perform(refused.copy(timestamp_sec = Double.NaN)))
        assertEquals(bytes, store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        assertEquals(FeedShellResult.ReceiveTimeWritten("x", ok = false), feed(FakeStore(), pool = null).perform(FeedOperation.WriteReceiveTime("x", 1.0)))
    }

    // -- the round trip, on the real feed ---------------------------------------------

    /**
     * The owner's three receipts, on the REAL `activity_feed`: stored under
     * the clock's "today" with no mark, read through this executor and a pool
     * that knows their block. The core asks for each one's block time, this
     * shell reads it and rewrites the one record, and the feed — read again by
     * the core itself — files them under the day the money arrived. A record
     * the chain no longer answers for keeps the time it has: nothing is
     * guessed.
     */
    @Test
    fun `receipts stamped today end under the day their block was mined`() = runBlocking<Unit> {
        val store = FakeStore()
        val today = (clockMs / 1000).toLong()
        val stale = (1..3).map { n -> receive("100-0xfeed0$n-0", today - n, txHash = "0xfeed0$n") }
        // A fourth the endpoint does not know: it must stay exactly as it is.
        val unknown = receive("100-0xgone-0", today - 9, txHash = "0xgone")
        store.write(KeyValueStore.Keys.TRANSACTIONS, JSONArray().apply { (stale + unknown).forEach(::put) }.toString())
        val chain = chain(store) { method, params ->
            when (method) {
                "eth_getTransactionReceipt" ->
                    if (params.firstOrNull() == "0xgone") FakeRpcTransport.body(JSONObject.NULL) else FakeRpcTransport.body(receipt())
                "eth_getBlockByNumber" -> FakeRpcTransport.body(header())
                else -> FakeRpcTransport.body("0x")
            }
        }
        val executor = feed(store, chain.pool)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val host = CoreHost(
            bridge = ActivityFeedCore().asBridge(),
            scope = scope,
            initial = FeedView(),
            serializer = FeedView.serializer(),
            perform = JsonShell.perform(FeedOperation.serializer(), FeedShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(FeedOperation.serializer(), FeedShellResult.serializer(), fallback = FeedShellResult.HapticPlayed, answer = executor::neutralAnswer),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
        host.dispatch(FeedEvent.AccountSwitched(wallet), FeedEvent.serializer())

        // The feed, once the repair's own re-read is in: three rows at the block's time.
        val view = withTimeout(20_000) {
            host.view.first { view -> view.rows.filterIsInstance<FeedRow.Item>().count { it.item.timestamp == blockTime.toDouble() } == 3 }
        }
        val items = view.rows.filterIsInstance<FeedRow.Item>().map { it.item }.associateBy { it.id }
        assertEquals("the one nobody could check keeps the time it has", (today - 9).toDouble(), items.getValue("100-0xgone-0").timestamp, 0.0)
        // Two days now, the block's and the clock's — the three no longer under "today".
        val days = view.rows.filterIsInstance<FeedRow.Header>().map { it.day_start_ms }
        assertEquals(2, days.distinct().size)
        assertEquals(items.getValue("100-0xfeed01-0").day_start_ms, days.minOrNull())

        // The store: the three rewritten and marked, the fourth untouched and unmarked.
        val stored = rows(store).associateBy { it.getString("id") }
        for (n in 1..3) {
            val row = stored.getValue("100-0xfeed0$n-0")
            assertEquals(blockTime, row.getLong("timestamp"))
            assertEquals(true, row.getBoolean("timeVerified"))
            assertEquals("0.001", row.getString("value"))
        }
        assertEquals(fields(unknown), fields(stored.getValue("100-0xgone-0")))
        // Each asked about once — a receipt read and a block read — and never again once rewritten.
        for (n in 1..3) {
            assertEquals("0xfeed0$n", 1, chain.asked.count { it == Asked("eth_getTransactionReceipt", listOf("0xfeed0$n")) })
        }
        assertEquals(3, chain.asked.count { it.method == "eth_getBlockByNumber" })

        // The next rounds (a tick each) reach the fourth: its transaction is
        // not known to the endpoint, so there is no block to read, nothing is
        // written, and it keeps the time it has.
        withTimeout(20_000) {
            while (chain.asked.none { it == Asked("eth_getTransactionReceipt", listOf("0xgone")) }) {
                host.dispatch(FeedEvent.FocusTick, FeedEvent.serializer())
                kotlinx.coroutines.delay(50)
            }
        }
        assertEquals(3, chain.asked.count { it.method == "eth_getBlockByNumber" })
        assertEquals(fields(unknown), fields(rows(store).single { it.getString("id") == "100-0xgone-0" }))
        for (n in 1..3) {
            assertEquals("never twice for one record", 1, chain.asked.count { it == Asked("eth_getTransactionReceipt", listOf("0xfeed0$n")) })
        }
        assertTrue("never a clock: no stored time is the device's", rows(store).none { it.getLong("timestamp") == (clockMs / 1000).toLong() })
    }
}
