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
import uniffi.vela_core_uniffi.BalanceDashboardCore

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
    scope: CoroutineScope,
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

    init {
        // The knot: the executor emits `chain_assets_arrived` as each chain
        // lands, and the machine that consumes those events is built FROM the
        // executor. Tied here, once, rather than pretended away with a lazy.
        executor.stream = { event -> balanceHost.dispatch(event, BalanceEvent.serializer()) }
    }

    /** What the home screen renders. */
    val balances: StateFlow<BalanceView> = balanceHost.view

    /**
     * Open the wallet for an address.
     *
     * Safe to call on every entry: the core supersedes an in-flight read rather
     * than stacking another one.
     */
    suspend fun open(address: String) {
        pool.start()
        balanceHost.dispatch(
            BalanceEvent.PrivacyHydrated(executor.storedPrivacy()),
            BalanceEvent.serializer(),
        )
        balanceHost.dispatch(BalanceEvent.AccountChanged(address), BalanceEvent.serializer())
    }

    /** A pull-to-refresh, or the screen coming back into view. */
    fun refresh(force: Boolean = false, pull: Boolean = false) =
        balanceHost.dispatch(
            BalanceEvent.RefreshRequested(force = force, pull = pull),
            BalanceEvent.serializer(),
        )

    fun togglePrivacy() =
        balanceHost.dispatch(BalanceEvent.PrivacyToggled, BalanceEvent.serializer())

    fun focused() = balanceHost.dispatch(BalanceEvent.AppFocused, BalanceEvent.serializer())

    fun backgrounded() =
        balanceHost.dispatch(BalanceEvent.AppBackgrounded, BalanceEvent.serializer())
}
