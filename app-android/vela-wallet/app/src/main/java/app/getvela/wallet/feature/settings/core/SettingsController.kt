package app.getvela.wallet.feature.settings.core

import android.content.Context
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.wallet.core.RpcPool
import java.time.Instant
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit
import java.util.Locale
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.StateFlow
import uniffi.vela_core_uniffi.DisplayCurrencyCore
import uniffi.vela_core_uniffi.FeeTierPrefCore
import uniffi.vela_core_uniffi.SignPrefCore
import uniffi.vela_core_uniffi.NetworkAdminCore

/**
 * The settings surface's machines, app-resident.
 *
 * One per process and created on first use, not at launch (research D5): a
 * person who never opens Settings should not pay for its boot, and a machine
 * rebuilt per screen would re-read storage on every rotation and flash the USD
 * placeholder over a settled choice.
 *
 * What a screen sees is a `StateFlow` of a core view and a handful of named
 * intents. It never sees a host, a driver, a `JSONObject` or an operation —
 * that separation is what lets spec 041 add six more machines here without any
 * screen learning about it.
 */
class SettingsController(
    context: Context,
    scope: CoroutineScope,
    /**
     * The chain reader, for the display currency's rate.
     *
     * Nullable because the settings machines must boot without one: the pool
     * needs the network list this very controller produces, so demanding it at
     * construction would be a cycle. A settings surface with no pool simply
     * cannot price a currency, and the core reads that as "not right now" —
     * which is the same thing it read for the whole of spec 040.
     */
    pool: RpcPool? = null,
    /** Spec 043: the settings screen's "clear caches" reaches the relay client too. */
    clearBundlerCache: () -> Unit = {},
) {

    private val store = VelaStore(context)

    /**
     * What this device points one service at: the person's override, or the
     * default the core supplies.
     *
     * A named function rather than a lambda at each site, because these are
     * read by objects built BEFORE the machine that answers them — the
     * probes and the rate resolver both need an endpoint the network machine
     * has not loaded yet. Resolving at call time is what makes that legal, and
     * an explicit return type is what lets the compiler see it.
     */
    /** Spec 047: the chain-data base every logo is fetched from — follows the endpoints table as it is edited. */
    internal fun ethereumDataBase(): kotlinx.coroutines.flow.Flow<String> =
        kotlinx.coroutines.flow.flow { networkHost.view.collect { emit(endpointUrl(NetEndpointField.EthereumData)) } }

    internal fun endpointUrl(field: NetEndpointField): String =
        networkHost.view.value.endpoints
            .firstOrNull { row -> row.field == field }
            ?.let { row -> row.value.ifBlank { row.default_value } }
            .orEmpty()

    private val rates = pool?.let {
        CurrencyRates(
            pool = it,
            store = store,
            endpoint = { endpointUrl(NetEndpointField.FiatRates) },
        )
    }

    /**
     * The network checks, pointed at the ethereum-data service this device is
     * configured for — which is this machine's own view, so a person who
     * redirects that endpoint searches THEIR index rather than the default one.
     */
    private val probes = NetworkProbes(
        chainIndexUrl = { endpointUrl(NetEndpointField.EthereumData) },
    )

    private val currencyExecutor = CurrencyExecutor(
        store = store,
        primaryLocale = { primaryLocale(context) },
        rates = { code -> rates?.resolve(code) },
    )

    /** Spec 045: the same rate waterfall, for a code the batch prices in (`null` = cannot price, never 1). */
    suspend fun fiatRate(code: String): Double? = rates?.resolve(code)

    private val currencyHost = CoreHost(
        bridge = DisplayCurrencyCore().asBridge(),
        scope = scope,
        // What the screen renders before the core has read anything: the same
        // USD placeholder the core itself starts from, uncommitted, unpriced.
        initial = CurrencyView(code = "USD", rate = null, committed = false),
        serializer = CurrencyView.serializer(),
        perform = JsonShell.perform(
            CurrencyOperation.serializer(),
            CurrencyShellResult.serializer(),
            currencyExecutor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            CurrencyOperation.serializer(),
            CurrencyShellResult.serializer(),
            fallback = CurrencyShellResult.StoredCode(null),
            answer = currencyExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("settings.currency.fault", "core fault", error) },
    )

    private val networkExecutor = NetworkAdminExecutor(
        store = store,
        probes = probes,
        // A person who has just corrected a URL should be routed to it, not
        // around it: the pool remembers which endpoints failed, and this is
        // where that memory is asked to forget.
        invalidatePools = { chainId ->
            // `null` means every chain — the settings screen's "clear caches".
            // A chain id means only that one's endpoints were reconfigured.
            if (chainId == null) pool?.invalidateAll() else pool?.refreshChain(chainId.toInt())
        },
        clearBundlerCache = clearBundlerCache,
    )

    private val networkHost = CoreHost(
        bridge = NetworkAdminCore().asBridge(),
        scope = scope,
        // `loaded = false` until storage answers. The screen keeps showing what
        // it had rather than blanking the network list for a frame.
        initial = NetView(),
        serializer = NetView.serializer(),
        perform = JsonShell.perform(
            NetOperation.serializer(),
            NetShellResult.serializer(),
            networkExecutor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            NetOperation.serializer(),
            NetShellResult.serializer(),
            fallback = NetShellResult.StoreLoaded(),
            answer = networkExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("settings.network.fault", "core fault", error) },
    )

    /** The display currency the core has settled on. */
    val currency: StateFlow<CurrencyView> = currencyHost.view

    // -- the default transaction speed (spec 068; Android's since 069) --------

    private val feeTierExecutor = FeeTierExecutor(store)

    private val feeTierHost = CoreHost(
        bridge = FeeTierPrefCore().asBridge(),
        scope = scope,
        // The factory `fast`, uncommitted: what every send did before there
        // was a choice, so a surface drawn before the read lands shows today's
        // behaviour rather than a guess.
        initial = FeeTierPrefView(),
        serializer = FeeTierPrefView.serializer(),
        perform = JsonShell.perform(
            FeeTierPrefOperation.serializer(),
            FeeTierPrefShellResult.serializer(),
            feeTierExecutor::perform,
        ),
        escapedFailure = JsonShell.escapedFailure(
            FeeTierPrefOperation.serializer(),
            FeeTierPrefShellResult.serializer(),
            fallback = FeeTierPrefShellResult.StoredTier(null),
            answer = feeTierExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("settings.feeTier.fault", "core fault", error) },
    )

    /** The stored default speed — Settings shows it, every send starts at it. */
    val feeTier: StateFlow<FeeTierPrefView> = feeTierHost.view

    /** Re-read the preference; coalesced while a read is out. */
    fun refreshFeeTier() =
        feeTierHost.dispatch(FeeTierPrefEvent.Refresh, FeeTierPrefEvent.serializer())

    /** An explicit pick in Settings — and only in Settings. */
    fun chooseFeeTier(tier: app.getvela.wallet.feature.send.core.FeeTier) =
        feeTierHost.dispatch(FeeTierPrefEvent.UserChose(tier), FeeTierPrefEvent.serializer())

    // -- how this device signs by default (spec 071) ---------------------------

    private val signPrefExecutor = SignPrefExecutor(store)

    private val signPrefHost = CoreHost(
        bridge = SignPrefCore().asBridge(),
        scope = scope,
        // `auto` and the official page, uncommitted: what signing did before
        // there was a choice.
        initial = SignPrefView(),
        serializer = SignPrefView.serializer(),
        perform = JsonShell.perform(SignPrefOperation.serializer(), SignPrefShellResult.serializer(), signPrefExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(
            SignPrefOperation.serializer(),
            SignPrefShellResult.serializer(),
            fallback = SignPrefShellResult.Stored(),
            answer = signPrefExecutor::neutralAnswer,
        ),
        onFault = { error -> VelaLog.failure("settings.signPref.fault", "core fault", error) },
    )

    /** The default "Sign with" and the Clear Signer page — Settings shows them, every signature starts at them. */
    val signPref: StateFlow<SignPrefView> = signPrefHost.view

    fun refreshSignPref() = signPrefHost.dispatch(SignPrefEvent.Refresh, SignPrefEvent.serializer())

    /** Settings only: a signing sheet's pick is one request's. */
    fun chooseSignMethod(method: String) =
        signPrefHost.dispatch(SignPrefEvent.MethodChosen(method), SignPrefEvent.serializer())

    fun submitSignerUrl(text: String) =
        signPrefHost.dispatch(SignPrefEvent.SignerUrlSubmitted(text), SignPrefEvent.serializer())

    fun resetSignerUrl() = signPrefHost.dispatch(SignPrefEvent.SignerUrlReset, SignPrefEvent.serializer())


    /** The networks, endpoints and provider keys this device holds. */
    val networks: StateFlow<NetView> = networkHost.view

    /**
     * Re-read the preference. Cheap by design — a second `refresh` while one is
     * in flight supersedes it, so a screen may call this on every entry.
     */
    fun refreshCurrency() =
        currencyHost.dispatch(CurrencyEvent.Refresh, CurrencyEvent.serializer())

    /** An explicit pick in the currency sheet. User choice wins over any seed. */
    fun chooseCurrency(code: String) =
        currencyHost.dispatch(CurrencyEvent.UserChose(code), CurrencyEvent.serializer())

    // -- networks ------------------------------------------------------------
    //
    // One method per thing a person can do on the drawn pages. Each is one
    // event; none of them decides anything.

    /** Read the stored networks, endpoints and keys. Safe to call on entry. */
    fun startNetworks() = net(NetEvent.Started)

    fun searchNetworks(query: String) = net(NetEvent.SearchInput(query))

    fun selectChain(chainId: Long, keepCustomRpc: Boolean = false) =
        net(NetEvent.ChainSelected(chainId, keepCustomRpc))

    fun editCustomRpc(value: String) = net(NetEvent.CustomRpcEdited(value))

    /** The clock is the shell's: the core takes none, so the stamp comes from here. */
    fun confirmAddNetwork() = net(NetEvent.AddConfirmed(nowIso()))

    /**
     * A chain picked from the search results.
     *
     * Ends at the core's `checked` phase with a verdict, so the person sees
     * which contracts were found and presses the button themselves.
     */
    fun selectChain(chainId: Long) =
        net(NetEvent.ChainSelected(chain_id = chainId, keep_custom_rpc = false))

    /**
     * Add a chain by id with **no confirm step** — the scan path.
     *
     * The core calls this `auto`: on a compatible verdict it saves the network
     * and clears the wizard in one move. That is right for a QR code or a deep
     * link, where there is no list to have picked from; it is wrong for
     * somebody browsing search results, who would watch the screen empty itself
     * and never see the checks. Wiring a row tap here was exactly that bug.
     */
    fun addNetworkByChainId(chainId: Long) =
        net(NetEvent.AddByChainIdRequested(chainId, nowIso()))

    fun resetWizard() = net(NetEvent.WizardReset)

    fun deleteNetwork(id: String) = net(NetEvent.DeleteConfirmed(id))

    fun expandOverride(chainId: Long) = net(NetEvent.OverrideExpanded(chainId))

    fun editOverride(chainId: Long, field: NetOverrideField, value: String) {
        VelaLog.event("net", "override edited", "chain" to chainId, "field" to field.name, "chars" to value.length)
        net(NetEvent.OverrideFieldEdited(chainId, field, value))
    }

    /**
     * The commit point for an override.
     *
     * The core validates and saves on blur, not on every keystroke — which is
     * why an editable field must report BOTH: the edits keep the box in sync,
     * and this is what asks the core to accept them.
     */
    fun commitOverride(chainId: Long) {
        VelaLog.event("net", "override committed", "chain" to chainId)
        net(NetEvent.OverrideBlurred(chainId))
    }

    /** Spec 048: a provider's 检查密钥 — the core probes the key it was given. */
    fun testProvider(provider: NetProviderId) {
        VelaLog.event("net", "provider test", "provider" to provider.name)
        net(NetEvent.ProviderTestRequested(provider))
    }

    fun openEndpoints() = net(NetEvent.EndpointsOpened)

    fun editEndpoint(field: NetEndpointField, value: String) =
        net(NetEvent.EndpointEdited(field, value))

    fun commitEndpoint(field: NetEndpointField) = net(NetEvent.EndpointBlurred(field))

    fun resetEndpoints() = net(NetEvent.ResetEndpointsToDefaults)

    fun openProviders() = net(NetEvent.ProvidersOpened)

    fun editProviderKey(provider: NetProviderId, value: String) =
        net(NetEvent.ProviderKeyEdited(provider, value))

    fun commitProviderKey(provider: NetProviderId) = net(NetEvent.ProviderKeyBlurred(provider))

    private fun net(event: NetEvent) = networkHost.dispatch(event, NetEvent.serializer())

    private fun nowIso(): String =
        DateTimeFormatter.ISO_INSTANT.format(Instant.now().truncatedTo(ChronoUnit.SECONDS))

    private companion object {
        /**
         * The device's primary locale, for the region seed.
         *
         * The FIRST of the configured locales, not the JVM default: a person
         * with `[ja-JP, en-US]` set is telling the system what they prefer, and
         * `Locale.getDefault()` on Android can be either depending on what the
         * app declared support for.
         */
        fun primaryLocale(context: Context): Locale? =
            context.resources.configuration.locales.takeIf { it.size() > 0 }?.get(0)
    }
}
