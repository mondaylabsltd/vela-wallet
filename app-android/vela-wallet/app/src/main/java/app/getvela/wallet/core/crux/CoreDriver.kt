package app.getvela.wallet.core.crux

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONObject

/**
 * Platform-shell plumbing for a Crux core, blind to product semantics.
 *
 * It knows how to dispatch an event, perform the effects that come back, and
 * hand the answers to the core. It knows nothing about wallets — which is why
 * every machine the app drives shares it instead of each screen re-deriving the
 * same failure and cancellation rules.
 *
 * ```text
 *   ViewModel                CoreDriver               executor
 *       │ dispatch(event) ───────►│
 *       │                         │ perform(operation) ──────►│  (a Job each)
 *       │◄──── onView(view) ──────│◄─────────── result json ──│
 *       │                         │ resolveEffect(id, result) …until it drains
 * ```
 *
 * Three properties are the whole contract, and each is a bug that is easy to
 * write and hard to see:
 *
 * 1. **Bridge calls are serialized.** `dispatch` and `resolveEffect` both mutate
 *    the core and both emit a view. Two coroutines resolving at the same instant
 *    would interleave their `onView` calls, and the LAST one to arrive — not the
 *    latest state — would be what the screen renders. The gate makes the order
 *    the core's, not the scheduler's.
 * 2. **Nothing thrown by an executor reaches the loop.** `perform` owes a result
 *    variant for every failure; if one still escapes, it is a shell bug and the
 *    loop reports it through `onFault` rather than dying and leaving the core
 *    waiting forever on an effect nobody will answer.
 * 3. **A cancelled effect is not answered.** The core asked for it to be
 *    abandoned, so it is not waiting — resolving it would push a stale answer
 *    into a machine that moved on.
 */
class CoreDriver(
    private val bridge: CoreBridge,
    private val scope: CoroutineScope,
    /** Perform one operation and return the result JSON. Must not throw. */
    private val perform: suspend (operation: JSONObject) -> String,
    /** Called on every committed view, in the order the core produced them. */
    private val onView: (JSONObject) -> Unit,
    /**
     * The answer to give when [perform] itself threw — the machine's own
     * "something went wrong" variant for that operation.
     *
     * A parameter rather than a call, because the variant is the MACHINE's:
     * contacts answers `written`, onboarding answers `passkey_failed`, and a
     * driver that knew which was which would be a driver that knows about
     * wallets. What it does know is that an unanswered effect is a spinner
     * that never stops, so this must always produce something.
     */
    private val escapedFailure: (operation: JSONObject, error: Throwable) -> String,
    /** A shell fault: a malformed event, an escaped exception. Never a user error. */
    private val onFault: (Throwable) -> Unit = {},
) {
    private val gate = Mutex()
    private val running = mutableMapOf<ULong, Job>()
    /** The operation behind each in-flight effect, so a refused answer can be re-answered as its failure (spec 048). */
    private val operations = mutableMapOf<ULong, JSONObject>()
    /** Effects already answered with their failure — a second refusal only reports. */
    private val escaped = mutableSetOf<ULong>()
    private var disposed = false

    /** Emit the core's current view without sending anything. */
    fun start() {
        scope.launch {
            gate.withLock {
                runCatching { JSONObject(bridge.view()) }
                    .onSuccess(::commit)
                    .onFailure(onFault)
            }
        }
    }

    /** Send one event, as the JSON the core's `Event` deserializes from. */
    fun dispatch(eventJson: String) {
        scope.launch {
            gate.withLock {
                runCatching { JSONObject(bridge.dispatch(eventJson)) }
                    .onSuccess(::apply)
                    .onFailure(onFault)
            }
        }
    }

    /**
     * Stop driving. In-flight effects are cancelled and their answers dropped:
     * a view produced after the screen has gone has nowhere to render.
     */
    fun dispose() {
        disposed = true
        running.values.forEach { it.cancel() }
        running.clear()
    }

    // -- the loop ------------------------------------------------------------

    /** Caller holds [gate]. */
    private fun apply(result: JSONObject) {
        commit(result.optJSONObject("view") ?: JSONObject())

        result.optJSONArray("cancelled_effect_ids")?.let { cancelled ->
            for (i in 0 until cancelled.length()) {
                val id = cancelled.optLong(i).toULong()
                running.remove(id)?.cancel()
            }
        }

        val effects = result.optJSONArray("effects")
        for (i in 0 until (effects?.length() ?: 0)) {
            val effect = effects?.optJSONObject(i) ?: continue
            val id = effect.optLong("id").toULong()
            val operation = effect.optJSONObject("operation") ?: continue
            operations[id] = operation
            run(id, operation)
        }
    }

    private fun commit(view: JSONObject) {
        // A view produced before disposal can still arrive after it (an effect
        // resolving while the screen unmounts). Dropping it keeps the UI from
        // being asked to render into a torn-down tree.
        if (!disposed) onView(view)
    }

    private fun run(id: ULong, operation: JSONObject) {
        if (disposed) return
        val job = scope.launch {
            val resultJson = try {
                perform(operation)
            } catch (cancellation: CancellationException) {
                // Property 3: the core abandoned this operation. It is not
                // waiting for an answer, and giving it one would be the bug.
                running.remove(id)
                throw cancellation
            } catch (error: Throwable) {
                // Property 2. The executor owes a variant for every expected
                // failure, so reaching here means a shell bug — but the core
                // must still be unblocked, or the flow stops with a spinner.
                onFault(error)
                escapedFailure(operation, error)
            }
            running.remove(id)
            resolve(id, resultJson)
        }
        running[id] = job
    }

    private suspend fun resolve(id: ULong, resultJson: String) {
        gate.withLock {
            if (disposed) return
            runCatching { JSONObject(bridge.resolveEffect(id, resultJson)) }
                .onSuccess {
                    operations.remove(id)
                    escaped.remove(id)
                    apply(it)
                }
                .onFailure { error ->
                    onFault(error)
                    // Property 3 (spec 048): the core refused the answer — a shape it
                    // cannot read, the way the retired client's records were — and
                    // the machine is still waiting on this effect. It gets the
                    // effect's own failure once; a second refusal only reports.
                    // Device-found 2026-09-12 on the web: the alternative is a
                    // session that says `loading` forever.
                    val operation = operations[id]
                    if (operation != null && escaped.add(id)) {
                        val failure = runCatching { escapedFailure(operation, error) }.getOrNull()
                        if (failure != null) {
                            runCatching { JSONObject(bridge.resolveEffect(id, failure)) }
                                .onSuccess { operations.remove(id); apply(it) }
                                .onFailure(onFault)
                        }
                    } else {
                        operations.remove(id)
                    }
                }
        }
    }
}
