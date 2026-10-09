package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.CableConn
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.HybridCeremony
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.yield
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * Issue #459: putting the phone's code away ends THAT ceremony as the core's
 * quiet cancel — a `PasskeyFailure(Cancelled)`, never a coroutine
 * cancellation (which is never answered, and leaves the core busy) — at the
 * scan, at a channel still opening, and under an exchange already running.
 * And it ends that ceremony only: a retry or recovery's second signature is a
 * new session the old dismissal cannot reach.
 */
class HybridDismissalTest {
    private fun session() = HybridCeremony.Session(ByteArray(32), ByteArray(16))

    private class FakeConn : CableConn {
        override val channel = "test"
        var closed = false
        override fun writeFrame(bytes: ByteArray) = Unit
        override fun readFrame(): ByteArray = ByteArray(0)
        override fun close() {
            closed = true
        }
    }

    private inline fun expectCancelled(block: () -> Unit) {
        try {
            block()
            fail("expected the ceremony to end as a cancel")
        } catch (failure: PasskeyFailure) {
            assertEquals(FailureKind.Cancelled, failure.kind)
        }
    }

    @Test
    fun `a dismissal ends a scan that is still waiting, as the core's quiet cancel`() = runBlocking {
        val session = session()
        val scan = CompletableDeferred<String?>()
        val waiting = async(Dispatchers.Default, start = CoroutineStart.UNDISPATCHED) {
            runCatching { session.unlessDismissed(scan) }
        }
        yield()
        session.dismiss()
        val outcome = withTimeout(5_000) { waiting.await() }
        val failure = outcome.exceptionOrNull()
        assertTrue("a PasskeyFailure, not a coroutine cancellation: $failure", failure is PasskeyFailure)
        assertEquals(FailureKind.Cancelled, (failure as PasskeyFailure).kind)
    }

    @Test
    fun `an answer that came first is the answer`() = runBlocking {
        val session = session()
        assertEquals("phone", session.unlessDismissed(CompletableDeferred("phone")))
        assertFalse(session.dismissed)
    }

    @Test
    fun `a dismissal closes the channel an exchange is blocked on`() {
        val session = session()
        val conn = FakeConn()
        session.hold(conn)
        assertFalse(conn.closed)
        session.dismiss()
        assertTrue("closing is what unblocks the exchange", conn.closed)
        // Twice is once.
        session.dismiss()
    }

    @Test
    fun `a channel that opens after the dismissal is closed and refused`() {
        val session = session()
        session.dismiss()
        val late = FakeConn()
        expectCancelled { session.hold(late) }
        assertTrue(late.closed)
    }

    @Test
    fun `a dismissal ends its own ceremony and never the next one`() = runBlocking {
        val first = session()
        first.dismiss()
        val second = session()
        assertFalse("a retry starts undismissed", second.dismissed)
        assertEquals("phone", second.unlessDismissed(CompletableDeferred("phone")))
        val conn = FakeConn()
        second.hold(conn)
        assertFalse(conn.closed)
    }
}
