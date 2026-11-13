package app.getvela.wallet.feature.onboarding.core

import android.content.Context
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.data.VelaStore
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
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
class AccountStore internal constructor(private val store: KeyValueStore) {

    /** The app's own DataStore; tests hand in a fake (spec 043). */
    constructor(context: Context) : this(VelaStore(context))

    /**
     * The account records live in the wallet's one key-value space, beside the
     * contacts and networks the other machines keep. Sharing the file is not a
     * convenience: `vela.serviceEndpoints` is read from here AND by the
     * settings machine, and two DataStore instances over one file is a runtime
     * error, not a merge.
     */

    /**
     * Read the account list: the records the core reads, exactly as stored, in
     * the stored order. §18 item 2: a record this build cannot read — or a
     * stored value that is no list — is counted ([unreadable]) and kept, never
     * dropped; the welcome and the account switcher say it. The rule is the
     * core's (`stored_accounts::read`), the same on every shell.
     */
    suspend fun loadAccounts(): JSONArray {
        val answer = JSONObject(uniffi.vela_core_uniffi.storedAccountsRead(readRaw(KEY_ACCOUNTS)))
        noteUnreadable(answer.optInt("unreadable"), answer.optJSONArray("unreadable_at"))
        return answer.optJSONArray("accounts") ?: JSONArray()
    }

    /**
     * Upsert by id. The whole record is written — see the invariant above —
     * and every record this build cannot read stays where it was, its bytes as
     * they were (§18 item 2, the core's `stored_accounts::upsert`): a list
     * shortened by what could not be read is never written. Signing in again
     * is the way back: it writes the account's record beside them.
     */
    suspend fun saveAccount(account: JSONObject) {
        val answer = uniffi.vela_core_uniffi.storedAccountsUpsert(readRaw(KEY_ACCOUNTS), account.toString())
        val write = written(answer, "upsert")
        // ANDROID-8 (docs/KNOWN-BUGS.md): a record vanished from this list once
        // and nothing said so. Every upsert now leaves a count behind.
        VelaLog.event(
            "accounts", "upsert",
            "before" to write.optInt("before"), "after" to write.optInt("after"),
            "replaced" to write.optBoolean("replaced"), "unreadable_kept" to write.optInt("unreadable_kept"),
        )
        writeRaw(KEY_ACCOUNTS, write.getString("stored"))
        noteUnreadable(write.optInt("unreadable_kept"), null)
    }

    /** The core's answer to a write, or a throw: nothing to write is never an empty list. */
    private fun written(answer: String, what: String): JSONObject =
        runCatching { JSONObject(answer) }.getOrNull()
            ?: throw IllegalArgumentException("accounts $what: the record cannot be written")

    /** Remove through the core, every other record — one this build cannot read too — kept as it was. */
    private suspend fun removeWhere(by: JSONObject, what: String) {
        val write = written(uniffi.vela_core_uniffi.storedAccountsRemove(readRaw(KEY_ACCOUNTS), by.toString()), what)
        VelaLog.event(
            "accounts", what,
            "before" to write.optInt("before"), "after" to write.optInt("after"),
            "unreadable_kept" to write.optInt("unreadable_kept"),
        )
        writeRaw(KEY_ACCOUNTS, write.getString("stored"))
        noteUnreadable(write.optInt("unreadable_kept"), null)
    }

    /**
     * Keep the count the welcome and the switcher say, and log it once as it
     * changes: positions and sizes, never contents (a record holds addresses
     * and public keys, which no log line carries).
     */
    private fun noteUnreadable(count: Int, at: JSONArray?) {
        val was = unreadableCount.value
        unreadableCount.value = count
        if (count != was && count > 0) {
            val where = at?.let { list ->
                (0 until list.length()).joinToString(", ") { i ->
                    val r = list.optJSONObject(i)
                    "#${r?.optInt("position")} (${r?.optInt("bytes")} bytes)"
                }
            }
            VelaLog.event("accounts", "unreadable kept as stored", "count" to count, "at" to where)
        }
    }

