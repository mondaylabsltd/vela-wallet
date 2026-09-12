package app.getvela.wallet.core.data

import android.content.Context
import app.getvela.wallet.core.diagnostics.VelaLog
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * Spec 048, debug builds only: the scripted device pass needs the store to
 * hold records the app itself would never write (the retired client's
 * spelling). A file dropped through `run-as` — `files/vela.seed.json` — is
 * applied verbatim before the session reads, then deleted, so it acts once.
 *
 * The object's keys are store keys, or an operation on a list key:
 * - `"vela.accounts": "<raw value>"`          — write the value as is
 * - `"append:vela.accounts": "<json object>"` — append one raw element to the list
 * - `"remove:vela.accounts": "<id>"`          — drop the elements whose `id` matches
 *
 * Append and remove exist so a test can add a foreign record beside the
 * accounts already on the phone and take it away again without ever
 * rewriting the person's own list.
 */
object DebugSeed {
    suspend fun apply(context: Context, store: KeyValueStore) {
        val file = File(context.filesDir, "vela.seed.json")
        if (!file.exists()) return
        runCatching {
            val seed = JSONObject(file.readText())
            for (key in seed.keys()) {
                val value = seed.getString(key)
                when {
                    key.startsWith("append:") -> {
                        val listKey = key.removePrefix("append:")
                        val list = runCatching { JSONArray(store.read(listKey) ?: "[]") }.getOrDefault(JSONArray())
                        list.put(JSONObject(value))
                        store.write(listKey, list.toString())
                    }
                    key.startsWith("remove:") -> {
                        val listKey = key.removePrefix("remove:")
                        val list = runCatching { JSONArray(store.read(listKey) ?: "[]") }.getOrDefault(JSONArray())
                        val kept = JSONArray()
                        for (i in 0 until list.length()) {
                            val element = list.optJSONObject(i) ?: continue
                            if (element.optString("id") != value) kept.put(element)
                        }
                        store.write(listKey, kept.toString())
                    }
                    else -> store.write(key, value)
                }
            }
            VelaLog.event("debug.seed", "applied", "keys" to seed.length())
        }.onFailure { VelaLog.failure("debug.seed", "seed not applied", it) }
        file.delete()
    }
}
