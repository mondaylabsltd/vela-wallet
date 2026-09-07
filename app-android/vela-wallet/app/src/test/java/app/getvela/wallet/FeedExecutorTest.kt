package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.FeedOperation
import app.getvela.wallet.feature.wallet.core.FeedShellResult
import app.getvela.wallet.feature.wallet.core.FeedTxKind
import app.getvela.wallet.feature.wallet.core.FeedTxStatus
import java.util.Calendar
import java.util.TimeZone
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The activity store, and the one number the core cannot compute.
 *
 * `day_start_ms` is local midnight on THIS device. Every case about it is
 * really a case about somebody in Shanghai or Los Angeles seeing their evening
 * payment filed under the wrong day.
 */
class FeedExecutorTest {

    private val store = FakeStore()

    private fun executor(
        accounts: List<FeedExecutor.FeedOwnAccount> = emptyList(),
        resolve: suspend (String) -> String? = { null },
    ) = FeedExecutor(
        store = store,
        ownAccounts = { accounts },
        resolveName = resolve,
        now = { 1_757_000_000_000.0 },
    )

    private fun storedRow(
        id: String = "tx-1",
        timestamp: Double = 1_756_900_000.0,
        type: String? = "receive",
        status: String = "confirmed",
    ) = JSONObject()
        .put("id", id)
        .put("txHash", "0xabc")
        .put("from", "0x9F3c000000000000000000000000000000000000")
        .put("to", "0x1111111111111111111111111111111111111111")
        .put("value", "50")
        .put("symbol", "USDC")
        .put("decimals", 6)
        .put("chainId", 137)
        .put("timestamp", timestamp)
        .put("status", status)
        .apply { if (type != null) put("type", type) }

    private suspend fun write(vararg rows: JSONObject) {
        val array = JSONArray()
        rows.forEach(array::put)
        store.write(KeyValueStore.Keys.TRANSACTIONS, array.toString())
    }

    @Test
    fun `a stored record crosses with its fields intact`() = runBlocking {
        write(storedRow())

        val result = executor().perform(FeedOperation.ReadTxStore("0x1111", 7))

        val loaded = result as FeedShellResult.StoreLoaded
        assertEquals("the read id must be echoed, or a celebration is stranded", 7, loaded.read_id)
        val record = loaded.records.single()
        assertEquals("tx-1", record.id)
        assertEquals("50", record.value)
        assertEquals(6, record.decimals)
        assertEquals(137, record.chain_id)
        assertEquals(FeedTxStatus.Confirmed, record.status)
        assertEquals(FeedTxKind.Receive, record.kind)
    }

    /**
     * The day boundary, computed with the device's calendar.
     *
     * `timestamp - timestamp % 86400` gives UTC midnight, which is a different
     * day from local midnight for most of the planet for part of every day.
     */
    @Test
    fun `a day starts at local midnight, not at UTC midnight`() = runBlocking {
        val zone = TimeZone.getTimeZone("Asia/Shanghai")
        val previous = TimeZone.getDefault()
        TimeZone.setDefault(zone)
        try {
            // 2026-09-07 21:30 in Shanghai — still 13:30 UTC, so a UTC-midnight
            // calculation lands on the same date here, but the MIDNIGHT it
            // reports is eight hours late.
            val calendar = Calendar.getInstance(zone).apply {
                set(2026, Calendar.SEPTEMBER, 7, 21, 30, 0)
                set(Calendar.MILLISECOND, 0)
            }
            val seconds = calendar.timeInMillis / 1000.0

            write(storedRow(timestamp = seconds))
            val loaded = executor().perform(FeedOperation.ReadTxStore("0x1111", 1))
                    as FeedShellResult.StoreLoaded

            val expected = Calendar.getInstance(zone).apply {
                set(2026, Calendar.SEPTEMBER, 7, 0, 0, 0)
                set(Calendar.MILLISECOND, 0)
            }.timeInMillis
            assertEquals(expected.toDouble(), loaded.records.single().day_start_ms, 0.0)
        } finally {
            TimeZone.setDefault(previous)
        }
    }

    @Test
    fun `a legacy record with no type is left for the core to read as a send`() = runBlocking {
        write(storedRow(type = null))

        val loaded = executor().perform(FeedOperation.ReadTxStore("0x1111", 1))
                as FeedShellResult.StoreLoaded

        // NOT defaulted here: the core's rule is `kind ?? send`, and a shell
        // that decided would be a second place that rule lives.
        assertNull(loaded.records.single().kind)
    }

