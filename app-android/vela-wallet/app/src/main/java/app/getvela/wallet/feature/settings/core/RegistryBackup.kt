package app.getvela.wallet.feature.settings.core

import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import org.json.JSONArray
import org.json.JSONObject

/**
 * Where a wallet's founding record stands on Ethereum, and — when it is not
 * there — the one call that would put it there (spec 062 §5a). The Android
 * transport for `vela_core::registry_backup`.
 *
 * The walk is the core's: five `eth_call`s against the registry contract, on
 * Gnosis (where the record lives) and on Ethereum (where the backup goes). The
 * index service is never asked — a backup that needed our server to work would
 * not be a backup from it — and no passkey is involved: the registry stored the
 * original calldata and froze its signature domain, so the Gnosis bytes verify
 * on Ethereum as they are. Nothing is cached: "not backed up" next to a fee
 * must never be stale.
 */
class RegistryBackup(
    /**
     * `eth_call` on one chain: the RAW `result` hex — a bare `0x` included,
     * which is a chain saying "no such contract here" — or `null` when nobody
     * answered. Silence is never a verdict.
     */
    private val ethCall: suspend (chainId: Int, to: String, data: String) -> String?,
    /** `registryBackupStep` from the core. A seam so the JVM tests need no native library. */
    private val step: (address: String, foundingKeyHex: String, answersJson: String, targetChain: UInt?) -> String,
) {
    enum class State { Unavailable, NotRegistered, BackedUp, NotBackedUp, CouldNotCheck }

    /** The one transaction that performs the backup. */
    data class Call(val chainId: Int, val to: String, val data: String)

    data class Check(val state: State, val call: Call?)

    suspend fun check(address: String, foundingKeyHex: String, targetChain: Int? = null): Check {
        val answers = JSONArray()
        repeat(MAX_ROUNDS) {
            val next = runCatching { JSONObject(step(address, foundingKeyHex, answers.toString(), targetChain?.toUInt())) }
                .getOrNull() ?: return COULD_NOT
            if (next.optString("type") == "ask") {
                val requests = next.optJSONArray("requests") ?: return COULD_NOT
                coroutineScope {
                    (0 until requests.length()).map { index ->
                        val request = requests.getJSONObject(index)
                        async { perform(request) }
                    }.awaitAll()
                }.forEach(answers::put)
                return@repeat
            }
            val state = when (next.optString("state")) {
                "unavailable" -> State.Unavailable
                "not_registered" -> State.NotRegistered
                "backed_up" -> State.BackedUp
                "not_backed_up" -> State.NotBackedUp
                else -> State.CouldNotCheck
            }
            // `optString` turns a JSON null into "null" (org.json); `optJSONObject` says "absent".
            val call = next.optJSONObject("call")?.let {
                Call(chainId = it.getInt("chain_id"), to = it.getString("to"), data = it.getString("data"))
            }
            // The core offers a call exactly when there is something to do.
            return if (state == State.NotBackedUp && call == null) COULD_NOT else Check(state, call)
        }
        return COULD_NOT
    }

    private suspend fun perform(request: JSONObject): JSONObject {
        val id = request.optString("id")
        fun answer(outcome: String, body: String? = null) =
            JSONObject().put("id", id).put("outcome", outcome).put("body", body ?: JSONObject.NULL)
        return runCatching {
            val result = ethCall(request.getInt("chain_id"), request.getString("to"), request.getString("data"))
            if (result == null) answer("failed") else answer("ok", result)
        }.getOrElse { answer("failed") }
    }

    companion object {
        private const val MAX_ROUNDS = 16
        private val COULD_NOT = Check(State.CouldNotCheck, null)
    }
}
