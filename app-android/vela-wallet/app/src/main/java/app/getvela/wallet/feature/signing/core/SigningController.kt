package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.feature.send.core.TrustedSigner
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.SpeedControl
import app.getvela.wallet.feature.send.core.TrackHandoff
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.TrustAssetDelta
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.ApprovalGuardCore
import uniffi.vela_core_uniffi.ClearSigningCore
import uniffi.vela_core_uniffi.SignRequestCore

/** One request from a page, with the shell's facts attached. */
data class IncomingRequest(
    val id: String,
    val method: String,
    val paramsJson: String,
    val origin: String,
    /** The tab that asked; the answer goes there and nowhere else. */
    val transportId: String,
    val chainId: Int,
    /**
     * The address the site was shown (spec 070): the signer is pinned to it,
     * and `sign_request` refuses (4100) rather than sign as anyone else.
     * `null` for the wallet's own requests.
     */
    val grantedAddress: String? = null,
)

/**
 * The signing sheet's journey — four machines, one sheet (spec 044 T032;
 * the desktop's `wallet/signing_host.rs`).
 *
 * `sign_request` owns the request's life; `clear_signing` says what the
 * transaction DOES; `approval_guard` decides what an approval may be edited
 * to; `fee_policy` prices it. The sheet reads all four, which is why they
 * are born and die together here rather than as residents: a request that
 * ended must not leave a decoded intent or a half-edited allowance behind
 * for the next one to inherit.
 *
 * The machine has to know the world before it can judge a request against
 * it: without `NetworksChanged` and `AccountsChanged` first, it refuses
 * every transaction with 4902 (the desktop found this by running).
 */
