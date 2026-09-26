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
    private var askSeq = 0L
    private val previews = mutableListOf<FeeSession>()

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

        fun event(deployed: Boolean) = FeeEvent.QuoteRequested(
            chain_id = chainId,
            account = account,
            deployed = deployed,
            public_key_available = publicKeyAvailable,
            tier = tier,
            calls = calls,
            fee_token = feeToken,
            auto_fee_token = autoFeeToken,
        )
    }

    /** How a [quote] ended. */
    sealed class Quoted {
        /** Settled: a new estimate, or the core's failure. */
        data class Settled(val view: FeeView) : Quoted()

        /** A newer question took this one's place before it was asked. */
        data object Superseded : Quoted()

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
        @Volatile var ask: QuoteAsk? = null
        @Volatile var deployed: Boolean? = null
        /** The deployment read before the dispatch — as much "measuring" as the core's `busy`. */
        @Volatile var reading = false
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
            watch = scope.launch { host.commits.collect { reportQuotes() } }
            return this
        }

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
        answering = true
        try {
            val asked = askInForce(ask) ?: return Quoted.Superseded
            val before = asked.before
            // Woken by `commits`, NOT by `view`: a re-quote at the same price is
            // a view that `equals` the one before the request, and a StateFlow
            // never delivers that to a collector that missed the `busy` in
            // between (see `CoreHost.commits`).
            //
            // And only by a commit made AFTER this question was dispatched:
            // `commits` replays its current value to a new collector, and the
            // dispatch reaches the core through its inbox a moment later — so
            // a view still carrying the LAST attempt's `failed` satisfied the
            // wait at once, and a Max pressed after a failed warm-up was
            // answered with that stale failure in milliseconds (the full
            // balance, no fee held back) instead of with its own quote. A
            // promoted preview is another session and already settled.
            @OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
            val settled = withTimeoutOrNull(QUOTE_TIMEOUT_MS) {
                inForce
                    .flatMapLatest { s -> s.host.commits.map { commit -> s to commit } }
                    .first { (s, commit) ->
                        val view = s.view
                        (s !== asked.session || commit > asked.commitsAtDispatch) &&
                            !s.reading && !view.busy && ((view.fee != null && view.fee !== before) || view.failed != null)
                    }.first.view
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

    /**
     * The quote in force went stale (the policy's TTL): ask the same question
     * again, if one was asked and nothing is measuring. The fresh estimate
     * arrives on [fee].
     */
    fun requoteStale(): Boolean {
        val session = inForce.value
        val ask = session.ask ?: return false
        val deployed = session.deployed ?: return false
        if (session.reading || session.view.busy) return false
        VelaLog.event("$area.fee", "re-quote on stale", "chain" to ask.chainId)
        session.host.dispatch(ask.event(deployed), FeeEvent.serializer())
        return true
    }

    /** A question dispatched: to which session, the estimate it held before, and its commit count just before the dispatch. */
    private class Asked(val session: FeeSession, val before: FeeEstimateView?, val commitsAtDispatch: Long)

    /**
     * Price `ask` on the session in force: the deployment read first, then the
     * question. The ask is recorded NOW, before the read, so a preview of the
     * previous operation can never be promoted over a question still out.
     * Returns what was asked of whom, or `null` when a newer ask superseded
     * this one during the read.
     */
    private suspend fun askInForce(ask: QuoteAsk): Asked? {
        val (session, seq) = synchronized(sessionLock) {
            val session = inForce.value
            askSeq += 1
            generation += 1
            session.ask = ask
            session.deployed = null
            session.reading = true
            session.generation = generation
            session to askSeq
        }
        reportQuotes()
        val deployed = relay.isDeployed(ask.chainId, ask.account) ?: false
        val asked = synchronized(sessionLock) {
            if (seq != askSeq || inForce.value !== session) return null
            session.reading = false
            session.deployed = deployed
            val asked = Asked(session, session.view.fee, session.host.commits.value)
            session.host.dispatch(ask.event(deployed), FeeEvent.serializer())
            asked
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
            in_force = TierQuote(busy = main.reading || main.view.busy, fee = main.view.fee),
            previews = previews.mapNotNull { session ->
                val tier = session.ask?.tier ?: return@mapNotNull null
                TierPreviewQuote(tier = tier, busy = session.reading || session.view.busy, fee = session.view.fee)
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
                } else if (!main.reading && !main.view.busy && !answering) {
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
            ask != null && ask.tier == tier && ask.sameOperation(mainAsk) && !session.reading &&
                !session.view.busy && session.view.fee?.tier == tier
        }
        if (index < 0) return false
        val promoted = previews.removeAt(index)
        // A deployment read still out for the demoted session must not
        // dispatch onto the one now in force.
        askSeq += 1
        main.reading = false
        if (main.ask != null && main.deployed != null) previews.add(main) else main.dispose()
        inForce.value = promoted
        VelaLog.event("$area.speed", "promoted", "tier" to tier.name)
        return true
    }

    /**
     * Rule 2: a preview for every tier the core names, pricing the same
     * operation as the session in force at the same generation; every other
     * preview disposed. Nothing is created while the session in force is
     * still reading — its question is not settled enough to replay.
     */
    private fun syncPreviews(wanted: List<FeeTier>) {
        val main = inForce.value
        val base = main.ask?.takeIf { !main.reading && main.deployed != null }
        val deployed = main.deployed
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
        if (base == null || deployed == null) return
        for (tier in wanted) {
            if (previews.any { it.ask?.tier == tier }) continue
            val session = newSession()
            val ask = base.copy(tier = tier)
            session.ask = ask
            session.deployed = deployed
            session.generation = generation
            previews.add(session.start())
            session.host.dispatch(ask.event(deployed), FeeEvent.serializer())
        }
    }

    // -- intents ---------------------------------------------------------------------

    /**
     * A new operation: the one-shot pick, a free upgrade and the fold die
     * with the one before it (spec 068), and it starts at the stored default.
     */
    fun reset() {
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
     * and the other tiers are re-priced with it.
     */
    fun refresh() {
        val session = inForce.value
        val ask = session.ask ?: return
        relay.invalidateFeeSignals(ask.chainId)
        if (session.reading) return
        if (session.deployed == null) {
            // The last attempt never reached the core, so `Requote` would be a
            // no-op and the control a dead button: ask again for real.
            scope.launch { askInForce(ask) }
            return
        }
        synchronized(sessionLock) {
            generation += 1
            session.generation = generation
        }
        session.host.dispatch(FeeEvent.Requote, FeeEvent.serializer())
        speedPass()
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
