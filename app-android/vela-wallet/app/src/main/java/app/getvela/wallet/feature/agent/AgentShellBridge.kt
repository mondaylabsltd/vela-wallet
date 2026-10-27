package app.getvela.wallet.feature.agent

import android.os.Looper
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.Preferences
import app.getvela.wallet.core.data.ThemePreference
import app.getvela.wallet.feature.contacts.core.ContactsController
import app.getvela.wallet.feature.onboarding.core.SessionController
import app.getvela.wallet.feature.send.core.SendAlertKind
import app.getvela.wallet.feature.send.core.SendController
import app.getvela.wallet.feature.send.core.SendOpenParams
import app.getvela.wallet.feature.settings.core.NetworkSpacePreference
import app.getvela.wallet.feature.settings.core.SettingsController
import app.getvela.wallet.feature.wallet.core.WalletController
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.json.JSONObject
import uniffi.vela_core_uniffi.AgentSendOpen
import uniffi.vela_core_uniffi.AgentShell

/** The app's own controllers and doors, as the agent reaches them — the container's. */
interface AgentWallet {
    val session: SessionController
    val contacts: ContactsController
    val wallet: WalletController
    val settings: SettingsController
    val send: SendController
    val preferences: Preferences

    /** The device's space (spec 105): what every list shows. */
    val networkSpace: NetworkSpacePreference

    /** The theme now. */
    val theme: ThemePreference
    suspend fun setTheme(value: ThemePreference)

    /** Apply a language preference (a tag, or `auto`) now. */
    fun applyLanguage(tag: String)

    /** The dApp signing request on its sheet (`SignRequestView` JSON), if any. */
    fun signingJson(): String?

    /** Explore, the dApp browser (spec 107). */
    val browser: app.getvela.wallet.feature.browser.core.BrowserController

    /** Spec 107 E8: the wallets the person lets the agent use. */
    fun agentWallets(): List<String>

    /** The signing sheet in front, if one is (a dApp's request answered there). */
    fun signingSheet(): app.getvela.wallet.feature.signing.core.SigningController?

    /** Erase This Device; what survived (empty when everything went). */
    suspend fun erase(): List<String>

    /** Clear one of the storage page's items. */
    suspend fun clearStorageItem(item: String)

    /** Clear the caches. */
    suspend fun clearCaches()

    /** Make `address` the account in front; `false` when no wallet has it. */
    suspend fun switchAccount(address: String): Boolean
}

/**
 * What this app does for its agent ([AgentShell], spec 104 phase 7): its
 * views, its controllers' calls, its screens, its send.
 *
 * Every call comes from the agent's own thread and may block it. The
 * controllers' intents and views are thread-safe, so most are answered right
 * there; what is the navigation host's ([AgentPage]) and [perform] hop to the
 * main thread and wait. The calls made under [AgentStore.reviewing] come from
 * the main thread itself and are answered where they are.
 */
internal class AgentShellBridge(private val agent: AgentStore) : AgentShell {
    private val wallet: AgentWallet get() = agent.wallet

    // -- the wallet, read --------------------------------------------------------

    override fun views(): String = AgentWorld.json(wallet, onMain { agent.page?.name() ?: "welcome" })

    override fun page(): String = onMain { agent.page?.name() ?: "welcome" }

    override fun signing(): String? = wallet.signingJson()

    override fun changed() = agent.changed()

    // -- doing -------------------------------------------------------------------

    override fun perform(action: String): String {
        if (onMainThread()) return AgentPerform.refused("not_ready", "the wallet could not do it from here")
        return runBlocking(Dispatchers.Main.immediate) { AgentPerform(wallet, agent).perform(action) }
    }

    override fun reveal(screen: String) {
        val opened = runCatching { JSONObject(screen) }.getOrNull() ?: return
        onMain { agent.page?.show(opened) }
    }

    // -- the send ----------------------------------------------------------------

    override fun sendOpen(): AgentSendOpen? {
        val send = wallet.send
        if (!send.journeyOpen.value || send.closed.value) return null
        val view = send.sendJson.value ?: return null
        return AgentSendOpen(send.opens.value.toULong(), view)
    }

    override fun sendAlert(): String? = wallet.send.alert.value?.let(::alertName)

    override fun sendStart(params: String): ULong? {
        val open = runCatching { Wire.json.decodeFromString(SendOpenParams.serializer(), params) }.getOrNull() ?: return null
        val send = wallet.send
        val before = send.opens.value
        val asked = onMain { agent.page?.let { it.openSend(open); true } ?: false }
        if (!asked) return null
        // The send opens once the wallet route has drawn it: a moment later.
        return waitFor(5_000) { if (send.journeyOpen.value && send.opens.value != before) send.opens.value.toULong() else null }
    }

    override fun sendDispatch(event: String) = wallet.send.dispatchJson(event)

