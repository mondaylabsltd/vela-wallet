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
import app.getvela.wallet.feature.wallet.core.HoldingsFeed
import app.getvela.wallet.feature.wallet.core.HoldingsRound
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import org.json.JSONObject
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
 * machine as `FeeUpdated` / `FeeBusyChanged` / `FeeTokenChanged`.
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
    /** Spec 069: the stored default speed, and the resolved number preset its gas bids are written in. */
    private val preferredTier: () -> FeeTier = { FeeTier.Standard },
    private val numberPreset: () -> String = { "comma_dot" },
    /** Spec 071/102: the signing page, for an account whose venue is one. */
    trustedSigner: () -> TrustedSigner? = { null },
    /** The corpus, for the sentences a signature that cannot be made is told in (spec 102). */
    words: (key: String, vars: Map<String, String>) -> String = { key, _ -> key },
    /**
     * Spec 078: the asset list's holdings — the picker is answered from the
     * balance machine's settled round and follows every later one. `null`
     * (tests) answers from [balances] as it stands and follows nothing.
     */
    private val holdings: HoldingsFeed? = null,
    /** The tracker handoff; the wallet controller binds it (phase 4). */
    var onTrackSubmitted: (TrackHandoff) -> Unit = { handoff ->
        VelaLog.event("send.track", "no tracker bound", "hash" to handoff.userOpHash.take(12))
    },
    /** Spec 082 RJ1: the tracker's `Withdrawn`; the wallet controller binds it. */
    var onTrackWithdrawn: (userOpHash: String, recordIds: List<String>) -> Unit = { hash, _ ->
        VelaLog.event("send.track", "no tracker bound: withdraw", "hash" to hash.take(12))
    },
) {

    private val _alert = MutableStateFlow<SendAlertKind?>(null)

    /** The core's refusal to print, until the screen dismisses it. */
    val alert: StateFlow<SendAlertKind?> = _alert

    private val _closed = MutableStateFlow(false)

    /** The core said `Close`: the flow host pops the send. */
    val closed: StateFlow<Boolean> = _closed

    private val hapticPort: (SendHapticKind) -> Unit = haptic
    private val refreshPort: () -> Unit = refreshBalances
    private val feedChangedPort: () -> Unit = feedChanged
    private val accountPort: StoreAccountPort = StoreAccountPort(accountStore)

    private val feeExecutor = FeeExecutor(
        relay = relay,
        keyHexes = { address: String -> accountPort.keysOf(address).map { it.publicKeyHex } },
        // The quote measures the inner calls the way the submit spine does.
        measureCall = { chainId, from, to, valueHex, data ->
            (pool.call(chainId, "eth_estimateGas", listOf(JSONObject().put("from", from).put("to", to).put("value", valueHex).put("data", data))) as? RpcResult.Body)
                ?.json?.takeIf { it.has("result") && !it.isNull("result") }?.optString("result")?.takeIf { it.startsWith("0x") }
        },
    )

    // -- the speed control (spec 069) ----------------------------------------------

    /** The fee sessions and their reconcile step — the very class the signing sheet runs. */
    private val speedControl = SpeedControl(scope, relay, feeExecutor, preferredTier, numberPreset, area = "send")

    /** The speed control, as the core decided it — the send form draws this. */
    val speed: StateFlow<FeeSpeedView> = speedControl.speed

    private val ports: SendExecutor.SendPorts = object : SendExecutor.SendPorts {
        override suspend fun addNetwork(chainId: Long): SendAddNetworkOutcome = addNetwork(chainId)
        override fun signingStarted() {
            dispatch(SendEvent.SigningStarted)
        }

        override fun trackSubmitted(handoff: TrackHandoff) {
            onTrackSubmitted(handoff)
        }

        // Spec 082 RJ1: signed and hashed, nothing posted — the core writes
        // the rows ahead and clears the POST once they are on disk.
        override fun opSigned(userOpHash: String, submitBlock: Long?) {
            dispatch(SendEvent.OpSigned(user_op_hash = userOpHash, submit_block = submitBlock, now_ms = System.currentTimeMillis().toDouble()))
        }

        override fun trackWithdrawn(userOpHash: String, recordIds: List<String>) {
            onTrackWithdrawn(userOpHash, recordIds)
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

        override fun tokensPartial(tokens: List<SendToken>) {
            dispatch(SendEvent.TokensPartial(tokens))
        }

        override fun holdingsHanded(round: HoldingsRound?) {
            handedRound = round?.atMs
            holdingsLoaded = true
        }
    }

    // -- the asset list's holdings (spec 078) -----------------------------------------

    /** The round this attempt's `fetch_tokens` was answered from (`null` = none, follow the first). */
    @Volatile
    private var handedRound: Double? = null

    /** This attempt's picker has had its answer; only then do later rounds follow. */
    @Volatile
    private var holdingsLoaded = false

    /** Whose send this is — a round for another account is not this screen's. */
    @Volatile
    private var sendAccount: String? = null

    /**
     * A round the dashboard settled after the one the picker was answered
     * from: handed to the core as `HoldingsUpdated`, mapped exactly as the
     * answer was. What follows — the picker always, the form's balance and a
     * Max on it, never a confirm page — is the core's to say. Without this a
     * refresh started from Send (or a poll, or a landed transfer) reached the
     * home screen only, and this screen kept the old balance.
     */
    private fun followHoldings(round: HoldingsRound?) {
        round ?: return
        val account = sendAccount ?: return
        if (!holdingsLoaded || _closed.value || !round.address.equals(account, ignoreCase = true)) return
        if (handedRound == round.atMs) return
        handedRound = round.atMs
        VelaLog.event("send.tokens", "holdings updated", "tokens" to round.view.tokens.size)
        dispatch(SendEvent.HoldingsUpdated(SendExecutor.sendTokens(round.view)))
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
                autoFeeToken: Boolean,
            ): SendFeeOutcome = requestQuote(chainId, account, calls, gasFeeToken, publicKeyAvailable, autoFeeToken)
        },
        ports = ports,
        identity = identity,
        trustedSigner = trustedSigner,
        words = words,
        holdings = holdings,
        // The picker is open: the chains the person holds value on are read
        // ahead — at the speed in force — so the quote a pick starts finds its
        // reads done and has only its simulation left.
        prewarm = { account, chainIds ->
            scope.launch {
                runCatching { relay.prewarmFees(account, chainIds, speedControl.speed.value.tier) }
                    .onFailure { VelaLog.failure("send.prewarm", "read-ahead failed", it) }
            }
        },
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

    /**
     * Every commit of the send machine. Wait on THIS for "the core answered",
     * never on [send]: a view that `equals` the last one is never re-emitted,
     * so an answer that changes nothing visible wakes no collector of the view
     * (`CoreHost.commits`, the 2026-09-19 CI flake).
     */
    val commits: StateFlow<Long> = sendHost.commits

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

    /**
     * The fee session in force — whichever session that is right now, so a
     * promotion swaps what the fee row and the fee-token sheet read without
     * either of them knowing (spec 069). `pending` folded into `busy`.
     */
    val fee: StateFlow<FeeView> = speedControl.fee

    /**
     * Spec 102: the fee session in force and the speed control exactly as the
     * core wrote them — the hand-off card's fee row (`handoffFeeRow`) reads
     * them back, so the card restates what this send was priced at.
     */
    val feeJson: StateFlow<String?> = speedControl.feeJson
    val speedJson: StateFlow<String?> = speedControl.speedJson

    /** The fee view of the session pricing `tier` — for formatting that option's fee. */
    fun feeViewOf(tier: FeeTier): FeeView? = speedControl.feeViewOf(tier)

    init {
        sendHost.start()
        speedControl.start()
        holdings?.let { feed -> scope.launch { feed.settled.collect(::followHoldings) } }
        // The send leaving its form ends any free-upgrade question.
        scope.launch {
            sendHost.view.collect { view -> speedControl.stage(view.stage == SendStage.EnterDetails) }
        }
        // Issue #424: when the core stops a send on the relay, and lowers the
        // stop again, the device log says so — with the stage it happened on.
        // 0.9.6 drew the funding stop on the confirm page only; a stop opened
        // on the form left no trace anywhere.
        scope.launch {
            var last: String? = null
            sendHost.view.collect { view ->
                val stop = stopName(view)
                if (stop != last) {
                    last = stop
                    val status = view.treasury_bootstrap
                    VelaLog.event(
                        "send.stop", stop ?: "lowered",
                        "stage" to view.stage.name,
                        "chain" to (view.relay_unreachable?.chain_id ?: status?.chain_id),
                        "balance" to status?.balance, "floor" to status?.floor,
                    )
                }
            }
        }
        // The fee session's later word flows into the send machine: a
        // re-quote after a fee-token pick or a TTL expiry replaces the
        // estimate the confirm screen shows (desktop `sync_fee_to_send`).
        scope.launch {
            var lastFee: FeeEstimateView? = null
            var lastBusy = false
            var lastStale = false
            fee.collect { view ->
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
                    if (view.stale && !view.busy &&
                        current.stage == SendStage.Confirm && current.tx_status == SendTxStatus.Idle && !current.sending
                    ) {
                        speedControl.requoteStale()
                    }
                }
            }
        }
        // The card's coin in force names the fee row while no estimate is in
        // hand (SendView.fee_coin): when nobody chose, the fee machine picks a
        // coin that can pay, and a quote that then fails leaves that coin in
        // force with nothing else to say so. Weighed again whenever the coin,
        // the chain the session prices, or the send form moves (every commit:
        // a form that reaches the priced chain is what lets a coin said before
        // it had one through).
        scope.launch {
            combine(fee, speedControl.pricingChainId, sendHost.commits) { view, pricing, _ -> view.fee_token to pricing }
                .collect { (feeToken, pricing) -> tellFeeToken(feeToken, pricing) }
        }
    }

    private fun dispatch(event: SendEvent) {
        sendHost.dispatch(event, SendEvent.serializer())
    }

    /** What this send journey was last told of the fee card's coin; [open] forgets it. */
    private val feeTokenWord = FeeTokenWord()

    /** The event number of this journey's `Open` — under [feeTokenWord]'s lock. */
    private var openEvent = 0L

    /**
     * `FeeTokenChanged` under the core's bridge rule ([FeeTokenWord]): the
     * card's coin [feeToken], its session pricing [pricing], against the
     * form's chain. Under the word's lock with the dispatch, so a word can
     * never be queued ahead of the `Open` that would wipe it while this side
     * believes it was heard — and read only from a view the `Open` has
     * reached, so the form's chain is this journey's, not the last one's.
     */
    private fun tellFeeToken(feeToken: String?, pricing: Int?) = synchronized(feeTokenWord) {
        if (!sendHost.applied(openEvent)) return@synchronized
        feeTokenWord.news(feeToken, pricing, formChain(send.value))?.let(::dispatch)
    }

    // -- the fee bridge (research D7) ----------------------------------------------

    private suspend fun requestQuote(
        chainId: Int,
        account: String,
        calls: List<FeeCall>,
        gasFeeToken: String?,
        publicKeyAvailable: Boolean,
        autoFeeToken: Boolean,
    ): SendFeeOutcome = when (val quoted = speedControl.quote(chainId, account, publicKeyAvailable, calls, gasFeeToken, autoFeeToken)) {
        SpeedControl.Quoted.Superseded -> SendFeeOutcome.Failed(SendEstimateFailure.Other)
        SpeedControl.Quoted.TimedOut -> SendFeeOutcome.Failed(SendEstimateFailure.Timeout)
        is SpeedControl.Quoted.Settled -> {
            val settled = quoted.view
            val estimate = settled.fee
            when {
                settled.failed != null -> SendFeeOutcome.Failed(failure(settled.failed))
                estimate != null -> SendFeeOutcome.Ok(estimate)
                else -> SendFeeOutcome.Failed(SendEstimateFailure.Other)
            }
        }
    }

    private fun failure(failed: FeeFailure): SendEstimateFailure = when (failed) {
        FeeFailure.MissingPublicKey -> SendEstimateFailure.MissingPublicKey
        FeeFailure.FeeTokenUnavailable -> SendEstimateFailure.FeeTokenUnavailable
        FeeFailure.QuoteUnavailable -> SendEstimateFailure.QuoteUnavailable
        FeeFailure.CalculationFailed -> SendEstimateFailure.CalculationFailed
        FeeFailure.EstimateFailed -> SendEstimateFailure.EstimateFailed
        FeeFailure.GasQuoteTooHigh -> SendEstimateFailure.GasQuoteTooHigh
        // The send machine's vocabulary has no word for a chain node that did
        // not answer (spec 082 RJ13 is the signing sheet's fee row): the quote
        // could not be had, and asking again is right.
        is FeeFailure.ChainRead -> SendEstimateFailure.QuoteUnavailable
        // Spec 083 fee: the relay's "this operation fails", as the send
        // screen has always said that refusal.
        FeeFailure.WouldFail -> SendEstimateFailure.EstimateFailed
    }

    // -- intents ---------------------------------------------------------------------

    private val _opens = MutableStateFlow(0L)

    /**
     * How many attempts have begun. A screen keyed on it resets what it keeps
     * of its own (an open fee sheet) whenever the machine is re-opened — also
     * when a payment request re-opens a Send that was already on screen
     * ([PaymentHandOff]), which no navigation change announces.
     */
    val opens: StateFlow<Long> = _opens

    /** 发送 tapped: one attempt begins. */
    fun open(account: SendAccountRef?, display: SendDisplayContext, params: SendOpenParams = SendOpenParams()) {
        _opens.value += 1
        _closed.value = false
        _alert.value = null
        // A new attempt holds no round yet: its own `fetch_tokens` picks one.
        sendAccount = account?.address
        holdingsLoaded = false
        handedRound = null
        // A new send starts at the stored default: the one-shot pick, a free
        // upgrade and the fold all die with the send before it (spec 068).
        speedControl.reset()
        // The core's Open starts a journey that has been told nothing; the
        // fee bridge's next word about the card's coin goes, whatever it is,
        // once the form is on the chain the fee session prices.
        synchronized(feeTokenWord) {
            feeTokenWord.forget()
            openEvent = sendHost.dispatchNumbered(
                SendEvent.Open(account = account, params = params, display = display),
                SendEvent.serializer(),
            )
        }
    }

    // -- the speed control (spec 069) -----------------------------------------------

    /**
     * The stored default or the number preset moved: a send already open
     * follows the new default until the person picks a speed on it.
     */
    fun preferenceChanged() = speedControl.preferenceChanged()

    /** Fold or unfold the control. */
    fun toggleSpeed() = speedControl.toggle()

    /** A tap on an option — one-shot, never the stored preference. */
    fun pickSpeed(tier: FeeTier) = speedControl.pick(tier)

    /** The refresh control: measure again, the held readings dropped first (issue 212). */
    fun refreshFee() = speedControl.refresh()

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

    /** "Use X for the empty rows": one typed figure into every row that has none; the core judges each row. */
    fun splitFillEmpty(amount: String) =
        dispatch(SendEvent.RecipientsChanged(SplitRows.emptyFilled(send.value.recipients, amount)))

    /** A whole list at once — a group, a batch; ids minted by the core. */
    fun seedSplit(recipients: List<SendRecipientDraft>) = dispatch(SendEvent.SeedSplitRecipients(recipients))

    /** The rows ADDED after the ones already typed (issue #271); the core keeps the started rows and drops the blank ones. */
    fun appendSplit(recipients: List<SendRecipientDraft>) = dispatch(SendEvent.AppendSplitRecipients(recipients))

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
        _importReplaces.value = false
        dispatch(SendEvent.OpenBatchImport)
        batchHost.dispatch(
            BatchEvent.Open(
                token = BatchToken(symbol = token.symbol, decimals = token.decimals, balance = token.balance, price_usd = token.price_usd),
                currency_code = currencyCode(),
                // The room the form has left, not a flat 60: an import ADDS to the
                // rows already started, so its "only the first N" must be true of
                // what the append then keeps (the web's `split_import_room`).
                max_recipients = send.value.split_import_room,
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
     * Whether this import REPLACES the recipients already on the form (the
     * web's `importReplaces`). Adding is the default — issue #271: an import
     * used to seed, which silently threw away a recipient typed by hand — and
     * replacing is one tap away, said in words before anything is imported.
     * A choice about one import, so every open resets it.
     */
    private val _importReplaces = MutableStateFlow(false)
    val importReplaces: StateFlow<Boolean> = _importReplaces

    fun toggleImportReplaces() { _importReplaces.value = !_importReplaces.value }

    /**
     * Apply: the core parsed and priced the rows; the send machine adds them
     * (or, when chosen, seeds its split) from exactly those — ids minted
     * there, nothing recomputed. Both events also shut the sheet.
     */
    fun batchApply() {
        val view = batch.value
        if (!view.can_apply || view.recipients.isEmpty()) return
        batchHost.dispatch(BatchEvent.Apply, BatchEvent.serializer())
        val rows = view.recipients.map { SendRecipientDraft(id = "", address = it.address, amount = it.amount, name = it.name) }
        if (_importReplaces.value) seedSplit(rows) else appendSplit(rows)
        _importReplaces.value = false
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

    /** Which relay stop the core has up, for the log; `null` = none. */
    private fun stopName(view: SendView): String? = when {
        view.relay_unreachable != null -> "unreachable"
        view.treasury_bootstrap != null -> "funding"
        else -> null
    }

    fun continueTapped() {
        // Issue #424: a "Continue does nothing" report needs to know the tap
        // reached the core, and what the core's gate said when it did.
        val view = send.value
        VelaLog.event(
            "send.continue", "tapped",
            "stage" to view.stage.name, "can" to view.can_continue,
            "chain" to view.selected_token?.chain_id,
            "stop" to stopName(view),
        )
        dispatch(SendEvent.Continue)
    }

    fun back() = dispatch(SendEvent.Back)

    /** Issue #326: the form's token card, where the core offers it (`can_change_token`). */
    fun changeToken() = dispatch(SendEvent.ChangeToken)

    fun editAmount() = dispatch(SendEvent.EditAmount)

    fun chooseFeeToken(token: String?) = dispatch(SendEvent.ChooseFeeToken(token))

    fun slideConfirm() = dispatch(SendEvent.SlideConfirm)

    fun cancelSigning() = dispatch(SendEvent.CancelSigning)

    fun retryAfterBootstrap() = dispatch(SendEvent.RetryAfterBootstrap)

    fun dismissTreasurySheet() = dispatch(SendEvent.DismissTreasurySheet)

    fun retryRelayUnreachable() = dispatch(SendEvent.RetryRelayUnreachable)

    fun dismissRelayUnreachable() = dispatch(SendEvent.DismissRelayUnreachable)

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
    }
}

/**
 * What one send journey was last told of the fee card's coin in force
 * (`FeeView.fee_token`, by `FeeTokenChanged`), and the chain it is a coin on —
 * the core's one bridge rule (the doc on `send::Event::FeeTokenChanged`; iOS
 * `SendStore.feeTokenChanged`).
 *
 * The send machine files the word against the form's chain at the moment it
 * is said and drops it while the form has no chain. So the coin is told only
 * while the fee session prices the form's own chain, and whenever the pair
 * (chain, coin) differs from what this journey was last told — a coin first
 * seen before the form had a chain is told once it has one. A freshly opened
 * journey has been told nothing: its first word goes, `null` (the chain's own
 * coin) included. Not thread-safe: the controller holds its lock.
 */
internal class FeeTokenWord {
    private var told: Pair<Int, String?>? = null

    /**
     * The event that tells [feeToken], priced on [pricing] while the form is on
     * [form] — or `null` when there is nothing to say.
     */
    fun news(feeToken: String?, pricing: Int?, form: Int?): SendEvent.FeeTokenChanged? {
        if (pricing == null || pricing != form) return null
        val said = pricing to feeToken
        if (told == said) return null
        told = said
        return SendEvent.FeeTokenChanged(feeToken)
    }

    /** A new journey: nothing told yet. */
    fun forget() {
        told = null
    }
}

/** The chain the send form is on: the selected token's, else the sweep's (the core's `form_chain`). */
internal fun formChain(view: SendView): Int? = view.selected_token?.chain_id ?: view.multi_chain_id

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
        // `ble` with `usb` and `nfc` (as tokens: the retired `cable` is not one):
        // whatever reports one is a key to present, and the method is what
        // sends a ceremony down the security-key path.
        val hints = transports.split(',').map { it.trim() }
        val method = when {
            transports.contains("hybrid") && !transports.contains("internal") -> KeyMethod.Hybrid
            transports.contains("usb") || transports.contains("nfc") || "ble" in hints -> KeyMethod.SecurityKey
            else -> KeyMethod.Platform
        }
        return transports to method
    }

    /** The record as the core wrote it — read by the core's `signing_plan`, never rebuilt field by field. */
    override suspend fun accountJson(address: String): String? =
        record { it.optString("address").equals(address, ignoreCase = true) }?.toString()

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
