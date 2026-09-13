package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import java.util.Calendar
import java.util.TimeZone
import kotlinx.coroutines.delay
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONArray
import org.json.JSONObject

/**
 * The only place the `activity_feed` core touches the outside world.
 *
 * Six operations. Not one decision about what the feed SAYS: the day grouping,
 * the batch folding, the de-duplication, the tombstones, the celebration and
 * its anti-stale generation token are all `activity_feed.rs`. This class reads
 * and writes a JSON array, sleeps, buzzes, and looks up a name.
 *
 * Port source: `app-web/vela-wallet/src/lib/wallet/core/feed-executor.ts`.
 *
 * **The one number the core cannot compute.** `day_start_ms` is local midnight
 * for a record's timestamp, on THIS device, in THIS timezone — the core has no
 * clock and no locale, so the shell owns it. It is computed here with the
 * device's own calendar rather than by dividing by 86,400,000, which would put
 * every row in UTC days and mis-group a payment made in the evening anywhere
 * east of Greenwich.
 */
class FeedExecutor(
    private val store: KeyValueStore,
    /** The person's own accounts: a local name, resolved without a network. */
    private val ownAccounts: () -> List<FeedOwnAccount>,
    /** A name from somewhere other than the device — contacts, a name service. */
    private val resolveName: suspend (String) -> String? = { null },
    private val haptic: () -> Unit = {},
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
) {

    data class FeedOwnAccount(val address: String, val name: String)

    /**
     * Receipt discovery; answers how many records were genuinely new.
     *
     * Settable for the same reason [BalanceExecutor.stream] is: the cycle is
     * real. The scan drives the `token_trust` machine and writes through this
     * executor's own store, so it cannot be built until this exists. Assigned
     * once, by the controller, immediately after both halves are built.
     */
    var scan: suspend (String) -> Int = { 0 }

    /** Writers queue behind one another — the `withTxLock` port. */
    private val writeLock = Mutex()

    suspend fun perform(operation: FeedOperation): FeedShellResult = when (operation) {

        is FeedOperation.ReadTxStore -> FeedShellResult.StoreLoaded(
            // The WHOLE store, unfiltered: the core owns the account filter, and
            // it needs the rows this account does not own to fold a batch's
            // siblings back together.
            records = loadRecords(),
            now_ms = now(),
            // Echoed so the core can tell WHICH read this answers. A tick issues
            // the read and the scan together; without the echo a stale read
            // consumes the celebration the sync earned, and a real receipt lands
            // with no toast, no glow and no buzz.
            read_id = operation.read_id,
        )

        is FeedOperation.ScanIncomingTransfers ->
            FeedShellResult.SyncCompleted(new_count = scan(operation.address).coerceAtLeast(0))

        is FeedOperation.DeleteTxRecord -> {
            deleteRecord(operation.id)
            FeedShellResult.DeleteCommitted(id = operation.id)
        }

        is FeedOperation.ResolveRecipientIdentity -> {
            // The person's OWN accounts first: a local name, no network at all.
            val own = ownAccounts().firstOrNull {
                it.address.equals(operation.addr, ignoreCase = true)
            }
            FeedShellResult.AliasResolved(
                addr = operation.addr,
                // `null` is "no identity anywhere", never an invented name.
                name = own?.name ?: resolveName(operation.addr),
            )
        }

        is FeedOperation.Timer -> {
            delay(operation.ms.toLong())
            FeedShellResult.ToastExpired(generation = operation.generation)
        }

        is FeedOperation.Haptic -> {
            haptic()
            FeedShellResult.HapticPlayed
        }
    }

    /** What to answer when the shell itself threw. Exhaustive by construction. */
    fun neutralAnswer(operation: FeedOperation): FeedShellResult = when (operation) {
        // An unreadable store is an EMPTY store, not a missing answer: the
        // store is the source of truth even about emptiness.
        is FeedOperation.ReadTxStore ->
            FeedShellResult.StoreLoaded(emptyList(), now(), operation.read_id)
        is FeedOperation.ScanIncomingTransfers -> FeedShellResult.SyncCompleted(0)
        // The write did not happen, and saying so is what lets the core lift
        // its tombstone and put the row back rather than losing it silently.
        is FeedOperation.DeleteTxRecord -> FeedShellResult.DeleteFailed(operation.id)
        is FeedOperation.ResolveRecipientIdentity ->
            FeedShellResult.AliasResolved(operation.addr, null)
        is FeedOperation.Timer -> FeedShellResult.ToastExpired(operation.generation)
        is FeedOperation.Haptic -> FeedShellResult.HapticPlayed
    }

    // -- the store -----------------------------------------------------------

    /**
     * Every stored record, in the Expo bytes.
     *
     * A row that cannot be read is skipped rather than failing the read: one
     * corrupt entry written by an older version must not hide a person's whole
     * history.
     */
    private suspend fun loadRecords(): List<FeedTxRecord> {
        val raw = store.read(KeyValueStore.Keys.TRANSACTIONS) ?: return emptyList()
        val array = runCatching { JSONArray(raw) }.getOrNull() ?: return emptyList()
        return (0 until array.length()).mapNotNull { index ->
            val row = array.optJSONObject(index) ?: return@mapNotNull null
            runCatching { record(row) }.getOrNull()
        }
    }

    private fun record(row: JSONObject): FeedTxRecord? {
        val id = row.optString("id").ifBlank { return null }
        val timestamp = row.numberOrNull("timestamp") ?: return null
        return FeedTxRecord(
            id = id,
            user_op_hash = row.optString("userOpHash"),
            tx_hash = row.optString("txHash"),
            from = row.optString("from"),
            to = row.optString("to"),
            to_name = row.stringOrNull("toName"),
            value = row.optString("value"),
            symbol = row.optString("symbol"),
            decimals = row.optInt("decimals", 18),
            logo_urls = row.optJSONArray("logoUrls")?.let { urls ->
                (0 until urls.length()).mapNotNull { urls.optString(it).ifBlank { null } }
            },
            chain_id = row.optInt("chainId"),
            timestamp = timestamp,
            day_start_ms = dayStartMs(timestamp),
            status = when (row.optString("status")) {
                "confirmed" -> FeedTxStatus.Confirmed
                "failed" -> FeedTxStatus.Failed
                else -> FeedTxStatus.Pending
            },
            kind = when (row.optString("type")) {
                "send" -> FeedTxKind.Send
                "receive" -> FeedTxKind.Receive
                "dapp_tx", "dappTx" -> FeedTxKind.DappTx
                "sign_message", "signMessage" -> FeedTxKind.SignMessage
                "sign_typed_data", "signTypedData" -> FeedTxKind.SignTypedData
                "connect" -> FeedTxKind.Connect
                // A record with no type is a legacy row; the CORE reads a
                // missing kind as a send, so nothing is decided here.
                else -> null
            },
            usd = row.stringOrNull("usd"),
        )
    }

    private suspend fun deleteRecord(id: String) = writeLock.withLock {
        val raw = store.read(KeyValueStore.Keys.TRANSACTIONS) ?: return@withLock
        val array = runCatching { JSONArray(raw) }.getOrNull() ?: return@withLock
        val kept = JSONArray()
        var removed = false
        for (index in 0 until array.length()) {
            val row = array.optJSONObject(index) ?: continue
            if (row.optString("id") == id) removed = true else kept.put(row)
        }
        if (removed) store.write(KeyValueStore.Keys.TRANSACTIONS, kept.toString())
    }

    /**
     * Merge discovered records in: de-duped by id, newest first, capped.
     *
     * Answers how many were actually new — the number the celebration is built
     * on, so a re-scan that finds the same receipt again must answer zero or
     * the phone buzzes every ten seconds for money that arrived yesterday.
     */
    suspend fun mergeRecords(incoming: List<JSONObject>): Int = writeLock.withLock {
        if (incoming.isEmpty()) return@withLock 0
        val raw = store.read(KeyValueStore.Keys.TRANSACTIONS)
        val existing = raw?.let { runCatching { JSONArray(it) }.getOrNull() } ?: JSONArray()

        val known = HashSet<String>()
        for (index in 0 until existing.length()) {
            existing.optJSONObject(index)?.optString("id")?.let(known::add)
        }
        val fresh = incoming.filter { it.optString("id") !in known }
        if (fresh.isEmpty()) return@withLock 0

        val merged = ArrayList<JSONObject>(fresh)
        for (index in 0 until existing.length()) {
            existing.optJSONObject(index)?.let(merged::add)
        }
        merged.sortByDescending { it.optDouble("timestamp", 0.0) }

        val capped = JSONArray()
        merged.take(TX_CAP).forEach(capped::put)
        store.write(KeyValueStore.Keys.TRANSACTIONS, capped.toString())

        VelaLog.event("feed.merge", "stored", "new" to fresh.size, "total" to capped.length())
        fresh.size
    }

    /**
     * Write rows the SEND path produced — at submit, before tracking begins
     * (spec 043, the core's ordering invariant ⑥). Same lock, same camelCase
     * shape, same cap as the incoming scan. A row whose id is already stored
     * is REPLACED, not duplicated: a retried submit re-persists the same
     * record, and the feed must show one row for one operation.
     *
     * Answers whether the store took the write; the core hears
     * `records_persisted` only on `true`, so a storage fault stops the
     * tracker from following a row nobody can see.
     */
    suspend fun writeRecords(rows: List<JSONObject>): Boolean = writeLock.withLock {
        if (rows.isEmpty()) return@withLock true
        val raw = store.read(KeyValueStore.Keys.TRANSACTIONS)
        val existing = raw?.let { runCatching { JSONArray(it) }.getOrNull() } ?: JSONArray()
        val incomingIds = rows.mapNotNull { it.optString("id").ifBlank { null } }.toSet()

        val merged = ArrayList<JSONObject>(rows)
        for (index in 0 until existing.length()) {
            val row = existing.optJSONObject(index) ?: continue
            if (row.optString("id") !in incomingIds) merged.add(row)
        }
        merged.sortByDescending { it.optDouble("timestamp", 0.0) }

        val capped = JSONArray()
        merged.take(TX_CAP).forEach(capped::put)
        val ok = store.write(KeyValueStore.Keys.TRANSACTIONS, capped.toString())
        VelaLog.event("feed.write", if (ok) "stored" else "refused", "rows" to rows.size)
        ok
    }

    /**
     * The tracker's verdict onto the rows it followed: `status` becomes
     * `confirmed` or `failed`, `txHash` fills in when the relay named one.
     * Rows not named are untouched; a patch for an id nobody stored is a
     * no-op, not an error — a row may have been deleted while in flight.
     */
    suspend fun patchRecords(ids: List<String>, status: String, txHash: String?): Boolean =
        writeLock.withLock {
            if (ids.isEmpty()) return@withLock true
            val raw = store.read(KeyValueStore.Keys.TRANSACTIONS) ?: return@withLock true
            val array = runCatching { JSONArray(raw) }.getOrNull() ?: return@withLock true
            val wanted = ids.toSet()
            var touched = false
            for (index in 0 until array.length()) {
                val row = array.optJSONObject(index) ?: continue
                if (row.optString("id") !in wanted) continue
                row.put("status", status)
                if (!txHash.isNullOrBlank()) row.put("txHash", txHash)
                touched = true
            }
            if (!touched) return@withLock true
            store.write(KeyValueStore.Keys.TRANSACTIONS, array.toString())
        }

    /**
     * The rows the tracker must still follow after a restart: no terminal
     * status, and of a kind that was submitted from here (`send`, `dapp_tx`).
     * A received transfer is never "pending" in this sense — the scan wrote
     * it confirmed.
     */
    suspend fun pendingRecords(): List<JSONObject> {
        val raw = store.read(KeyValueStore.Keys.TRANSACTIONS) ?: return emptyList()
        val array = runCatching { JSONArray(raw) }.getOrNull() ?: return emptyList()
        return (0 until array.length()).mapNotNull { index ->
            val row = array.optJSONObject(index) ?: return@mapNotNull null
            val terminal = row.optString("status") in setOf("confirmed", "failed")
            val tracked = row.optString("type") in setOf("send", "dapp_tx", "dappTx")
            row.takeIf { !terminal && tracked && it.optString("userOpHash").isNotBlank() }
        }
    }

    /**
     * Local midnight for an epoch-seconds timestamp, in milliseconds.
     *
     * The device's calendar, not arithmetic: `timestamp - timestamp % 86400`
     * is UTC midnight, which puts an evening payment in Shanghai on the wrong
     * day and a morning one in Los Angeles on the day before.
     */
    private fun dayStartMs(timestampSeconds: Double): Double {
        val calendar = Calendar.getInstance(TimeZone.getDefault())
        calendar.timeInMillis = (timestampSeconds * 1000).toLong()
        calendar.set(Calendar.HOUR_OF_DAY, 0)
        calendar.set(Calendar.MINUTE, 0)
        calendar.set(Calendar.SECOND, 0)
        calendar.set(Calendar.MILLISECOND, 0)
        return calendar.timeInMillis.toDouble()
    }

    /**
     * A JSON number that may have been written in scientific notation.
     *
     * `org.json` renders a large double as `1.725E12`, and `optDouble` reads it
     * back — but a value stored as a string does not parse, and one written by
     * the Expo app may be either. 040 shipped this bug once already.
     */
    private fun JSONObject.numberOrNull(key: String): Double? {
        if (!has(key) || isNull(key)) return null
        val direct = optDouble(key, Double.NaN)
        if (!direct.isNaN()) return direct
        return optString(key).trim().toDoubleOrNull()
    }

    private fun JSONObject.stringOrNull(key: String): String? =
        if (!has(key) || isNull(key)) null else optString(key).ifBlank { null }

    private companion object {
        /** The newest 200 records, the same cap the Expo store applied. */
        const val TX_CAP = 200
    }
}
