package app.getvela.wallet.feature.onboarding.core

import android.content.Context
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.data.VelaStore
import org.json.JSONArray
import org.json.JSONObject

/**
 * On-device storage for the wallet's account list.
 *
 * Keys and record shapes are byte-compatible with the other three clients (data-model
 * §6), so a person who created a wallet on the web or the desktop is not stranded
 * here — and, more sharply, so the SAME wallet reads back the same on all four.
 *
 * ONE invariant governs every function below. `Account` carries both the legacy
 * scalar key fields and the full `keys` array, and the core derives the address
 * from **all** keys. A mapper that copies an account field by field and drops
 * `keys` does not merely lose data — it silently "repairs" a multi-key account
 * into a different, wrong, single-key Safe on the next restore, at an address
 * nothing can deploy. So nothing here reshapes an account: **records go in and
 * come out whole**, as the JSON the core emitted. That is why the store's
 * vocabulary is `JSONObject` rather than a Kotlin data class — a data class is
 * exactly the shape that invites a field-by-field copy.
 */
class AccountStore(context: Context) {

    /**
     * The account records live in the wallet's one key-value space, beside the
     * contacts and networks the other machines keep. Sharing the file is not a
     * convenience: `vela.serviceEndpoints` is read from here AND by the
     * settings machine, and two DataStore instances over one file is a runtime
     * error, not a merge.
     */
    private val store = VelaStore(context)

    /** Read the account list. Order is the core's, never re-sorted here. */
    suspend fun loadAccounts(): JSONArray = readList(KEY_ACCOUNTS)

    /** Upsert by id. The whole record is written — see the invariant above. */
    suspend fun saveAccount(account: JSONObject) {
        val id = account.optString("id")
        val accounts = loadAccounts()
        val merged = JSONArray()
        var replaced = false
        for (i in 0 until accounts.length()) {
            val existing = accounts.optJSONObject(i) ?: continue
            if (existing.optString("id") == id) {
                merged.put(account)
                replaced = true
            } else {
                merged.put(existing)
            }
        }
        if (!replaced) merged.put(account)
        writeRaw(KEY_ACCOUNTS, merged.toString())
    }

    /**
     * Missing, garbage and negative all read as 0.
     *
     * A negative index would make the session render an empty address with a
     * wallet present, which the core forbids — so it fails closed here rather
     * than arriving at the wire.
     */
    suspend fun loadActiveIndex(): Int =
        readRaw(KEY_ACTIVE_INDEX)?.trim()?.toIntOrNull()?.takeIf { it > 0 } ?: 0

    suspend fun saveActiveIndex(index: Int) = writeRaw(KEY_ACTIVE_INDEX, index.toString())

    suspend fun loadPendingUploads(): JSONArray = readList(KEY_PENDING_UPLOADS)

    /** Keyed by `id`, which for a pending upload IS the credential id of its
     *  first founding key — the scalar fields mirror `members[0]`. */
    suspend fun savePendingUpload(record: JSONObject) {
        val id = record.optString("id")
        val kept = JSONArray()
        val pending = loadPendingUploads()
        for (i in 0 until pending.length()) {
            val existing = pending.optJSONObject(i) ?: continue
            if (existing.optString("id") != id) kept.put(existing)
        }
        kept.put(record)
        writeRaw(KEY_PENDING_UPLOADS, kept.toString())
    }

    suspend fun removePendingUpload(credentialIdHex: String) {
        val kept = JSONArray()
        val pending = loadPendingUploads()
        for (i in 0 until pending.length()) {
            val existing = pending.optJSONObject(i) ?: continue
            if (existing.optString("id") != credentialIdHex) kept.put(existing)
        }
        writeRaw(KEY_PENDING_UPLOADS, kept.toString())
    }

    suspend fun hasPendingUploads(): Boolean = loadPendingUploads().length() > 0

    /**
     * Forget which wallet this device is signed into — the account list and the
     * active index, and NOTHING else.
     *
     * The scope is the decision, not an implementation detail. Contacts, history,
     * custom tokens and networks, endpoints and preferences belong to the
     * ACCOUNT rather than to the session, and the account comes back intact
     * because its address derives from the passkey rather than from disk. The
     * pending-upload outbox is excluded for a second, independent reason: a
     * record there is a public key the registry never confirmed, and the next
     * launch can still retry it — but a deleted record can never be retried, and
     * that credential becomes unfindable at sign-in.
     */
    suspend fun clearSignedInWallet() {
        store.remove(KEY_ACCOUNTS, KEY_ACTIVE_INDEX)
    }

    /** The passkey-index endpoint override, when the person set one. */
    suspend fun loadRegistryUrl(): String? =
        readRaw(KEY_SERVICE_ENDPOINTS)
            ?.let { runCatching { JSONObject(it) }.getOrNull() }
            ?.nullableString("passkeyIndexURL")

    suspend fun saveRegistryUrl(url: String?) {
        val endpoints = readRaw(KEY_SERVICE_ENDPOINTS)
            ?.let { runCatching { JSONObject(it) }.getOrNull() }
            ?: JSONObject()
        if (url.isNullOrBlank()) endpoints.remove("passkeyIndexURL")
        else endpoints.put("passkeyIndexURL", url)
        writeRaw(KEY_SERVICE_ENDPOINTS, endpoints.toString())
    }

    // -- raw access ----------------------------------------------------------

    private suspend fun readRaw(key: String): String? = store.read(key)

    private suspend fun writeRaw(key: String, value: String) {
        store.write(key, value)
    }

    /**
     * Corrupt JSON reads as an empty list rather than throwing.
     *
     * A damaged list must not make the wallet permanently unopenable, and every
     * write replaces the whole list anyway — but the wallet itself is not lost
     * either way: its address derives from the passkey, so signing in rebuilds
     * the record.
     */
    private suspend fun readList(key: String): JSONArray {
        val raw = readRaw(key) ?: return JSONArray()
        return runCatching { JSONArray(raw) }.getOrElse { JSONArray() }
    }

    private companion object {
        const val KEY_ACCOUNTS = "vela.accounts"
        const val KEY_ACTIVE_INDEX = "vela.activeAccountIndex"
        const val KEY_PENDING_UPLOADS = "vela.pendingUploads"
        val KEY_SERVICE_ENDPOINTS = KeyValueStore.Keys.SERVICE_ENDPOINTS
    }
}
