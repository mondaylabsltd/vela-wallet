package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.data.KeyValueStore
import uniffi.vela_core_uniffi.storageIsCacheKey
import uniffi.vela_core_uniffi.storageIsErasableKey
import uniffi.vela_core_uniffi.storageItemOfKey
import uniffi.vela_core_uniffi.storageItems
import uniffi.vela_core_uniffi.storageRecordsIn

/**
 * What the device actually holds (spec 047 D4; spec 072): every key the
 * store has, filed under the drawn rows, with its byte size and — where the
 * value is a list — its record count. WHICH row a key is, which keys "clear
 * all caches" takes and which an erase takes are the core's catalog
 * (`vela_core::storage_catalog`, shared with every Vela); this object only
 * enumerates, measures and deletes. Erasing sweeps every erasable key and
 * verifies by re-enumeration, because a hand-kept delete-list is wrong by
 * default (028's finding).
 */
object DeviceStorage {
    enum class Group(val id: String) {
        User("user"), Cache("cache"), Sessions("sessions");

        companion object {
            fun of(id: String): Group = entries.firstOrNull { it.id == id } ?: User
        }
    }

    class Item(val id: String, val group: Group)

    /** The rows, in the order the page draws them — the core's. */
    val ITEMS: List<Item> = storageItems().map { Item(it.id, Group.of(it.group)) }

    /** Which drawn row a key belongs to; `null` = the accounts and preferences, nobody's row. */
    fun itemOfKey(key: String): String? = storageItemOfKey(key)

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

    /** A list's length (the core's rule); `null` when the value is not a list. */
    internal fun recordsOf(value: String): Int? = storageRecordsIn(value)?.toInt()

    suspend fun clear(store: KeyValueStore, itemId: String): Boolean {
        val keys = store.allKeys().filter { itemOfKey(it) == itemId }
        return keys.isEmpty() || store.remove(*keys.toTypedArray())
    }

    /** Exactly the cache group's keys — never a preference such as the hidden balance. */
    suspend fun clearCaches(store: KeyValueStore): Boolean {
        val keys = store.allKeys().filter(::storageIsCacheKey)
        return keys.isEmpty() || store.remove(*keys.toTypedArray())
    }

    /**
     * Delete every erasable key (the core's rule: the `vela.` namespace but its
     * keep-list); the keys still present afterwards — empty means erased.
     */
    suspend fun erase(store: KeyValueStore): List<String> {
        val doomed = store.allKeys().filter(::storageIsErasableKey)
        if (doomed.isNotEmpty()) store.remove(*doomed.toTypedArray())
        return store.allKeys().filter(::storageIsErasableKey)
    }
}
