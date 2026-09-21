package app.getvela.wallet.feature.wallet.core

import android.content.Context
import app.getvela.wallet.core.crux.CoreHost
import org.json.JSONObject
import uniffi.vela_core_uniffi.ManageTokensCore
import app.getvela.wallet.feature.send.core.MtokView
import app.getvela.wallet.feature.send.core.MtokShellResult
import app.getvela.wallet.feature.send.core.MtokOperation
import app.getvela.wallet.feature.send.core.MtokNetwork
import app.getvela.wallet.feature.send.core.MtokExecutor
import app.getvela.wallet.feature.send.core.MtokEvent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.delay
import uniffi.vela_core_uniffi.TxTrackerCore
import app.getvela.wallet.feature.send.core.TrackerExecutor
import app.getvela.wallet.feature.send.core.TrackView
import app.getvela.wallet.feature.send.core.TrackStatus
import app.getvela.wallet.feature.send.core.TrackShellResult
import app.getvela.wallet.feature.send.core.TrackRecordStatus
import app.getvela.wallet.feature.send.core.TrackOperation
import app.getvela.wallet.feature.send.core.TrackEvent
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetView
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.ActivityFeedCore
import uniffi.vela_core_uniffi.PaymentRequestCore
import uniffi.vela_core_uniffi.ReceiveWatchCore
import uniffi.vela_core_uniffi.BalanceDashboardCore
import uniffi.vela_core_uniffi.TokenTrustCore

/**
 * The wallet home's machines, app-resident.
 *
 * Holds the pool — which is not a screen's to own, because every read-path
 * machine in the app shares its ban map and its endpoint statistics — and the
 * balance dashboard that reads through it.
 *
 * Created lazily by the composition root and never torn down: a machine
 * rebuilt per screen re-runs its boot, and on this screen that means re-reading
 * a dozen chains every time somebody switches tabs.
 */
private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

/**
 * Issue 188 (the web's feed resident): a transfer the scan just found moved
 * the balances too, so the hero refetches past the token cache. The core
 * sets `new_item_id` only for a genuinely-new incoming record, never on the
 * first pass — so this fires once per arrival: the row glows, the figure
 * follows. Before this the total stood still until the next focus.
 */
internal fun refreshOnArrival(scope: CoroutineScope, feed: StateFlow<FeedView>, refresh: () -> Unit) = scope.launch {
    var seen = feed.value.new_item_id
    feed.collect { view ->
        val id = view.new_item_id
        if (id != null && id != seen) refresh()
        seen = id
    }
}

