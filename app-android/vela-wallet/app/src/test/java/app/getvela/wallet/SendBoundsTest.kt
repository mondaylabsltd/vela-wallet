package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.send.core.ChainRecovery
import app.getvela.wallet.feature.send.core.PrepareBound
import app.getvela.wallet.feature.send.core.TrackEvent
import app.getvela.wallet.feature.send.core.TrackHandoff
import app.getvela.wallet.feature.send.core.submitFacts
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import uniffi.vela_core_uniffi.TxTrackerCore

/**
 * The shell's two clocks of send rounds 3–6 — the bounded wait before the
 * passkey (`submit_slow`, the web's `prepare-bound.ts`) and the network asked
 * again after it failed (`chain_answered`, `chain-recovery.ts`) — with their
 * times shortened, and the lost reply's nonce handed to the REAL tracker
 * (`submit_facts`).
 */
class SendBoundsTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun stop() = scope.cancel()

    private suspend fun <T> bounded(
        slow: CopyOnWriteArrayList<Pair<Long, Boolean>>,
        busy: Boolean = false,
        passThrough: (Throwable) -> Boolean = { false },
        work: suspend (() -> Unit) -> T,
    ): T = PrepareBound.run(
        busy = { busy },
        onSlow = { elapsed, b -> slow += elapsed to b },
        passThrough = passThrough,
        slowMs = 50,
        limitMs = 400,
        tickMs = 20,
        work = work,
    )

    @Test
    fun `past the slow mark the wait is said every tick, and the gate ends it`() = runBlocking {
        val slow = CopyOnWriteArrayList<Pair<Long, Boolean>>()
        val result = bounded(slow, busy = true) { gate ->
            delay(130)
            gate()
            val said = slow.size
            delay(100)
            assertEquals("nothing is said once the passkey opened", said, slow.size)
            "signed"
        }
        assertEquals("signed", result)
        assertTrue("said while slow (${slow.size})", slow.size >= 2)
        assertEquals(50L, slow.first().first)
        assertTrue("busy as the pool says", slow.all { it.second })
        assertTrue("the time grows", slow.zipWithNext().all { (a, b) -> b.first > a.first })
    }

    @Test
    fun `a fast submit says nothing`() = runBlocking {
        val slow = CopyOnWriteArrayList<Pair<Long, Boolean>>()
        bounded(slow) { gate -> gate(); delay(120); 1 }
        assertTrue(slow.isEmpty())
    }

    @Test
    fun `past the limit it gives up as the network's, and the passkey never opens after`() = runBlocking {
        val slow = CopyOnWriteArrayList<Pair<Long, Boolean>>()
        val opened = AtomicInteger(0)
        try {
            bounded(slow) { gate ->
                try {
                    awaitCancellation()
                } finally {
                    // A late gate after the give-up refuses.
                    runCatching { gate(); opened.incrementAndGet() }
                }
            }
            fail("given up")
        } catch (before: PrepareBound.BeforePasskey) {
            assertTrue(before.timedOut)
            assertFalse(before.busy)
        }
        assertEquals("no passkey after the wait was given up", 0, opened.get())
    }

    @Test
    fun `a failure before the gate is the bound's to judge, after it is the passkey's own`() = runBlocking {
        val slow = CopyOnWriteArrayList<Pair<Long, Boolean>>()
        val before = runCatching { bounded<Unit>(slow) { throw IllegalStateException("nonce unread") } }.exceptionOrNull()
        assertTrue(before is PrepareBound.BeforePasskey)
        assertFalse((before as PrepareBound.BeforePasskey).timedOut)
        assertEquals("nonce unread", before.cause?.message)

        val kept = runCatching { bounded<Unit>(slow, passThrough = { it is UnsupportedOperationException }) { throw UnsupportedOperationException("venue") } }.exceptionOrNull()
        assertTrue("a refusal that keeps its meaning", kept is UnsupportedOperationException)

        val after = runCatching { bounded<Unit>(slow) { gate -> gate(); throw IllegalArgumentException("passkey failed") } }.exceptionOrNull()
        assertTrue("after the gate, as thrown", after is IllegalArgumentException)
    }

    @Test
    fun `a cancel from outside is passed on, never told as the network's`() = runBlocking {
        val slow = CopyOnWriteArrayList<Pair<Long, Boolean>>()
        val job = scope.async { bounded(slow) { awaitCancellation() } }
        delay(60)
        job.cancel()
        val error = runCatching { job.await() }.exceptionOrNull()
        assertTrue("cancelled ($error)", error is kotlinx.coroutines.CancellationException && error !is PrepareBound.BeforePasskey)
    }

    // -- chain recovery -----------------------------------------------------------------

    @Test
    fun `the failed network is asked until it answers, then told once`() = runBlocking {
        val answered = CompletableDeferred<Int>()
        val reads = AtomicInteger(0)
        val recovery = ChainRecovery(
            scope = scope,
            read = { chain -> assertEquals(100, chain); reads.incrementAndGet() >= 3 },
            answered = { answered.complete(it) },
            everyMs = 20,
            readMs = 50,
        )
        recovery.follow(100)
        assertEquals(100, withTimeout(2_000) { answered.await() })
        assertEquals(3, reads.get())
        delay(80)
        assertEquals("nothing is asked once it answered", 3, reads.get())
    }

    @Test
    fun `a read that hangs counts as no answer, and stopping asks nothing more`() = runBlocking {
        val reads = AtomicInteger(0)
        val told = AtomicInteger(0)
        val recovery = ChainRecovery(
            scope = scope,
            read = { reads.incrementAndGet(); awaitCancellation() },
            answered = { told.incrementAndGet() },
            everyMs = 20,
            readMs = 30,
        )
        recovery.follow(100)
        delay(170)
        assertTrue("asked again after a hung read (${reads.get()})", reads.get() >= 2)
        recovery.follow(null)
        val stopped = reads.get()
        delay(120)
        assertEquals(stopped, reads.get())
        assertEquals(0, told.get())
    }

    @Test
    fun `back online asks at once, not at the next turn`() = runBlocking {
        val online = MutableStateFlow(false)
        val answered = CompletableDeferred<Int>()
        val recovery = ChainRecovery(
            scope = scope,
            read = { online.value },
            backOnline = online,
            answered = { answered.complete(it) },
            everyMs = 10_000,
            readMs = 50,
        )
        recovery.follow(7)
        delay(50)
        assertFalse(answered.isCompleted)
        online.value = true
        assertEquals(7, withTimeout(2_000) { answered.await() })
    }

    // -- submit facts --------------------------------------------------------------------

    @Test
    fun `a lost reply's nonce goes to the tracker after the hand-off, and nothing else does`() {
        val lost = TrackHandoff(userOpHash = "0x" + "aa".repeat(32), recordIds = listOf("r1"), chainId = 100, maybeSent = true, sender = SAFE, nonce = "0x7")
        assertEquals(TrackEvent.SubmitFacts(user_op_hash = lost.userOpHash, nonce = "0x7"), lost.submitFacts())
        assertNull("accepted: nothing in doubt", lost.copy(maybeSent = false).submitFacts())
        assertNull("no nonce: nothing to read", lost.copy(nonce = null).submitFacts())
        assertNull("no records", lost.copy(recordIds = emptyList()).submitFacts())

        // The real tracker takes it: with a nonce and a sender it reads the account's nonce while in doubt.
        val script = CoreScript(TxTrackerCore().asBridge()) { operation ->
            when (operation.optString("type")) {
                "now", "clock" -> """{"type":"clock","now_ms":1757000000000}"""
                else -> null
            }
        }
        fun send(event: TrackEvent) = script.dispatch(Wire.json.encodeToString(TrackEvent.serializer(), event))
        send(lost.event())
        send(lost.submitFacts()!!)
        send(TrackEvent.Tick)
        assertTrue("the tracker reads the nonce (waiting: ${script.waiting()})", "read_nonce" in script.waiting())
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    }
}
