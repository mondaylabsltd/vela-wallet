package app.getvela.wallet

import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.wallet.core.TrustDeltaKind
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SimDeltasTest {
    private val me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val other = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"

    private fun topic(address: String) = "0x" + "0".repeat(24) + address.removePrefix("0x").lowercase()
    private fun word(value: Long) = "0x" + value.toString(16).padStart(64, '0')
    private fun transfer(token: String, from: String, to: String, value: Long) = JSONObject()
        .put("address", token)
        .put("topics", org.json.JSONArray().put(SimDeltas.TRANSFER_TOPIC).put(topic(from)).put(topic(to)))
        .put("data", word(value))

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
    fun `deltas net per token, the sentinel is the native coin, zero and strangers drop`() {
        val logs = listOf(
            transfer(SimDeltas.NATIVE_SENTINEL, me, other, 1_000),
            transfer(usdc, other, me, 12_000_000),
            transfer(usdc, me, other, 2_000_000),
            transfer(usdc, other, "0x1111111111111111111111111111111111111111", 5),
            transfer(usdc, me, me, 7),
            transfer(usdc, other, me, 0),
        )
        val deltas = SimDeltas.deriveDeltas(logs, me)
        assertEquals(2, deltas.size)
        assertEquals(TrustDeltaKind.Native, deltas[0].kind)
        assertEquals("-1000", deltas[0].delta)
        assertEquals(TrustDeltaKind.Erc20, deltas[1].kind)
        assertEquals(usdc.lowercase(), deltas[1].token)
        assertEquals("10000000", deltas[1].delta)
    }

    @Test
    fun `only the logs of succeeded calls count, and an error answer is nothing`() {
        val ok = JSONObject().put("status", "0x1").put("logs", org.json.JSONArray().put(transfer(usdc, other, me, 1)))
        val failed = JSONObject().put("status", "0x0").put("logs", org.json.JSONArray().put(transfer(usdc, other, me, 99)))
        val result = JSONObject().put("result", org.json.JSONArray().put(JSONObject().put("calls", org.json.JSONArray().put(ok).put(failed))))
        val logs = SimDeltas.logsOf(result)!!
        assertEquals(1, logs.size)
        assertNull(SimDeltas.logsOf(JSONObject().put("error", JSONObject().put("code", -32601))))
        assertNull(SimDeltas.logsOf(JSONObject().put("result", org.json.JSONArray())))
    }

    @Test
    fun `values become node hex`() {
        assertEquals("0x0", SimDeltas.hexValue(null))
        assertEquals("0x0", SimDeltas.hexValue("0x0000"))
        assertEquals("0x38d7ea4c68000", SimDeltas.hexValue("1000000000000000"))
        assertEquals("0xde0b6b3a7640000", SimDeltas.hexValue("0x0de0b6b3a7640000"))
    }
}
