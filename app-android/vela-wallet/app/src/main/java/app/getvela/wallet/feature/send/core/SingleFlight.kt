package app.getvela.wallet.feature.send.core

import kotlinx.coroutines.CompletableDeferred

/**
 * One read in flight per question.
 *
 * The fee session for the speed in force and a preview session for each other
 * speed (spec 069) ask the SAME reads at the same instant — the deployment
 * status, the gas signals, the relay's gas quote, the in-band rows, the
 * simulation of one exact operation. The caches beside those reads only help
 * the NEXT asker: three askers arriving together each missed them and each
 * went to the network, so the three speed rows landed one after another, as
 * each of three identical requests happened to come back (founder, 2026-09-26).
 *
 * Here the first asker reads and the rest wait for its answer: one request,
 * every row settled by it at the same moment (the desktop's `single_flight.rs`,
 * the web's shared promises).
 *
 * A reader that fails or is cancelled abandons the slot: a waiter then reads
 * for itself rather than inherit somebody else's cancellation.
 */
internal class SingleFlight<K : Any, V> {
    private val lock = Any()
    private val inFlight = HashMap<K, CompletableDeferred<Answer<V>?>>()

    /** Boxed, so a `null` answer is an answer and `null` itself means "abandoned". */
    private class Answer<V>(val value: V)

    /** How many reads are out right now (tests). */
    val size: Int get() = synchronized(lock) { inFlight.size }

    /** `read()` once for everybody asking `key` while it runs. */
    suspend fun run(key: K, read: suspend () -> V): V {
        while (true) {
            val (slot, leader) = synchronized(lock) {
                val held = inFlight[key]
                if (held != null) {
                    held to false
                } else {
                    val fresh = CompletableDeferred<Answer<V>?>()
                    inFlight[key] = fresh
                    fresh to true
                }
            }
            if (!leader) {
                // Our own cancellation surfaces here as it should; an abandoned
                // slot (`null`) sends us round to read for ourselves.
                slot.await()?.let { return it.value }
                continue
            }
            try {
                val value = read()
                slot.complete(Answer(value))
                return value
            } finally {
                // Failed or cancelled before completing: abandoned.
                slot.complete(null)
                synchronized(lock) { if (inFlight[key] === slot) inFlight.remove(key) }
            }
        }
    }
}
