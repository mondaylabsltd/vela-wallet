package app.getvela.wallet

import app.getvela.wallet.feature.signing.core.ClearRisk
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.TrustDeltaKind
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 082 RG6: this app builds the `eth_simulateV1` request and hands the
 * pool's answer to the core as it came; the core's `simOutcome` says what it
 * means. What is pinned here is the MAPPING — a node that could not check is
 * a caution, a failing call a danger with its reason, a clean run deltas —
 * and that nothing of the old parser survives in this app.
 */
class SimDeltasTest {
    private val me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val other = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"
    private val transferTopic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"

    private fun topic(address: String) = "0x" + "0".repeat(24) + address.removePrefix("0x").lowercase()
    private fun word(value: Long) = "0x" + value.toString(16).padStart(64, '0')
    private fun transfer(token: String, from: String, to: String, value: Long) = JSONObject()
        .put("address", token)
        .put("topics", JSONArray().put(transferTopic).put(topic(from)).put(topic(to)))
        .put("data", word(value))

    private fun answer(vararg calls: JSONObject): RpcResult =
        RpcResult.Body(JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", JSONArray().put(JSONObject().put("calls", JSONArray(calls.toList())))))

    private fun noticeOf(answer: RpcResult?) = SimDeltas.notice(SimDeltas.outcome(me, answer))

    /** `Error(string)` ABI of [text]: selector, offset, length, bytes. */
    private fun errorString(text: String): String {
        val bytes = text.toByteArray()
        val hex = bytes.joinToString("") { "%02x".format(it) }.padEnd(((bytes.size + 31) / 32) * 64, '0')
        return "0x08c379a0" + "20".padStart(64, '0') + bytes.size.toString(16).padStart(64, '0') + hex
    }

    @Test
    fun `the payload is one block-state call with every call, data only when present`() {
        val params = SimDeltas.payload(me, listOf(SimDeltas.Call(other, "1000000000000000", "0x"), SimDeltas.Call(usdc, null, "0xa9059cbb")))!!
        val body = params.getJSONObject(0)
        assertEquals("latest", params.getString(1))
        assertTrue(body.getBoolean("traceTransfers"))
        val calls = body.getJSONArray("blockStateCalls").getJSONObject(0).getJSONArray("calls")
        assertEquals(2, calls.length())
        assertEquals("0x38d7ea4c68000", calls.getJSONObject(0).getString("value"))
        assertTrue(!calls.getJSONObject(0).has("data"))
        assertEquals("0xa9059cbb", calls.getJSONObject(1).getString("data"))
        assertEquals("0x0", calls.getJSONObject(1).getString("value"))
        assertNull(SimDeltas.payload(me, emptyList()))
        assertNull(SimDeltas.payload(me, listOf(SimDeltas.Call("", null, null))))
    }

    @Test
    fun `a node that could not check is a caution, never a danger and never nothing-moves`() {
        val unavailable = SigningController.SimOutcome.Notice(ClearRisk.Caution, "componentsUi.signing.simUnavailableWarning")
        assertEquals("the pool gave up", unavailable, noticeOf(RpcResult.Failed(rateLimited = false)))
        assertEquals("nothing was asked", unavailable, noticeOf(null))
        val refused = RpcResult.Body(JSONObject().put("error", JSONObject().put("code", -32601).put("message", "the method eth_simulateV1 does not exist")))
        assertEquals("the node does not offer it", unavailable, noticeOf(refused))
        val crashed = RpcResult.Body(JSONObject().put("error", JSONObject().put("code", -32603).put("message", "method handler crashed")))
        assertEquals(unavailable, noticeOf(crashed))
        assertEquals("an empty answer is not a check", unavailable, noticeOf(RpcResult.Body(JSONObject().put("result", JSONArray()))))
        assertEquals(unavailable, SimDeltas.couldNotCheck())
    }

    @Test
    fun `a failing call is a danger, with its revert reason made safe to print`() {
        val reverted = noticeOf(answer(JSONObject().put("status", "0x0").put("returnData", errorString("STF")).put("logs", JSONArray())))!!
        assertEquals(ClearRisk.Danger, reverted.risk)
        assertEquals("componentsUi.signing.simWillFailReason", reverted.key)
        assertEquals("STF", reverted.reason)
        val bare = noticeOf(answer(JSONObject().put("status", "0x0").put("returnData", "0x").put("logs", JSONArray())))!!
        assertEquals(ClearRisk.Danger, bare.risk)
        assertEquals("componentsUi.signing.simWillFail", bare.key)
    }

    @Test
    fun `a clean run carries the core's deltas and no notice`() {
        val record = SimDeltas.outcome(
            me,
            answer(
                JSONObject().put("status", "0x1").put("returnData", "0x").put(
                    "logs",
                    JSONArray()
                        .put(transfer("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", me, other, 1_000))
                        .put(transfer(usdc, other, me, 5_000))
                        .put(transfer(usdc, other, "0x1111111111111111111111111111111111111111", 7)),
                ),
            ),
        )
        assertEquals("deltas", record.kind)
        assertNull(SimDeltas.notice(record))
        val deltas = SimDeltas.deltas(record)!!
        assertEquals(listOf(TrustDeltaKind.Native, TrustDeltaKind.Erc20), deltas.map { it.kind })
        assertEquals(listOf("-1000", "5000"), deltas.map { it.delta })
        assertTrue(deltas[1].token.equals(usdc, ignoreCase = true))
        // Checked, nothing of theirs moves: an empty list, still a check.
        val quiet = SimDeltas.outcome(me, answer(JSONObject().put("status", "0x1").put("returnData", "0x").put("logs", JSONArray())))
        assertEquals("deltas", quiet.kind)
        assertTrue(SimDeltas.deltas(quiet)!!.isEmpty())
    }

    /**
     * A clean run whose moves this app cannot read (a delta kind it has no
     * name for) is "could not check" — never the empty list, which the sheet
     * draws as "checked, nothing of yours moves": a calm verdict nobody made.
     */
    @Test
    fun `deltas this app cannot read are not an empty check`() {
        val unreadable = uniffi.vela_core_uniffi.SimOutcomeRecord(
            kind = "deltas",
            deltasJson = """[{"kind":"a_kind_this_build_has_no_name_for","token":null,"delta":"-1000"}]""",
            revertReason = null,
            noticeRisk = null,
            noticeKey = null,
        )
        assertNull(SimDeltas.deltas(unreadable))
    }

    @Test
    fun `values become node hex`() {
        assertEquals("0x0", SimDeltas.hexValue(null))
        assertEquals("0x0", SimDeltas.hexValue("0x0000"))
        assertEquals("0x38d7ea4c68000", SimDeltas.hexValue("1000000000000000"))
        assertEquals("0xde0b6b3a7640000", SimDeltas.hexValue("0x0de0b6b3a7640000"))
    }
}