    override fun sendClose() {
        val send = wallet.send
        // A Done already sent closes by itself: a second one would land on the
        // next journey.
        waitFor(1_500) { if (send.closed.value || !send.journeyOpen.value) true else null }
        if (send.journeyOpen.value && !send.closed.value) send.done()
        // Gone with its screens, so the next send opens on a clean stack.
        waitFor(3_000) { if (!send.journeyOpen.value) true else null }
    }

    override fun account(): String = wallet.session.view.value.address

    override fun balancesSettled(account: String): Boolean =
        wallet.wallet.holdingsRound.value?.address.equals(account, ignoreCase = true)

    override fun holdings(): String =
        wallet.wallet.balanceJson.value?.let { runCatching { JSONObject(it).optJSONArray("tokens")?.toString() }.getOrNull() } ?: "[]"

    override fun tracker(): String =
        wallet.wallet.trackerJson?.let { runCatching { JSONObject(it).optJSONArray("entries")?.toString() }.getOrNull() } ?: "[]"

    // `SendExecutor.network`'s own spelling of a token's network.
    override fun networkId(chainId: UInt): String = "chain-$chainId"

    // -- -------------------------------------------------------------------------

    private fun alertName(kind: SendAlertKind): String = runCatching {
        Wire.json.encodeToJsonElement(SendAlertKind.serializer(), kind).jsonObject["type"]?.jsonPrimitive?.content
    }.getOrNull() ?: "alert"

    private fun onMainThread() = Looper.myLooper() == Looper.getMainLooper()

    /** Run [body] on the main thread and answer with it: inline when already there. */
    private fun <T> onMain(body: () -> T): T =
        if (onMainThread()) body() else runBlocking(Dispatchers.Main.immediate) { body() }

    /** Look until [read] answers, or [ms] pass. Off the main thread only. */
    private fun <T> waitFor(ms: Long, read: () -> T?): T? {
        var waited = 0L
        while (true) {
            read()?.let { return it }
            if (waited >= ms || onMainThread()) return null
            Thread.sleep(50)
            waited += 50
        }
    }
}

/** The wallet as the core's `World` reads it. */
internal object AgentWorld {
    /**
     * The views as JSON, under the names `World` reads — the machines' own
     * views, whole. While nobody is signed in only the session: the other
     * machines belong to an account.
     */
    fun json(wallet: AgentWallet, page: String = "welcome"): String {
        val parts = mutableListOf("\"session\":${wallet.session.viewJson.value ?: "null"}")
        if (wallet.session.view.value.address.isNotEmpty()) {
            parts += "\"contacts\":${wallet.contacts.viewJson.value ?: "null"}"
            parts += "\"balance\":${wallet.wallet.balanceJson.value ?: "null"}"
            parts += "\"feed\":${wallet.wallet.feedJson.value ?: "null"}"
            // The ledger has not read its stores yet: nothing to judge by.
            if (wallet.settings.networks.value.loaded) {
                wallet.settings.networksJson.value?.let { parts += "\"networks\":$it" }
            }
            parts += "\"currency\":${wallet.settings.currencyJson.value ?: "null"}"
            parts += "\"fee_tier\":${wallet.settings.feeTierJson.value ?: "null"}"
            // Spec 107: Explore — the tabs, the lists, a dApp's request.
            parts += "\"explore\":${wallet.browser.agentFacts(sheet(wallet, page))}"
        }
        parts += "\"preferences\":${preferences(wallet)}"
        // Spec 107 E8: the wallets the person lets the agent use.
        parts += "\"agent_wallets\":${org.json.JSONArray(wallet.agentWallets())}"
        // Spec 105: the space in front — the core reads only its networks and
        // balances to the agent, and refuses a transfer on the other's.
        parts += "\"network_space\":\"${wallet.networkSpace.space.value.wire}\""
        return "{${parts.joinToString(",")}}"
    }

    /**
     * The dApp request whose sheet the person sees: a connect's sheet is
     * Explore's, over its tab; a signature's is the signing sheet; a network's
     * is Settings' add-network sheet, up while the request is.
     */
    private fun sheet(wallet: AgentWallet, page: String): String? {
        val dapp = wallet.browser.dapp.value
        dapp.consent?.let { consent ->
            val selected = wallet.browser.explore.value.selected_tab
            return if (page == "explore" && selected == consent.tab) "connect:${consent.tab}" else null
        }
        dapp.signing?.let { signing ->
            return if (wallet.signingSheet() != null) "sign:${signing.tab}:${signing.id}" else null
        }
        return dapp.adding_network?.let { "add_network:${it.tab}:${it.id}" }
    }

    /** The preferences in the catalog's spellings. The phone shares no usage statistics. */
    fun preferences(wallet: AgentWallet): JSONObject {
        val prefs = wallet.preferences.view.value
        return JSONObject()
            .put("theme", wallet.theme.storageValue)
            .put("language", prefs.language)
            .put("text_scale", prefs.textScale.wire)
            .put("number_format", prefs.numberFormat.wire)
            .put("date_format", prefs.dateFormat.wire)
            .put("time_format", prefs.timeFormat.wire)
            .put("analytics", false)
    }
}
