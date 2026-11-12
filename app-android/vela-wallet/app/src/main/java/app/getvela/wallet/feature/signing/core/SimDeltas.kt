package app.getvela.wallet.feature.signing.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.send.core.FeeBalanceChange
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.TrustAssetDelta
import app.getvela.wallet.feature.wallet.core.TrustDeltaKind
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
     * The reply envelope the core reads (contract §8, R22): the first node's
     * JSON-RPC body (`{"result": …}` or `{"error": …}`), the second,
     * independent node's beside it (`witness`) or the person's own node alone
     * (`ownNode`) — built by the core's helper over the pool's corroborated
     * answer — or `{"unreachable": true}` when the pool gave up or nothing
     * was asked. A body with no witness is one node's word: `unconfirmed`.
     */
    fun replyJson(answer: RpcResult?): String = RpcResult.corroboratedEnvelope(answer)

    /**
     * The core's reading of the answer for [user] (`simOutcome`). [valueWei]
     * (§39): the native value the request sends from the account, as the
     * wallet decoded it ([nativeValue]) — a node's "No asset changes" never
     * erases it (`sim_outcome::with_decoded_value`); `null` when zero.
     */
    fun outcome(user: String, answer: RpcResult?, valueWei: String? = null): SimOutcomeRecord =
        simOutcome(user, replyJson(answer), valueWei)

    /**
     * §39: the native value [calls] send from the account, as the wallet
     * decoded them — each call's `value` (decimal or `0x`-hex), summed for a
     * batch; `null` when it is zero, or a value cannot be read (unknown is
     * never a figure).
     */
    fun nativeValue(calls: List<Call>): String? {
        var sum = java.math.BigInteger.ZERO
        for (call in calls) {
            val raw = call.value.orEmpty().trim()
            if (raw.isEmpty()) continue
            val value = if (raw.startsWith("0x") || raw.startsWith("0X")) {
                raw.drop(2).ifEmpty { "0" }.toBigIntegerOrNull(16)
            } else {
                raw.toBigIntegerOrNull()
            } ?: return null
            if (value.signum() < 0) return null
            sum += value
        }
        return sum.takeIf { it.signum() > 0 }?.toString()
    }

    /** The core's notice, drawn in the core's tone; `null` for a clean run (its deltas are judged instead). */
    fun notice(record: SimOutcomeRecord): SigningController.SimOutcome.Notice? {
        val key = record.noticeKey ?: return null
        val risk = record.noticeRisk
            ?.let { runCatching { Wire.json.decodeFromString(ClearRisk.serializer(), "\"$it\"") }.getOrNull() }
            ?: ClearRisk.Caution
        return SigningController.SimOutcome.Notice(
            risk = risk, key = key, reason = record.revertReason,
            // dapp-15: the core's `SimVerdict.recheck` — "no node answered",
            // the line is about the network: asked again once it answers.
            // R22: also a check nobody independent confirmed — "Check again".
            recheck = record.recheck,
            // R22: one node's word, or two that disagree — the sheet also says
            // how to check with a node the person trusts (`ownNodeHint`).
            ownNodeHint = record.kind == KIND_UNCONFIRMED,
            // dapp-15: only a node nobody reached is watched for coming back;
            // a node that answered alone would answer at once, and again.
            watchNode = record.kind == KIND_UNREACHABLE,
            verdict = verdictJson(record),
        )
    }

    /**
     * R22: the core's verdict as the confirm gate reads it back
     * (`sign_confirm_state`'s `sim_json`, a `SimVerdict`): the record as it
     * came, its deltas as the parsed array.
     */
    fun verdictJson(record: SimOutcomeRecord): String = verdictObject(record).toString()

    private fun verdictObject(record: SimOutcomeRecord): kotlinx.serialization.json.JsonObject =
        kotlinx.serialization.json.buildJsonObject {
            put("kind", kotlinx.serialization.json.JsonPrimitive(record.kind))
            put("deltas", runCatching { Wire.json.parseToJsonElement(record.deltasJson) }.getOrDefault(kotlinx.serialization.json.JsonArray(emptyList())))
            put("revert_reason", kotlinx.serialization.json.JsonPrimitive(record.revertReason))
            put("notice_risk", kotlinx.serialization.json.JsonPrimitive(record.noticeRisk))
            put("notice_key", kotlinx.serialization.json.JsonPrimitive(record.noticeKey))
            put("no_change_key", kotlinx.serialization.json.JsonPrimitive(record.noChangeKey))
            put("recheck", kotlinx.serialization.json.JsonPrimitive(record.recheck))
            record.source?.let { put("source", kotlinx.serialization.json.JsonPrimitive(it)) }
            record.sourceKey?.let { put("source_key", kotlinx.serialization.json.JsonPrimitive(it)) }
        }

    /**
     * Spec 103: the simulation as the agent's review reads it
     * (`agent_review::SimEnded`) — the core's [record] as it came
     * (`SimVerdict`), and the trust machine's [judged] moves, if any.
     */
    fun ended(record: SimOutcomeRecord, judged: app.getvela.wallet.feature.wallet.core.TrustSimView?): kotlinx.serialization.json.JsonElement =
        kotlinx.serialization.json.buildJsonObject {
            put("verdict", verdictObject(record))
            put(
                "judged",
                judged?.let { Wire.json.encodeToJsonElement(app.getvela.wallet.feature.wallet.core.TrustSimView.serializer(), it) }
                    ?: kotlinx.serialization.json.JsonNull,
            )
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

    /**
     * A clean run's deltas as the fee machine reads them (spec 083 fee, issue
     * #411; the desktop's `fee_balance_changes`): what the operation moves of
     * each asset, native or by contract. The core derived each from the
     * coin's OWN `Transfer` logs (the contract that emitted them) or the
     * node's trace of native value — nothing a site's contract can emit on a
     * coin's behalf. A token move with no contract names no coin and is
     * dropped: never read as the native one.
     */
    fun feeBalanceChanges(deltas: List<TrustAssetDelta>): List<FeeBalanceChange> = deltas.mapNotNull { delta ->
        when (delta.kind) {
            TrustDeltaKind.Native -> FeeBalanceChange(token = null, delta = delta.delta)
            TrustDeltaKind.Erc20 -> delta.token?.let { FeeBalanceChange(token = it, delta = delta.delta) }
        }
    }

    /** The core's "could not check" line, for a run whose moves nobody could judge. */
    fun couldNotCheck(): SigningController.SimOutcome.Notice =
        // Not about the network (moves nobody could read or judge, a reading
        // that threw): asking the node again would say the same — no recheck.
        // R22: the gate reads it as the could-not-check it is (unread, "… anyway").
        notice(simOutcome("", UNREACHABLE, null))?.copy(recheck = false, watchNode = false)
            ?: SigningController.SimOutcome.Notice(ClearRisk.Caution, "componentsUi.signing.simUnavailableWarning")

    /**
     * A check under which nothing of the person's moves, as the core reads
     * one (`simOutcome` over a clean run with no logs): no judgment, and the
     * core's own line for it (`no_change_key`, "No asset changes"). For the
     * room the sheet keeps and for a board; a session's comes from the trust
     * machine's judged view, which carries the same key.
     */
    fun nothingMoves(): SigningController.SimOutcome = NOTHING_MOVES

    private val NOTHING_MOVES: SigningController.SimOutcome by lazy {
        // R22: a check two independent nodes agreed on (the witness beside it).
        val clean = JSONObject().put("result", JSONArray().put(JSONObject().put("calls", JSONArray().put(JSONObject().put("status", "0x1").put("logs", JSONArray())))))
        val record = simOutcome("", JSONObject(clean.toString()).put("witness", clean).toString(), null)
        notice(record) ?: SigningController.SimOutcome.Ready(emptyList(), noChangeKey = record.noChangeKey, verdict = verdictJson(record))
    }

    /**
     * R22: the core's notice for a check only one node answered (`unconfirmed`,
     * a clean run with no witness) — for the room the sheet keeps; a
     * session's comes from its own corroborated read.
     */
    fun oneNodeOnly(): SigningController.SimOutcome? = ONE_NODE

    private val ONE_NODE: SigningController.SimOutcome? by lazy {
        val clean = JSONObject().put("result", JSONArray().put(JSONObject().put("calls", JSONArray().put(JSONObject().put("status", "0x1").put("logs", JSONArray())))))
        notice(simOutcome("", clean.toString(), null))
    }

    /**
     * The core's "expected to fail" line at the LONGEST reason it prints — a
     * revert whose `Error(string)` runs past the core's cap, read by the core
     * like any other (`simOutcome`), so the reason comes back cut where the
     * core cuts it. Never shown: the signing sheet measures it to know how
     * tall that verdict can be, and keeps that room.
     */
    fun longestRevert(): SigningController.SimOutcome.Notice? = LONGEST_REVERT

    private val LONGEST_REVERT: SigningController.SimOutcome.Notice? by lazy {
        reverted("ERC20: transfer amount exceeds the balance of the sending account, and more of it")
    }

    /**
     * The core's notice for a call that fails with [reason] — its own reading
     * (`simOutcome`) of a node's answer carrying that `Error(string)`:
     * `{"result":[{"calls":[{"status":"0x0","returnData":…}]}]}`. For the
     * room above and for a board; a session's comes from its node.
     */
    fun reverted(reason: String): SigningController.SimOutcome.Notice? {
        val bytes = reason.toByteArray(Charsets.UTF_8)
        fun word(value: Int) = value.toString(16).padStart(64, '0')
        val text = bytes.joinToString("") { "%02x".format(it) }.let { hex -> hex.padEnd((hex.length + 63) / 64 * 64, '0') }
        val data = "0x08c379a0" + word(32) + word(bytes.size) + text
        val reply = JSONObject().put(
            "result",
            JSONArray().put(JSONObject().put("calls", JSONArray().put(JSONObject().put("status", "0x0").put("returnData", data)))),
        ).toString()
        return notice(simOutcome("", reply, null))
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

    private const val UNREACHABLE = """{"unreachable":true}"""

    /** R22: the core's verdict kinds this file tells apart (`SimVerdict.kind`). */
    const val KIND_UNCONFIRMED = "unconfirmed"
    const val KIND_UNREACHABLE = "unreachable"
}