class WalletController(
    context: Context,
    private val networks: StateFlow<NetView>,
    /**
     * The one way this app reads a chain.
     *
     * Passed in rather than built here. It was built here while the balance
     * dashboard was its only consumer; the currency rate is the second, and two
     * pools would mean two opinions about which endpoints are dead — the exact
     * thing this object exists to prevent.
     */
    val pool: RpcPool,
    private val scope: CoroutineScope,
    /** This person's other accounts — a counterparty name with no network call. */
    private val ownAccounts: () -> List<FeedExecutor.FeedOwnAccount> = { emptyList() },
    /** The money-in buzz. */
    private val haptic: () -> Unit = {},
    /**
     * Is the app in front of somebody?
     *
     * The receive watcher polls, and it must stop when the phone goes into a
     * pocket — the core stops the session the moment the shell says the app is
     * not active, so this answer is what ends the polling.
     */
    private val foreground: () -> Boolean = { true },
    /** Spec 043: the relay the tracker polls; `null` keeps the tracker off (tests of the read path). */
    relay: RelayClient? = null,
    /** A confirmation landed while the app was away: the platform's notification. */
    notifyConfirmed: (userOpHash: String, chainId: Int, txHash: String) -> Unit = { _, _, _ -> },
    /** Pending hashes remain and the app went to the background: keep polling from a worker. */
    backgroundPoll: () -> Unit = {},
) {

    private val store = VelaStore(context)

    /**
     * What each chain holds and how to price it — read from the ethereum-data
     * service the `network_admin` machine points at, so a person who redirects
     * that endpoint is honoured here too rather than by a second reader.
     */
    private val chains = ChainData(
        endpoint = {
            networks.value.endpoints
                .firstOrNull { it.field == NetEndpointField.EthereumData }
                ?.let { it.value.ifBlank { it.default_value } }
                .orEmpty()
        },
    )

    private val executor = BalanceExecutor(
        pool = pool,
        networks = networks,
        store = store,
        chainInfo = chains::forChain,
        mainnetPrices = ChainlinkPrices(pool)::prices,
    )

    private val balanceHost = CoreHost(
        bridge = BalanceDashboardCore().asBridge(),
        scope = scope,
        initial = BalanceView(),
        serializer = BalanceView.serializer(),
        perform = JsonShell.perform(
            BalanceOperation.serializer(),
            BalanceShellResult.serializer(),
            executor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            BalanceOperation.serializer(),
            BalanceShellResult.serializer(),
            fallback = BalanceShellResult.PrivacyWritten,
            answer = executor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("wallet.balance.fault", "core fault", error) },
    )

    // -- what arrives --------------------------------------------------------

    private val trustExecutor = TrustExecutor(
        pool = pool,
        store = store,
        // An admitted token must show up in the balances now, not when the
        // chain document's half-hour cache lapses.
        invalidate = { chains.clear() },
    )

    private val trustHost = CoreHost(
        bridge = TokenTrustCore().asBridge(),
        scope = scope,
        initial = TrustView(),
        serializer = TrustView.serializer(),
        perform = JsonShell.perform(
            TrustOperation.serializer(),
            TrustShellResult.serializer(),
            trustExecutor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            TrustOperation.serializer(),
            TrustShellResult.serializer(),
            fallback = TrustShellResult.CacheInvalidated,
            answer = trustExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("wallet.trust.fault", "core fault", error) },
    )

    /** Internal for the send path (spec 043): it writes the pending row here. */
    internal val feedExecutor = FeedExecutor(
        store = store,
        ownAccounts = ownAccounts,
        haptic = haptic,
    )

    /**
     * The bridge between the two machines.
     *
     * Its inputs are all facts about THIS device that no core can know: which
     * chains this person holds something on, which ERC-20s they already hold
     * (the trusted receive set), and what a chain's coin is called.
     */
    private val scanner = IncomingScan(
        trust = trustHost,
        feed = feedExecutor,
        chainInfo = chains::forChain,
        heldChains = {
            // Wait for the first balance read to settle — `last_refreshed_at_ms`
            // is the core's own "a read finished" stamp, and it is the only
            // signal that separates "holds nothing" from "has not looked yet".
            // A read that never settles falls through to whatever is known.
            withTimeoutOrNull(HELD_CHAINS_WAIT_MS) {
                balanceHost.view.first { it.last_refreshed_at_ms != null }
            }
            balanceHost.view.value.tokens.map { it.chain_id }.distinct()
        },
        heldTokens = { chainId ->
            balanceHost.view.value.tokens
                .filter { it.chain_id == chainId }
                .mapNotNull { it.token_address }
        },
        nativeSymbol = { chainId ->
            networks.value.networks.firstOrNull { it.chain_id.toInt() == chainId }
                ?.native_symbol
                .orEmpty()
        },
    )

    private val feedHost = CoreHost(
        bridge = ActivityFeedCore().asBridge(),
        scope = scope,
        initial = FeedView(),
        serializer = FeedView.serializer(),
        perform = JsonShell.perform(
            FeedOperation.serializer(),
            FeedShellResult.serializer(),
            feedExecutor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            FeedOperation.serializer(),
            FeedShellResult.serializer(),
            fallback = FeedShellResult.HapticPlayed,
            answer = feedExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("wallet.feed.fault", "core fault", error) },
    )

    // -- add a token (spec 043 T045) ------------------------------------------

    private val mtokExecutor = MtokExecutor(
        store = store,
        ethCall = { chainId, to, data ->
            (pool.call(chainId, "eth_call", listOf(JSONObject().put("to", to).put("data", data), "latest")) as? RpcResult.Body)
                ?.json?.optString("result")?.takeIf { it.startsWith("0x") && it != "0x" }
        },
        // The token cache is gone: the balance walk prices the new row.
        onInvalidated = { refresh(force = true) },
        haptic = haptic,
    )

    private val mtokHost = CoreHost(
        bridge = ManageTokensCore().asBridge(),
        scope = scope,
        initial = MtokView(),
        serializer = MtokView.serializer(),
        perform = JsonShell.perform(
            MtokOperation.serializer(),
            MtokShellResult.serializer(),
            mtokExecutor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            MtokOperation.serializer(),
            MtokShellResult.serializer(),
            fallback = MtokShellResult.CacheInvalidated,
            answer = mtokExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("wallet.mtok.fault", "core fault", error) },
    )

    /** The add-token sheet's state, the core's. */
    val manageTokens: StateFlow<MtokView> = mtokHost.view

    /** The sheet opened: load the already-added tokens. */
    fun openAddToken() = mtokHost.dispatch(MtokEvent.Start, MtokEvent.serializer())

    /**
     * A character typed or pasted. The core judges validity and clears the
     * cards; a well-formed address is looked up on every network at once.
     */
    fun addTokenInput(text: String) {
        mtokHost.dispatch(MtokEvent.AddressInput(text), MtokEvent.serializer())
        if (ADDRESS.matches(text.trim())) {
            val rows = networks.value.networks.map { MtokNetwork(chain_id = it.chain_id.toInt(), name = it.display_name) }
            if (rows.isNotEmpty()) mtokHost.dispatch(MtokEvent.DetectRequested(rows), MtokEvent.serializer())
        }
    }

    fun addTokenSave(chainId: Int) = mtokHost.dispatch(MtokEvent.SaveRequested(chainId), MtokEvent.serializer())

    fun deleteCustomToken(id: String) = mtokHost.dispatch(MtokEvent.DeleteRequested(id), MtokEvent.serializer())

    // -- the receive screen ---------------------------------------------------

    private val receiveExecutor = ReceiveExecutor(
        store = store,
        // The watcher compares BALANCES, so it reads the balance machine's own
        // view rather than starting a second read of the same chains. `null`
        // means "no read has settled yet" — a fetch failure, not an empty
        // wallet, which the core would otherwise see as everything withdrawn.
        snapshot = {
            balanceHost.view.value
                .takeIf { it.last_refreshed_at_ms != null }
                ?.tokens
                ?.map { token ->
                    TokenSnapshot(
                        id = "${token.chain_id}:${token.token_address ?: "native"}",
                        symbol = token.symbol,
                        chain_id = token.chain_id,
                        balance = token.balance.toDoubleOrNull() ?: 0.0,
                        price_usd = token.price_usd,
                    )
                }
        },
        foreground = foreground,
        haptic = haptic,
    )

    private val watchHost = CoreHost(
        bridge = ReceiveWatchCore().asBridge(),
        scope = scope,
        initial = ReceiveWatchView(),
        serializer = ReceiveWatchView.serializer(),
        perform = JsonShell.perform(
            ReceiveWatchOperation.serializer(),
            ReceiveWatchShellResult.serializer(),
            receiveExecutor::performWatch,
        ),
        escapedFailure = JsonShell.escapedFailure(
            ReceiveWatchOperation.serializer(),
            ReceiveWatchShellResult.serializer(),
            fallback = ReceiveWatchShellResult.Signalled,
            answer = receiveExecutor::neutralWatchAnswer,
        ),
        onFault = { error -> VelaLog.failure("wallet.watch.fault", "core fault", error) },
    )

    private val requestHost = CoreHost(
        bridge = PaymentRequestCore().asBridge(),
        scope = scope,
        initial = PaymentRequestView(),
        serializer = PaymentRequestView.serializer(),
        perform = JsonShell.perform(
            PaymentRequestOperation.serializer(),
            PaymentRequestShellResult.serializer(),
            receiveExecutor::performRequest,
        ),
        escapedFailure = JsonShell.escapedFailure(
            PaymentRequestOperation.serializer(),
            PaymentRequestShellResult.serializer(),
            fallback = PaymentRequestShellResult.AckWritten,
            answer = receiveExecutor::neutralRequestAnswer,
        ),
        onFault = { error -> VelaLog.failure("wallet.request.fault", "core fault", error) },
    )

    /** Money noticed while somebody is looking at their own QR code. */
    val watch: StateFlow<ReceiveWatchView> = watchHost.view

    /** What that QR code says. */
    val request: StateFlow<PaymentRequestView> = requestHost.view

    /**
     * Open the receive screen for an address.
     *
     * The watch is single-shot by design — the core runs one session, stops
     * after five minutes, and stops early if the app goes into a pocket. This
     * is called on every entry, and a second `start` while one is running is
     * the core's to ignore.
     */
    fun openReceive(address: String, payBaseUrl: String) {
        requestHost.dispatch(
            PaymentRequestEvent.Start(
                account = address,
                recipient = address,
                base_url = payBaseUrl,
            ),
            PaymentRequestEvent.serializer(),
        )
        watchHost.dispatch(ReceiveWatchEvent.Start, ReceiveWatchEvent.serializer())
    }

    fun receiveMode(mode: ReceiveMode) =
        requestHost.dispatch(PaymentRequestEvent.ModeChanged(mode), PaymentRequestEvent.serializer())

    fun receiveAmount(text: String) =
        requestHost.dispatch(
            PaymentRequestEvent.AmountChanged(text),
            PaymentRequestEvent.serializer(),
        )

    fun receiveAsset(token: BalanceToken, networkName: String) =
        requestHost.dispatch(
            PaymentRequestEvent.AssetPicked(
                chain_id = token.chain_id,
                token_address = token.token_address,
                symbol = token.symbol,
                decimals = token.decimals,
                network_name = networkName,
            ),
            PaymentRequestEvent.serializer(),
        )

    /** The warning gate's confirm. */
    fun acknowledgeReceive() =
        requestHost.dispatch(PaymentRequestEvent.Acknowledge, PaymentRequestEvent.serializer())

    // -- the tracker (spec 043 phase 4) ------------------------------------------------

    private val notifyPort: (String, Int, String) -> Unit = notifyConfirmed
    private val backgroundPollPort: () -> Unit = backgroundPoll

    private val trackerExecutor: TrackerExecutor? = relay?.let { relayClient ->
        TrackerExecutor(
            relay = relayClient,
            feed = feedExecutor,
            ports = object : TrackerExecutor.TrackerPorts {
                override fun recordsPatched(ids: List<String>, status: TrackRecordStatus, txHash: String?) {
                    feedReconciled(ids.size)
                }

                override fun notifyConfirmed(userOpHash: String, chainId: Int, txHash: String) {
                    // Only when nobody is looking: a person on the receipt page
                    // sees the verdict there.
                    if (!foreground()) notifyPort(userOpHash, chainId, txHash)
                }

                override fun receiptLogsConfirmed(from: String, chainId: Int, logs: List<TrustReceiptLog>) {
                    trustHost.dispatch(
                        TrustEvent.ReceiptLogsConfirmed(from = from, chain_id = chainId, logs = logs),
                        TrustEvent.serializer(),
                    )
                }
            },
        )
    }

    private val trackerHost: CoreHost<TrackView>? = trackerExecutor?.let { tracker ->
        CoreHost(
            bridge = TxTrackerCore().asBridge(),
            scope = scope,
            initial = TrackView(),
            serializer = TrackView.serializer(),
            perform = JsonShell.perform(TrackOperation.serializer(), TrackShellResult.serializer()) { operation ->
                tracker.perform(operation).also { result ->
                    VelaLog.event("tracker.arm", operation::class.simpleName ?: "?", "answer" to (result::class.simpleName ?: "?"))
                }
            },
            escapedFailure = JsonShell.escapedFailure(
                TrackOperation.serializer(),
                TrackShellResult.serializer(),
                fallback = TrackShellResult.Notified,
                answer = tracker::neutralAnswer,
            ),
            onFault = { error -> VelaLog.failure("wallet.tracker.fault", "core fault", error) },
        )
    }

    /** Every hash the core is following, with its verdict. */
    val tracker: StateFlow<TrackView> = trackerHost?.view ?: MutableStateFlow(TrackView())

    /**
     * A verdict for the send that is on screen: the three the send machine
     * accepts as `ReceiptUpdate`, once per change; a slow or unreachable poll
     * sends nothing (the desktop's `on_tracker`, invariant ⑤).
     */
    var onTrackVerdict: (userOpHash: String, status: TrackStatus, txHash: String?) -> Unit = { _, _, _ -> }

    private val lastVerdict = HashMap<String, TrackStatus>()

    /** The send machine's `track_submitted`: the row is on disk, follow the hash. */
    fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int) {
        val host = trackerHost
        if (host == null) {
            VelaLog.event("tracker", "no relay: not tracking", "hash" to userOpHash.take(12))
            return
        }
        host.dispatch(
            TrackEvent.Submitted(user_op_hash = userOpHash, record_ids = recordIds, chain_id = chainId),
            TrackEvent.serializer(),
        )
        // Device-found (spec 043 phase 4): a person taps confirm and leaves
        // before the relay answers. `backgrounded()` ran with nothing pending,
        // the submit landed afterwards, and no clock ticked until the next
        // resume. The handoff itself hands the worker the clock when nobody is
        // in front.
        if (!foreground()) backgroundPollPort()
    }

    /** The clock the core owns the cadence of; the shell only says "now". */
    fun trackerTick() {
        trackerHost?.dispatch(TrackEvent.Tick, TrackEvent.serializer())
    }

    /** Back in front: re-read what is still pending and poll it. */
    fun trackerResumed() {
        trackerHost?.dispatch(TrackEvent.AppResumed, TrackEvent.serializer())
    }

    /** Anything still without a verdict? (the background worker's stop condition) */
    fun trackerHasPending(): Boolean = tracker.value.entries.any { it.status == TrackStatus.Pending }

    init {
        // The knot: the executor emits `chain_assets_arrived` as each chain
        // lands, and the machine that consumes those events is built FROM the
        // executor. Tied here, once, rather than pretended away with a lazy.
        executor.stream = { event -> balanceHost.dispatch(event, BalanceEvent.serializer()) }
        // The second knot, the same shape: the scan writes through the feed's
        // own store, so it is built FROM the executor it then serves.
        feedExecutor.scan = { address -> scanner.runOnce(address) }
        // Issue 188: an incoming transfer moves the total too.
        refreshOnArrival(scope, feedHost.view) { refresh(force = true) }
        trackerHost?.let { host ->
            scope.launch {
                host.view.collect { view ->
                    view.entries.forEach { entry ->
                        val key = entry.user_op_hash.lowercase()
                        if (lastVerdict[key] == entry.status) return@forEach
                        lastVerdict[key] = entry.status
                        when (entry.status) {
                            TrackStatus.Confirmed, TrackStatus.Dropped, TrackStatus.Rejected, TrackStatus.FeeHeld ->
                                onTrackVerdict(entry.user_op_hash, entry.status, entry.tx_hash)
                            TrackStatus.Pending, TrackStatus.Unreachable, TrackStatus.AcceptedNotLanded -> Unit
                        }
                    }
                }
            }
            // The foreground clock: 3 s, as the web and desktop residents tick,
            // only while something is pending and somebody is looking.
            scope.launch {
                while (true) {
                    delay(TRACKER_TICK_MS)
                    if (foreground() && trackerHasPending()) host.dispatch(TrackEvent.Tick, TrackEvent.serializer())
                }
            }
        }
    }

    /** What the home screen renders. */
    val balances: StateFlow<BalanceView> = balanceHost.view

    /**
     * Spec 047 D8: the `/pay` grammar is the core's — `LinkOpened` on the
     * request machine, the answer read from `pay` / `pay_valid`. `null` = not a
     * request this wallet honours.
     */
    suspend fun validatePayLink(to: String?, chain: String?, token: String?, amount: String?, sym: String?, dec: String?, net: String?): PayRequest? {
        val before = requestHost.view.value
        requestHost.dispatch(
            PaymentRequestEvent.LinkOpened(to = to, chain = chain, token = token, amount = amount, sym = sym, dec = dec, net = net),
            PaymentRequestEvent.serializer(),
        )
        // The verdict of THIS link, not the one the model kept from the last:
        // the view the dispatch published is a new object; only when nothing
        // was published yet do we wait for it.
        val settled = requestHost.view.value.takeIf { it !== before }
            ?: kotlinx.coroutines.withTimeoutOrNull(5_000L) { requestHost.view.first { it !== before } }
        if (settled == null) {
            VelaLog.event("paylink", "no verdict from the core within 5 s")
            return null
        }
        return if (settled.pay_valid == true) settled.pay else null
    }

    /**
     * Spec 046 US1: the simulated deltas judged by the trust machine — the two
     * facts it judges a RECEIVED amount by are pushed first (what this wallet
     * holds on the chain, the registry's stables and wrapped native), then
     * `SimDeltasComputed`; the answer is read once `sim.ready`. Never a write.
     */
    suspend fun judgeSimDeltas(address: String, chainId: Int, deltas: List<TrustAssetDelta>): List<TrustSimJudgment>? {
        val held = balances.value.tokens
            .filter { it.chain_id == chainId }
            .mapNotNull { it.token_address?.lowercase() }
        trustHost.dispatch(TrustEvent.HeldTokensSnapshot(address = address.lowercase(), chain_id = chainId, tokens = held), TrustEvent.serializer())
        runCatching { chains.forChain(chainId) }.getOrNull()?.let { info ->
            trustHost.dispatch(
                TrustEvent.RegistryTokensSnapshot(
                    chain_id = chainId,
                    stables = info.stables.mapNotNull { it.contract?.lowercase() },
                    wrapped_native = info.wrappedNative?.lowercase(),
                ),
                TrustEvent.serializer(),
            )
        }
        trustHost.dispatch(TrustEvent.SimDeltasComputed(address = address.lowercase(), chain_id = chainId, deltas = deltas), TrustEvent.serializer())
        val settled = kotlinx.coroutines.withTimeoutOrNull(15_000L) {
            trustHost.view.first { view -> view.sim?.let { it.ready && it.chain_id == chainId && it.address.equals(address, ignoreCase = true) } == true }
        } ?: return null
        return settled.sim?.judgments
    }

    /** The activity feed: day-grouped rows, already in render order. */
    val feed: StateFlow<FeedView> = feedHost.view

    /**
     * Open the wallet for an address.
     *
     * Safe to call on every entry: the core supersedes an in-flight read rather
     * than stacking another one.
     */
    suspend fun open(address: String) {
        pool.start()
        val hidden = executor.storedPrivacy()
        balanceHost.dispatch(BalanceEvent.PrivacyHydrated(hidden), BalanceEvent.serializer())
        balanceHost.dispatch(BalanceEvent.AccountChanged(address), BalanceEvent.serializer())
        // The feed's own privacy rule: while figures are hidden the receipt
        // toast is suppressed — but the row still glows and the phone still
        // buzzes. That distinction is the core's, and it needs telling.
        feedHost.dispatch(FeedEvent.PrivacyChanged(hidden), FeedEvent.serializer())
        feedHost.dispatch(FeedEvent.AccountSwitched(address), FeedEvent.serializer())
        trackerResumed()
    }

    /** A pull-to-refresh, or the screen coming back into view. */
    /** Spec 048: a token detail's 收款 — the request machine shows THAT asset's code. */
    fun assetPicked(chainId: Int, tokenAddress: String?, symbol: String, decimals: Int, networkName: String) {
        VelaLog.event("receive", "asset picked", "chain" to chainId, "symbol" to symbol)
        requestHost.dispatch(PaymentRequestEvent.AssetPicked(chain_id = chainId, token_address = tokenAddress, symbol = symbol, decimals = decimals, network_name = networkName), PaymentRequestEvent.serializer())
    }

    /** Spec 047 (the founder, 2026-09-12): the home's account switcher — the core keeps the per-account totals it shows. */
    fun switcherOpened(addresses: List<String>) = balanceHost.dispatch(BalanceEvent.SwitcherOpened(addresses), BalanceEvent.serializer())

    fun switcherClosed() = balanceHost.dispatch(BalanceEvent.SwitcherClosed, BalanceEvent.serializer())

    fun refresh(force: Boolean = false, pull: Boolean = false) =
        balanceHost.dispatch(
            BalanceEvent.RefreshRequested(force = force, pull = pull),
            BalanceEvent.serializer(),
        )

    fun togglePrivacy() {
        balanceHost.dispatch(BalanceEvent.PrivacyToggled, BalanceEvent.serializer())
        feedHost.dispatch(
            FeedEvent.PrivacyChanged(!balanceHost.view.value.hidden),
            FeedEvent.serializer(),
        )
    }

    fun focused() {
        balanceHost.dispatch(BalanceEvent.AppFocused, BalanceEvent.serializer())
        feedHost.dispatch(FeedEvent.FocusTick, FeedEvent.serializer())
        trackerResumed()
    }

    fun backgrounded() {
        balanceHost.dispatch(BalanceEvent.AppBackgrounded, BalanceEvent.serializer())
        if (trackerHasPending()) backgroundPollPort()
    }

    /** The Activity surface's near-real-time poll, while it is on screen. */
    fun liveTick() = feedHost.dispatch(FeedEvent.LiveTick, FeedEvent.serializer())

    /**
     * Rows changed under the feed — a send wrote its pending row (spec 043),
     * the tracker patched a verdict (phase 4): re-read the store now rather
     * than at the next focus tick, so the home shows the row at submit.
     */
    fun feedReconciled(resolved: Int = 0) =
        feedHost.dispatch(FeedEvent.ReconcileCompleted(resolved_count = resolved), FeedEvent.serializer())

    /** The network chip filter; `null` = every chain. */
    fun filterChain(chainId: Int?) =
        feedHost.dispatch(FeedEvent.ChainFilterChanged(chainId), FeedEvent.serializer())

    /** Remove one row: optimistic in the core, persisted by the executor. */
    fun deleteActivity(id: String) =
        feedHost.dispatch(FeedEvent.DeleteRequested(id), FeedEvent.serializer())

    private companion object {
        /**
         * How long a receipt scan waits for the balance read that tells it
         * which chains to watch. Long enough for a dozen chains on a phone
         * connection, short enough that a stalled read does not hold the scan
         * open — it proceeds with what it knows, and the next tick asks again.
         */
        const val HELD_CHAINS_WAIT_MS = 12_000L

        /** The residents' tick (web `TICK_MS`, desktop `TICK`). */
        const val TRACKER_TICK_MS = 3_000L
    }
}

