package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.SpeedControl
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
    receiptWaitMs: Long = 120_000L,
    receiptPollMs: Long = 3_000L,
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

    /** The tracker is handed the hash only once every record it names is on disk (043's ordering invariant). */
    private fun tryHandoff() {
        val handoff = synchronized(handoffLock) {
            val handoff = pendingHandoff ?: return
            if (!handoff.record_ids.all { it in persistedRecords }) return
            pendingHandoff = null
            handoff
        }
        ports.trackSubmitted(handoff.user_op_hash, handoff.record_ids, handoff.chain_id)
    }

    interface Ports : SignExecutor.Ports {
        /** The tracker follows the accepted operation to its verdict (043's `trackSubmitted`). */
        fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int)

        /** The descriptor endpoint base. */
        fun dataBase(): String

        /** `eth_call` through the pool: `(result, reverted)`. */
        suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean>

        /** Spec 046 US1: the simulated balance changes, judged by the trust machine; `null` = could not simulate. */
        suspend fun simulate(chainId: Int, wallet: String, calls: List<SimDeltas.Call>): List<TrustSimJudgment>? = null
    }

    /** What the simulation said (spec 046 US1): pending (`null`), the judgments, or unavailable. */
    sealed class SimOutcome {
        data class Ready(val judgments: List<TrustSimJudgment>) : SimOutcome()
        data object Unavailable : SimOutcome()
    }

    private val _sim = MutableStateFlow<SimOutcome?>(null)
    val sim: StateFlow<SimOutcome?> = _sim

    private val signExecutor = SignExecutor(
        spine = UserOpSpine(relay, accounts, signer, measureCall, signMethod = { signMethod.value }),
        relay = relay,
        feed = feed,
        ports = object : SignExecutor.Ports by ports {
            override fun opSubmitted(id: String, userOpHash: String) {
                ports.opSubmitted(id, userOpHash)
                dispatchSign(SignEvent.OpSubmitted(id = id, user_op_hash = userOpHash, now_ms = now()))
            }

            override fun recordPersisted(recordId: String) {
                persistedRecords += recordId
                ports.recordPersisted(recordId)
                tryHandoff()
            }

            override fun respond(transportId: String, id: String, json: org.json.JSONObject) {
                ports.respond(transportId, id, json)
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
    )
    private val clearExecutor = ClearExecutor(dataBase = { ports.dataBase() }, ethCall = { c, to, d -> ports.ethCall(c, to, d) })
    private val guardExecutor = GuardExecutor(ethCall = { c, to, d -> ports.ethCall(c, to, d).first })
    private val feeExecutor = FeeExecutor(relay = relay, keyHexes = { address -> accounts.keysOf(address).map { it.publicKeyHex } })

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
     * "Sign with": WHERE the passkey that signs this request is. This controller
     * lives for one request, so the choice cannot outlive the question it was
     * made for. `auto` is the wallet's stored route, untouched.
     */
    val signMethod = MutableStateFlow("auto")
    val signWithOpen = MutableStateFlow(false)

    /** `null` toggles the list; an id picks a method and closes it. */
    fun signWith(id: String?) {
        if (id == null) {
            signWithOpen.value = !signWithOpen.value
            return
        }
        if (id in setOf("auto", "platform", "hybrid", "security_key")) signMethod.value = id
        signWithOpen.value = false
    }

    /**
     * The fee row's coin list (the web's `feeOpen`, ef49b6b1). Which coins pay,
     * what each costs and which cannot are the fee machine's; the pick is a
     * quote PARAMETER the core re-prices the operation in, and the approve
     * carries the same view's `fee_token` (issue #262: an account holding USDT
     * and no ETH was quoted in ETH with no way to choose the coin it has).
     */
    val feeOpen = MutableStateFlow(false)

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

    private var handedOff = false

    private fun dispatchSign(event: SignEvent) = signHost.dispatch(event, SignEvent.serializer())

    fun open(request: IncomingRequest) {
        _request.value = request
        // Each request starts at the stored default: a pick is one-shot.
        speedControl.reset()
        dispatchSign(SignEvent.NetworksChanged(knownChains()))
        dispatchSign(SignEvent.AccountsChanged(signers, 0))
        dispatchSign(
            SignEvent.RequestArrived(
                id = request.id, method = request.method, params_json = request.paramsJson, origin = request.origin,
                transport_id = request.transportId, dedicated_transport = true, per_request_chain = request.chainId,
                dapp = null, granted_address = null, requested_address = null, request_ts_ms = null, now_ms = now(),
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
            speedControl.ask(request.chainId, wallet.address, publicKeyAvailable = true, calls = feeCalls, feeToken = null)
            // Spec 046 US1: the one block a site cannot author. Read only.
            scope.launch {
                val judged = runCatching { ports.simulate(request.chainId, wallet.address, calls.map { SimDeltas.Call(it.to, it.value, it.data) }) }
                    .onFailure { VelaLog.failure("signing.sim", "simulation failed", it) }
                    .getOrNull()
                _sim.value = judged?.let { SimOutcome.Ready(it) } ?: SimOutcome.Unavailable
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
        }
        // The controller's own scope (the container's is Main.immediate); no
        // dispatcher is forced so the JVM test can drive it too.
        scope.launch {
            signHost.view.collect { view ->
                // A free upgrade is decided only while the person can still
                // choose — never under a slide that has already gone.
                speedControl.stage(view.surface == SignSurface.Sheet && !view.is_signing && !view.is_submitting)
                view.tracker_handoff?.takeIf { !handedOff }?.let { handoff ->
                    handedOff = true
                    synchronized(handoffLock) { pendingHandoff = handoff }
                    tryHandoff()
                }
                if (view.surface == SignSurface.Hidden && view.request == null && _request.value != null && answered) _closed.value = true
            }
        }
    }

    @Volatile
    private var answered = false

    /** Called by the container's response port so the sheet closes only once the page has its answer. */
    fun markAnswered() { answered = true; if (sign.value.surface == SignSurface.Hidden) _closed.value = true }

    init {
        // One request, one controller: its fee sessions end with it.
        scope.launch { closed.first { it }; speedControl.dispose() }
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
    fun swipeDismissed() = dispatchSign(SignEvent.SwipeDismissed)
    fun fundingCancelled() = dispatchSign(SignEvent.FundingCancelled)
    fun guardPreset(mode: GuardEditorMode) = guardHost.dispatch(GuardEvent.PresetSelected(mode), GuardEvent.serializer())
    fun guardCustomAmount(text: String) = guardHost.dispatch(GuardEvent.CustomAmountChanged(text), GuardEvent.serializer())
    fun guardRevoke() = guardHost.dispatch(GuardEvent.RevokeChosen, GuardEvent.serializer())
    fun guardGrant() = guardHost.dispatch(GuardEvent.GrantDeliberatelyChosen, GuardEvent.serializer())

    companion object {
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
        )

        /** The first call of a request: `(to, data, value)` (the desktop's `first_call`). */
        fun firstCall(paramsJson: String): Triple<String?, String?, String?>? {
            val first = runCatching { JSONArray(paramsJson).optJSONObject(0) }.getOrNull() ?: return null
            val call = first.optJSONArray("calls")?.optJSONObject(0) ?: first
            fun field(name: String) = call.optString(name).ifBlank { null }
            return Triple(field("to"), field("data"), field("value"))
        }

        fun clearKickoff(method: String, paramsJson: String, chainId: Int, origin: String?): ClearSigningEvent? = when {
            method == "eth_sendTransaction" || method == "wallet_sendCalls" -> {
                val call = firstCall(paramsJson)
                ClearSigningEvent.ResolveTransaction(to = call?.first, data = call?.second, value = call?.third, chain_id = chainId, locale = ClearLocale.fromFormats(Formats.current))
            }
            method.contains("signTypedData") -> ClearSigningEvent.ResolveTypedData(typed_data_json = typedDataOf(paramsJson), chain_id = chainId, locale = ClearLocale.fromFormats(Formats.current))
            method == "personal_sign" || method == "eth_sign" -> ClearSigningEvent.MessagePresented(
                method = if (method == "eth_sign") ClearSignMethod.EthSign else ClearSignMethod.PersonalSign,
                params = stringParams(paramsJson),
                request_origin = origin,
            )
            else -> null
        }

        /** `eth_signTypedData_v4`'s payload: `[address, json]` — the JSON is the SECOND parameter. */
        fun typedDataOf(paramsJson: String): String {
            val params = runCatching { JSONArray(paramsJson) }.getOrNull() ?: return ""
            val second = params.opt(1) ?: return ""
            return if (second is String) second else second.toString()
        }

        fun stringParams(paramsJson: String): List<String> {
            val params = runCatching { JSONArray(paramsJson) }.getOrNull() ?: return emptyList()
            return (0 until params.length()).mapNotNull { params.opt(it) as? String }
        }
    }
}
