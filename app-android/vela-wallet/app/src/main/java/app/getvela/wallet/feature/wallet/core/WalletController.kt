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
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.dropWhile
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
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
import app.getvela.wallet.feature.send.core.TrackHandoff
import app.getvela.wallet.feature.send.core.submitFacts
import app.getvela.wallet.feature.send.core.TrackEntryView
import app.getvela.wallet.feature.send.core.TrackOutcome
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
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

/**
 * Spec 105: [view] with only the space in front's holdings — [chains], the
 * space's chain ids; `null` before the network list is read (nothing to
 * narrow by). A total that counted a holding of the other space is not shown
 * (`display_total_usd` withheld): the switch reads the balances again, and
 * until that round lands the other space's coins never stand in for these.
 */
internal fun inSpace(view: BalanceView, chains: Set<Int>?): BalanceView {
    if (chains == null) return view
    val tokens = view.tokens.filter { it.chain_id in chains }
    val unpriced = view.unpriced_tokens.filter { it.chain_id in chains }
    if (tokens.size == view.tokens.size && unpriced.size == view.unpriced_tokens.size) return view
    return view.copy(tokens = tokens, unpriced_tokens = unpriced, display_total_usd = null)
}

class WalletController(
    context: Context,
    /**
     * The networks of the space in front (spec 105) — the chains the balances
     * read, receive offers and a token is looked up on. The pool reads every
     * chain's endpoints from the whole list, not from this.
     */
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
    /** Spec 105: the testnet space is in front — no saved total is read or written. */
    private val testnet: () -> Boolean = { false },
    /** r9-17 (core §9.4): the networks [address] lives on — a multisig's record's; empty for a Vela account. */
    private val accountNetworks: (address: String) -> List<Int> = { emptyList() },
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
        chainInfo = chains::read,
        mainnetPrices = ChainlinkPrices(pool)::prices,
        testnet = testnet,
    )

    /**
     * P2.4: the rows the home last showed, per account and space — handed
     * to the core (`holdings_restored`) so the list opens on what was last
     * known (the web's `holdings-cache.ts`).
     */
    private val holdingsCache = HoldingsCache(store)

    /** The space in front, as the holdings cache keys it (`mainnet` / `testnet`). */
    private fun spaceKey(): String = if (testnet()) "testnet" else "mainnet"

    /**
     * The rows kept for the account and space in front, handed to the core,
     * which lets them stand until each chain answers (web `#restoreRows`).
     * The core ignores them for another account, or once a round settled.
     */
    private suspend fun restoreRows(address: String? = balanceHost.view.value.address) {
        if (address == null) return
        val tokens = runCatching { holdingsCache.load(address, spaceKey()) }.getOrDefault(emptyList())
        if (tokens.isNotEmpty()) {
            balanceHost.dispatch(BalanceEvent.HoldingsRestored(address, tokens), BalanceEvent.serializer())
        }
    }

    /** The last `FetchSettled` the executor answered: whose, and its `now_ms`. */
    private val lastSettle = java.util.concurrent.atomic.AtomicReference<Pair<String, Double>?>(null)

    /** The balances' view as the core wrote it — the AI agent reads the wallet whole (spec 104). */
    val balanceJson: StateFlow<String?> get() = balanceHost.viewJson

    private val balanceHost = CoreHost(
        bridge = BalanceDashboardCore().asBridge(),
        scope = scope,
        initial = BalanceView(),
        serializer = BalanceView.serializer(),
        perform = JsonShell.perform(
            BalanceOperation.serializer(),
            BalanceShellResult.serializer(),
        ) { operation ->
            executor.perform(operation).also { result ->
                // Which round is about to settle, and its stamp: the view says
                // it settled once it carries this stamp for this account.
                if (result is BalanceShellResult.FetchSettled) lastSettle.set(result.address to result.now_ms)
            }
        },
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
        // PR 3: a stored receipt's block time is read through the same pool
        // the trust machine's reads go through.
        pool = pool,
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
        accountNetworks = accountNetworks,
        spaceChainIds = { networks.value.networks.map { it.chain_id.toInt() } },
        addedChainIds = { networks.value.networks.filter { it.is_custom }.map { it.chain_id.toInt() } },
        store = store,
    )

    /** The activity feed's view as the core wrote it — the AI agent reads the wallet whole (spec 104). */
    val feedJson: StateFlow<String?> get() = feedHost.viewJson

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
        // The payment the person is waiting for shows in the history now: the
        // feed reads its rows again and scans (`live_tick`, as the web's watcher
        // does), without waiting out the scan's back-off.
        deposited = { feedHost.dispatch(FeedEvent.LiveTick, FeedEvent.serializer()) },
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

    /** Spec 090: the "include network" switch — the core decides what the code then says. */
    fun includeNetwork(include: Boolean) =
        requestHost.dispatch(
            PaymentRequestEvent.IncludeNetworkChanged(include),
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

                // Spec 082 RE8: an op of ours landed (or failed with gas
                // spent) — the balance moved; read it now, not at the next pull.
                override fun holdingsMoved(chainId: Int) {
                    refresh(force = true)
                }
            },
        )
    }

    /** The chain tracker's view as the core wrote it (`null` without a tracker) — for the AI agent's transfers (spec 104). */
    val trackerJson: String? get() = trackerHost?.viewJson?.value

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

    private val _inFlightOps = MutableStateFlow<List<app.getvela.wallet.feature.send.core.InFlightOp>>(emptyList())

    /**
     * The operations holding their account's nonce (one in flight per
     * account and network), as the core reads them — `inFlightOps` of the
     * tracker's view JSON EXACTLY as the core wrote it ([CoreHost.viewJson]).
     * Never of a re-encoded [TrackView]: a mirror that dropped `sender` would
     * hold nothing, one that dropped `stalled` would hold forever. Updated on
     * every tracker render; the send and signing machines are told each new
     * list (they dedupe an identical one).
     */
    val inFlightOps: StateFlow<List<app.getvela.wallet.feature.send.core.InFlightOp>> = _inFlightOps

    /** `inFlightOps` of one tracker view JSON; `[]` when the core reads nothing from it. */
    private fun inFlightOf(viewJson: String): List<app.getvela.wallet.feature.send.core.InFlightOp> =
        runCatching {
            app.getvela.wallet.core.crux.Wire.json.decodeFromString(
                kotlinx.serialization.builtins.ListSerializer(app.getvela.wallet.feature.send.core.InFlightOp.serializer()),
                uniffi.vela_core_uniffi.inFlightOps(viewJson),
            )
        }.onFailure { VelaLog.failure("tracker.inflight", "could not read the ops in flight", it) }
            .getOrDefault(emptyList())

    /**
     * A tracked op whose standing changed — its status OR its outcome (spec
     * 082: a lost reply the relay then acknowledges changes only the outcome).
     * What that means for the send on screen is the core's
     * (`sendReceiptOutcomeOf`), not this file's.
     */
    var onTrackVerdict: (TrackEntryView) -> Unit = { _ -> }

    private val lastVerdict = HashMap<String, Pair<TrackStatus, TrackOutcome>>()

    /**
     * The send machine's `track_submitted` (or a dApp's `tracker_handoff`):
     * the rows are on disk, follow the hash — as a lost reply when it was one
     * (spec 082).
     */
    fun trackSubmitted(handoff: TrackHandoff) {
        val host = trackerHost
        if (host == null) {
            VelaLog.event("tracker", "no relay: not tracking", "hash" to handoff.userOpHash.take(12))
            return
        }
        VelaLog.event(
            "tracker", "handed",
            "op" to handoff.userOpHash.take(12), "chain" to handoff.chainId,
            "maybeSent" to handoff.maybeSent, "admitted" to handoff.admitted, "submitBlock" to handoff.submitBlock,
            "sender" to handoff.sender?.take(10),
        )
        host.dispatch(handoff.event(), TrackEvent.serializer())
        // r10 follow-up (core §9.4): the person sent from the account on
        // screen — scan for what came back (a swap's other half, a refund)
        // at once, and start the back-off again. The send's own row reaches
        // the feed by its record (`ReconcileCompleted`), not by this.
        val shown = balanceHost.view.value.address
        if (followingCadence && (handoff.sender == null || handoff.sender.equals(shown, ignoreCase = true))) {
            presence(FeedPresenceChange.Sent)
        }
        // Round 3 (send-5): a lost reply's nonce — the tracker reads the chain
        // with it instead of waiting the relay's grace out.
        handoff.submitFacts()?.let { host.dispatch(it, TrackEvent.serializer()) }
        // Device-found (spec 043 phase 4): a person taps confirm and leaves
        // before the relay answers. `backgrounded()` ran with nothing pending,
        // the submit landed afterwards, and no clock ticked until the next
        // resume. The handoff itself hands the worker the clock when nobody is
        // in front.
        if (!foreground()) backgroundPollPort()
    }

    /**
     * Spec 082 RJ1: a written-ahead op proven never sent (the sign machine's
     * `tracker_withdraw`, the send machine's `track_withdrawn`) — the tracker
     * drops those rows, patches nothing, reads no balance.
     */
    fun trackWithdrawn(userOpHash: String, recordIds: List<String>) {
        val host = trackerHost ?: return
        VelaLog.event("tracker", "withdrawn: never sent", "op" to userOpHash.take(12), "rows" to recordIds.size)
        host.dispatch(TrackEvent.Withdrawn(user_op_hash = userOpHash, record_ids = recordIds), TrackEvent.serializer())
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

    /**
     * Anything the core is still following — in its wait window OR past it
     * (spec 079). The foreground clock used to stop at the window's end, when
     * the status turns `accepted_not_landed`: an op still on its way was then
     * asked about only on the next resume, and "Vela keeps checking" was not
     * true (device-found: a landed op never showed as landed). The core paces
     * the asking itself — 3 s, 12 s, then once a minute, then every five — so
     * ticking while `polling` costs nothing it did not decide to spend.
     */
    fun trackerFollowing(): Boolean = tracker.value.entries.any { it.polling }

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
            // One in flight per account and network: every render's ops, read
            // by the core from its own view JSON (never the mirror).
            scope.launch {
                host.viewJson.collect { json ->
                    if (json == null) return@collect
                    val ops = inFlightOf(json)
                    if (ops != _inFlightOps.value) {
                        VelaLog.event("tracker.inflight", "ops", "n" to ops.size, "chains" to ops.joinToString(",") { it.chain_id.toString() })
                    }
                    _inFlightOps.value = ops
                }
            }
            scope.launch {
                host.view.collect { view ->
                    view.entries.forEach { entry ->
                        val key = entry.user_op_hash.lowercase()
                        val standing = entry.status to entry.outcome
                        if (lastVerdict[key] == standing) return@forEach
                        lastVerdict[key] = standing
                        onTrackVerdict(entry)
                    }
                }
            }
            // The foreground clock: 3 s, as the web and desktop residents tick,
            // while the core is still following something and somebody is
            // looking (spec 079: not only inside the wait window).
            scope.launch {
                while (true) {
                    delay(TRACKER_TICK_MS)
                    if (foreground() && trackerFollowing()) host.dispatch(TrackEvent.Tick, TrackEvent.serializer())
                }
            }
        }
    }

    /**
     * The space in front's chains (spec 105), `null` before the network list
     * is read — what the balances and the feed are narrowed to.
     */
    private fun spaceChains(view: NetView): Set<Int>? =
        view.takeIf { it.loaded && it.networks.isNotEmpty() }?.networks?.map { it.chain_id.toInt() }?.toSet()

    /** What the home screen renders: the space in front's holdings (spec 105). */
    val balances: StateFlow<BalanceView> = combine(balanceHost.view, networks) { view, nets -> inSpace(view, spaceChains(nets)) }
        .stateIn(scope, SharingStarted.Eagerly, inSpace(balanceHost.view.value, spaceChains(networks.value)))

    private val _holdingsRound = MutableStateFlow<HoldingsRound?>(null)

    /**
     * The last balance round the dashboard SETTLED, with the view it settled
     * into — `null` until one has. Settled means the machine took the round:
     * its view carries that round's stamp (`last_refreshed_at_ms`) for that
     * account. The stamp alone cannot say so — it survives an account switch,
     * so "a read finished" would be true of an account nobody has read yet.
     *
     * The asset list and Send read the SAME holdings through this (spec 078):
     * Send answers its picker from a settled round and follows every later one.
     */
    val holdingsRound: StateFlow<HoldingsRound?> = _holdingsRound

    /**
     * A settled round's networks with something held (spam is not money),
     * added to [KeyValueStore.Keys.HELD_CHAIN_IDS] — the evidence the
     * one-time settling of the shown networks reads (`shown_networks::settle`:
     * never hide a network that holds the person's money, as far as the
     * device knows). Written once a round has settled even when empty, so the
     * settling knows the holdings were read.
     */
    private fun noteHeldChains(view: BalanceView) {
        val held = (view.tokens + view.unpriced_tokens)
            .filter { !it.spam && it.balance.any { c -> c in '1'..'9' } }
            .map { it.chain_id.toLong() }
        scope.launch {
            runCatching {
                val before = store.read(KeyValueStore.Keys.HELD_CHAIN_IDS)
                    ?.let { raw -> runCatching { org.json.JSONArray(raw) }.getOrNull() }
                    ?.let { array -> (0 until array.length()).mapNotNull { array.opt(it) as? Number }.map { it.toLong() } }
                val union = ((before ?: emptyList()) + held).distinct().sorted()
                if (before == null || union.size != before.size) {
                    store.write(KeyValueStore.Keys.HELD_CHAIN_IDS, org.json.JSONArray(union).toString())
                }
            }
        }
    }

    /** The asset list's holdings, as Send reads them. */
    val holdings: HoldingsFeed = object : HoldingsFeed {
        override val view: StateFlow<BalanceView> = balances
        override val settled: StateFlow<HoldingsRound?> = _holdingsRound
    }

    init {
        // Every commit, not every distinct view (`CoreHost.commits`).
        scope.launch {
            balanceHost.commits.collect {
                val view = balanceHost.view.value
                val (address, stamp) = lastSettle.get() ?: return@collect
                if (view.address.equals(address, ignoreCase = true) &&
                    view.last_refreshed_at_ms == stamp &&
                    _holdingsRound.value?.let { it.atMs == stamp && it.address.equals(address, ignoreCase = true) } != true
                ) {
                    _holdingsRound.value = HoldingsRound(address = address, atMs = stamp, view = inSpace(view, spaceChains(networks.value)))
                    noteHeldChains(view)
                    // P2.4: a settled round's rows are kept for the next visit (home-1, home-3).
                    val space = spaceKey()
                    scope.launch { runCatching { holdingsCache.save(address, space, view.tokens) } }
                }
            }
        }
        // Spec 105: the feed draws only the space in front's records — told on
        // boot (once the list is read) and on every change of the list, which a
        // switch of space is. The other space's records stay stored.
        scope.launch {
            networks.map(::spaceChains).distinctUntilChanged().collect { chains ->
                if (chains != null) feedHost.dispatch(FeedEvent.SpaceChainsChanged(chains.toList()), FeedEvent.serializer())
            }
        }
        // r12-3 (core §13.2): the pool's networks out of reach, told to the
        // balance machine on every change — the incoming scan, a balance
        // watch or a send may be the read that lost one, and the home must not
        // stay "Live" over it. The leading empty set is the machine's own
        // starting word, so it is not said.
        scope.launch {
            pool.view.map { it.out_of_reach_chains }.distinctUntilChanged().dropWhile { it.isEmpty() }.collect(::poolUnreached)
        }
    }

    /**
     * r12-3 (core §13.2, `pool_unreached`): the networks the RPC pool finds
     * out of reach now. The core reads again at once, forced, a network the
     * shown account is read on that the home does not say yet — or owes it
     * to the round already out, or to the next focus.
     */
    fun poolUnreached(chainIds: List<Int>) {
        VelaLog.event("balance", "pool unreached", "chains" to chainIds)
        balanceHost.dispatch(BalanceEvent.PoolUnreached(chainIds), BalanceEvent.serializer())
    }

    /**
     * Spec 105: the space in front changed — every chain read again (forced:
     * past the background gate), so the total is the new space's.
     */
    fun spaceChanged() {
        VelaLog.event("balance", "space changed")
        // P2.4: the core starts the account over for the new space (home-7) —
        // its cache, a forced read — and the rows kept for that space open
        // the list (web `networkSpace.subscribe` → `space_changed`).
        balanceHost.dispatch(BalanceEvent.SpaceChanged, BalanceEvent.serializer())
        scope.launch { restoreRows() }
    }

    /**
     * Spec 047 D8: the `/pay` grammar is the core's — `LinkOpened` on the
     * request machine, the answer read from `pay` / `pay_valid`, and why not
     * from `pay_refusal` — the person is told what is wrong with the link.
     */
    suspend fun validatePayLink(to: String?, chain: String?, token: String?, amount: String?, sym: String?, dec: String?, net: String?): PayVerdict {
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
            return PayVerdict.Refused(null)
        }
        return PayVerdict.of(settled)
    }

    /**
     * Spec 046 US1: the simulated deltas judged by the trust machine — the two
     * facts it judges a RECEIVED amount by are pushed first (what this wallet
     * holds on the chain, the registry's stables and wrapped native), then
     * `SimDeltasComputed`; the answer is read once `sim.ready`. Never a write.
     * The judged view whole: its judgments, and the line the core says when
     * nothing of the person's moves (`no_change_key`).
     */
    suspend fun judgeSimDeltas(address: String, chainId: Int, deltas: List<TrustAssetDelta>): TrustSimView? {
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
        return settled.sim
    }

    /**
     * The activity feed: day-grouped rows, already in render order — as the
     * screens draw it. `next_scan` is the cadence's (this controller follows
     * it on [feedHost] directly) and is left out here: a quiet scan's one
     * render changes only the timer, and handed on it redrew the whole wallet
     * route — the 200-row history mid-fling (a 60–70 ms frame on a Redmi
     * K40, every scan). Equal views are not emitted again.
     */
    val feed: StateFlow<FeedView> = feedHost.view
        .map { view -> if (view.next_scan == null) view else view.copy(next_scan = null) }
        .stateIn(scope, SharingStarted.Eagerly, feedHost.view.value.copy(next_scan = null))

    /**
     * Open the wallet for an address.
     *
     * Safe to call on every entry: the core supersedes an in-flight read rather
     * than stacking another one.
     */
    suspend fun open(address: String, livesOn: List<Int> = emptyList()) {
        pool.start()
        val hidden = executor.storedPrivacy()
        balanceHost.dispatch(BalanceEvent.PrivacyHydrated(hidden), BalanceEvent.serializer())
        val changed = balanceHost.view.value.address != address
        // r10-1 (core §9.3): where it lives first, so the stamp of this
        // account moves only when those networks answered.
        balanceHost.dispatch(BalanceEvent.LivesOn(address, livesOn), BalanceEvent.serializer())
        balanceHost.dispatch(BalanceEvent.AccountChanged(address), BalanceEvent.serializer())
        // P2.4: the first frame of another account is what it held last time
        // (web `setAccount` → `#restoreRows`). Queued after the switch, so the
        // core reads it for this account.
        if (changed) restoreRows(address)
        // The feed's own privacy rule: while figures are hidden the receipt
        // toast is suppressed — but the row still glows and the phone still
        // buzzes. That distinction is the core's, and it needs telling.
        feedHost.dispatch(FeedEvent.PrivacyChanged(hidden), FeedEvent.serializer())
        feedHost.dispatch(FeedEvent.AccountSwitched(address), FeedEvent.serializer())
        // r9-17 (core §9.4): from the first open on, the scan's cadence is the
        // core's (`next_scan`); this shell only says how the page stands.
        if (!followingCadence) {
            followingCadence = true
            presence(FeedPresenceChange.Started)
            followCadence()
        }
        trackerResumed()
    }

    // -- r9-17: the incoming scan's cadence is the core's (core §9.4) ----------

    @Volatile private var followingCadence = false
    @Volatile private var lastInteractionMs = System.currentTimeMillis()
    private var scanTimer: kotlinx.coroutines.Job? = null

    private fun presenceNow(): FeedPresence {
        val now = System.currentTimeMillis()
        return FeedPresence(visible = foreground(), since_interaction_ms = (now - lastInteractionMs).coerceAtLeast(0).toDouble(), now_ms = now.toDouble())
    }

    private fun presence(change: FeedPresenceChange) =
        feedHost.dispatch(FeedEvent.PresenceChanged(presenceNow(), change), FeedEvent.serializer())

    /** ONE timer per `next_scan.seq`, answered with `scan_due` carrying the same seq; a new seq drops the old timer. */
    private fun followCadence() {
        scope.launch {
            feedHost.view.map { it.next_scan }.distinctUntilChanged().collect { next ->
                scanTimer?.cancel()
                if (next == null) return@collect
                scanTimer = scope.launch {
                    kotlinx.coroutines.delay(next.after_ms.coerceAtLeast(0))
                    feedHost.dispatch(FeedEvent.ScanDue(next.seq, presenceNow()), FeedEvent.serializer())
                }
            }
        }
    }

    /** The person touched the app (a tap, a scroll): back from idle — the core's active pace. Said at most every few seconds. */
    fun interacted() {
        val now = System.currentTimeMillis()
        val idle = now - lastInteractionMs
        lastInteractionMs = now
        if (followingCadence && idle > INTERACTION_SAID_MS) presence(FeedPresenceChange.Interacted)
    }

    /**
     * r10 follow-up (core §9.4): a scroll, fling or drag on the home or the
     * history started ([on]) or settled. A scan that comes due meanwhile is
     * held by the core and runs once the list is still, so no scan lands in
     * the middle of a gesture. Said on the main thread, at each edge only.
     */
    fun interacting(on: Boolean) {
        if (on) lastInteractionMs = System.currentTimeMillis()
        if (followingCadence) feedHost.dispatch(FeedEvent.Interacting(on, presenceNow()), FeedEvent.serializer())
    }

    /** Back online: what failed offline can be read now. */
    fun online() {
        if (followingCadence) presence(FeedPresenceChange.Online)
    }

    /**
     * P2.4 (perf-10): the signed-in account, for a screen that draws no
     * balance of its own — Settings, the address book (the web's
     * `noteAccount`). The core sets it and reads its cached total, so the
     * switcher and the privacy mask are right, but asks no network until
     * something shows the balance. Nothing for the account already set.
     */
    suspend fun noteAccount(address: String) {
        if (address.isBlank()) return
        val changed = balanceHost.view.value.address != address
        if (changed) {
            val hidden = executor.storedPrivacy()
            balanceHost.dispatch(BalanceEvent.PrivacyHydrated(hidden), BalanceEvent.serializer())
        }
        balanceHost.dispatch(BalanceEvent.AccountNoted(address), BalanceEvent.serializer())
        if (changed) restoreRows(address)
    }

    /** Spec 048: a token detail's 收款 — the request machine shows THAT asset's code. */
    fun assetPicked(chainId: Int, tokenAddress: String?, symbol: String, decimals: Int, networkName: String) {
        VelaLog.event("receive", "asset picked", "chain" to chainId, "symbol" to symbol)
        requestHost.dispatch(PaymentRequestEvent.AssetPicked(chain_id = chainId, token_address = tokenAddress, symbol = symbol, decimals = decimals, network_name = networkName), PaymentRequestEvent.serializer())
    }

    /** Spec 047 (the founder, 2026-09-12): the home's account switcher — the core keeps the per-account totals it shows. */
    fun switcherOpened(addresses: List<String>) = balanceHost.dispatch(BalanceEvent.SwitcherOpened(addresses), BalanceEvent.serializer())

    fun switcherClosed() = balanceHost.dispatch(BalanceEvent.SwitcherClosed, BalanceEvent.serializer())

    /**
     * The RPC fix worked for this chain (the rescue sheet watched its probe
     * answer): the core drops it from the failed set and reads again — the
     * web's `balance.fixChainResolved`. Without this the hero kept naming a
     * chain the person had just repaired until the next throttled refresh.
     */
    fun fixChainResolved(chainId: Int) {
        VelaLog.event("balance", "fix chain resolved", "chain" to chainId)
        balanceHost.dispatch(BalanceEvent.FixChainResolved(chainId), BalanceEvent.serializer())
    }

    /**
     * Spec 092: the list of networks the wallet cannot reach is on screen. The
     * core reads every chain again now and keeps re-reading while it stays
     * open, so one that has come back leaves it (and the home's count).
     */
    fun unreachableListOpened() =
        balanceHost.dispatch(BalanceEvent.UnreachableListOpened, BalanceEvent.serializer())

    /** That list closed: the re-reads stop. */
    fun unreachableListClosed() =
        balanceHost.dispatch(BalanceEvent.UnreachableListClosed, BalanceEvent.serializer())

    /**
     * r13 S4: a multisig execution landed (its Activity rows are written).
     * The multisig's holdings moved: read them now, past the background gate
     * — what the tracker's `holdingsMoved` does for an op of a Vela account's
     * (Assets kept 0.5 USDC ~2.5 min while the chain said 0.49). And the
     * feed hears `sent` (core §9.4): the scan looks for what came back.
     * Read once more a few seconds on: an endpoint a block behind the one
     * that gave the receipt answers the old figure, and nothing else would
     * read it again before the next poll.
     */
    fun multisigExecuted() {
        refresh(force = true)
        if (followingCadence) presence(FeedPresenceChange.Sent)
        scope.launch {
            kotlinx.coroutines.delay(LANDED_REREAD_MS)
            refresh(force = true)
        }
    }

    fun refresh(force: Boolean = false, pull: Boolean = false) =
        balanceHost.dispatch(
            BalanceEvent.RefreshRequested(force = force, pull = pull),
            BalanceEvent.serializer(),
        )

    /**
     * The person asked for fresh figures (issue 462): the hero's "↻ Updated"
     * control, or the pull gesture. What iOS's pull-to-refresh sends —
     * every chain read again past the background gate (`force`), marked as
     * the person's own (`pull`, so the core's `refreshing` holds until THIS
     * round settles and the control turns until then) — and the activity
     * tick, so a payment that landed shows in the list too.
     */
    fun pullRefresh() {
        VelaLog.event("balance", "pull refresh")
        refresh(force = true, pull = true)
        // r10 follow-up (core §9.4): the person's own refresh — the store
        // read again and a scan at once, and the scan's back-off starts over.
        if (followingCadence) presence(FeedPresenceChange.Refreshed) else feedHost.dispatch(FeedEvent.FocusTick, FeedEvent.serializer())
    }

    fun togglePrivacy() {
        balanceHost.dispatch(BalanceEvent.PrivacyToggled, BalanceEvent.serializer())
        feedHost.dispatch(
            FeedEvent.PrivacyChanged(!balanceHost.view.value.hidden),
            FeedEvent.serializer(),
        )
    }

    fun focused() {
        balanceHost.dispatch(BalanceEvent.AppFocused, BalanceEvent.serializer())
        // r9-17: shown again — the core scans at once (its cadence, not a tick of ours).
        if (followingCadence) presence(FeedPresenceChange.Shown) else feedHost.dispatch(FeedEvent.FocusTick, FeedEvent.serializer())
        trackerResumed()
    }

    fun backgrounded() {
        balanceHost.dispatch(BalanceEvent.AppBackgrounded, BalanceEvent.serializer())
        if (followingCadence) presence(FeedPresenceChange.Hidden)
        if (trackerHasPending()) backgroundPollPort()
    }

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

    /** Spec 093: a contact's page opened (its address) or closed (`null`); the feed then carries its rows. */
    fun filterContact(address: String?) =
        feedHost.dispatch(FeedEvent.ContactFilterChanged(address), FeedEvent.serializer())

    /** Remove one row: optimistic in the core, persisted by the executor. */
    fun deleteActivity(id: String) =
        feedHost.dispatch(FeedEvent.DeleteRequested(id), FeedEvent.serializer())

    /** Spec 093: a dApp record's stored request, read by id when its technical details open. */
    suspend fun storedRequest(id: String): String? = feedExecutor.storedRequest(id)

    private companion object {
        /** An interaction is said to the core at most this often (the cadence's active window is two minutes). */
        const val INTERACTION_SAID_MS = 5_000L
        /** r13 S4: a landed multisig execution's balances are read again this long after the first read. */
        const val LANDED_REREAD_MS = 6_000L
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

/** One balance round the dashboard settled: whose, when (`last_refreshed_at_ms`), and the view it settled into. */
data class HoldingsRound(val address: String, val atMs: Double, val view: BalanceView)

/**
 * The asset list's holdings, as another screen reads them — one source, the
 * balance machine: [view] live (a first round still streaming in), [settled]
 * the last round it took.
 */
interface HoldingsFeed {
    val view: StateFlow<BalanceView>
    val settled: StateFlow<HoldingsRound?>
}
