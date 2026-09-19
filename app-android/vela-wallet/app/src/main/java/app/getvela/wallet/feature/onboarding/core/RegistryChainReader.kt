package app.getvela.wallet.feature.onboarding.core

import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import org.json.JSONObject

/**
 * The registry CONTRACT, read directly — what sign-in falls back to when the
 * index service cannot be reached (spec 062).
 *
 * The index is a cache of the contract. When it is gone — never when it
 * answers; a refusal is an answer — the same two read questions are put to the
 * contract itself: on Gnosis, where the record lives, then on Ethereum, where a
 * person may have backed it up precisely for this. `vela_core::registry_chain`
 * plans the calls and answers in the index's own JSON shapes, so every guard in
 * [RegistryClient] runs unchanged on either source. What is left here is what
 * only a shell can do: perform the `eth_call`s.
 */
class RegistryChainReader(
    /** The RAW `result` hex, or `null` when that chain did not answer. */
    private val ethCall: suspend (chainId: Int, to: String, data: String) -> String?,
    private val keyPlan: (publicKeyHex: String) -> String? = { uniffi.vela_core_uniffi.registryChainKeyPlan(it) },
    private val keyStatus: (hasEntryHex: String, groupsHex: String) -> String? =
        { entry, groups -> uniffi.vela_core_uniffi.registryChainKeyStatus(entry, groups) },
    private val unitPlan: (unitId: Long) -> String? = { uniffi.vela_core_uniffi.registryChainUnitPlan(it.toUInt()) },
    private val unitBody: (unitId: Long, unitHex: String, membersHex: String) -> String? =
        { id, unit, members -> uniffi.vela_core_uniffi.registryChainUnit(id.toUInt(), unit, members) },
) {
    /** The `?publicKey=` body and the chain that gave it, or `null` when nobody answered. */
    suspend fun keyProfile(publicKeyHex: String): Pair<Int, JSONObject>? {
        val plan = keyPlan(publicKeyHex)?.let(::JSONObject) ?: return null
        val chains = plan.optJSONArray("chains") ?: return null
        for (i in 0 until chains.length()) {
            val chainId = chains.optInt(i)
            val results = read(chainId, plan) ?: continue
            val profile = keyStatus(results[0], results[1])?.let(::JSONObject) ?: continue
            // A chain that knows the key founded nothing is only believed when it
            // is the record's home: Ethereum holds what somebody backed up.
            val founded = profile.optJSONObject("groups")?.optJSONArray("unitIds")?.length() ?: 0
            if (chainId != HOME_CHAIN && founded == 0) continue
            return chainId to profile
        }
        return null
    }

    /** The `?unitId=` body from ONE chain — unit ids are per deployment. */
    suspend fun unit(chainId: Int, unitId: Long): JSONObject? {
        val plan = unitPlan(unitId)?.let(::JSONObject) ?: return null
        val results = read(chainId, plan) ?: return null
        return unitBody(unitId, results[0], results[1])?.let(::JSONObject)
    }

    /** Both calls of a plan on one chain; `null` unless BOTH were answered. */
    private suspend fun read(chainId: Int, plan: JSONObject): List<String>? = coroutineScope {
        val calls = plan.optJSONArray("calls") ?: return@coroutineScope null
        if (calls.length() != 2) return@coroutineScope null
        val results = (0 until 2).map { i ->
            val call = calls.getJSONObject(i)
            async { ethCall(chainId, call.optString("to"), call.optString("data")) }
        }.awaitAll()
        results.filterNotNull().takeIf { it.size == 2 }
    }

    companion object {
        /** Where the index's own unit ids come from. */
        const val HOME_CHAIN = 100
    }
}
