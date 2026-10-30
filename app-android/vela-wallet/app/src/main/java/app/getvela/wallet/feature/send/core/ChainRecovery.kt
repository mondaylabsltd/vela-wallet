package app.getvela.wallet.feature.send.core

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * The network answering again, after a send failed as the network's (round
 * 6) — the web's `chain-recovery.ts`, rule for rule.
 *
 * A submit whose reads before the passkey did not get through ends on its
 * review with the network's words ("Couldn't reach Arc Testnet — nothing was
 * signed or sent") and Confirm & Send shut (`tx_error` = `chain_unreachable` /
 * `chain_busy`). While that line is up this asks the form's network one
 * bounded read at a time — every [EVERY_MS], and at once when the device
 * reaches a server again ([backOnline]) — and the first answer is told
 * ([answered]): the controller tells the core (`chain_answered`), which opens
 * the confirm, and asks the fee again. "Reachable again" alone is not an
 * answer: the line stays until the network itself has answered.
 */
internal class ChainRecovery(
    private val scope: CoroutineScope,
    /** One read of the chain — `true` when the network answered it. */
    private val read: suspend (chainId: Int) -> Boolean,
    /** Whether the device reaches a server (`NetHealth.online`): its turn back to `true` asks at once. */
    private val backOnline: Flow<Boolean> = emptyFlow(),
    private val answered: (chainId: Int) -> Unit,
    /** How often, and how long one read may take — the web's; tests shorten them. */
    private val everyMs: Long = EVERY_MS,
    private val readMs: Long = READ_MS,
) {
    private var watching: Int? = null
    private var job: Job? = null

    /**
     * Follow [chainId] — the chain whose failure is on the review now — or,
     * with `null`, stop: the line went for any other reason (the person's own
     * Try again, leaving the review, another error). The same chain asked
     * again keeps the watch it has.
     */
    @Synchronized
    fun follow(chainId: Int?) {
        if (chainId == watching) return
        job?.cancel()
        job = null
        watching = chainId
        chainId ?: return
        job = scope.launch {
            val nudges = Channel<Unit>(Channel.CONFLATED)
            // A turn back online, never the value it stands at now.
            val online = launch { backOnline.distinctUntilChanged().drop(1).filter { it }.collect { nudges.trySend(Unit) } }
            try {
                while (isActive) {
                    // Every EVERY_MS, or at once on the way back online.
                    withTimeoutOrNull(everyMs) { nudges.receive() }
                    val ok = withTimeoutOrNull(readMs) { runCatching { read(chainId) }.getOrDefault(false) } ?: false
                    if (ok && isActive) {
                        val still = synchronized(this@ChainRecovery) {
                            (watching == chainId).also { if (it) watching = null }
                        }
                        if (still) answered(chainId)
                        return@launch
                    }
                }
            } finally {
                online.cancel()
            }
        }
    }

    companion object {
        /** How often the network is asked while its failure is on the review. */
        const val EVERY_MS = 5_000L

        /** How long one read may take before it counts as no answer. */
        const val READ_MS = 4_000L

        /**
         * The chain to follow for [view] — the form's network while the
         * review carries its failure, else `null` (the web's `recoveryChain`).
         */
        fun chainOf(view: SendView): Int? {
            if (view.tx_status != SendTxStatus.Error) return null
            if (view.tx_error != SendTxErrorKey.ChainUnreachable && view.tx_error != SendTxErrorKey.ChainBusy) return null
            return formChain(view)
        }
    }
}
