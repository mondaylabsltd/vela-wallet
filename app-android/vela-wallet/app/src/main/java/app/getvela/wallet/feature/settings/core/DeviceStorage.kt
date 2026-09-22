package app.getvela.wallet.feature.settings.core

import android.content.Context
import android.webkit.CookieManager
import android.webkit.WebStorage
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.data.ThemePreferenceRepository
import app.getvela.wallet.core.diagnostics.CrashReport
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.marks.LogoStore
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
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

    /**
     * Erase this device — **everything**, not just the `vela.*` shelf
     * (spec 081 FR-017, `contracts/erase-device.md`).
     *
     * ## What [erase] above was missing, and why that mattered
     *
     * `erase` sweeps one DataStore file. That file is not the app: a person
     * who tapped 抹除此设备 and then reopened the wallet still found their
     * **theme** (a second DataStore named `settings`, deliberately outside the
     * wallet store so that signing out cannot reach it), the **crash record**
     * from their last bad session (`vela_crash` SharedPreferences), the
     * **logs**, the **logo cache** — and, worst of the five, every site they
     * had visited in the in-app browser, because WebView keeps its cookies,
     * localStorage, IndexedDB and cache in `app_webview/` where nothing in
     * this file had ever looked. "Erase this device" that leaves a dApp
     * session cookie behind is not a partial erase; it is a false statement
     * about a destructive action somebody chose.
     *
     * ## The order
     *
     * Storage first (it is the part with a keep-list and a verification), then
     * the platform stores, then the files. Every step is independently
     * best-effort: one failure must not abandon the four after it, because a
     * half-swept device is the outcome this whole contract exists to avoid.
     * What survives is REPORTED — the caller shows it and keeps the person
     * signed in, exactly as the web module's `EraseIncompleteError` does.
     *
     * @return the `vela.*` keys and the store names still present afterwards;
     *   empty means the device is clean.
     */
    suspend fun eraseDevice(
        context: Context,
        store: KeyValueStore,
        keep: Set<String>,
    ): List<String> {
        val appContext = context.applicationContext
        val left = erase(store, keep).toMutableList()

        // 2. The theme, in its own DataStore. Signing out must not reach it;
        //    erasing the device must.
        if (!ThemePreferenceRepository(appContext).clear()) left.add("settings")

        // 3. The last crash, kept so the sheet can show it once on relaunch.
        if (!CrashReport.eraseAll(appContext)) left.add(CrashReport.FILE)

        // 4. The in-app browser: cookies, DOM storage, and the directories
        //    WebView keeps them in. The two managers must be touched on the
        //    main thread — WebView refuses off it, and a silently skipped
        //    cookie jar is exactly the bug this step exists for.
        withContext(Dispatchers.Main) {
            runCatching {
                CookieManager.getInstance().removeAllCookies(null)
                CookieManager.getInstance().flush()
                WebStorage.getInstance().deleteAllData()
            }
        }
        withContext(Dispatchers.IO) {
            for (name in WEBVIEW_DIRS) {
                if (!wipe(File(appContext.dataDir, name))) left.add(name)
            }
            // 5. The logs (debug builds write them; a release build has none).
            if (!VelaLog.eraseFiles()) left.add("logs")
            // 6. Every cache this app has: logos, images, HTTP. They rebuild.
            LogoStore.clear()
            appContext.cacheDir.listFiles()?.forEach { wipe(it) }
            appContext.externalCacheDir?.listFiles()?.forEach { wipe(it) }
        }
        return left
    }

    /**
     * Where WebView puts a browsed site's cookies, localStorage, IndexedDB,
     * service workers and HTTP cache. `deleteAllData()` covers the DOM stores
     * the API knows about; these directories are the rest, and they are what
     * survives a "clear storage" that only asked the managers.
     */
    private val WEBVIEW_DIRS = listOf("app_webview", "cache/WebView", "app_textures")

    /** Recursive delete. A path that was never there is already erased. */
    private fun wipe(path: File): Boolean = runCatching {
        if (!path.exists()) return@runCatching true
        path.deleteRecursively()
    }.getOrDefault(false)
}
