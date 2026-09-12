package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeEvent
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeOperation
import app.getvela.wallet.feature.send.core.FeeShellResult
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.ApprovalGuardCore
import uniffi.vela_core_uniffi.ClearSigningCore
import uniffi.vela_core_uniffi.FeePolicyCore
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
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
    receiptWaitMs: Long = 120_000L,
    receiptPollMs: Long = 3_000L,
) {
    private val persistedRecords = java.util.Collections.synchronizedSet(HashSet<String>())
    private var pendingHandoff: SignTrackerHandoff? = null

    /** The tracker is handed the hash only once every record it names is on disk (043's ordering invariant). */
    private fun tryHandoff() {
        val handoff = pendingHandoff ?: return
        if (handoff.record_ids.all { it in persistedRecords }) {
            pendingHandoff = null
            ports.trackSubmitted(handoff.user_op_hash, handoff.record_ids, handoff.chain_id)
        }
    }

    interface Ports : SignExecutor.Ports {
        /** The tracker follows the accepted operation to its verdict (043's `trackSubmitted`). */
        fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int)

        /** The descriptor endpoint base. */
        fun dataBase(): String

        /** `eth_call` through the pool: `(result, reverted)`. */
        suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean>
    }

    private val signExecutor = SignExecutor(
        spine = UserOpSpine(relay, accounts, signer),
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
    private val feeHost: CoreHost<FeeView> = CoreHost(
        bridge = FeePolicyCore().asBridge(), scope = scope, initial = FeeView(), serializer = FeeView.serializer(),
        perform = JsonShell.perform(FeeOperation.serializer(), FeeShellResult.serializer(), feeExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(FeeOperation.serializer(), FeeShellResult.serializer(), fallback = FeeShellResult.TtlElapsed, answer = feeExecutor::neutralAnswer),
        onFault = { error -> VelaLog.failure("sign.fee.fault", "core fault", error) },
    )

    val sign: StateFlow<SignView> = signHost.view
    val clear: StateFlow<ClearSigningView> = clearHost.view
    val guard: StateFlow<GuardView> = guardHost.view
    val fee: StateFlow<FeeView> = feeHost.view

    private val _request = MutableStateFlow<IncomingRequest?>(null)
    val request: StateFlow<IncomingRequest?> = _request

    private val _closed = MutableStateFlow(false)

    /** The request was answered: the sheet may go, and this controller with it. */
    val closed: StateFlow<Boolean> = _closed

    private var handedOff = false

    private fun dispatchSign(event: SignEvent) = signHost.dispatch(event, SignEvent.serializer())

    fun open(request: IncomingRequest) {
        _request.value = request
        dispatchSign(SignEvent.NetworksChanged(knownChains()))
        dispatchSign(SignEvent.AccountsChanged(listOf(wallet), 0))
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
            scope.launch {
                val deployed = relay.isDeployed(request.chainId, wallet.address) ?: return@launch
                feeHost.dispatch(
                    FeeEvent.QuoteRequested(
                        chain_id = request.chainId, account = wallet.address, deployed = deployed, public_key_available = true,
                        tier = FeeTier.Fast, calls = calls.map { FeeCall(to = it.to, value = it.value, data = it.data) }, fee_token = null,
                    ),
                    FeeEvent.serializer(),
                )
            }
        }
        // The controller's own scope (the container's is Main.immediate); no
        // dispatcher is forced so the JVM test can drive it too.
        scope.launch {
            signHost.view.collect { view ->
                view.tracker_handoff?.takeIf { !handedOff }?.let { handoff ->
                    handedOff = true
                    pendingHandoff = handoff
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

    fun approve() = dispatchSign(SignEvent.ApproveTapped(approveOpts(fee.value, clear.value, guard.value)))
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
            gas_fee_token = null,
            quoted_fee = fee.fee?.let { SignQuotedFee(amount = it.total_wei, recipient = it.fee_recipient.orEmpty()) },
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
                ClearSigningEvent.ResolveTransaction(to = call?.first, data = call?.second, value = call?.third, chain_id = chainId, locale = ClearLocale())
            }
            method.contains("signTypedData") -> ClearSigningEvent.ResolveTypedData(typed_data_json = typedDataOf(paramsJson), chain_id = chainId, locale = ClearLocale())
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
