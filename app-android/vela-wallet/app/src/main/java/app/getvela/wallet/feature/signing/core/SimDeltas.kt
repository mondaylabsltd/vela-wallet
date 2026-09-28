package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.TrustAssetDelta
import kotlinx.serialization.builtins.ListSerializer
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.SimOutcomeRecord
import uniffi.vela_core_uniffi.simOutcome

/**
 * The simulation's shell half (spec 046 D1): the `eth_simulateV1` payload for
 * the request's calls, and the pool's answer handed to the core as it came.
 *
 * Spec 082 RG6: what the answer MEANS is the core's `sim_outcome` — a node
 * that could not simulate is a caution ("could not check"), a call that fails
 * is a danger with its sanitised revert reason, and only a clean run carries
 * deltas (the per-asset moves the trust machine then judges). This file used
 * to parse the logs itself, and read every non-answer as "unavailable" in a
 * tone of its own and a revert as "nothing changes".
 */
object SimDeltas {
    class Call(val to: String, val value: String?, val data: String?)

    /** The `eth_simulateV1` params: one block-state call carrying every call; `null` when there is nothing to simulate. */
    fun payload(from: String, calls: List<Call>): JSONArray? {
        val body = body(from, calls) ?: return null
        return JSONArray().put(body).put("latest")
    }

    /** The first param alone (the pool takes `[body, "latest"]` as a list). */
    fun body(from: String, calls: List<Call>): JSONObject? {
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
        return JSONObject()
            .put("blockStateCalls", JSONArray().put(block))
            .put("validation", false)
            .put("traceTransfers", true)
            .put("returnFullTransactions", false)
    }

    /**
     * The reply envelope the core reads (contract §8): the JSON-RPC body as it
     * came (`{"result": …}` or `{"error": …}`), or `{"unreachable": true}` when
     * the pool gave up or nothing was asked.
     */
    fun replyJson(answer: RpcResult?): String = when (answer) {
        is RpcResult.Body -> answer.json.toString()
        is RpcResult.RangeCapped, is RpcResult.Failed, null -> UNREACHABLE
    }

    /** The core's reading of the answer for [user] (`simOutcome`). */
    fun outcome(user: String, answer: RpcResult?): SimOutcomeRecord = simOutcome(user, replyJson(answer))

    /** The core's notice, drawn in the core's tone; `null` for a clean run (its deltas are judged instead). */
    fun notice(record: SimOutcomeRecord): SigningController.SimOutcome.Notice? {
        val key = record.noticeKey ?: return null
        val risk = record.noticeRisk
            ?.let { runCatching { Wire.json.decodeFromString(ClearRisk.serializer(), "\"$it\"") }.getOrNull() }
            ?: ClearRisk.Caution
        return SigningController.SimOutcome.Notice(risk = risk, key = key, reason = record.revertReason)
    }

    /**
     * A clean run's per-asset moves for the signing account (empty = nothing
     * of theirs moves). `null` when this app cannot read them — "could not
     * check", never the empty list: that is a calm verdict nobody made.
     */
    fun deltas(record: SimOutcomeRecord): List<TrustAssetDelta>? =
        runCatching { Wire.json.decodeFromString(ListSerializer(TrustAssetDelta.serializer()), record.deltasJson) }
            .onFailure { VelaLog.failure("signing.sim", "the core's deltas could not be read", it) }
            .getOrNull()

    /** The core's "could not check" line, for a run whose moves nobody could judge. */
    fun couldNotCheck(): SigningController.SimOutcome.Notice =
        notice(simOutcome("", UNREACHABLE))
            ?: SigningController.SimOutcome.Notice(ClearRisk.Caution, "componentsUi.signing.simUnavailableWarning")

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

    private const val UNREACHABLE = """{"unreachable":true}"""
}
