package app.getvela.wallet.core.crux

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.serialization.KSerializer
import org.json.JSONObject

/**
 * One Crux machine, hosted: its driver, its executor and the current view as a
 * `StateFlow` the UI collects.
 *
 * This is the piece that keeps "add a machine" from meaning "write plumbing".
 * `SessionController` was these four fields hand-rolled for one machine; doing
 * that twenty-three more times is how four clients end up with four subtly
 * different cancellation stories.
 *
 * ```text
 *   screen ──intent──► Controller ──event──► CoreHost ──► CoreDriver ──► core
 *   screen ◄──model─── Controller ◄──view─── CoreHost ◄─────────────────┘
 * ```
 *
 * A host holds **no product knowledge**: it does not know what its view means,
 * only how to keep it current. What a screen should draw from that view is a
 * `…Live.kt` builder's job, and what a tap means is a controller's.
 *
 * **Lifetime.** A host outlives the screens that show it (research D5): it is
 * created lazily by the composition root and never torn down, because a
 * machine rebuilt per screen re-runs its boot and flashes an empty address
 * book at somebody who has fifty contacts.
 *
 * **On the double parse.** The driver speaks `org.json` (the onboarding
 * executors, 1,400 lines of them, take a `JSONObject` operation), while wire
 * types are decoded with `kotlinx.serialization`. So a view crosses one
 * `toString()` on its way here. That costs a re-serialize per view update,
 * which is nothing at the rate a settings screen changes — and it is
 * deliberately confined to [commit], so the machine that finally makes it
 * matter (041's streaming balances) has exactly one place to fix.
 */
class CoreHost<V : Any>(
    bridge: CoreBridge,
    scope: CoroutineScope,
    /** What the screen renders before the core has said anything. */
    initial: V,
    private val serializer: KSerializer<V>,
    /** The machine's executor. Answers every operation, throws for none. */
    perform: suspend (operation: JSONObject) -> String,
    /** The machine's own failure variant, for an exception that escaped [perform]. */
    escapedFailure: (operation: JSONObject, error: Throwable) -> String,
    private val onFault: (Throwable) -> Unit = {},
) {
    private val _view = MutableStateFlow(initial)

    /** The core's current view. Never a partially-applied one. */
    val view: StateFlow<V> = _view.asStateFlow()

    private val _commits = MutableStateFlow(0L)

    /**
     * How many views the core has committed — for a caller waiting on something
     * the core DID rather than on what the view now says.
     *
     * [view] cannot be waited on for that. A `StateFlow` drops a value that
     * `equals` the last one a collector saw, and views are data classes: a
     * machine that goes `idle(A) → busy → idle(A')` with `A' == A` — a second
     * fee quote at the same price — looks, to a collector that was not
     * scheduled during `busy`, like nothing happened at all. It is never woken,
     * and whatever it was waiting for never arrives (the send flow's quote
     * wait sat out its whole 30 s this way on a loaded CI runner). A counter
     * never repeats, so every commit wakes its collectors; read [view] then.
     */
    val commits: StateFlow<Long> = _commits.asStateFlow()

    private val driver = CoreDriver(
        bridge = bridge,
        scope = scope,
        perform = perform,
        onView = ::commit,
        escapedFailure = escapedFailure,
        onFault = onFault,
    )

    /** Emit the core's current view without sending anything. */
    fun start() = driver.start()

    /** Send one event, already encoded. */
    fun dispatch(eventJson: String) = driver.dispatch(eventJson)

    /** Send one event as its wire type — the form every caller should prefer. */
    fun <E> dispatch(event: E, eventSerializer: KSerializer<E>) =
        driver.dispatch(Wire.json.encodeToString(eventSerializer, event))

    fun dispose() = driver.dispose()

    /**
     * A view the shell cannot understand does not become a wrong screen.
     *
     * Decoding throws only for a value this build has no name for — an enum
     * variant Rust gained and Kotlin has not (see [Wire]). Rendering that as a
     * default is precisely the silent mis-render the configuration is chosen to
     * prevent, so the fault is reported and the last view we *did* understand
     * stays on screen: visibly stuck, which is findable, rather than
     * confidently wrong, which is not.
     */
    private fun commit(viewJson: JSONObject) {
        runCatching { Wire.json.decodeFromString(serializer, viewJson.toString()) }
            .onSuccess {
                _view.value = it
                _commits.value += 1
            }
            .onFailure(onFault)
    }
}
