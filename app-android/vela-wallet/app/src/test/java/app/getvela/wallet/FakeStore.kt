package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore

/**
 * An in-memory [KeyValueStore] for the executor suites.
 *
 * The reason this exists rather than a DataStore on a Robolectric context: the
 * answers most worth testing are the ones for storage that **fails**, and a
 * real store will not fail on request. [refuseWrites] is the whole point —
 * every executor owes the core an answer even when the platform says no, and
 * that promise is otherwise only checked in production.
 */
class FakeStore(initial: Map<String, String> = emptyMap()) : KeyValueStore {

    val values: MutableMap<String, String> = initial.toMutableMap()

    /** When true, every write and removal reports failure and changes nothing. */
    var refuseWrites: Boolean = false

    /** When true, reads report "nothing stored" — a corrupt or unreadable file. */
    var refuseReads: Boolean = false

    override suspend fun read(key: String): String? =
        if (refuseReads) null else values[key]

    override suspend fun write(key: String, value: String): Boolean {
        if (refuseWrites) return false
        values[key] = value
        return true
    }

    override suspend fun remove(vararg keys: String): Boolean {
        if (refuseWrites) return false
        keys.forEach { values.remove(it) }
        return true
    }
}
