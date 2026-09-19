package app.getvela.wallet.core.crux

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
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
 * 1. **Bridge calls are serialized, AND IN ORDER.** `dispatch` and
 *    `resolveEffect` both mutate the core and both emit a view. Two coroutines
 *    resolving at the same instant would interleave their `onView` calls, and
 *    the LAST one to arrive — not the latest state — would be what the screen
 *    renders. Serialized is not enough, which this driver learned the slow way:
 *    it used to `launch` a coroutine per event and let them race for a mutex, so
 *    the core saw every event exactly once and in whatever order the thread
 *    pool felt like. Two events sent back to back — two quick taps — could
 *    arrive reversed. It hid for weeks as five "flaky" machine tests, each
 *    explained separately (2026-09-19; `CoreDriverOrderTest` is the proof and
 *    fails deterministically against the mutex). Everything that touches the
 *    core now goes through ONE inbox with ONE consumer: first in, first
 *    applied. Effects START in the order the core asked for them and run
 *    undispatched until they first suspend, so an effect that never suspends —
 *    every store write — also COMPLETES in order, and the last document the
 *    core produced is the one on disk. This is what a single-threaded shell
 *    (the web's) gets for free.
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
    /** One thing the core must be told, in the order it happened. */
    private sealed interface Turn {
        data object Start : Turn
        data class Event(val json: String) : Turn
        data class Answer(val id: ULong, val resultJson: String) : Turn
    }

    /** Unlimited, so `dispatch` never suspends and never drops: it is called from click handlers. */
    private val inbox = Channel<Turn>(Channel.UNLIMITED)

    // Touched only by the consumer below — which is what lets them be plain maps.
    private val running = mutableMapOf<ULong, Job>()
    /** The operation behind each in-flight effect, so a refused answer can be re-answered as its failure (spec 048). */
    private val operations = mutableMapOf<ULong, JSONObject>()
    /** Effects already answered with their failure — a second refusal only reports. */
    private val escaped = mutableSetOf<ULong>()
    @Volatile
    private var disposed = false

    init {
        // THE consumer. Every bridge call happens here, one at a time, in the
        // order its turn was queued.
        scope.launch {
            for (turn in inbox) {
                if (disposed) break
                when (turn) {
                    Turn.Start ->
                        runCatching { JSONObject(bridge.view()) }.onSuccess(::commit).onFailure(onFault)
                    is Turn.Event ->
                        runCatching { JSONObject(bridge.dispatch(turn.json)) }.onSuccess(::apply).onFailure(onFault)
                    is Turn.Answer -> resolve(turn.id, turn.resultJson)
                }
            }
        }
    }

    /** Emit the core's current view without sending anything. */
    fun start() {
        inbox.trySend(Turn.Start)
    }

    /** Send one event, as the JSON the core's `Event` deserializes from. */
    fun dispatch(eventJson: String) {
        inbox.trySend(Turn.Event(eventJson))
    }

    /**
     * Stop driving. In-flight effects are cancelled and their answers dropped:
     * a view produced after the screen has gone has nowhere to render.
     */
    fun dispose() {
        disposed = true
        inbox.close()
        // A snapshot: the consumer may be mid-turn on another thread.
        running.values.toList().forEach { it.cancel() }
    }

    // -- the loop ------------------------------------------------------------

    /** Consumer only. */
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
        // UNDISPATCHED: the effect starts NOW, on this thread, and runs until it
        // first suspends — so effects begin in the order the core listed them,
        // and one that never suspends (a store write) is finished before the
        // next begins. Anything that does real waiting suspends almost at once
        // and carries on concurrently, exactly as before.
        val job = scope.launch(start = CoroutineStart.UNDISPATCHED) {
            val resultJson = try {
                perform(operation)
            } catch (cancellation: CancellationException) {
                // Property 3: the core abandoned this operation. It is not
                // waiting for an answer, and giving it one would be the bug.
                throw cancellation
            } catch (error: Throwable) {
                // Property 2. The executor owes a variant for every expected
                // failure, so reaching here means a shell bug — but the core
                // must still be unblocked, or the flow stops with a spinner.
                onFault(error)
                escapedFailure(operation, error)
            }
            // The answer takes its turn like everything else.
            inbox.trySend(Turn.Answer(id, resultJson))
        }
        // An effect that finished without suspending has already queued its
        // answer; tracking its completed Job is harmless (cancel is a no-op)
        // and it is dropped when the answer is applied.
        running[id] = job
    }

    /** Consumer only. */
    private fun resolve(id: ULong, resultJson: String) {
        running.remove(id)
        if (disposed) return
        run {
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
