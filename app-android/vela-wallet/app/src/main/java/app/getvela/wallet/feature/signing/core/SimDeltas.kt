package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.feature.wallet.core.TrustAssetDelta
import app.getvela.wallet.feature.wallet.core.TrustDeltaKind
import java.math.BigInteger
import org.json.JSONArray
import org.json.JSONObject

/**
 * The simulation's shell half (spec 046 D1, the desktop's `executor/sim.rs`
 * verbatim): the `eth_simulateV1` payload for the request's calls, and the
 * net asset deltas from the logs of the calls that succeeded. Transfer logs
 * touching the wallet are netted per token; the `0xeeee…` sender is the
 * native coin. What the deltas MEAN — trusted, unverified, how to show a
 * received amount — is the core's (`token_trust::judge_delta`); nothing
 * here is ever written anywhere.
 */
object SimDeltas {
    const val TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    const val NATIVE_SENTINEL = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"

    class Call(val to: String, val value: String?, val data: String?)

    /** The `eth_simulateV1` params: one block-state call carrying every call; `null` when there is nothing to simulate. */
    fun payload(from: String, calls: List<Call>): JSONArray? {
        val first = calls.firstOrNull() ?: return null
        if (first.to.isEmpty()) return null
        val entries = JSONArray()
        calls.forEach { call ->
            val entry = JSONObject().put("from", from).put("to", call.to).put("value", hexValue(call.value))
            val data = call.data.orEmpty()
            if (data.isNotEmpty() && data != "0x") entry.put("data", data)
            entries.put(entry)
        }
        val block = JSONObject().put("calls", entries)
        val body = JSONObject()
            .put("blockStateCalls", JSONArray().put(block))
            .put("validation", false)
            .put("traceTransfers", true)
            .put("returnFullTransactions", false)
        return JSONArray().put(body).put("latest")
    }

    /** The logs of every succeeded call in every simulated block; `null` when the answer is an error or empty. */
    fun logsOf(result: JSONObject): List<JSONObject>? {
        if (result.opt("error") != null && !result.isNull("error")) return null
        val blocks = result.optJSONArray("result") ?: return null
        if (blocks.length() == 0) return null
        val logs = ArrayList<JSONObject>()
        for (b in 0 until blocks.length()) {
            val calls = blocks.optJSONObject(b)?.optJSONArray("calls") ?: continue
            for (c in 0 until calls.length()) {
                val call = calls.optJSONObject(c) ?: continue
                if (!succeeded(call)) continue
                val entries = call.optJSONArray("logs") ?: continue
                for (l in 0 until entries.length()) entries.optJSONObject(l)?.let(logs::add)
            }
        }
        return logs
    }

    private fun succeeded(call: JSONObject): Boolean = when (val status = call.opt("status")) {
        is String -> status == "0x1"
        is Number -> status.toLong() == 1L
        else -> call.opt("error") == null || call.isNull("error")
    }

    /** Net Transfer deltas for `user`, in first-seen order, zero totals dropped. */
    fun deriveDeltas(logs: List<JSONObject>, user: String): List<TrustAssetDelta> {
        val me = user.lowercase()
        val order = ArrayList<String>()
        val totals = HashMap<String, BigInteger>()
        for (log in logs) {
            val topics = log.optJSONArray("topics") ?: continue
            if (topics.length() != 3) continue
            if (!topics.optString(0).equals(TRANSFER_TOPIC, ignoreCase = true)) continue
            val from = topicAddress(topics.optString(1))
            val to = topicAddress(topics.optString(2))
            if (from != me && to != me) continue
            val value = firstWord(log.optString("data")) ?: continue
            if (value.signum() == 0) continue
            val address = log.optString("address").lowercase()
            if (address.isEmpty()) continue
            val key = if (address == NATIVE_SENTINEL) "native" else address
            if (key !in totals) order.add(key)
            var total = totals[key] ?: BigInteger.ZERO
            if (to == me) total += value
            if (from == me) total -= value
            totals[key] = total
        }
        return order.mapNotNull { key ->
            val delta = totals[key] ?: BigInteger.ZERO
            if (delta.signum() == 0) return@mapNotNull null
            if (key == "native") {
                TrustAssetDelta(kind = TrustDeltaKind.Native, token = null, delta = delta.toString())
            } else {
                TrustAssetDelta(kind = TrustDeltaKind.Erc20, token = key, delta = delta.toString())
            }
        }
    }

    /** A 32-byte topic → the address in its low 20 bytes, lowercase. */
    internal fun topicAddress(topic: String): String {
        val hex = topic.removePrefix("0x").lowercase()
        return if (hex.length >= 40) "0x" + hex.takeLast(40) else ""
    }

    /** The first 32-byte word of `data` as an unsigned integer. */
    internal fun firstWord(data: String?): BigInteger? {
        val hex = data.orEmpty().removePrefix("0x")
        if (hex.isEmpty()) return null
        val word = hex.take(64)
        return runCatching { BigInteger(word, 16) }.getOrNull()
    }

    /** A value as the node wants it: `0x`-hex; decimal input converted; empty → `0x0`. */
    internal fun hexValue(value: String?): String {
        val raw = value.orEmpty().trim()
        if (raw.isEmpty()) return "0x0"
        if (raw.startsWith("0x") || raw.startsWith("0X")) {
            val digits = raw.drop(2).trimStart('0')
            return "0x" + digits.ifEmpty { "0" }
        }
        val parsed = raw.toBigIntegerOrNull() ?: return "0x0"
        return "0x" + parsed.toString(16)
    }
}
