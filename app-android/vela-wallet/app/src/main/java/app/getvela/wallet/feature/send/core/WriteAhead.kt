package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.userOpWriteAheadWaitMs

/**
 * The write-ahead gate (spec 082 RJ1): a record exists before the bytes leave.
 *
 * A submit that has signed its op and computed the hash tells the core so
 * (`OpSigned`), and waits here. The core writes the record ahead, hands it to
 * the tracker, and only once the write is acknowledged answers `ClearToPost`,
 * which [clear] turns into the waiting submit's go. No clearance within the
 * core's `userOpWriteAheadWaitMs()` is no POST: nothing left the device, and
 * the submit reports "not sent". A quit, a crash or a window close anywhere
 * after the write leaves one pending "may have been sent" row the tracker
 * resolves on the next launch (DX9).
 *
 * The gate also refuses a clearance for a record the store did not take
 * ([written]): the core hears every persist acknowledged (a storage fault must
 * not hang the machine), so the shell's own "it is on disk" is checked here,
 * where the POST is decided.
 *
 * Keys are the caller's: the dApp path keys on request id and hash, the
 * wallet's Send on the hash alone.
 */
class WriteAhead(
    /** How long a submit waits for its clearance; `null` = the core's `userOpWriteAheadWaitMs()`. */
    private val waitMs: Long? = null,
) {
    private val waiting = ConcurrentHashMap<String, CompletableDeferred<Boolean>>()
    private val onDisk = ConcurrentHashMap.newKeySet<String>()

    /** The store took the record(s) of [userOpHash]. */
    fun written(userOpHash: String) {
        onDisk += userOpHash.lowercase()
    }

    /**
     * Register [key] BEFORE telling the core the op is signed — the clearance
     * can come back before the caller starts waiting.
     *
     * [userOpHash]: the op about to be written ahead. Only a write that comes
     * after this call counts for it (082 review of T245): the same op signed
     * again — same nonce, calls and fee, so the same hash (DX9's runs shared
     * one) — must not be cleared by the mark an earlier attempt's write left,
     * when the store refuses this attempt's.
     */
    fun expect(key: String, userOpHash: String? = null): Clearance {
        userOpHash?.let { onDisk.remove(it.lowercase()) }
        val deferred = CompletableDeferred<Boolean>()
        waiting[key] = deferred
        return Clearance(key, deferred)
    }

    /**
     * The core's `ClearToPost` for [key]: the waiting submit may POST — when
     * the store really holds [userOpHash]'s record. `false` when nobody was
     * waiting (a submit that already gave up): nothing is posted for it.
     */
    fun clear(key: String, userOpHash: String): Boolean {
        // One write stands behind one clearance: the mark is used up here,
        // whether or not anybody is still waiting for it.
        val stored = onDisk.remove(userOpHash.lowercase())
        val deferred = waiting[key] ?: run {
            VelaLog.event("userop.write_ahead", "clearance for nobody: nothing posted", "op" to userOpHash.take(12))
            return false
        }
        if (!stored) VelaLog.event("userop.write_ahead", "cleared, but the store never took the record: not posting", "op" to userOpHash.take(12))
        deferred.complete(stored)
        return stored
    }

    inner class Clearance(private val key: String, private val deferred: CompletableDeferred<Boolean>) {
        /** `true` = cleared, POST now; `false` = no clearance in time, or none this store can stand behind. */
        suspend fun await(): Boolean {
            val wait = waitMs ?: userOpWriteAheadWaitMs().toLong()
            val started = System.currentTimeMillis()
            return try {
                val cleared = withTimeoutOrNull(wait) { deferred.await() }
                when (cleared) {
                    true -> VelaLog.event("userop.write_ahead", "cleared", "ms" to (System.currentTimeMillis() - started))
                    false -> Unit
                    null -> VelaLog.event("userop.write_ahead", "no clearance in time: nothing posted", "waitMs" to wait)
                }
                cleared == true
            } finally {
                waiting.remove(key, deferred)
            }
        }
    }
}
