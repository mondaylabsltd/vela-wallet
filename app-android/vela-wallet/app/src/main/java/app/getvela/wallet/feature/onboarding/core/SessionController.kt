package app.getvela.wallet.feature.onboarding.core

import app.getvela.wallet.core.crux.CoreDriver
import app.getvela.wallet.core.crux.asBridge
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import org.json.JSONObject
import uniffi.vela_core_uniffi.SessionCore

/**
 * The session machine, app-resident.
 *
 * One per process, outliving every screen — which is the whole reason it is not
 * a ViewModel. `allowed_route` is the route guard for the entire app, and a
 * guard that is recreated whenever a screen is recreated would spend the first
 * frame after every rotation reporting `loading` and bouncing the person back to
 * onboarding.
 *
 * The division of labour is the contract's: **the core decides WHAT is allowed,
 * this class decides nothing, and the navigation host decides WHEN to move.**
 */
class SessionController(private val store: AccountStore, scope: CoroutineScope) {

    private val executor = SessionExecutor(store)
    private val _view = MutableStateFlow(
        SessionView(
            loading = true,
            hasWallet = false,
            address = "",
            activeIndex = 0,
            accounts = emptyList(),
            allowedRoute = SessionRoute.Loading,
            signOut = null,
        ),
    )

    /** The current view. `loading` until storage has been read. */
    val view: StateFlow<SessionView> = _view

    private val driver = CoreDriver(
        bridge = SessionCore().asBridge(),
        scope = scope,
        perform = { operation -> executor.perform(operation) },
        onView = { json -> _view.value = SessionView.from(json) },
        escapedFailure = OnboardingExecutor::escapedFailure,
    )

    /** Read storage and settle on a route. Called once, at launch. */
    fun boot() = driver.dispatch(event("boot"))

    /**
     * Hand a finished onboarding over.
     *
     * `mode` is the core's own `CompletionMode` object, forwarded UNTOUCHED from
     * the onboarding machine to the session machine. It carries either a whole
     * restored account list or a single new account, and reshaping it here — for
     * instance by pulling out an address and rebuilding a record — is exactly
     * the field-by-field copy that drops `keys` and re-derives a different,
     * wrong, single-key wallet.
     */
    fun accountEstablished(mode: JSONObject) =
        driver.dispatch(JSONObject().put("type", "account_established").put("mode", mode).toString())

    fun switchAccount(index: Int) =
        driver.dispatch(JSONObject().put("type", "switch_account").put("index", index).toString())

    /**
     * Append an account the ONBOARDING machines did not write (spec 043's
     * parallel space). The session core's `add_account` persists only the
     * active index — the record itself is the create/login machines' write
     * (`ShellOperation::SaveAccount`) — so the record is written first. Then,
     * once the boot has answered (its `accounts_loaded` may land before or
     * after that write), the account is either already in the list — switch
     * to it — or appended; never both.
     */
    suspend fun addAccount(record: JSONObject) {
        store.saveAccount(record)
        val settled = view.first { !it.loading }
        val address = record.optString("address")
        val present = settled.accounts.firstOrNull { it.address.equals(address, ignoreCase = true) }
        if (present != null) {
            switchAccount(present.index)
        } else {
            accountEstablished(JSONObject().put("type", "add_account").put("account", record))
        }
    }

    /**
     * The parallel space's exit (spec 043): drop the fixture record and hand
     * the session the store's list again through `set_wallet` — the same
     * shape a boot restores, because `Boot` itself runs once per process.
     * Not a sign-out: nothing else on the device changes.
     */
    suspend fun removeFixtureAccount(credentialIdHex: String) {
        store.removeAccount(credentialIdHex)
        store.saveActiveIndex(0)
        val accounts = store.loadAccounts()
        accountEstablished(
            JSONObject().put("type", "set_wallet").put("accounts", accounts).put("active_index", 0),
        )
    }

    fun signOut() = driver.dispatch(event("sign_out"))

    fun signOutConfirmed() = driver.dispatch(event("sign_out_confirmed"))

    fun signOutDismissed() = driver.dispatch(event("sign_out_dismissed"))

    /** The endpoint override, for the surface an unreachable index opens. */
    suspend fun registryUrl(): String = store.loadRegistryUrl() ?: RegistryClient.DEFAULT_REGISTRY_URL

    suspend fun setRegistryUrl(url: String) {
        store.saveRegistryUrl(url.takeIf { it.isNotBlank() && it != RegistryClient.DEFAULT_REGISTRY_URL })
    }

    private fun event(type: String): String = JSONObject().put("type", type).toString()
}
