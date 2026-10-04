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
    /**
     * Every committed view, in the core's order, on the driver's loop and
     * before the effects that came with it start — for a view field that is
     * an instruction rather than a state (the sign machine's tracker
     * hand-off). [view] cannot carry one: a collector not scheduled between
     * two commits sees only the second, and whatever rode the first is gone.
     * Must not block.
     */
    private val onCommit: (V) -> Unit = {},
) {
    private val _view = MutableStateFlow(initial)

    /** The core's current view. Never a partially-applied one. */
    val view: StateFlow<V> = _view.asStateFlow()

    private val _viewJson = MutableStateFlow<String?>(null)

    /**
     * The committed view exactly as the core wrote it — for a core function
     * that reads a whole view back (spec 099: `signConfirmState`). [view] may
     * be a subset of it (a mirror leaves out what this client does not draw),
     * so re-encoding [view] would hand the core less than it said. Set before
     * [view], so whoever reads [view] finds this at least as new. `null`
     * before the first commit.
     */
    val viewJson: StateFlow<String?> = _viewJson.asStateFlow()

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

    /** How many events the core had applied when the view now in [view] was committed. */
    @Volatile
    private var viewEvents = 0L

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

    /**
     * [dispatch], for a caller that will wait for the core's answer to THIS
     * event: returns its number, for [applied].
     *
     * A commit counted from the moment of the dispatch is not "after the event".
     * The driver applies its inbox in order, and whatever was already queued in
     * it — the answers to the LAST question — is applied and committed after the
     * dispatch but before the event. The send's pre-check quote took the
     * warm-up's settled view that way, then read the view again and found its
     * own question just starting: `busy`, no fee, "estimate failed" (the
     * 2026-10-01 CI flake in `SendMachineTest`; a real Continue tapped as the
     * warm-up lands does the same).
     */
    fun <E> dispatchNumbered(event: E, eventSerializer: KSerializer<E>): Long =
        driver.dispatchNumbered(Wire.json.encodeToString(eventSerializer, event))

    /**
     * Whether the core had applied event [number] ([dispatchNumbered]) when the
     * view now in [view] was committed. Ask THIS first and read [view] after:
     * a `true` then guarantees the view read is that event's or a later one.
     */
    fun applied(number: Long): Boolean = viewEvents >= number

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
        val raw = viewJson.toString()
        runCatching { Wire.json.decodeFromString(serializer, raw) }
            .onSuccess {
                // In this order: the raw view, the view, then what it reflects,
                // then the wake-up. A collector woken by the commit — or anyone
                // who reads `applied` first — never pairs a count with an older view.
                _viewJson.value = raw
                _view.value = it
                viewEvents = driver.eventsApplied
                _commits.value += 1
                // A throw here would end the driver's loop: reported instead.
                runCatching { onCommit(it) }.onFailure(onFault)
            }
            .onFailure(onFault)
    }
}