class SigningController(
    private val scope: CoroutineScope,
    private val relay: RelayClient,
    feed: FeedExecutor,
    private val accounts: SendExecutor.AccountPort,
    signer: () -> UserOpSigner,
    private val knownChains: () -> List<Int>,
    /** The signing wallet: address and the pinned credential. */
    private val wallet: SignAccountRef,
    private val ports: Ports,
    /** `eth_estimateGas` for one inner call, from the Safe's address (`UserOpSpine.measureCall`). */
    measureCall: suspend (chainId: Int, from: String, to: String, valueHex: String, data: String) -> String? =
        { _, _, _, _, _ -> null },
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
    /**
     * The stored default speed (spec 069): a dApp transaction is priced — and,
     * through the quoted fee, submitted — at the speed Settings names, which
     * is `fast` for everybody who never chose, until the sheet's own speed
     * control picks another. The number preset writes each speed's gas bid.
     */
    private val preferredTier: () -> FeeTier = { FeeTier.Fast },
    private val numberPreset: () -> String = { "comma_dot" },
    /** `null`: the core's answer window (`dappReceiptWaitMs`, spec 082 RA12); tests pin a number. */
    receiptWaitMs: Long? = null,
    receiptPollMs: Long = 3_000L,
    /** Spec 071: the Trusted Signer, for an account that signed in through it. */
    trustedSigner: () -> TrustedSigner? = { null },
) {
    /** The machine's signer rows (`AccountsChanged`): the one wallet this request was opened for. */
    private val signers = listOf(wallet)

    private val persistedRecords = java.util.Collections.synchronizedSet(HashSet<String>())
    private var pendingHandoff: SignTrackerHandoff? = null

    /**
     * Guards [pendingHandoff]: the view collector sets it and the executor's
     * persist callback races it, so without the lock the tracker could be
     * handed the op twice, or never (a write the other thread doesn't see).
     */
    private val handoffLock = Any()

    /**
     * The tracker is handed the hash only once every record it names is on
     * disk (043's ordering invariant), and each hand-off only once.
     *
     * Spec 082 (review of RJ1): once per `(hash, ids, maybe_sent, admitted)`,
     * never per record. The write-ahead hands the op over "may have been
     * sent" before its POST; the relay taking it hands over the SAME hash and
     * ids again, `admitted`. Keyed by record alone, that second hand-off was
     * never fed — the tracker kept an accepted op in doubt, and two relay
     * `not_found` answers past the grace ended it "not sent" with its record
     * failed over money that landed.
     */
    private fun tryHandoff() {
        val handoff = synchronized(handoffLock) {
            val handoff = pendingHandoff ?: return
            if (!handoff.record_ids.all { it in persistedRecords }) return
            pendingHandoff = null
            if (!handedKeys.add(handoffKey(handoff))) return
            handedOps += handoff.user_op_hash.lowercase()
            handoff
        }
        VelaLog.event(
            "sign.tracker", "handed",
            "op" to handoff.user_op_hash.take(12), "maybeSent" to handoff.maybe_sent, "admitted" to handoff.admitted,
        )
        ports.trackSubmitted(
            TrackHandoff(
                userOpHash = handoff.user_op_hash,
                recordIds = handoff.record_ids,
                chainId = handoff.chain_id,
                maybeSent = handoff.maybe_sent,
                submitBlock = handoff.submit_block,
                admitted = handoff.admitted,
            ),
        )
        followTracker()
    }

    /** One hand-off, as fed: the op, its records, and the two flags. */
    private data class HandoffKey(val op: String, val ids: List<String>, val maybeSent: Boolean, val admitted: Boolean)

    private fun handoffKey(handoff: SignTrackerHandoff) =
        HandoffKey(handoff.user_op_hash.lowercase(), handoff.record_ids.sorted(), handoff.maybe_sent, handoff.admitted)

    /** The hand-offs this request has fed the tracker (guarded by [handoffLock]). */
    private val handedKeys = HashSet<HandoffKey>()

    /** The ops this request handed the tracker — whose entries it forwards as `OpTracked` (guarded by [handoffLock]). */
    private val handedOps = HashSet<String>()

    /** The withdrawals already fed to the tracker, once per value (guarded by [handoffLock]). */
    private val withdrawn = HashSet<SignTrackerWithdraw>()

    /** The last `(status, tx hash)` forwarded per op (guarded by [handoffLock]). */
    private val forwarded = HashMap<String, Pair<app.getvela.wallet.feature.send.core.TrackStatus, String?>>()

    @Volatile
    private var following: kotlinx.coroutines.Job? = null

    /**
     * Spec 082 RJ4: the answer follows the tracker. Every change of the
     * tracker's entry for an op this request handed over reaches the core as
     * `OpTracked`; the core decides whether it answers the page (a terminal
     * verdict past `OpSubmitted`) or waits — never this file.
     */
    private fun followTracker() {
        val tracker = ports.trackerView() ?: return
        synchronized(handoffLock) {
            if (following != null) return
            following = scope.launch {
                tracker.collect { view ->
                    val changed = synchronized(handoffLock) {
                        view.entries.filter { entry ->
                            val op = entry.user_op_hash.lowercase()
                            op in handedOps && forwarded.put(op, entry.status to entry.tx_hash) != (entry.status to entry.tx_hash)
                        }
                    }
                    changed.forEach { entry ->
                        dispatchSign(SignEvent.OpTracked(entry.user_op_hash, entry.status, entry.tx_hash, now()))
                    }
                }
            }
        }
    }

    /**
     * Spec 082 RJ4 (review of T246): the tracker holds the op from the
     * write-ahead on, so it can see the op land while the POST is still out (a
     * slow relay). That entry reached the core as `OpTracked` before the core
     * held the op (`OpSubmitted`) and was dropped there — and a final entry
     * does not change again, so [followTracker] never sent it twice: the page
     * waited out its whole window for the op hash. Once the core holds the op,
     * a landing already known is handed over once more. Only a landing (a tx
     * hash, a chain fact): a "not sent" judged while the POST was still out
     * came before the POST finished, and the tracker's own later word decides.
     */
    private fun forwardLanding(userOpHash: String) {
        val op = userOpHash.lowercase()
        val entry = ports.trackerView()?.value?.entries
            ?.firstOrNull { it.user_op_hash.equals(op, ignoreCase = true) }
            ?.takeIf { !it.tx_hash.isNullOrBlank() }
            ?: return
        val handed = synchronized(handoffLock) {
            (op in handedOps).also { if (it) forwarded[op] = entry.status to entry.tx_hash }
        }
        if (!handed) return
        VelaLog.event("sign.tracker", "landing seen during the POST: forwarded again", "op" to op.take(12), "status" to entry.status)
        dispatchSign(SignEvent.OpTracked(entry.user_op_hash, entry.status, entry.tx_hash, now()))
    }

    /** Spec 082 RJ1: a write-ahead op proven never sent — the tracker forgets it, once per value. */
    private fun feedWithdraw(withdraw: SignTrackerWithdraw) {
        val fresh = synchronized(handoffLock) { withdrawn.add(withdraw) }
        if (!fresh) return
        VelaLog.event("sign.tracker", "withdrawn: never sent", "op" to withdraw.user_op_hash.take(12))
        ports.trackWithdrawn(withdraw.user_op_hash, withdraw.record_ids)
    }

    interface Ports : SignExecutor.Ports {
        /** The tracker follows the operation to its verdict (043's `trackSubmitted`; 082's flags ride along). */
        fun trackSubmitted(handoff: TrackHandoff)

        /** Spec 082 RJ1: the tracker's `Withdrawn` — a written-ahead op proven never sent. */
        fun trackWithdrawn(userOpHash: String, recordIds: List<String>) {}

        /** Spec 082 RJ4: the tracker's entries, which this request forwards to the core as `OpTracked`; `null` = none. */
        fun trackerView(): StateFlow<app.getvela.wallet.feature.send.core.TrackView>? = null

        /** The descriptor endpoint base. */
        fun dataBase(): String

        /** `eth_call` through the pool: `(result, reverted)`. */
        suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean>

        /**
         * Spec 046 US1 / 082 RG6: `eth_simulateV1` with these params, through
         * the pool — its answer as it came. `null` = this host has no
         * simulator (nothing is drawn). What the answer MEANS is the core's
         * (`simOutcome`), never the port's.
         */
        suspend fun simulate(chainId: Int, params: List<Any?>): RpcResult? = null

        /** The core's deltas, judged by the trust machine; `null` = could not judge. */
        suspend fun judgeDeltas(chainId: Int, wallet: String, deltas: List<TrustAssetDelta>): List<TrustSimJudgment>? = null
    }

    /**
     * What the simulation said (spec 046 US1, 082 RG6): pending (`null`), the
     * checked moves ([Ready]; empty = nothing of theirs moves), or the core's
     * notice — a revert (danger) or "could not check" (caution).
     */
    sealed class SimOutcome {
        data class Ready(val judgments: List<TrustSimJudgment>) : SimOutcome()
        data class Notice(val risk: ClearRisk, val key: String, val reason: String? = null) : SimOutcome()
    }

    private val _sim = MutableStateFlow<SimOutcome?>(null)
    val sim: StateFlow<SimOutcome?> = _sim

    private val spine = UserOpSpine(relay, accounts, signer, measureCall, trustedSigner = trustedSigner)

    /**
     * Spec 079 (owner: one slide, not two): this account signs through the
     * Trusted Signer's page, whose own slide is the consent — so the sheet
     * offers a button that goes there instead of a second slide. Read once per
     * request from the same route the spine will sign over.
     */
    private val _trustedSignerRoute = MutableStateFlow(false)
    val trustedSignerRoute: StateFlow<Boolean> = _trustedSignerRoute

    private val signExecutor = SignExecutor(
        spine = spine,
        relay = relay,
        feed = feed,
        ports = object : SignExecutor.Ports by ports {
            override fun opSubmitted(id: String, submitted: UserOpSpine.Submitted) {
                ports.opSubmitted(id, submitted)
                dispatchSign(
                    SignEvent.OpSubmitted(
                        id = id,
                        user_op_hash = submitted.userOpHash,
                        now_ms = now(),
                        maybe_sent = submitted.maybeSent,
                        submit_block = submitted.submitBlock,
                    ),
                )
                // The core holds the op now: a landing the tracker saw while
                // the POST was out is handed over again (082 review of T246).
                forwardLanding(submitted.userOpHash)
            }

            // Spec 082 RJ1: signed and hashed, nothing posted — the core writes
            // the record ahead and clears the POST once it is on disk.
            override fun opSigned(id: String, userOpHash: String, submitBlock: Long?) {
                ports.opSigned(id, userOpHash, submitBlock)
                dispatchSign(SignEvent.OpSigned(id = id, user_op_hash = userOpHash, submit_block = submitBlock, now_ms = now()))
            }

            // Spec 082 RA9: the sheet's words follow the ceremony — "preparing"
            // until the passkey is up, "waiting for your signature" while it
            // is, "submitting" after. The core guards them by request id.
            override fun ceremonyStarted(id: String) {
                ports.ceremonyStarted(id)
                dispatchSign(SignEvent.CeremonyStarted(id))
            }

            override fun ceremonyDone(id: String) {
                ports.ceremonyDone(id)
                dispatchSign(SignEvent.CeremonyDone(id))
            }

            override fun askerGone(): Boolean = askerLeft

            override fun recordPersisted(recordId: String) {
                persistedRecords += recordId
                ports.recordPersisted(recordId)
                tryHandoff()
            }

            override fun respond(transportId: String, id: String, payload: SignResponsePayload) {
                ports.respond(transportId, id, payload)
                // The page has its answer: once the core has cleared the sheet
                // this controller may go.
                markAnswered()
            }

            // The rows the machine was given, and the request it is judging:
            // what `SwitchActiveAccount` is checked against.
            override fun signerAt(index: Int): String? = signers.getOrNull(index)?.address

            override fun intendedSigner(): String? = signHost.view.value.request?.signer_address
        },
        now = now,
        receiptWaitMs = receiptWaitMs,
        receiptPollMs = receiptPollMs,
        origin = { _request.value?.origin.orEmpty() },
        // A page in this app's browser — never the wallet's own requests (the key backup).
        originSeenByBrowser = { _request.value?.let { it.transportId != app.getvela.wallet.feature.signing.SigningLive.WALLET_TRANSPORT } ?: false },
    )
    private val clearExecutor = ClearExecutor(dataBase = { ports.dataBase() }, ethCall = { c, to, d -> ports.ethCall(c, to, d) })
    private val guardExecutor = GuardExecutor(ethCall = { c, to, d -> ports.ethCall(c, to, d).first })
    private val feeExecutor = FeeExecutor(relay = relay, keyHexes = { address -> accounts.keysOf(address).map { it.publicKeyHex } }, measureCall = measureCall)

    private val signHost: CoreHost<SignView> = CoreHost(
        bridge = SignRequestCore().asBridge(), scope = scope, initial = SignView(), serializer = SignView.serializer(),
        perform = JsonShell.perform(SignOperation.serializer(), SignShellResult.serializer(), signExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(SignOperation.serializer(), SignShellResult.serializer(), fallback = SignShellResult.Responded, answer = signExecutor::neutralAnswer),
        onFault = { error -> VelaLog.failure("sign.fault", "core fault", error) },
    )
    private val clearHost: CoreHost<ClearSigningView> = CoreHost(
        bridge = ClearSigningCore().asBridge(), scope = scope, initial = ClearSigningView(), serializer = ClearSigningView.serializer(),
        perform = JsonShell.perform(ClearOperation.serializer(), ClearShellResult.serializer(), clearExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(ClearOperation.serializer(), ClearShellResult.serializer(), fallback = ClearShellResult.Clock(0.0), answer = clearExecutor::neutralAnswer),
        onFault = { error -> VelaLog.failure("sign.clear.fault", "core fault", error) },
    )
    private val guardHost: CoreHost<GuardView> = CoreHost(
        bridge = ApprovalGuardCore().asBridge(), scope = scope, initial = GuardView(), serializer = GuardView.serializer(),
        perform = JsonShell.perform(GuardOperation.serializer(), GuardShellResult.serializer(), guardExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(GuardOperation.serializer(), GuardShellResult.serializer(), fallback = GuardShellResult.MetaResolved(null), answer = guardExecutor::neutralAnswer),
        onFault = { error -> VelaLog.failure("sign.guard.fault", "core fault", error) },
    )
    /**
     * The fee sessions and the speed control (spec 069) — the very class the
     * send form runs, so the sheet's speeds, previews, free upgrade and
     * "the price you tap is the price you get" cannot drift from the form's.
     */
    private val speedControl = SpeedControl(scope, relay, feeExecutor, preferredTier, numberPreset, area = "sign").start()

    val sign: StateFlow<SignView> = signHost.view
    val clear: StateFlow<ClearSigningView> = clearHost.view
    val guard: StateFlow<GuardView> = guardHost.view
    /** The fee in force — whichever session prices the tier in force right now. */
    val fee: StateFlow<FeeView> = speedControl.fee

    /** The speed control, as the core decided it — the sheet's fee card draws this. */
    val speed: StateFlow<FeeSpeedView> = speedControl.speed

    /** The fee view of the session pricing `tier` — for formatting that option's fee. */
    fun feeViewOf(tier: FeeTier): FeeView? = speedControl.feeViewOf(tier)

    private val _request = MutableStateFlow<IncomingRequest?>(null)
    val request: StateFlow<IncomingRequest?> = _request

    /**
     * The fee row's coin list (the web's `feeOpen`, ef49b6b1). Which coins pay,
     * what each costs and which cannot are the fee machine's; the pick is a
     * quote PARAMETER the core re-prices the operation in, and the approve
     * carries the same view's `fee_token` (issue #262: an account holding USDT
     * and no ETH was quoted in ETH with no way to choose the coin it has).
     */
    val feeOpen = MutableStateFlow(false)

    @Volatile
    private var requoting: kotlinx.coroutines.Job? = null

    /**
     * The automatic re-quotes after [first] (spec 079 FR-008, spec 082 RJ12):
     * the core's wait before each (`feeRequoteDelayMs`), each re-ask bounded
     * by `feeRequoteTimeoutMs()` — one that has not answered by then is a
     * failure again, and the schedule goes on (the next ask abandons it).
     * Ends when the fee is back, when a failure no retry fixes is reached, or
     * once the sheet is approved or closed. Every step is a `fee:` line.
     */
    private suspend fun requoteLoop(chainId: Int, first: app.getvela.wallet.feature.send.core.FeeFailure) {
        val timeoutMs = uniffi.vela_core_uniffi.feeRequoteTimeoutMs().toLong()
        var failure = first
        var cause = causeOf(first)
        var attempt = 0
        while (true) {
            attempt += 1
            val wait = uniffi.vela_core_uniffi.feeRequoteDelayMs(failure.wire, attempt.toUInt())?.toLong()
            if (wait == null) {
                VelaLog.event("fee", "quote failed chain=$chainId cause=$cause: no re-quote fixes it")
                return
            }
            VelaLog.event("fee", "quote failed chain=$chainId cause=$cause re-quote #$attempt in $wait ms")
            kotlinx.coroutines.delay(wait)
            if (!mayRequote(signHost.view.value, answered)) return
            // A tap on the row (or a coin picked) asked meanwhile, and it came back.
            if (fee.value.failed == null && fee.value.fee != null) {
                VelaLog.event("fee", "quote back chain=$chainId after ${attempt - 1} re-quotes")
                return
            }
            val started = System.currentTimeMillis()
            when (val quoted = speedControl.requote(timeoutMs)) {
                is SpeedControl.Quoted.Settled -> {
                    val next = quoted.view.failed
                    if (next == null) {
                        VelaLog.event("fee", "quote back chain=$chainId after $attempt re-quotes", "ms" to (System.currentTimeMillis() - started))
                        return
                    }
                    failure = next
                    cause = causeOf(next)
                }
                // Not answered in time: a failure again, of the same kind the
                // schedule was following.
                SpeedControl.Quoted.TimedOut -> cause = "timeout"
                // Somebody asked another question (a speed, a coin): its own answer drives the row.
                SpeedControl.Quoted.Superseded -> return
            }
        }
    }

    /** A failure as the `fee:` lines name it: the core's word, and whether a chain read was rate-limited. */
    private fun causeOf(failure: app.getvela.wallet.feature.send.core.FeeFailure): String = when (failure) {
        is app.getvela.wallet.feature.send.core.FeeFailure.ChainRead -> if (failure.rate_limited) "chain_read(rate_limited)" else "chain_read"
        else -> failure.name
    }

    /** A tap on the fee row: a failed quote is asked again; with more than one coin, the list opens or closes. */
    fun feeTapped() {
        val view = fee.value
        when {
            // Measured again for real — the held readings dropped first.
            view.failed != null -> speedControl.refresh()
            view.options.size > 1 -> feeOpen.value = !feeOpen.value
        }
    }

    /**
     * A coin from the list (`null` = the native coin); a coin that cannot pay
     * is refused by the core. Every speed is re-priced in it, so a speed
     * picked next is still paid in the coin chosen (spec 069).
     */
    fun pickFee(contract: String?) {
        speedControl.chooseFeeToken(contract)
        feeOpen.value = false
    }

    private val _closed = MutableStateFlow(false)

    /** The request was answered: the sheet may go, and this controller with it. */
    val closed: StateFlow<Boolean> = _closed


    private fun dispatchSign(event: SignEvent) = signHost.dispatch(event, SignEvent.serializer())

    fun open(request: IncomingRequest) {
        _request.value = request
        scope.launch {
            val first = runCatching { accounts.keysOf(wallet.address) }.getOrNull()?.firstOrNull() ?: return@launch
            _trustedSignerRoute.value = runCatching { spine.routeFor(wallet.address, first).method == app.getvela.wallet.feature.onboarding.core.KeyMethod.TrustedSigner }.getOrDefault(false)
        }
        // Each request starts at the stored default: a pick is one-shot.
        speedControl.reset()
        dispatchSign(SignEvent.NetworksChanged(knownChains()))
        dispatchSign(SignEvent.AccountsChanged(signers, 0))
        dispatchSign(
            SignEvent.RequestArrived(
                id = request.id, method = request.method, params_json = request.paramsJson, origin = request.origin,
                transport_id = request.transportId, dedicated_transport = true, per_request_chain = request.chainId,
                dapp = null, granted_address = request.grantedAddress, requested_address = null, request_ts_ms = null, now_ms = now(),
            ),
        )
        // What it does. A transaction decodes from its call; typed data and a
        // plain message are their own rungs of the same ladder. The BROWSER's
        // fact about who is asking, never the page's claim.
        clearKickoff(request.method, request.paramsJson, request.chainId, request.origin.ifBlank { null })?.let { clearHost.dispatch(it, ClearSigningEvent.serializer()) }
        guardHost.dispatch(
            GuardEvent.ApprovalDetected(method = request.method, params_json = request.paramsJson, chain_id = request.chainId, wallet_address = wallet.address, read_only = false, now_ms = now()),
            GuardEvent.serializer(),
        )
        SignExecutor.callsOf(request.method, request.paramsJson)?.let { calls ->
            val feeCalls = calls.map { FeeCall(to = it.to, value = it.value, data = it.data) }
            // Nobody has chosen the fee coin for this request yet: the fee
            // machine pays in one that can, and the approve carries the view's
            // `fee_token` — the coin it picked — exactly as it carries a tap
            // (`approveOpts`). A chip tap ends auto (`SpeedControl.chooseFeeToken`).
            speedControl.ask(request.chainId, wallet.address, publicKeyAvailable = true, calls = feeCalls, feeToken = null, autoFeeToken = true)
            // Spec 046 US1: the one block a site cannot author. Read only.
            scope.launch {
                _sim.value = simulated(request.chainId, calls.map { SimDeltas.Call(it.to, it.value, it.data) }) ?: return@launch
            }
            // A quote goes stale while the person reads (the policy's TTL);
            // while the sheet is still up and nothing is signing, ask again —
            // otherwise the slide stays shut with no way to open it.
            scope.launch {
                fee.collect { fee ->
                    val view = signHost.view.value
                    if (fee.stale && !fee.busy && view.surface == SignSurface.Sheet && !view.is_signing && !view.is_submitting && !answered) {
                        speedControl.requoteStale()
                    }
                }
            }
            // Spec 079: a quote that failed for a reason that can pass (the
            // relay unreachable, a busy estimate, the chain's node) is asked
            // again on the core's schedule while the sheet is up and nothing
            // is signing. On the Xiaomi the row said "点击重试" with the relay
            // down and stayed that way after it came back. Spec 082 RJ12: 3 s,
            // 6 s, then every 8 s, each re-ask bounded by the core's
            // `feeRequoteTimeoutMs()` — the fee is back within 14 s of the
            // relay returning — and every step on the log.
            scope.launch {
                // The sheet's own state too: a failure that landed before the
                // sheet was up is asked about once it is.
                kotlinx.coroutines.flow.combine(fee, signHost.view) { fee, view -> fee to view }.collect { (fee, view) ->
                    if (fee.failed == null || fee.busy || requoting?.isActive == true) return@collect
                    if (!mayRequote(view, answered)) return@collect
                    // A failure no retry fixes (a missing key, a calculation): the row says so once.
                    if (uniffi.vela_core_uniffi.feeRequoteDelayMs(fee.failed.wire, 1u) == null) return@collect
                    requoting = scope.launch { requoteLoop(request.chainId, fee.failed) }
                }
            }
        }
        // The controller's own scope (the container's is Main.immediate); no
        // dispatcher is forced so the JVM test can drive it too.
        scope.launch {
            signHost.view.collect { view ->
                // A free upgrade is decided only while the person can still
                // choose — never under a slide that has already gone.
                speedControl.stage(view.surface == SignSurface.Sheet && !view.is_signing && !view.is_submitting)
                view.tracker_handoff?.let { handoff ->
                    val fresh = synchronized(handoffLock) {
                        (handoffKey(handoff) !in handedKeys).also { if (it) pendingHandoff = handoff }
                    }
                    if (fresh) tryHandoff()
                }
                view.tracker_withdraw?.let(::feedWithdraw)
                if (view.surface == SignSurface.Hidden && view.request == null && _request.value != null && answered) _closed.value = true
            }
        }
    }

    /**
     * The simulation, read by the core (spec 082 RG6): the pool's answer goes
     * to `simOutcome` as it came, and only a `deltas` verdict is judged by the
     * trust machine. `null` when this host has no simulator.
     */
    private suspend fun simulated(chainId: Int, calls: List<SimDeltas.Call>): SimOutcome? {
        val params = SimDeltas.payload(wallet.address, calls)?.let { array -> (0 until array.length()).map(array::get) }
        val answer = if (params == null) {
            null
        } else {
            runCatching { ports.simulate(chainId, params) }
                .onFailure { VelaLog.failure("signing.sim", "simulation failed", it) }
                .getOrElse { RpcResult.Failed(rateLimited = false) }
                ?: return null
        }
        val record = SimDeltas.outcome(wallet.address, answer)
        VelaLog.event("signing.sim", "outcome", "chain" to chainId, "kind" to record.kind)
        val notice = SimDeltas.notice(record)
        if (notice != null) return notice
        // Moves this build cannot read are not "nothing moves".
        val deltas = SimDeltas.deltas(record) ?: return SimDeltas.couldNotCheck()
        val judged = runCatching { ports.judgeDeltas(chainId, wallet.address, deltas) }
            .onFailure { VelaLog.failure("signing.sim", "the trust machine could not judge the deltas", it) }
            .getOrNull()
        // Checked, but nothing could say what the moves are: "could not check".
        return judged?.let { SimOutcome.Ready(it) } ?: SimDeltas.couldNotCheck()
    }

    @Volatile
    private var answered = false

    /** The page that asked is gone ([cancel]). */
    @Volatile
    private var askerLeft = false

    /** Called by the container's response port so the sheet closes only once the page has its answer. */
    fun markAnswered() { answered = true; if (sign.value.surface == SignSurface.Hidden) _closed.value = true }

    /**
     * The page that asked is gone (spec 070): it already holds its 4900 from
     * the browser core. The sheet closes without answering anybody; an
     * operation already at the relay keeps its record and its tracker.
     */
    fun cancel() {
        // Spec 082 RB2: from here nothing is signed or sent for this request
        // — the executor asks before the passkey and before the relay POST.
        askerLeft = true
        _request.value?.let { dispatchSign(SignEvent.TransportDropped(it.transportId)) }
        answered = true
        _closed.value = true
    }

    init {
        // One request, one controller: its fee sessions end with it, and so
        // do its ear on the tracker and its re-quotes.
        scope.launch { closed.first { it }; requoting?.cancel(); speedControl.dispose(); following?.cancel() }
    }

    fun approve() = dispatchSign(SignEvent.ApproveTapped(approveOpts(fee.value, clear.value, guard.value)))

    // -- the speed control (spec 069) -----------------------------------------------

    /** The stored default or the number preset moved: this sheet follows until a speed is picked on it. */
    fun preferenceChanged() = speedControl.preferenceChanged()

    /** Fold or unfold the control. */
    fun toggleSpeed() = speedControl.toggle()

    /** A tap on an option — one-shot, never the stored preference. */
    fun pickSpeed(tier: FeeTier) = speedControl.pick(tier)

    /** The refresh control: measure again, the held readings dropped first (issue 212). */
    fun refreshFee() = speedControl.refresh()
    fun reject() = dispatchSign(SignEvent.RejectTapped)
    fun dismiss() = dispatchSign(SignEvent.DismissTapped)
    fun swipeDismissed() {
        val now = sign.value
        if (now.is_signing || now.is_submitting || now.pending_op_hash != null) closedAfterApproval = true
        dispatchSign(SignEvent.SwipeDismissed)
    }

    /**
     * Spec 079: the person closed the sheet after approving. The operation goes
     * on and the page still gets its answer, but the ending does not come back
     * over whatever they went to (the iOS behaviour, owner-aligned 2026-09-28).
     */
    @Volatile
    var closedAfterApproval = false
        private set
    fun fundingCancelled() = dispatchSign(SignEvent.FundingCancelled)
    fun guardPreset(mode: GuardEditorMode) = guardHost.dispatch(GuardEvent.PresetSelected(mode), GuardEvent.serializer())
    fun guardCustomAmount(text: String) = guardHost.dispatch(GuardEvent.CustomAmountChanged(text), GuardEvent.serializer())
    /** One batch leg's chip / field — the core ignores the single-approval events on a batch. */
    fun guardLegPreset(index: Int, mode: GuardEditorMode) = guardHost.dispatch(GuardEvent.LegPresetSelected(index, mode), GuardEvent.serializer())
    fun guardLegCustomAmount(index: Int, text: String) = guardHost.dispatch(GuardEvent.LegCustomAmountChanged(index, text), GuardEvent.serializer())
    fun guardRevoke() = guardHost.dispatch(GuardEvent.RevokeChosen, GuardEvent.serializer())
    fun guardGrant() = guardHost.dispatch(GuardEvent.GrantDeliberatelyChosen, GuardEvent.serializer())

    companion object {
        /**
         * Spec 079: a failed quote is asked again only while the sheet is up and
         * nothing has been approved — not signing, not submitting, not submitted,
         * and the page not yet answered.
         */
        fun mayRequote(view: SignView, answered: Boolean): Boolean =
            view.surface == SignSurface.Sheet && !view.is_signing && !view.is_submitting && view.pending_op_hash == null && !answered

        /** What the confirm slides into: the fee as quoted, the guard's rewrite, the intent (the desktop's `approve_opts`). */
        fun approveOpts(fee: FeeView, clear: ClearSigningView, guard: GuardView): SignApproveOpts = SignApproveOpts(
            max_fee_per_gas = fee.fee?.max_fee_per_gas,
            bundler_cost_wei = null,
            // The coin the person picked pays, and the amount signed is in THAT
            // coin — the send core's own rule (`submit_user_op`): an ERC-20 fee's
            // amount rides in `fee_asset`, `total_wei` is 0 for it.
            gas_fee_token = fee.fee_token,
            quoted_fee = fee.fee?.let {
                val amount = when (val asset = it.fee_asset) {
                    is FeeAssetView.Erc20 -> asset.amount
                    FeeAssetView.Native -> it.total_wei
                }
                // …with the speed this very estimate was priced at, named on the wire
                // beside the amount (spec 069); the core drops a `rapid`.
                SignQuotedFee(amount = amount, recipient = it.fee_recipient.orEmpty(), tier = it.tier)
            },
            fee_collector = null,
            params_override_json = guard.rewritten_params_json,
            intent = clear.result?.intent,
            // The guard showed an unbounded amount and it was kept as the site
            // asked — the submit guard's only waiver, copied, never decided.
            unlimited_approved = guard.unlimited_consented,
        )

        /** The first call of a request: `(to, data, value)` (the desktop's `first_call`). */
        fun firstCall(paramsJson: String): Triple<String?, String?, String?>? {
            val first = runCatching { JSONArray(paramsJson).optJSONObject(0) }.getOrNull() ?: return null
            val call = first.optJSONArray("calls")?.optJSONObject(0) ?: first
            // A JSON null is absent — `optString` would read it as the text
            // "null". A present non-string (a number) goes as its text, so the
            // core refuses it rather than reading a calm "0" (spec 082 RC6).
            fun field(name: String) = if (call.isNull(name)) null else call.optString(name).ifBlank { null }
            return Triple(field("to"), field("data"), field("value"))
        }

        fun clearKickoff(method: String, paramsJson: String, chainId: Int, origin: String?): ClearSigningEvent? = when {
            method == "eth_sendTransaction" || method == "wallet_sendCalls" -> {
                val call = firstCall(paramsJson)
                ClearSigningEvent.ResolveTransaction(to = call?.first, data = call?.second, value = call?.third, chain_id = chainId, locale = ClearLocale.fromFormats(Formats.current))
            }
            method.contains("signTypedData") -> ClearSigningEvent.ResolveTypedData(typed_data_json = typedDataOf(method, paramsJson), chain_id = chainId, locale = ClearLocale.fromFormats(Formats.current))
            method == "personal_sign" || method == "eth_sign" -> ClearSigningEvent.MessagePresented(
                method = if (method == "eth_sign") ClearSignMethod.EthSign else ClearSignMethod.PersonalSign,
                params = stringParams(paramsJson),
                request_origin = origin,
            )
            else -> null
        }

        /**
         * The ONE document the request is read as — the core's
         * `typedDataDocument`, the same bytes the passkey's digest covers. The
         * audit of 2026-10-01: reading `params[1]` here previewed the benign
         * half of a legacy `[malicious, benign]` while the passkey signed the
         * malicious one. Empty when the request is not a valid typed-data
         * request — the core refuses it before a sheet.
         */
        fun typedDataOf(method: String, paramsJson: String): String =
            uniffi.vela_core_uniffi.typedDataDocument(method, paramsJson) ?: ""

        fun stringParams(paramsJson: String): List<String> {
            val params = runCatching { JSONArray(paramsJson) }.getOrNull() ?: return emptyList()
            return (0 until params.length()).mapNotNull { params.opt(it) as? String }
        }
    }
}
