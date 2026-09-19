package app.getvela.wallet.feature.send.core

import app.getvela.wallet.feature.scan.Eip681
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
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.FeePolicyCore
import uniffi.vela_core_uniffi.BatchImportCore
import app.getvela.wallet.feature.documents.DocumentPorts
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
    /** Spec 043 T048: a name for the recipient, through the app's waterfall. */
    identity: suspend (String) -> SendRecipientIdentity? = { null },
    /** Spec 045 US3: the display currency the batch prices in, the wallet's fiat-rate waterfall, the platform's documents. */
    private val currencyCode: () -> String = { "USD" },
    fiatRate: suspend (String) -> Double? = { null },
    documents: () -> DocumentPorts? = { null },
    /** Spec 046 US3: a scanned chain the wallet lacks goes to the settings machine. */
    addNetwork: suspend (Long) -> SendAddNetworkOutcome = { SendAddNetworkOutcome.NotFound },
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
        override suspend fun addNetwork(chainId: Long): SendAddNetworkOutcome = addNetwork(chainId)
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
        ports = ports,        identity = identity,
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

    // Spec 045 US3: the payroll batch — its own machine, hosted beside the send.
    private val batchExecutor = BatchExecutor(fiatRate = fiatRate, documents = documents)

    private val batchHost = CoreHost(
        bridge = BatchImportCore().asBridge(),
        scope = scope,
        initial = BatchView(),
        serializer = BatchView.serializer(),
        perform = JsonShell.perform(BatchOperation.serializer(), BatchShellResult.serializer(), batchExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(
            BatchOperation.serializer(),
            BatchShellResult.serializer(),
            fallback = BatchShellResult.FilePickFailed,
            answer = batchExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("send.batch.fault", "core fault", error) },
    )

    val batch: StateFlow<BatchView> = batchHost.view

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
            var lastStale = false
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
                // Spec 045 US4: a quote goes stale while the person reads the
                // confirm page (the policy's TTL). Re-ask once per stale flip,
                // with the same request, while the page is open and idle; the
                // fresh estimate flows back through FeeUpdated above.
                if (view.stale != lastStale) {
                    lastStale = view.stale
                    val current = send.value
                    val request = lastQuote
                    if (view.stale && !view.busy && request != null &&
                        current.stage == SendStage.Confirm && current.tx_status == SendTxStatus.Idle && !current.sending
                    ) {
                        VelaLog.event("send.fee", "re-quote on stale", "chain" to request.chain_id)
                        feeHost.dispatch(request, FeeEvent.serializer())
                    }
                }
            }
        }
    }

    private fun dispatch(event: SendEvent) {
        sendHost.dispatch(event, SendEvent.serializer())
    }

    // -- the fee bridge (research D7) ----------------------------------------------

    /** The last quote asked, re-asked verbatim when it goes stale on the confirm page (spec 045 US4). */
    @Volatile
    private var lastQuote: FeeEvent.QuoteRequested? = null

    private suspend fun requestQuote(
        chainId: Int,
        account: String,
        calls: List<FeeCall>,
        gasFeeToken: String?,
        publicKeyAvailable: Boolean,
    ): SendFeeOutcome {
        val deployed = relayRef.isDeployed(chainId, account) ?: false
        val before = feeHost.view.value.fee
        val request = FeeEvent.QuoteRequested(
            chain_id = chainId,
            account = account,
            deployed = deployed,
            public_key_available = publicKeyAvailable,
            tier = FeeTier.Fast,
            calls = calls,
            fee_token = gasFeeToken,
        )
        lastQuote = request
        feeHost.dispatch(request, FeeEvent.serializer())
        // Settled = not busy, and either a NEW estimate or a failure. The
        // reference check is what keeps a stale estimate from answering a
        // fresh request; every commit decodes a fresh object.
        //
        // Woken by `commits`, NOT by `view`: a re-quote at the same price is a
        // view that `equals` the one before the request, and a StateFlow never
        // delivers that to a collector that missed the `busy` in between — the
        // wait then ran out its whole timeout with the answer sitting in the
        // view (see `CoreHost.commits`).
        val settled = withTimeoutOrNull(QUOTE_TIMEOUT_MS) {
            feeHost.commits
                .map { feeHost.view.value }
                .first { view ->
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

    // -- Split (spec 045 US1): one token to several people; the rows are the
    // core's drafts and every edit goes back as the whole list.
    fun enterSplit() = dispatch(SendEvent.EnterSplitMode)

    fun splitAmount(id: String, amount: String) =
        dispatch(SendEvent.RecipientsChanged(SplitRows.amountEdited(send.value.recipients, id, amount)))

    fun splitAddress(id: String, address: String) =
        dispatch(SendEvent.RecipientsChanged(SplitRows.addressEdited(send.value.recipients, id, address)))

    fun splitRemove(id: String) = dispatch(SendEvent.RecipientsChanged(SplitRows.removed(send.value.recipients, id)))

    fun splitAdd() = dispatch(SendEvent.RecipientsChanged(SplitRows.appended(send.value.recipients)))

    /** A whole list at once — a group, a batch; ids minted by the core. */
    fun seedSplit(recipients: List<SendRecipientDraft>) = dispatch(SendEvent.SeedSplitRecipients(recipients))

    /** The picker for ONE split row: the picked address lands in that row. */
    fun openRowPicker(id: String) = dispatch(SendEvent.OpenContactPicker(id))

    /** 导入表格: the sheet opens on the send's flag and the batch machine opens on the token. */
    // -- Scanner (spec 046 US3): the core's flag opens the surface; a decode
    // becomes ScanResolved and the core decides what it means.
    fun openScanner() = dispatch(SendEvent.OpenScanner)

    fun closeScanner() = dispatch(SendEvent.CloseScanner)

    fun scanned(text: String) = dispatch(SendEvent.ScanResolved(scanOf(text)))

    fun openBatch() {
        val token = send.value.selected_token ?: return
        dispatch(SendEvent.OpenBatchImport)
        batchHost.dispatch(
            BatchEvent.Open(
                token = BatchToken(symbol = token.symbol, decimals = token.decimals, balance = token.balance, price_usd = token.price_usd),
                currency_code = currencyCode(),
                max_recipients = BATCH_MAX_RECIPIENTS,
            ),
            BatchEvent.serializer(),
        )
    }

    fun closeBatch() = dispatch(SendEvent.CloseBatchImport)

    fun batchUnit(unit: BatchUnit) = batchHost.dispatch(BatchEvent.SetUnit(unit), BatchEvent.serializer())

    fun batchFiatCode(code: String) = batchHost.dispatch(BatchEvent.SetFiatCode(code), BatchEvent.serializer())

    fun batchText(text: String) = batchHost.dispatch(BatchEvent.SetRawText(text), BatchEvent.serializer())

    fun batchPickFile() = batchHost.dispatch(BatchEvent.PickFileRequested, BatchEvent.serializer())

    fun batchTemplate() = batchHost.dispatch(BatchEvent.SaveTemplateRequested, BatchEvent.serializer())

    fun batchRate(text: String) = batchHost.dispatch(BatchEvent.EditRate(text), BatchEvent.serializer())

    fun batchResetRate() = batchHost.dispatch(BatchEvent.ResetRateToAuto, BatchEvent.serializer())

    /**
     * Apply: the core parsed and priced the rows; the send machine seeds its
     * split from exactly those (ids minted there), and nothing is recomputed.
     * `SeedSplitRecipients` also shuts the sheet.
     */
    fun batchApply() {
        val view = batch.value
        if (!view.can_apply || view.recipients.isEmpty()) return
        batchHost.dispatch(BatchEvent.Apply, BatchEvent.serializer())
        seedSplit(view.recipients.map { SendRecipientDraft(id = "", address = it.address, amount = it.amount, name = it.name) })
    }

    // -- Sweep (spec 045 US2): several tokens to one address. Whether the tick
    // boxes are showing is the shell's flag (the core's `multi_select_mode`
    // flips only when the selection is confirmed, as on the web and desktop).
    private val _sweepPicking = MutableStateFlow(false)
    val sweepPicking: StateFlow<Boolean> = _sweepPicking

    fun startSweep() {
        _sweepPicking.value = true
        dispatch(SendEvent.SetMultiNetwork(null))
    }

    fun cancelSweep() {
        _sweepPicking.value = false
        dispatch(SendEvent.SetMultiNetwork(null))
    }

    fun toggleSweep(tokenId: String) = SweepPick.tap(send.value, tokenId).forEach(::dispatch)

    fun selectAllValuable(visibleIds: List<String>) = SweepPick.selectAll(send.value, visibleIds).forEach(::dispatch)

    fun confirmSweep() {
        _sweepPicking.value = false
        dispatch(SendEvent.ConfirmMultiSelection)
    }

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

    internal companion object {
        /** A decoded text as the core's scan: a payment request through the tokenizer, anything else as text. */
        fun scanOf(text: String): SendScan = Eip681.parse(text)?.let { request ->
            SendScan.Request(
                recipient = request.recipient,
                chain_id = request.chainId?.toInt(),
                token_address = request.tokenAddress,
                amount_base_units = request.amountBaseUnits,
            )
        } ?: SendScan.Text(data = text.trim())

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

    override suspend fun keyRoutesJson(address: String): String {
        val record = record { it.optString("address").equals(address, ignoreCase = true) } ?: return "[]"
        val keys = record.optJSONArray("keys") ?: return "[]"
        val out = org.json.JSONArray()
        for (index in 0 until keys.length()) {
            val key = keys.optJSONObject(index) ?: continue
            out.put(org.json.JSONObject().put("credential_id", key.optString("credential_id")).put("transports", key.optString("transports")))
        }
        return out.toString()
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
