package app.getvela.wallet.feature.onboarding.core

import app.getvela.wallet.core.crux.CoreDriver
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.crux.asBridge
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import org.json.JSONArray
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
        escapedFailure = SessionExecutor::escapedFailure,
        // Spec 048: a session fault — the store's records refused by the core,
        // a malformed event — is written down. The driver answers the machine
        // with the effect's failure, so the route settles on onboarding rather
        // than `loading` forever; this line is how a support thread finds it.
        onFault = { error -> VelaLog.failure("session.fault", "the session machine faulted", error) },
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
     * Drop ONE wallet from this device and stay on the others (2026-09-23).
     *
     * `index` is the position in the ORIGINAL list, as `switchAccount` takes —
     * the rows carry it so a balance-sorted sheet cannot remove a stranger.
     * Removing the last one signs this device out, which the core decides, not
     * this call.
     */
    fun removeAccount(index: Int) =
        driver.dispatch(JSONObject().put("type", "remove_account").put("index", index).toString())

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
     *
     * The wallet in front afterwards is the one the person was on, not the
     * first one (device pass 2026-10-09: leaving always landed on account 1).
     * That is the account in front now when it is a real one — they switched
     * to it inside the space — and otherwise [returnTo], the address that was
     * in front when the space was entered. By address, a row's identity: a
     * position moves when a row goes.
     */
    suspend fun removeFixtureAccount(credentialIdHex: String, returnTo: String? = null) {
        val fixture = recordAddress(store.loadAccounts(), credentialIdHex)
        val inFront = view.value.address.takeUnless { it.isBlank() || it.equals(fixture, ignoreCase = true) }
        store.removeAccount(credentialIdHex)
        val accounts = store.loadAccounts()
        val index = indexOfAddress(accounts, inFront ?: returnTo)
        store.saveActiveIndex(index)
        accountEstablished(
            JSONObject().put("type", "set_wallet").put("accounts", accounts).put("active_index", index),
        )
    }

    /**
     * Spec 102: where [address]'s transactions and messages are reviewed and
     * signed on this device — [venueJson] exactly as the core offered it
     * (`signingVenueChoices`). By address (invariant ⑨). The core refuses a
     * venue that cannot reach the account's keys and writes nothing; otherwise
     * it saves the record, and the session view carries the new venue.
     */
    fun chooseSigningVenue(address: String, venueJson: String) {
        val venue = runCatching { JSONObject(venueJson) }.getOrNull() ?: return
        driver.dispatch(
            JSONObject().put("type", "signing_venue_chosen").put("address", address).put("venue", venue).toString(),
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

    internal companion object {
        /** The position of the record at [address] in [accounts]; 0 when it is not there (or none was given). */
        fun indexOfAddress(accounts: JSONArray, address: String?): Int {
            if (address.isNullOrBlank()) return 0
            for (index in 0 until accounts.length()) {
                val record = accounts.optJSONObject(index) ?: continue
                if (record.optString("address").equals(address, ignoreCase = true)) return index
            }
            return 0
        }

        private fun recordAddress(accounts: JSONArray, id: String): String? {
            for (index in 0 until accounts.length()) {
                val record = accounts.optJSONObject(index) ?: continue
                if (record.optString("id") == id) return record.optString("address")
            }
            return null
        }
    }
}