    /**
     * Missing, garbage and negative all read as 0.
     *
     * A negative index would make the session render an empty address with a
     * wallet present, which the core forbids — so it fails closed here rather
     * than arriving at the wire.
     */
    /**
     * Drop one account record by id, leaving the others as they are. The
     * parallel space's exit (spec 043): the fixture account it appended goes,
     * the person's real accounts stay.
     */
    suspend fun removeAccount(id: String) = removeWhere(JSONObject().put("id", id), "remove fixture")

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
        // A sign-out empties the list on purpose; said in the log so an empty
        // list later reads as that, not as ANDROID-8 again (2026-10-04: an
        // upsert's `before=0` could not tell the two apart).
        val stored = JSONObject(uniffi.vela_core_uniffi.storedAccountsRead(readRaw(KEY_ACCOUNTS)))
        val before = (stored.optJSONArray("accounts")?.length() ?: 0) + stored.optInt("unreadable")
        VelaLog.event("accounts", "sign out", "before" to before, "after" to 0)
        store.remove(KEY_ACCOUNTS, KEY_ACTIVE_INDEX)
        unreadableCount.value = 0
    }

    /**
     * Drop ONE account from the stored list, by ADDRESS — spec 017's narrow
     * half (2026-09-23: 「有时候不想退出所有，只想退出单个」).
     *
     * By address, not by id or position, because a row's identity is its
     * address (session invariant ⑨): a write that raced a re-sorted display
     * must not take a stranger. [`removeAccount`] above is a different job —
     * the parallel space dropping the fixture record it appended, which it
     * knows by id.
     *
     * A row that is no longer there is a no-op, not an error: the person asked
     * for it to be gone, and it is. The active index is the CORE's to re-state
     * afterwards; this only edits the list.
     */
    suspend fun removeAccountAtAddress(address: String) = removeWhere(JSONObject().put("address", address), "remove")

    /**
     * The index to ask, right now: the person's override, or the shipped
     * default when they have not set one (spec 081 FR-002 — the "empty means
     * the default" rule the core states in `NetServiceEndpoints::effective`).
     */
    suspend fun registryUrlOrDefault(): String =
        loadRegistryUrl()?.takeIf { it.isNotBlank() } ?: RegistryClient.DEFAULT_REGISTRY_URL

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
     * The pending-upload outbox's list. The account list is the core's to read
     * (`stored_accounts`, above), which keeps what it cannot read.
     */
    private suspend fun readList(key: String): JSONArray {
        val raw = readRaw(key) ?: return JSONArray()
        // Spec 048 (ANDROID-8): a value that is there but cannot be read is NOT an
        // empty list. Read as empty, the next write would drop what it could not
        // see. It throws instead, and nothing is written.
        return runCatching { JSONArray(raw) }.getOrElse { error ->
            throw IllegalStateException("stored list under $key is unreadable (${raw.length} chars)", error)
        }
    }

    /** The stored account list as written — for guards that must know whether the store holds records at all. */
    suspend fun rawAccounts(): String? = readRaw(KEY_ACCOUNTS)

    companion object {
        /**
         * §18 item 2: how many stored account records this device cannot read,
         * as of the last read or write — what the welcome and the account
         * switcher say instead of looking signed out. Process-wide, as the
         * records are (the desktop's `storage::unreadable_accounts`).
         */
        val unreadable: StateFlow<Int> get() = unreadableCount
        private val unreadableCount = MutableStateFlow(0)

        private const val KEY_ACCOUNTS = "vela.accounts"
        private const val KEY_ACTIVE_INDEX = "vela.activeAccountIndex"
        private const val KEY_PENDING_UPLOADS = "vela.pendingUploads"
        private val KEY_SERVICE_ENDPOINTS = KeyValueStore.Keys.SERVICE_ENDPOINTS
    }
}
