package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.FeePolicyCore
import uniffi.vela_core_uniffi.FeeSpeedCore

/**
 * The speed control of one fee surface — the send form, or the dApp signing
 * sheet (spec 069). One class, so the two surfaces cannot drift: the design
 * sheet says of the fee card that Send and signing "must not drift", and the
 * speed control is part of that card (the web's `SpeedControl`).
 *
 * One `fee_policy` session is IN FORCE — the quote the surface pre-checks
 * against, the fee row shows and the submit signs — and one PREVIEW per other
 * tier the `fee_speed` core wants priced. Every rule of the speed control is
 * that core's; what this class owns is the sessions and the reconcile step
 * every shell runs (`fee_speed.rs`): when the session in force is not pricing
 * the tier in force, PROMOTE the preview that already priced this operation at
 * that tier (the price tapped is the price paid, #681), else re-price once
 * nothing is measuring.
 *
 * All bookkeeping happens under [sessionLock]; a session's commits, the speed
 * view and the executor's quote all arrive on their own threads. Nothing
 * suspends while the lock is held.
 *
 * Runs on a child of [parent], so a surface that lives for one operation (the
 * signing sheet) ends every watcher here with [dispose].
 */
class SpeedControl(
    parent: CoroutineScope,
    private val relay: RelayClient,
    private val feeExecutor: FeeExecutor,
    /** The stored default speed, and the resolved number preset its gas bids are written in. */
    private val preferredTier: () -> FeeTier,
    private val numberPreset: () -> String,
    /** The log area this surface writes under: `send`, `sign`. */
    private val area: String,
) {
    private val scope = CoroutineScope(parent.coroutineContext + SupervisorJob(parent.coroutineContext[Job]))

    private val sessionLock = Any()
    private var nextSessionKey = 0L
    private var generation = 0L
    private val previews = mutableListOf<FeeSession>()

    /**
     * Spec 083 fee, issue #411: what the operation moves, per asset, as the
     * surface's own simulation measured it — with the calls it measured, so
     * it is told only to a session pricing those very calls (guarded by
     * [sessionLock]). The fee machine forgets it on every `QuoteRequested`,
     * so every session is told again right after each question it is asked
     * (the desktop's `speed_control::balance_changes`).
     */
    private var measured: Pair<List<FeeCall>, List<FeeBalanceChange>>? = null

    /** What a session was asked to price; compared minus the tier. */
    data class QuoteAsk(
        val chainId: Int,
        val account: String,
        val publicKeyAvailable: Boolean,
        val tier: FeeTier,
        val calls: List<FeeCall>,
        val feeToken: String?,
        /**
         * Nobody chose the fee coin: the fee machine pays in one that can.
         * Part of the operation, so every preview replays it and a promotion
         * never swaps an auto-priced quote for a picked one (or back).
         */
        val autoFeeToken: Boolean = false,
    ) {
        fun sameOperation(other: QuoteAsk): Boolean = copy(tier = other.tier) == other

        /**
         * `number`: the resolved preset the core writes a shortfall's amounts in (issue #408).
         *
         * Issue #483: the account's deployment is the fee machine's own first
         * read (`read_deployment`) — its failure is the fee's row, footer and
         * retry, worded by the core ("can't reach the chain" vs "internal"),
         * and every retry reads it again. `deployed` is then only the core's
         * starting assumption, never trusted.
         */
        fun event(number: String) = FeeEvent.QuoteRequested(
            chain_id = chainId,
            account = account,
            deployed = false,
            public_key_available = publicKeyAvailable,
            tier = tier,
            calls = calls,
            fee_token = feeToken,
            auto_fee_token = autoFeeToken,
            number = number,
            read_deployment = true,
        )
    }

    /** How a [quote] ended. */
    sealed class Quoted {
        /** Settled: a new estimate, or the core's failure. */
        data class Settled(val view: FeeView) : Quoted()

        data object TimedOut : Quoted()
    }

    /** One `fee_policy` session: in force, or a preview of another tier. */
    private inner class FeeSession(val key: Long) {
        val host = CoreHost(
            bridge = FeePolicyCore().asBridge(),
            scope = scope,
            initial = FeeView(),
            serializer = FeeView.serializer(),
            perform = JsonShell.perform(FeeOperation.serializer(), FeeShellResult.serializer(), feeExecutor::perform),
            escapedFailure = JsonShell.escapedFailure(
                FeeOperation.serializer(),
                FeeShellResult.serializer(),
                fallback = FeeShellResult.TtlElapsed,
                answer = feeExecutor::neutralAnswer,
            ),
            onFault = { error -> VelaLog.failure("$area.fee.fault", "core fault", error) },
        )
        /**
         * The question this session's core was last asked — set and dispatched
         * under [sessionLock] in one step: the deployment read is the core's
         * own (issue #483), so there is no shell read between the two.
         */
        @Volatile var ask: QuoteAsk? = null

        /**
         * The chain of the last question this session's core has APPLIED —
         * what its view's `fee_token` is a contract on. Moves only once a view
         * of that question is committed ([asked]), never ahead of it: a chain
         * named before its view lands would pair it with the last question's
         * coin.
         */
        val priced = MutableStateFlow<Int?>(null)

        /** The last question dispatched: its event number here and its chain. */
        @Volatile private var question: Pair<Long, Int>? = null

        /** A question went to the core as event [event]: [priced] follows once it is applied. */
        fun asked(event: Long, chainId: Int) {
            question = event to chainId
            follow()
        }

        private fun follow() {
            val (event, chainId) = question ?: return
            if (host.applied(event)) priced.value = chainId
        }

        @Volatile var generation = 0L
        private var watch: Job? = null

        /**
         * Begin reporting. Separate from construction: the first session is
         * built while this control is, and a watcher that fired on another
         * thread before `inForce` was assigned would read a field not yet set.
         */
        fun start(): FeeSession {
            host.start()
            // Every commit, not every distinct view: see `CoreHost.commits`.
            // First the chain this session has now priced (the fee-coin
            // bridge reads it), then the whole reconcile, not just the
            // report: a session that settled at another tier than the one in
            // force — asked before the speed core applied the stored speed,
            // say — is re-priced now that it is no longer measuring. Run only
            // on a speed change, that pass could miss the moment for good:
            // the speed core's view need not change when the session settles,
            // and the sheet then waited on "working out the fee" under a speed
            // it was never priced at. `speedPass` ends with `reportQuotes`.
            watch = scope.launch {
                host.commits.collect {
                    follow()
                    speedPass()
                }
            }
            return this
        }

        /** The core's view — what the surface shows, failure and all (issue #483: no shell overlay). */
        val view: FeeView get() = host.view.value

        fun dispose() {
            watch?.cancel()
            host.dispose()
        }
    }

    private val speedHost = CoreHost(
        bridge = FeeSpeedCore().asBridge(),
        scope = scope,
        initial = FeeSpeedView(),
        serializer = FeeSpeedView.serializer(),
        // `fee_speed` asks the shell for nothing; an operation would be a core bug.
        perform = { operation -> error("fee_speed asked for an operation: $operation") },
        escapedFailure = { operation, error -> throw IllegalStateException("fee_speed operation $operation", error) },
        onFault = { error -> VelaLog.failure("$area.speed.fault", "core fault", error) },
    )

    /** The speed control, as the core decided it — the surface draws this. */
    val speed: StateFlow<FeeSpeedView> = speedHost.view

    private fun newSession(): FeeSession = FeeSession(nextSessionKey++)

    private val inForce = MutableStateFlow(newSession())

    /**
     * The fee session in force — whichever session that is right now, so a
     * promotion swaps what the fee row and the fee-token sheet read without
     * either of them knowing.
     */
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val fee: StateFlow<FeeView> = inForce
        .flatMapLatest { session -> session.host.view }
        .stateIn(scope, SharingStarted.Eagerly, FeeView())

    /**
     * The chain the session in force prices: the chain of the last question
     * its core has applied ([FeeSession.priced]) — what [fee]'s `fee_token` is
     * a contract on. `null` before any question has. The send bridge tells the
     * card's coin only while this is the form's own chain ([FeeTokenWord]).
     */
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val pricingChainId: StateFlow<Int?> = inForce
        .flatMapLatest { session -> session.priced }
        .stateIn(scope, SharingStarted.Eagerly, null)

    /**
     * [fee] exactly as the session in force wrote it — for a core function
     * that reads the whole view back (spec 099: `signConfirmState`). The
     * account read's failure is in it (issue #483), so the footer the core
     * words from it names the same cause as the row. `null` before the
     * session first commits.
     */
    @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
    val feeJson: StateFlow<String?> = inForce
        .flatMapLatest { session -> session.host.viewJson }
        .stateIn(scope, SharingStarted.Eagerly, null)

    /**
     * The speed control exactly as the core wrote it — for a core function
     * that reads the whole view back (spec 102: `handoffFeeRow`). `null`
     * before its first commit.
     */
    val speedJson: StateFlow<String?> = speedHost.viewJson

    /** The fee view of the session pricing `tier` — for formatting that option's fee. */
    fun feeViewOf(tier: FeeTier): FeeView? = synchronized(sessionLock) {
        inForce.value.takeIf { it.ask?.tier == tier }?.view
            ?: previews.firstOrNull { it.ask?.tier == tier }?.view
    }

    /** A question is out that somebody awaits; nothing may re-price over it. */
    @Volatile
    private var answering = false

    private var lastOnForm: Boolean? = null

    /** Start the machines. Separate from construction for the same reason as [FeeSession.start]. */
    fun start(): SpeedControl {
        speedHost.start()
        inForce.value.start()
        speedHost.dispatch(FeeSpeedEvent.Reset, FeeSpeedEvent.serializer())
        // Every decision the speed core takes, carried out at once, in order:
        // promote or re-price the session in force FIRST, then bring the
        // previews in line — the other way round would dispose the very
        // session somebody tapped.
        scope.launch { speedHost.view.collect { speedPass() } }
        // The device log's `fee:` lines (spec 082 RJ12): the core retries a
        // failure itself now (PR 2 note 1), so the shell only says what it
        // sees — each failure, each re-ask, and the fee back.
        scope.launch {
            var last: FeeFailureView? = null
            fee.collect { view ->
                val failure = view.failure
                if (failure == last) return@collect
                val chain = synchronized(sessionLock) { inForce.value.ask?.chainId }
                when {
                    failure == null && last != null -> VelaLog.event("fee", "quote back chain=$chain")
                    failure != null && failure.retrying -> VelaLog.event("fee", "re-ask out chain=$chain cause=${failure.failure.wire}")
                    failure != null -> VelaLog.event(
                        "fee", "quote failed chain=$chain cause=${failure.failure.wire}",
                        "auto" to failure.auto_retry,
                    )
                }
                last = failure
            }
        }
        return this
    }

    // -- the questions ------------------------------------------------------------

    /**
     * Price the operation at the tier in force and wait for the answer — for a
     * surface whose machine asked (the send's `estimate_fee`). Settled = not
     * busy, and either a NEW estimate or a failure — on whichever session is
     * in force by then: a promotion of the same operation answers this
     * question with the quote that was tapped.
     */
    suspend fun quote(
        chainId: Int,
        account: String,
        publicKeyAvailable: Boolean,
        calls: List<FeeCall>,
        feeToken: String?,
        /** The send machine's word: nobody chose the coin on this form (`estimate_fee.auto_fee_token`). */
        autoFeeToken: Boolean = false,
    ): Quoted {
        // HOW FAST is the shell's to say (spec 068): the tier the speed core
        // has in force — the stored default, a one-shot pick, a free upgrade.
        val ask = QuoteAsk(chainId, account, publicKeyAvailable, speed.value.tier, calls, feeToken, autoFeeToken)
        return askAndWait(ask, QUOTE_TIMEOUT_MS)
    }

    private suspend fun askAndWait(ask: QuoteAsk, timeoutMs: Long): Quoted {
        answering = true
        try {
            val asked = askInForce(ask)
            val before = asked.before
            // Woken by `commits`, NOT by `view`: a re-quote at the same price is
            // a view that `equals` the one before the request, and a StateFlow
            // never delivers that to a collector that missed the `busy` in
            // between (see `CoreHost.commits`).
            //
            // And only by a view committed after the core APPLIED this question.
            // `commits` replays its current value to a new collector, so a view
            // still carrying the LAST attempt's `failed` once satisfied the wait
            // at once (a Max after a failed warm-up got the stale failure in
            // milliseconds). Counting commits from the dispatch was not enough
            // either: the question queues behind whatever the inbox already
            // holds, and the warm-up's last answer, applied after the dispatch
            // but before the question, settled the pre-check's wait with the
            // warm-up's quote (2026-10-01 CI flake; `CoreHost.dispatchNumbered`).
            // A promoted preview is another session and already settled.
            //
            // The view judged is the view returned. Reading `s.view` again after
            // the check once handed back the NEXT commit — this question just
            // begun, `busy` with no fee — which the send read as a failed estimate.
            @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
            val settled = withTimeoutOrNull(timeoutMs) {
                inForce
                    .flatMapLatest { s -> s.host.commits.map { s } }
                    .map { s ->
                        // `applied` before the view: then the view is at least the question's own.
                        val ours = s !== asked.session || s.host.applied(asked.event)
                        Triple(s, ours, s.view)
                    }
                    .first { (_, ours, view) ->
                        ours && !view.busy && ((view.fee != null && view.fee !== before) || view.failed != null)
                    }.third
            } ?: return Quoted.TimedOut
            return Quoted.Settled(settled)
        } finally {
            answering = false
            speedPass()
        }
    }

    /**
     * Price the operation at the tier in force and let the fee row show it
     * when it lands — for a surface nobody's machine is waiting on (the
     * signing sheet).
     */
    fun ask(
        chainId: Int,
        account: String,
        publicKeyAvailable: Boolean,
        calls: List<FeeCall>,
        feeToken: String?,
        /** Nobody has chosen the coin yet: the fee machine picks one that can pay. */
        autoFeeToken: Boolean = false,
    ) {
        val ask = QuoteAsk(chainId, account, publicKeyAvailable, speed.value.tier, calls, feeToken, autoFeeToken)
        scope.launch { askInForce(ask) }
    }

    /**
     * A fee coin picked for the operation in force (`null` = the native coin),
     * by a surface nobody's machine is waiting on (the dApp sheet).
     *
     * The coin is part of the operation every preview replays, so the whole
     * question is asked again with it and the other speeds follow in that
     * coin. Telling the session in force alone (`SelectFeeAsset`) would leave
     * the previews pricing the old coin — and a speed tapped next would promote
     * one, switching the payment back to a coin the person walked away from.
     */
    fun chooseFeeToken(feeToken: String?) {
        val ask = inForce.value.ask ?: return
        // Under auto the coin asked for is only a fallback, so tapping it is
        // still a choice — and from here on the coin is priced as picked.
        if (ask.feeToken == feeToken && !ask.autoFeeToken) return
        val next = ask.copy(feeToken = feeToken, autoFeeToken = false, tier = speed.value.tier)
        scope.launch { askInForce(next) }
    }

    /** A question dispatched: to which session, the estimate it held before, and the question's event number there (`CoreHost.dispatchNumbered`). */
    private class Asked(
        val session: FeeSession,
        val before: FeeEstimateView?,
        val event: Long,
    )

    /**
     * Price `ask` on the session in force: one `QuoteRequested`, recorded and
     * dispatched in one step. The account's deployment is the core's own first
     * read (issue #483 — `read_deployment`): a read the chain's nodes did not
     * answer is the fee's `chain_read`, one that never left the app its
     * `internal`, each with the fee's words, footer and retry — no shell
     * overlay, and no shell read a stale object could answer.
     */
    private fun askInForce(ask: QuoteAsk): Asked {
        val asked = synchronized(sessionLock) {
            val session = inForce.value
            generation += 1
            session.ask = ask
            session.generation = generation
            val before = session.view.fee
            val event = session.host.dispatchNumbered(ask.event(numberPreset()), FeeEvent.serializer())
            session.asked(event, ask.chainId)
            tellMeasured(session)
            Asked(session, before, event)
        }
        speedPass()
        return asked
    }

    // -- the reconcile step (`fee_speed.rs`) -------------------------------------

    /** Every session's state, whole, to the speed core. */
    private fun reportQuotes() = synchronized(sessionLock) {
        val main = inForce.value
        val event = FeeSpeedEvent.QuotesChanged(
            chain_id = main.ask?.chainId,
            in_force = TierQuote(busy = main.view.busy, fee = main.view.fee),
            previews = previews.mapNotNull { session ->
                val tier = session.ask?.tier ?: return@mapNotNull null
                TierPreviewQuote(tier = tier, busy = session.view.busy, fee = session.view.fee)
            },
        )
        speedHost.dispatch(event, FeeSpeedEvent.serializer())
    }

    /** Rule 1, then rule 2: the tier in force, then the previews beside it. */
    private fun speedPass() {
        val repriced = synchronized(sessionLock) {
            val view = speed.value
            val main = inForce.value
            val ask = main.ask
            var reprice: QuoteAsk? = null
            if (ask != null && ask.tier != view.tier) {
                if (promote(view.tier)) {
                    // Promoted: the quotes moved, say so.
                } else if (!main.view.busy && !answering) {
                    reprice = ask.copy(tier = view.tier)
                }
            }
            syncPreviews(view.previews)
            reprice
        }
        reportQuotes()
        // Never over a measurement that is out — a machine may be waiting on
        // it. What is left is a tier with nothing priced for it.
        if (repriced != null) scope.launch { askInForce(repriced) }
    }

    /**
     * THE PRICE YOU TAP IS THE PRICE YOU GET (issue 681): the preview that
     * already priced THIS operation at `tier` becomes the session in force —
     * no second question to the relay — and the one it replaces stays on as
     * its own tier's preview. Refused when that preview has no settled quote
     * of its own, or priced another operation than the one last asked.
     */
    private fun promote(tier: FeeTier): Boolean {
        val main = inForce.value
        val mainAsk = main.ask ?: return false
        val index = previews.indexOfFirst { session ->
            val ask = session.ask
            ask != null && ask.tier == tier && ask.sameOperation(mainAsk) &&
                !session.view.busy && session.view.fee?.tier == tier
        }
        if (index < 0) return false
        val promoted = previews.removeAt(index)
        previews.add(main)
        inForce.value = promoted
        VelaLog.event("$area.speed", "promoted", "tier" to tier.name)
        return true
    }

    /**
     * Rule 2: a preview for every tier the core names, pricing the same
     * operation as the session in force at the same generation; every other
     * preview disposed.
     */
    private fun syncPreviews(wanted: List<FeeTier>) {
        val main = inForce.value
        val base = main.ask
        val iterator = previews.iterator()
        while (iterator.hasNext()) {
            val session = iterator.next()
            val ask = session.ask
            val keep = base != null && ask != null && ask.tier in wanted && ask.sameOperation(base) &&
                session.generation == generation
            if (!keep) {
                iterator.remove()
                session.dispose()
            }
        }
        if (base == null) return
        for (tier in wanted) {
            if (previews.any { it.ask?.tier == tier }) continue
            val session = newSession()
            val ask = base.copy(tier = tier)
            session.ask = ask
            session.generation = generation
            previews.add(session.start())
            session.asked(session.host.dispatchNumbered(ask.event(numberPreset()), FeeEvent.serializer()), ask.chainId)
            tellMeasured(session)
        }
    }

    // -- what the operation moves (spec 083 fee, issue #411) -------------------------

    /**
     * The surface's own simulation of the operation answered: what it moves,
     * per asset — the coins' own `Transfer` logs and the node's trace of
     * native value ([app.getvela.wallet.feature.signing.core.SimDeltas.feeBalanceChanges]).
     * Which fee coin can pay is what the operation LEAVES of it, and a coin
     * the calls only name (a router's swap path) is one the machine may pay in
     * once this says enough of it is left.
     *
     * Kept for every question about [calls], and told now to each session that
     * was already asked one — the session in force and every speed preview
     * pricing those very calls. Never call this for a simulation
     * that could not check or that reverted: that is no measurement.
     */
    fun balanceChanges(calls: List<FeeCall>, changes: List<FeeBalanceChange>) {
        val told = synchronized(sessionLock) {
            measured = calls to changes
            (listOf(inForce.value) + previews).count { session ->
                session.ask != null && tellMeasured(session)
            }
        }
        VelaLog.event("$area.fee", "balance changes measured", "changes" to changes.size, "told" to told)
    }

    /**
     * Tell `session` what its operation moves, when it prices the calls that
     * were measured. Called under [sessionLock], straight after the session's
     * `QuoteRequested` — which clears it in the machine — so nothing is
     * dispatched between the question and the measurement.
     */
    private fun tellMeasured(session: FeeSession): Boolean {
        val (calls, changes) = measured ?: return false
        if (session.ask?.calls != calls) return false
        session.host.dispatch(FeeEvent.BalanceChangesMeasured(changes), FeeEvent.serializer())
        return true
    }

    // -- intents ---------------------------------------------------------------------

    /**
     * A new operation: the one-shot pick, a free upgrade and the fold die
     * with the one before it (spec 068), and it starts at the stored default.
     */
    fun reset() {
        synchronized(sessionLock) { measured = null }
        speedHost.dispatch(FeeSpeedEvent.Reset, FeeSpeedEvent.serializer())
        preferenceChanged()
    }

    /**
     * The stored default or the number preset moved: an operation already
     * open follows the new default until the person picks a speed on it.
     */
    fun preferenceChanged() =
        speedHost.dispatch(FeeSpeedEvent.Configure(preferredTier(), numberPreset()), FeeSpeedEvent.serializer())

    /** Whether the person is still choosing: a free upgrade is decided only then. */
    fun stage(onForm: Boolean) {
        synchronized(sessionLock) {
            if (onForm == lastOnForm) return
            lastOnForm = onForm
        }
        speedHost.dispatch(FeeSpeedEvent.StageChanged(onForm), FeeSpeedEvent.serializer())
    }

    /** Fold or unfold the control. */
    fun toggle() = speedHost.dispatch(FeeSpeedEvent.Toggle, FeeSpeedEvent.serializer())

    /** A tap on an option — one-shot, never the stored preference. */
    fun pick(tier: FeeTier) = speedHost.dispatch(FeeSpeedEvent.Pick(tier), FeeSpeedEvent.serializer())

    /**
     * The refresh control: measure again. The held readings go FIRST (issue
     * 212), so this is a new measurement rather than the number on screen,
     * and the other tiers are re-priced with it. After a failure the core
     * reads the account again too, past what it held (issue #483: `fresh`).
     */
    fun refresh() {
        val session = inForce.value
        val ask = session.ask ?: return
        relay.invalidateFeeSignals(ask.chainId)
        synchronized(sessionLock) {
            generation += 1
            session.generation = generation
        }
        session.host.dispatch(FeeEvent.Requote, FeeEvent.serializer())
        speedPass()
    }

    /**
     * The surface was left but this control lives on (the send form, one per
     * process): every session stops — its block-time re-pricing and the
     * core's own re-ask after a failure (PR 2 note 1) are cancelled with it —
     * and a fresh session, asked nothing, takes the one in force's place. The
     * next question starts there. Nothing to do when nothing was asked.
     */
    fun end() {
        val gone = synchronized(sessionLock) {
            val current = inForce.value
            if (current.ask == null && previews.isEmpty()) return
            generation += 1
            measured = null
            val stopped = previews.toList() + current
            previews.clear()
            inForce.value = newSession().start()
            stopped
        }
        gone.forEach { it.dispose() }
        VelaLog.event("$area.fee", "sessions ended", "sessions" to gone.size)
        reportQuotes()
    }

    /** The surface is gone: every session, and every watcher, with it. */
    fun dispose() {
        synchronized(sessionLock) {
            previews.forEach { it.dispose() }
            previews.clear()
            inForce.value.dispose()
        }
        speedHost.dispose()
        scope.cancel()
    }

    private companion object {
        /** Well above the core's own 15 s estimate timeout; a guard, not a policy. */
        const val QUOTE_TIMEOUT_MS = 30_000L
    }
}
