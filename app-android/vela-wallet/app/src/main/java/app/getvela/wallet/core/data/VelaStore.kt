package app.getvela.wallet.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.first

/**
 * The wallet's key-value space on this device: string keys, string values, one
 * file.
 *
 * **Why one.** `vela.serviceEndpoints` already has two readers — the onboarding
 * endpoint sheet and, from spec 040, the `network_admin` machine's settings
 * pages. Two DataStore files holding the same key name is how a person changes
 * their bundler in Settings and onboarding keeps using the old one, with
 * nothing in either code path looking wrong.
 *
 * **Why strings.** These keys are the *cross-client compatibility contract*:
 * the same names and the same camelCase JSON payloads the web and desktop
 * clients read and write, so a wallet's address book follows its owner between
 * devices. Typed accessors would invite reshaping on the way through, and a
 * reshaped record is a record the other clients no longer recognise. Values go
 * in and come out as the bytes the core produced.
 *
 * **What belongs here**: anything keyed `vela.*` that survives a restart and
 * belongs to the *account* — contacts, groups, networks, endpoints, display
 * preferences. **What does not**: the theme preference, which lives in its own
 * store precisely so that signing out cannot reach it.
 *
 * Reads and writes are best-effort by design (spec 040
 * `contracts/shell-operations.md`): a machine asked to save must still be
 * *answered* when saving fails, or it waits forever on an effect nobody will
 * resolve. So nothing here throws for a storage fault; the caller learns the
 * outcome from the return value.
 *
 * **Why an interface with one shipping implementation.** The second
 * implementation is the test one. Every executor in this app is a function from
 * an operation to an answer *over storage*, and the whole point of the unit
 * suite is to check those answers without a device — including the answers for
 * storage that fails, which a real DataStore will not produce on request.
 */
interface KeyValueStore {

    /** The stored value, or `null` when the key was never written. */
    suspend fun read(key: String): String?

    /** Write one key. Returns `false` when the platform refused. */
    suspend fun write(key: String, value: String): Boolean

    /**
     * Remove keys, in one edit.
     *
     * Together, because a caller removing two related keys means both or
     * neither: signing out leaves a device with an account list gone and an
     * active index pointing into it if only the first removal lands.
     */
    suspend fun remove(vararg keys: String): Boolean

    /** The keys spec 040's machines own. Names are the contract — see [VelaStore]. */
    object Keys {
        const val CONTACTS = "vela.contacts"
        const val CONTACTS_DISMISSED = "vela.contacts.dismissed"
        const val CONTACT_GROUPS = "vela.contactGroups"
        const val DISPLAY_CURRENCY = "vela.displayCurrency"
        const val CUSTOM_NETWORKS = "vela.customNetworks"

        /**
         * Singular — `networkConfig`, not `networkConfigs`.
         *
         * Verified against `app-web/.../settings/core/network-admin-executor.ts`,
         * where the same key is written. A plural here would be a store the
         * other clients cannot read, and nothing would report it: the settings
         * page would simply show no overrides.
         */
        const val NETWORK_CONFIG = "vela.networkConfig"
        const val RPC_PROVIDERS = "vela.rpcProviders"

        /** Shared with onboarding's endpoint sheet — the reason this class exists. */
        const val SERVICE_ENDPOINTS = "vela.serviceEndpoints"

        /**
         * Endpoints the pool has ruled against, as `{url, bannedAt, permanent}`
         * (spec 041). Shared with the other clients: a device that learned an
         * endpoint is dead should agree with its siblings rather than each
         * rediscovering it the hard way.
         */
        const val RPC_BANNED = "vela.rpc.banned"

        /** `address → {usd, at}`, 24h TTL — the Expo bytes (spec 041). */
        const val BALANCE_CACHE = "vela.balanceCache"

        /** Whether money figures are hidden on this device. */
        const val BALANCE_HIDDEN = "vela.balanceHidden"
    }
}

/** The shipping [KeyValueStore]: one DataStore file, shared by every machine. */
class VelaStore(private val context: Context) : KeyValueStore {

    override suspend fun read(key: String): String? =
        runCatching { context.velaStore.data.first()[stringPreferencesKey(key)] }.getOrNull()

    override suspend fun write(key: String, value: String): Boolean =
        runCatching { context.velaStore.edit { it[stringPreferencesKey(key)] = value } }.isSuccess

    override suspend fun remove(vararg keys: String): Boolean = runCatching {
        context.velaStore.edit { preferences ->
            keys.forEach { preferences.remove(stringPreferencesKey(it)) }
        }
    }.isSuccess
}

/**
 * The wallet's DataStore file.
 *
 * The name is `vela_onboarding` for compatibility, not for accuracy: it is what
 * every installed build already keeps its accounts in, and renaming it would
 * sign every existing person out. `AccountStore` opens the same file by the
 * same name.
 */
internal val Context.velaStore: DataStore<Preferences> by preferencesDataStore(
    name = "vela_onboarding",
)
