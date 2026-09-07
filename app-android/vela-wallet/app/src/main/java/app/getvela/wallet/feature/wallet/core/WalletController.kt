package app.getvela.wallet.feature.wallet.core

import android.content.Context
import app.getvela.wallet.core.crux.CoreHost
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
class WalletController(
    context: Context,
    private val networks: StateFlow<NetView>,
    private val scope: CoroutineScope,
    /** This person's other accounts — a counterparty name with no network call. */
    private val ownAccounts: () -> List<FeedExecutor.FeedOwnAccount> = { emptyList() },
    /** The money-in buzz. */
    private val haptic: () -> Unit = {},
) {

    private val store = VelaStore(context)

    /** The one way this app reads a chain. */
    val pool = RpcPool(
        store = store,
        endpoints = NetworkEndpointSource(networks),
        scope = scope,
    )

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

    private val feedExecutor = FeedExecutor(
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

    init {
        // The knot: the executor emits `chain_assets_arrived` as each chain
        // lands, and the machine that consumes those events is built FROM the
        // executor. Tied here, once, rather than pretended away with a lazy.
        executor.stream = { event -> balanceHost.dispatch(event, BalanceEvent.serializer()) }
        // The second knot, the same shape: the scan writes through the feed's
        // own store, so it is built FROM the executor it then serves.
        feedExecutor.scan = { address -> scanner.runOnce(address) }
    }

    /** What the home screen renders. */
    val balances: StateFlow<BalanceView> = balanceHost.view

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
    }

    /** A pull-to-refresh, or the screen coming back into view. */
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
    }

    fun backgrounded() =
        balanceHost.dispatch(BalanceEvent.AppBackgrounded, BalanceEvent.serializer())

    /** The Activity surface's near-real-time poll, while it is on screen. */
    fun liveTick() = feedHost.dispatch(FeedEvent.LiveTick, FeedEvent.serializer())

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
    }
}
