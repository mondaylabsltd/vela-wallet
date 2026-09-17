package app.getvela.wallet.feature.contacts.core

import app.getvela.wallet.core.data.KeyValueStore
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import org.json.JSONArray
import org.json.JSONObject

/**
 * The name a Vela user registered for a wallet, found by its ADDRESS — the
 * Android transport for `vela_core::registry_lookup` (issue 191).
 *
 * The v2 index cannot be asked about an address: `?walletRef=` answers `400`,
 * because an address is `f(all founding keys)` and no single entry owns one.
 * `RegistryClient.nameForAddress` asked exactly that for two specs, and the
 * index step of [IdentityResolver] silently never answered. The name is
 * reached through the chain instead — the Safe's `SafeWebAuthnSharedSigner`
 * configuration → `?publicKey=` → `?unitId=` — and every rule of that walk
 * (which chain first, which unit is believed, what a malformed blob means, how
 * long a verdict may be kept) is the core's, shared with the other three
 * shells. What is left here is what only a shell can do: perform the requests
 * and keep the verdict for as long as the core says.
 */
class RegistryNameLookup(
    private val store: KeyValueStore,
    /**
     * `eth_call` on one chain: the raw `result` hex — INCLUDING a bare `0x`,
     * which is a chain saying "no such contract here" — or `null` when nobody
     * answered. The distinction is the core's caching rule.
     */
    private val ethCall: suspend (chainId: Int, to: String, data: String) -> String?,
    /** `GET <configured index><path>`. */
    private val indexGet: suspend (path: String) -> IndexAnswer,
    /** `registryNameStep` from the core. A seam so the JVM tests need no native library. */
    private val step: (address: String, answersJson: String) -> String,
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
) {
    sealed interface IndexAnswer {
        data class Ok(val body: String) : IndexAnswer

        /** The index said 404: an answer. */
        data object NotFound : IndexAnswer

        /** Unreachable, timed out, any other status: nobody said anything. */
        data object Failed : IndexAnswer
    }

    suspend fun nameFor(address: String): String? {
        if (!ADDRESS.matches(address)) return null
        val key = address.lowercase()
        cached(key)?.let { return it.name }

        val answers = JSONArray()
        repeat(MAX_ROUNDS) {
            val next = runCatching { JSONObject(step(key, answers.toString())) }.getOrNull() ?: return null
            if (next.optString("type") == "ask") {
                val requests = next.optJSONArray("requests") ?: return null
                val round = coroutineScope {
                    (0 until requests.length()).map { index ->
                        val request = requests.getJSONObject(index)
                        async { perform(request) }
                    }.awaitAll()
                }
                round.forEach(answers::put)
                return@repeat
            }
            // `optString` turns a JSON null into the string "null" (org.json);
            // `optJSONObject` is the one that says "absent".
            val name = next.optJSONObject("found")?.optString("name")?.takeIf { it.isNotBlank() }
            when (next.optString("remember")) {
                "forever" -> if (name != null) {
                    val publicKey = next.optJSONObject("found")?.optString("public_key").orEmpty()
                    remember(key, JSONObject().put("name", name).put("publicKey", publicKey))
                }
                "briefly" -> if (name == null) remember(key, JSONObject().put("missAt", now()))
            }
            return name
        }
        return null
    }

    private suspend fun perform(request: JSONObject): JSONObject {
        val id = request.optString("id")
        fun answer(outcome: String, body: String? = null) =
            JSONObject().put("id", id).put("outcome", outcome).put("body", body ?: JSONObject.NULL)
        return runCatching {
            when (request.optString("type")) {
                "eth_call" -> {
                    val result = ethCall(request.getInt("chain_id"), request.getString("to"), request.getString("data"))
                    if (result == null) answer("failed") else answer("ok", result)
                }
                "index_get" -> when (val got = indexGet(request.getString("path"))) {
                    is IndexAnswer.Ok -> answer("ok", got.body)
                    IndexAnswer.NotFound -> answer("not_found")
                    IndexAnswer.Failed -> answer("failed")
                }
                else -> answer("failed")
            }
        }.getOrElse { answer("failed") }
    }

    private class Cached(val name: String?)

    /** `null` = nothing usable stored; `Cached(null)` = a miss still inside its window. */
    private suspend fun cached(key: String): Cached? {
        val raw = store.read(CACHE_KEY) ?: return null
        val entry = runCatching { JSONObject(raw).optJSONObject(key) }.getOrNull() ?: return null
        entry.optString("name").takeIf { it.isNotBlank() && entry.has("name") }?.let { return Cached(it) }
        val missAt = entry.optDouble("missAt", Double.NaN)
        return if (!missAt.isNaN() && now() - missAt < MISS_TTL_MS) Cached(null) else null
    }

    private suspend fun remember(key: String, entry: JSONObject) {
        val all = runCatching { JSONObject(store.read(CACHE_KEY) ?: "{}") }.getOrElse { JSONObject() }
        store.write(CACHE_KEY, all.put(key, entry).toString())
    }

    companion object {
        /** `{ "0xlowercase": { name, publicKey } | { missAt } }` — one document, as the identity cache is. */
        const val CACHE_KEY = "vela.indexName"

        /** `registry_lookup::MISS_TTL_MS` — a miss is only true for now. */
        const val MISS_TTL_MS = 6.0 * 60.0 * 60.0 * 1000.0

        /** A lookup is a handful of rounds; this only stops a contract bug from spinning. */
        private const val MAX_ROUNDS = 16
        private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")
    }
}
