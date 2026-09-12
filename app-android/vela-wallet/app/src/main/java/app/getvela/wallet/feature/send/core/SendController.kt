package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcPool
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.FeePolicyCore
import uniffi.vela_core_uniffi.SendCore
import uniffi.vela_core_uniffi.WalletKeyRecord

/**
 * The send path's host (spec 043 T028): the `send` machine and its `fee_policy`
 * session, the two seams between them, and the intents the screens raise.
 *
 * One attempt per `open`: the core's `Open` resets the machine, so the hosts
 * outlive the flow the way every other host in this app does. The controller
 * holds no step of its own — the stage, the validation, the lock and the
 * receipt are all the core's view (`data-model.md`).
 *
 * Two bridges between machines live here because both other shells keep them
 * in the shell (research D7): `estimate_fee` is answered by the live fee
 * session, and the fee session's later estimates flow back into the send
 * machine as `FeeUpdated` / `FeeBusyChanged`.
 */
class SendController(
    private val scope: CoroutineScope,
    relay: RelayClient,
    pool: RpcPool,
    feed: FeedExecutor,
    accountStore: AccountStore,
    balances: () -> BalanceView,
    networks: () -> NetView,
    signer: () -> UserOpSigner,
    haptic: (SendHapticKind) -> Unit,
    refreshBalances: () -> Unit,
    /** The feed re-reads the store; the wallet controller binds it. */
    feedChanged: () -> Unit = {},
    /** The tracker handoff; the wallet controller binds it (phase 4). */
    var onTrackSubmitted: (userOpHash: String, recordIds: List<String>, chainId: Int) -> Unit = { hash, _, _ ->
        VelaLog.event("send.track", "no tracker bound", "hash" to hash.take(12))
    },
) {

    private val _alert = MutableStateFlow<SendAlertKind?>(null)

    /** The core's refusal to print, until the screen dismisses it. */
    val alert: StateFlow<SendAlertKind?> = _alert

    private val _closed = MutableStateFlow(false)

    /** The core said `Close`: the flow host pops the send. */
    val closed: StateFlow<Boolean> = _closed

    private val relayRef = relay
    private val hapticPort: (SendHapticKind) -> Unit = haptic
    private val refreshPort: () -> Unit = refreshBalances
    private val feedChangedPort: () -> Unit = feedChanged
    private val accountPort: StoreAccountPort = StoreAccountPort(accountStore)

    private val feeExecutor = FeeExecutor(
        relay = relay,
        keyHexes = { address: String -> accountPort.keysOf(address).map { it.publicKeyHex } },
    )

    private val feeHost = CoreHost(
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
        onFault = { error -> VelaLog.failure("send.fee.fault", "core fault", error) },
    )

    private val ports: SendExecutor.SendPorts = object : SendExecutor.SendPorts {
        override fun signingStarted() {
            dispatch(SendEvent.SigningStarted)
        }

        override fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int) {
            onTrackSubmitted(userOpHash, recordIds, chainId)
        }

        override fun haptic(kind: SendHapticKind) {
            hapticPort(kind)
        }

        override fun alert(kind: SendAlertKind) {
            _alert.value = kind
        }

        override fun closed() {
            _closed.value = true
        }

        override fun refreshBalances() {
            refreshPort()
        }

        override fun recordsPersisted() {
            feedChangedPort()
        }
    }

    private val sendExecutor = SendExecutor(
        relay = relay,
        pool = pool,
        feed = feed,
        accounts = accountPort,
        balances = balances,
        networks = networks,
        signer = signer,
        feeQuoter = object : SendExecutor.FeeQuoter {
            override suspend fun quote(
                chainId: Int,
                account: String,
                calls: List<FeeCall>,
                gasFeeToken: String?,
                publicKeyAvailable: Boolean,
            ): SendFeeOutcome = requestQuote(chainId, account, calls, gasFeeToken, publicKeyAvailable)
        },
        ports = ports,
    )

    private val sendHost = CoreHost(
        bridge = SendCore().asBridge(),
        scope = scope,
        initial = SendView(),
        serializer = SendView.serializer(),
        perform = JsonShell.perform(SendOperation.serializer(), SendShellResult.serializer()) { operation ->
            // One line per arm on the device log: which operation, which
            // answer. The send path's first device run had no trace at all
            // between "Continue" and an alert, and this is what would have
            // said where it stopped.
            val result = sendExecutor.perform(operation)
            VelaLog.event("send.arm", operation::class.simpleName ?: "?", "answer" to (result::class.simpleName ?: "?"))
            result
        },
        escapedFailure = JsonShell.escapedFailure(
            SendOperation.serializer(),
            SendShellResult.serializer(),
            fallback = SendShellResult.Closed,
            answer = sendExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("send.fault", "core fault", error) },
    )

    /** The send machine's view — every screen reads this and nothing else. */
    val send: StateFlow<SendView> = sendHost.view

    /** The fee session's view — the fee-token sheet reads this. */
    val fee: StateFlow<FeeView> = feeHost.view

    init {
        sendHost.start()
        feeHost.start()
        // The fee session's later word flows into the send machine: a
        // re-quote after a fee-token pick or a TTL expiry replaces the
        // estimate the confirm screen shows (desktop `sync_fee_to_send`).
        scope.launch {
            var lastFee: FeeEstimateView? = null
            var lastBusy = false
            feeHost.view.collect { view ->
                if (view.busy != lastBusy) {
                    lastBusy = view.busy
                    dispatch(SendEvent.FeeBusyChanged(view.busy))
                }
                val estimate = view.fee
                if (estimate != null && estimate !== lastFee) {
                    lastFee = estimate
                    dispatch(SendEvent.FeeUpdated(estimate))
                }
            }
        }
    }

    private fun dispatch(event: SendEvent) {
        sendHost.dispatch(event, SendEvent.serializer())
    }

    // -- the fee bridge (research D7) ----------------------------------------------

    private suspend fun requestQuote(
        chainId: Int,
        account: String,
        calls: List<FeeCall>,
        gasFeeToken: String?,
        publicKeyAvailable: Boolean,
    ): SendFeeOutcome {
        val deployed = relayRef.isDeployed(chainId, account) ?: false
        val before = feeHost.view.value.fee
        feeHost.dispatch(
            FeeEvent.QuoteRequested(
                chain_id = chainId,
                account = account,
                deployed = deployed,
                public_key_available = publicKeyAvailable,
                tier = FeeTier.Fast,
                calls = calls,
                fee_token = gasFeeToken,
            ),
            FeeEvent.serializer(),
        )
        // Settled = not busy, and either a NEW estimate or a failure. The
        // reference check is what keeps a stale estimate from answering a
        // fresh request; every commit decodes a fresh object.
        val settled = withTimeoutOrNull(QUOTE_TIMEOUT_MS) {
            feeHost.view.first { view ->
                !view.busy && ((view.fee != null && view.fee !== before) || view.failed != null)
            }
        } ?: return SendFeeOutcome.Failed(SendEstimateFailure.Timeout)
        val estimate = settled.fee
        return when {
            settled.failed != null -> SendFeeOutcome.Failed(failure(settled.failed))
            estimate != null -> SendFeeOutcome.Ok(estimate)
            else -> SendFeeOutcome.Failed(SendEstimateFailure.Other)
        }
    }

    private fun failure(failed: FeeFailure): SendEstimateFailure = when (failed) {
        FeeFailure.MissingPublicKey -> SendEstimateFailure.MissingPublicKey
        FeeFailure.FeeTokenUnavailable -> SendEstimateFailure.FeeTokenUnavailable
        FeeFailure.QuoteUnavailable -> SendEstimateFailure.QuoteUnavailable
        FeeFailure.CalculationFailed -> SendEstimateFailure.CalculationFailed
        FeeFailure.EstimateFailed -> SendEstimateFailure.EstimateFailed
        FeeFailure.GasQuoteTooHigh -> SendEstimateFailure.GasQuoteTooHigh
    }

    // -- intents ---------------------------------------------------------------------

    /** 发送 tapped: one attempt begins. */
    fun open(account: SendAccountRef?, display: SendDisplayContext, params: SendOpenParams = SendOpenParams()) {
        _closed.value = false
        _alert.value = null
        dispatch(SendEvent.Open(account = account, params = params, display = display))
    }

    fun displayChanged(display: SendDisplayContext) = dispatch(SendEvent.DisplayChanged(display))

    fun refreshTokens() = dispatch(SendEvent.RefreshTokens)

    fun selectToken(tokenId: String) = dispatch(SendEvent.SelectToken(tokenId))

    fun setRecipient(recipient: String) = dispatch(SendEvent.SetRecipient(recipient))

    fun setAmount(amount: String) = dispatch(SendEvent.SetAmount(amount))

    fun toggleFiatInput() = dispatch(SendEvent.ToggleFiatInput)

    fun tapMax() = dispatch(SendEvent.TapMax)

    fun openContactPicker() = dispatch(SendEvent.OpenContactPicker(null))

    fun closeContactPicker() = dispatch(SendEvent.CloseContactPicker)

    fun pickedAddress(address: String) = dispatch(SendEvent.PickedAddress(address))

    fun continueTapped() = dispatch(SendEvent.Continue)

    fun back() = dispatch(SendEvent.Back)

    fun editAmount() = dispatch(SendEvent.EditAmount)

    fun chooseFeeToken(token: String?) = dispatch(SendEvent.ChooseFeeToken(token))

    fun slideConfirm() = dispatch(SendEvent.SlideConfirm)

    fun cancelSigning() = dispatch(SendEvent.CancelSigning)

    fun retryAfterBootstrap() = dispatch(SendEvent.RetryAfterBootstrap)

    fun dismissTreasurySheet() = dispatch(SendEvent.DismissTreasurySheet)

    fun retryAfterError() = dispatch(SendEvent.RetryAfterError)

    fun done() = dispatch(SendEvent.Done)

    /** The tracker's verdict for a hash this attempt submitted. */
    fun receiptUpdate(userOpHash: String, outcome: SendReceiptOutcome) =
        dispatch(SendEvent.ReceiptUpdate(userOpHash, outcome))

    fun dismissAlert() {
        _alert.value = null
    }

    private companion object {
        /** Well above the core's own 15 s estimate timeout; a guard, not a policy. */
        const val QUOTE_TIMEOUT_MS = 30_000L
    }
}

