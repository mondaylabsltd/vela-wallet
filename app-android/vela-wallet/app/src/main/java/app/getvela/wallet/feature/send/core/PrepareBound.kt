package app.getvela.wallet.feature.send.core

import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.cancel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch

/**
 * The wait before the passkey, bounded and said (r5reg-3) — the web's
 * `prepare-bound.ts`, rule for rule.
 *
 * A send reads several things from the network before its passkey can open —
 * whether the account is deployed, its nonce, the gas. With a chain's nodes
 * answering 429 those reads held the confirm on a spinner with nothing said.
 * This runs the submit with the reads bounded up to the moment the passkey
 * opens (the `gate` the work calls right before it): past [SLOW_MS] it
 * reports how long it has been, every second, and whether the network's nodes
 * say they are busy; past [LIMIT_MS] it gives up — the work is stopped and the
 * gate shut for good, so a passkey never opens after the wait was given up and
 * nothing is signed behind the person's back. Once the gate has opened the
 * bound is over: the ceremony has its own deadline.
 *
 * What a failure means is the core's (`SendSubmitFailure::ChainUnreachable`);
 * this only measures, and says which side of the gate a failure came from.
 */
internal object PrepareBound {
    /** How long the reads may take before the confirm says what it is waiting for. */
    const val SLOW_MS = 5_000L

    /** How long the reads may take before the submit gives up — nothing signed or sent. */
    const val LIMIT_MS = 45_000L

    /** How often the time said is brought up to date. */
    const val TICK_MS = 1_000L

    /**
     * A failure before the passkey opened: [cause] (`null` when the wait was
     * given up — [timedOut]), and whether the nodes said they were busy.
     */
    class BeforePasskey(override val cause: Throwable?, val busy: Boolean, val timedOut: Boolean) :
        Exception(cause?.message ?: "the network did not answer before the passkey; nothing was signed")

    /** The gate was asked to open after the wait was given up. */
    class GivenUp : CancellationException("the wait before the passkey was given up; nothing was signed")

    /**
     * Run [work], which calls its `gate` right before the passkey opens. Until
     * it does: [onSlow] each second past [SLOW_MS] with the time so far and
     * [busy], and a [BeforePasskey] (`timedOut`) past [LIMIT_MS]. A failure of
     * [work] before the gate is a [BeforePasskey] unless [passThrough] keeps
     * its own meaning; after the gate, as thrown. A cancel from outside (the
     * person's own Cancel) is passed on as it is.
     */
    suspend fun <T> run(
        busy: () -> Boolean,
        onSlow: (elapsedMs: Long, busy: Boolean) -> Unit,
        passThrough: (Throwable) -> Boolean = { false },
        /** The bound's three times — the web's; tests shorten them. */
        slowMs: Long = SLOW_MS,
        limitMs: Long = LIMIT_MS,
        tickMs: Long = TICK_MS,
        work: suspend (gate: () -> Unit) -> T,
    ): T {
        val opened = AtomicBoolean(false)
        val givenUp = AtomicBoolean(false)
        val gate = {
            if (givenUp.get()) throw GivenUp()
            opened.set(true)
        }
        try {
            return coroutineScope {
                val scope = this
                val ticker = launch {
                    delay(slowMs)
                    var elapsed = slowMs
                    while (!opened.get()) {
                        if (elapsed >= limitMs) {
                            givenUp.set(true)
                            scope.cancel(GivenUp())
                            return@launch
                        }
                        onSlow(elapsed, busy())
                        val step = minOf(tickMs, limitMs - elapsed)
                        delay(step)
                        elapsed += step
                    }
                }
                try {
                    work(gate)
                } finally {
                    ticker.cancel()
                }
            }
        } catch (cancelled: CancellationException) {
            // Given up: the network's failure — unless the caller itself was
            // cancelled meanwhile, which is passed on.
            if (givenUp.get() && !opened.get()) {
                currentCoroutineContext().ensureActive()
                throw BeforePasskey(null, busy(), timedOut = true)
            }
            throw cancelled
        } catch (failure: Throwable) {
            if (opened.get() || passThrough(failure)) throw failure
            throw BeforePasskey(failure, busy(), timedOut = false)
        }
    }
}