    @Test
    fun `a timestamp written in scientific notation still parses`() = runBlocking {
        // org.json renders a large double as `1.7569E9`, and a record written
        // by another client may hold it as a string. 040 shipped this bug once.
        val row = storedRow().put("timestamp", "1.7569E9")
        write(row)

        val loaded = executor().perform(FeedOperation.ReadTxStore("0x1111", 1))
                as FeedShellResult.StoreLoaded

        assertEquals(1_756_900_000.0, loaded.records.single().timestamp, 1.0)
    }

    @Test
    fun `one corrupt row does not hide a whole history`() = runBlocking {
        write(JSONObject().put("id", ""), storedRow(id = "good"))

        val loaded = executor().perform(FeedOperation.ReadTxStore("0x1111", 1))
                as FeedShellResult.StoreLoaded

        assertEquals(listOf("good"), loaded.records.map { it.id })
    }

    @Test
    fun `an unreadable store is an empty store, not a missing answer`() = runBlocking {
        store.write(KeyValueStore.Keys.TRANSACTIONS, "{not json")

        val loaded = executor().perform(FeedOperation.ReadTxStore("0x1111", 3))
                as FeedShellResult.StoreLoaded

        assertEquals(emptyList<Any>(), loaded.records)
        assertEquals(3, loaded.read_id)
    }

    @Test
    fun `a delete that did not happen reports a failure`() {
        // The core lifts its tombstone on `delete_failed` and puts the row
        // back. Reporting a phantom success would lose it from the screen while
        // it still sits in storage — and it would come back on the next launch.
        val answer = executor().neutralAnswer(FeedOperation.DeleteTxRecord("tx-1"))

        assertTrue(answer is FeedShellResult.DeleteFailed)
    }

    @Test
    fun `deleting removes exactly one row`() = runBlocking {
        write(storedRow(id = "a"), storedRow(id = "b"))

        executor().perform(FeedOperation.DeleteTxRecord("a"))

        val loaded = executor().perform(FeedOperation.ReadTxStore("0x1111", 1))
                as FeedShellResult.StoreLoaded
        assertEquals(listOf("b"), loaded.records.map { it.id })
    }

    @Test
    fun `the person's own account is named without a network call`() = runBlocking {
        var asked = false
        val subject = executor(
            accounts = listOf(FeedExecutor.FeedOwnAccount("0xAAAA", "Savings")),
            resolve = { asked = true; "should not be used" },
        )

        val answer = subject.perform(FeedOperation.ResolveRecipientIdentity("0xaaaa"))

        assertEquals("Savings", (answer as FeedShellResult.AliasResolved).name)
        assertTrue("a local name must not cost a lookup", !asked)
    }

    @Test
    fun `an unknown address resolves to nothing, never to an invented name`() = runBlocking {
        val answer = executor().perform(FeedOperation.ResolveRecipientIdentity("0xbbbb"))

        assertNull((answer as FeedShellResult.AliasResolved).name)
    }

    // -- the merge, which the celebration is built on ------------------------

    @Test
    fun `a re-scan finding the same receipt reports nothing new`() = runBlocking {
        // Every ten seconds, the same receipt is discovered again. If this
        // answered "1" each time, the phone would buzz and glow forever for
        // money that arrived yesterday.
        val subject = executor()
        val incoming = listOf(storedRow(id = "137-0xabc-2"))

        assertEquals(1, subject.mergeRecords(incoming))
        assertEquals(0, subject.mergeRecords(incoming))
        assertEquals(0, subject.mergeRecords(incoming))
    }

    @Test
    fun `merged records are newest first and capped`() = runBlocking {
        val subject = executor()
        val rows = (1..250).map { index ->
            storedRow(id = "tx-$index", timestamp = 1_700_000_000.0 + index)
        }

        assertEquals(250, subject.mergeRecords(rows))

        val loaded = subject.perform(FeedOperation.ReadTxStore("0x1111", 1))
                as FeedShellResult.StoreLoaded
        assertEquals(200, loaded.records.size)
        assertEquals("tx-250", loaded.records.first().id)
    }
}