/**
 * The account store as the send path reads it: a wallet's founding keys,
 * pinned key first (a legacy record projects its scalar fields as the sole
 * key — `keySetOf`), and how the pinned key's ceremony is routed.
 */
class StoreAccountPort(private val store: AccountStore) : SendExecutor.AccountPort {

    private suspend fun record(match: (org.json.JSONObject) -> Boolean): org.json.JSONObject? {
        val all = store.loadAccounts()
        for (index in 0 until all.length()) {
            val record = all.optJSONObject(index) ?: continue
            if (match(record)) return record
        }
        return null
    }

    override suspend fun keysOf(address: String): List<WalletKeyRecord> {
        val record = record { it.optString("address").equals(address, ignoreCase = true) } ?: return emptyList()
        val keys = record.optJSONArray("keys")
        if (keys == null || keys.length() == 0) {
            val id = record.optString("id")
            val pk = record.optString("public_key_hex")
            return if (id.isBlank() || pk.isBlank()) emptyList() else listOf(WalletKeyRecord(id, pk))
        }
        return (0 until keys.length()).mapNotNull { index ->
            val key = keys.optJSONObject(index) ?: return@mapNotNull null
            val id = key.optString("credential_id").ifBlank { return@mapNotNull null }
            val pk = key.optString("public_key_hex").ifBlank { return@mapNotNull null }
            WalletKeyRecord(id, pk)
        }
    }

    override suspend fun routingOf(address: String): Pair<String, KeyMethod> {
        val record = record { it.optString("address").equals(address, ignoreCase = true) }
        val transports = record?.optJSONArray("keys")?.optJSONObject(0)?.optString("transports").orEmpty()
        val method = when {
            transports.contains("hybrid") && !transports.contains("internal") -> KeyMethod.Hybrid
            transports.contains("usb") || transports.contains("nfc") -> KeyMethod.SecurityKey
            else -> KeyMethod.Platform
        }
        return transports to method
    }

    /**
     * `load_account_credential { account_id }`: the id the send was opened
     * with. The screens open it with the ADDRESS (the one fact every view
     * carries), onboarding's records are keyed by credential id — so both
     * spellings are accepted, and a device-found `AccountUnavailable` on the
     * first live Continue is why this comment exists.
     */
    override suspend fun publicKeyOf(accountId: String): String? =
        record { it.optString("id") == accountId || it.optString("address").equals(accountId, ignoreCase = true) }
            ?.optString("public_key_hex")?.ifBlank { null }
}
