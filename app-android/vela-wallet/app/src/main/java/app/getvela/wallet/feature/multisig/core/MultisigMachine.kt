package app.getvela.wallet.feature.multisig.core

import app.getvela.wallet.core.crux.CoreBridge
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonObject

/**
 * One of the core's multisig machines, running in this app: its view, its
 * events in, its operations out through [MultisigExecutor]. The machine
 * decides; this holds the view the screens draw (the web's
 * `shell/machine.svelte.ts`, iOS's `MultisigMachine.swift`).
 *
 * It runs on [CoreHost] / `CoreDriver`, the loop every machine of this app
 * shares — so the multisig machines get the same three properties the session
 * and send machines have: bridge calls serialised and in order, nothing thrown
 * reaching the loop, a cancelled effect never answered; an answer the core
 * refuses is answered once more with the operation's own failure. The driver
 * opts every machine in to `skip_unchanged_views` before its first answer
 * (`multisig_bridge.rs`), so an answer that moved nothing is not decoded.
 */
class MultisigMachine(
    bridge: CoreBridge,
    scope: CoroutineScope,
    ports: MultisigPorts,
    val name: String,
) {
    private val executor = MultisigExecutor(ports)

    private val host = CoreHost(
        bridge = bridge,
        scope = scope,
        initial = JsonObject(emptyMap()),
        serializer = JsonObject.serializer(),
        perform = executor.perform,
        escapedFailure = executor.escapedFailure,
        onFault = { error -> VelaLog.failure("multisig", "multisig $name fault", error) },
    )

    init {
        host.start()
    }

    /** The machine's view, as last committed (the raw JSON object). */
    val view: StateFlow<JsonObject> = host.view

    /** Ticks on every commit, also one that left the view equal. */
    val commits: StateFlow<Long> = host.commits

    /** Send one event (the JSON object the core's `Event` reads). */
    fun dispatch(event: JsonObject) = host.dispatch(event.toString())

    /** The first view [pick] finds something in (now, or a later one), or `null` past [timeoutMs]. */
    suspend fun <T : Any> until(timeoutMs: Long = 600_000, pick: (JsonObject) -> T?): T? {
        pick(view.value)?.let { return it }
        return withTimeoutOrNull(timeoutMs) {
            view.map { pick(it) }.first { it != null }
        }
    }

    /**
     * Send [event] and wait for the first view, from the one that applied it
     * on, that [pick] finds something in — never a view from before the event
     * (the driver applies its inbox in order, so a view read right after a
     * dispatch may still be the last question's: a create's "idle", a
     * proposal's last "done"). The web's `request`, which is synchronous there.
     */
    suspend fun <T : Any> request(event: JsonObject, timeoutMs: Long = 600_000, pick: (JsonObject) -> T?): T? {
        val number = host.dispatchNumbered(event, JsonObject.serializer())
        return withTimeoutOrNull(timeoutMs) {
            host.commits.map { if (host.applied(number)) pick(host.view.value) else null }.first { it != null }
        }
    }

    fun dispose() = host.dispose()
}
