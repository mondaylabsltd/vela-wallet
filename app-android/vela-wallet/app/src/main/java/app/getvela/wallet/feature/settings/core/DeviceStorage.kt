package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.data.KeyValueStore
import org.json.JSONArray
import org.json.JSONObject

/**
 * What the device actually holds (spec 047 D4, the web's `device-storage.ts`
 * ported): every key the store has, filed under the drawn item ids, with
 * its byte size and — where the value is a JSON array — its record count.
 * Clearing an item removes exactly its keys; erasing sweeps every key but
 * the keep-list and verifies by re-enumeration, because a hand-kept
 * delete-list is wrong by default (028's finding).
 */
object DeviceStorage {
    enum class Group(val id: String) { User("user"), Cache("cache"), Sessions("sessions") }

    class Item(val id: String, val group: Group)

    val ITEMS: List<Item> = listOf(
        Item("transactions", Group.User),
        Item("contacts", Group.User),
        Item("custom", Group.User),
        Item("browsing", Group.User),
        Item("balances", Group.Cache),
        Item("rates", Group.Cache),
        Item("scan", Group.Cache),
        Item("dapps", Group.Sessions),
    )

    /** The web's `itemOfKey`: which drawn row a key belongs to; `null` = the account records and preferences, which are nobody's row. */
    fun itemOfKey(key: String): String? = when {
        key == "vela.transactionHistory" -> "transactions"
        key.startsWith("vela.contacts") || key == "vela.contactGroups" -> "contacts"
        key == "vela.customTokens" || key == "vela.customNetworks" -> "custom"
        key == "vela.browserHistory" || key == "vela.explore" || key == "vela.bhist" -> "browsing"
        key == "vela.balanceCache" || key == "vela.balanceHidden" -> "balances"
        key.startsWith("vela.fiatRates") || key.startsWith("vela.fxRates") -> "rates"
        key.startsWith("vela.receiveWatch") || key.startsWith("vela.trust") || key == "vela.rpc.banned" -> "scan"
        key.startsWith("vela.perm.") -> "dapps"
        else -> null
    }

    class ItemReport(val id: String, val group: Group, val keys: List<String>, val bytes: Long, val records: Int?)

    class Report(val items: List<ItemReport>, val totalBytes: Long, val totalRecords: Int) {
        fun bytesOf(group: Group): Long = items.filter { it.group == group }.sumOf { it.bytes }
    }

    suspend fun measure(store: KeyValueStore): Report {
        val keys = store.allKeys()
        val values = keys.associateWith { store.read(it).orEmpty() }
        val items = ITEMS.map { item ->
            val own = keys.filter { itemOfKey(it) == item.id }.sorted()
            val bytes = own.sumOf { values[it].orEmpty().toByteArray(Charsets.UTF_8).size.toLong() }
            val records = own.mapNotNull { key -> recordsOf(values[key].orEmpty()) }.takeIf { it.isNotEmpty() }?.sum()
            ItemReport(item.id, item.group, own, bytes, records)
        }
        return Report(items, items.sumOf { it.bytes }, items.sumOf { it.records ?: 0 })
    }

    /** A JSON array's length, or an object's `items`/`contacts`/`entries` array; `null` when the value is not a list. */
    internal fun recordsOf(value: String): Int? {
        val trimmed = value.trim()
        if (trimmed.startsWith("[")) return runCatching { JSONArray(trimmed).length() }.getOrNull()
        if (trimmed.startsWith("{")) {
            val obj = runCatching { JSONObject(trimmed) }.getOrNull() ?: return null
            for (name in listOf("items", "contacts", "entries", "records", "grants")) {
                obj.optJSONArray(name)?.let { return it.length() }
            }
        }
        return null
    }

    suspend fun clear(store: KeyValueStore, itemId: String): Boolean {
        val keys = store.allKeys().filter { itemOfKey(it) == itemId }
        return keys.isEmpty() || store.remove(*keys.toTypedArray())
    }

    suspend fun clearCaches(store: KeyValueStore): Boolean {
        val keys = store.allKeys().filter { key -> ITEMS.firstOrNull { it.id == itemOfKey(key) }?.group == Group.Cache }
        return keys.isEmpty() || store.remove(*keys.toTypedArray())
    }

    /** The keys still present after the sweep (empty = erased). The keep-list is named by the caller. */
    suspend fun erase(store: KeyValueStore, keep: Set<String>): List<String> {
        val doomed = store.allKeys().filter { it !in keep }
        if (doomed.isNotEmpty()) store.remove(*doomed.toTypedArray())
        return store.allKeys().filter { it !in keep }
    }
}
