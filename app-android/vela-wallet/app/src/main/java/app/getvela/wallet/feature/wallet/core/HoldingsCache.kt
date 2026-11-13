package app.getvela.wallet.feature.wallet.core

import app.getvela.wallet.core.data.KeyValueStore
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

/**
 * The rows the home last showed, per account and space — P2.4, the web's
 * `services/holdings-cache.ts` on this device.
 *
 * Kept so a home opened while a network is down, or before the slowest
 * network has answered, opens its Assets list on what was last known instead
 * of a skeleton (home-1, home-3): the balance core is handed them once
 * (`holdings_restored`) and decides what they may stand for — last-known rows
 * a chain's own answer replaces, kept for a chain that does not answer.
 *
 * The web's bytes: one key (`vela.holdingsCache`, the core's storage catalog
 * lists it under "balances", so Settings' cache clear removes it), entries
 * `"<address lowercased>#<space>" → {at, tokens}`, a 24 h TTL. The testnet
 * space keeps its rows too — they are coins, not a value.
 */
class HoldingsCache(
    private val store: KeyValueStore,
    private val now: () -> Long = System::currentTimeMillis,
) {

    /** Keep the rows a settled round left on the home for [address] in [space]. */
    suspend fun save(address: String, space: String, tokens: List<BalanceToken>) {
        val doc = read().toMutableMap()
        doc[key(address, space)] = buildJsonObject {
            put("at", now())
            put("tokens", json.encodeToJsonElement(tokens))
        }
        runCatching { store.write(KEY, JsonObject(doc).toString()) }
    }

    /** The rows last kept, or none when there are none or they are a day old. */
    suspend fun load(address: String, space: String): List<BalanceToken> {
        val entry = read()[key(address, space)] as? JsonObject ?: return emptyList()
        val at = entry["at"]?.jsonPrimitive?.longOrNull ?: return emptyList()
        if (now() - at > TTL_MS) return emptyList()
        val rows = entry["tokens"] as? JsonArray ?: return emptyList()
        // A stored row is only believed in the shape the core reads.
        return rows.mapNotNull { row -> runCatching { json.decodeFromJsonElement<BalanceToken>(row) }.getOrNull() }
    }

    private suspend fun read(): Map<String, kotlinx.serialization.json.JsonElement> {
        val raw = runCatching { store.read(KEY) }.getOrNull() ?: return emptyMap()
        // A corrupt store reads as empty.
        return runCatching { json.parseToJsonElement(raw).jsonObject }.getOrNull() ?: emptyMap()
    }

    companion object {
        const val KEY: String = "vela.holdingsCache"
        const val TTL_MS: Long = 24L * 60 * 60 * 1000

        fun key(address: String, space: String): String = "${address.trim().lowercase()}#$space"

        private val json = Json { ignoreUnknownKeys = true; explicitNulls = true; encodeDefaults = true }
    }
}
