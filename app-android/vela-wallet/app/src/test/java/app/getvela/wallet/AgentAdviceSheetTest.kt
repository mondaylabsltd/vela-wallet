package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.signing.SigningAdvice
import app.getvela.wallet.feature.signing.core.SheetFacts
import app.getvela.wallet.feature.signing.core.SigningController
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.AgentCore
import uniffi.vela_core_uniffi.AgentSendOpen
import uniffi.vela_core_uniffi.AgentShell
import uniffi.vela_core_uniffi.intentReview

/**
 * Spec 103 on the phone: the agent's opinion card is the core's — the phone
 * agent (`vela-agent`) holds the opinion and hands the card its standing
 * (`AgentCore.advice`). What this app owes it is the sheet's facts beside the
 * request (`AgentShell.signing`'s `sheet` member, `agent_review::SheetFacts`):
 * the request itself — not the sign machine's whole view — and, for THIS
 * request only, its fee and simulation. The fee is what an opinion is bound
 * to: moved past its band, the core greys the card.
 */
class AgentAdviceSheetTest {

    @Test
    fun `the agent reads the request itself, with the sheet's facts beside it`() {
        val handed = JSONObject(signing(fee = fee(100, "1000000000000000"))!!)
        assertEquals("r1", handed.getString("id"))
        assertEquals("eth_sendTransaction", handed.getString("method"))
        assertFalse("the request, not the machine's whole view", handed.has("request"))
        val sheet = handed.getJSONObject("sheet")
        assertEquals(100, sheet.getJSONObject("fee").getJSONObject("fee").getInt("chain_id"))
        assertEquals(JSONObject(CLEAR).toString(), sheet.getJSONObject("clear").toString())
        assertEquals("https://data.example/v1", sheet.getString("descriptor_base"))
        assertTrue("a transaction's check is out", sheet.getBoolean("simulation_pending"))
        assertTrue(sheet.isNull("simulation"))
    }

    @Test
    fun `only this request's fee and simulation are its facts`() {
        // A figure for another chain is a previous request's.
        val otherChain = JSONObject(signing(fee = fee(8453, "7"))!!).getJSONObject("sheet")
        assertTrue(otherChain.isNull("fee"))
        // A simulation that ended for another request is not this one's.
        val ended = buildJsonObject { put("verdict", JsonPrimitive("x")) }
        val stale = JSONObject(signing(simFacts = "r0" to ended, sim = null)!!).getJSONObject("sheet")
        assertTrue(stale.isNull("simulation"))
        assertFalse("nothing out, nothing pending", stale.getBoolean("simulation_pending"))
        val mine = JSONObject(signing(simFacts = "r1" to ended)!!).getJSONObject("sheet")
        assertEquals("x", mine.getJSONObject("simulation").getString("verdict"))
        assertFalse(mine.getBoolean("simulation_pending"))
        // A message is not on chain: nothing is pending for it.
        val message = JSONObject(signing(kind = "personal_sign", method = "personal_sign")!!).getJSONObject("sheet")
        assertFalse(message.getBoolean("simulation_pending"))
        // No request on the sheet: nothing to hand.
        assertNull(SheetFacts.signing("{\"request\":null}", null, null, null, null, null, null))
        assertNull(SheetFacts.signing(null, null, null, null, null, null, null))
        assertNull(SheetFacts.signing("not json", null, null, null, null, null, null))
    }

    /**
     * The binding end to end through the core's own rules — the review the
     * agent reads (`intent_rules::review` over what this app hands it) and
     * the standing its card is drawn with (`intent_advice_standing`, what
     * `AgentCore.advice` reads): current at the fee it was given for, given
     * at another fee once the fee moved past its band, given for an earlier
     * version once the request changed.
     */
    @Test
    fun `an opinion greys when the fee or the request no longer matches`() {
        val given = review(signing(fee = fee(100, "1000000000000000"))!!)
        val digest = given.getJSONObject("signing").getString("review_digest")
        val feeGiven = given.getJSONObject("signing").get("fee").toString()
        assertNotEquals("the review shows the sheet's fee", "null", feeGiven)
        val advice = SigningAdvice.Accepted(
            review_digest = digest,
            verdict = SigningAdvice.Verdict.Caution,
            summary = "A plain transfer.",
            fee = Wire.json.parseToJsonElement(feeGiven),
        )
        fun standingOf(handed: String): SigningAdvice.StandingAnswer {
            val now = review(handed).getJSONObject("signing")
            return SigningAdvice.standing(advice, now.getString("review_digest"), now.get("fee").toString())
        }
        assertEquals(SigningAdvice.Standing.Current, standingOf(signing(fee = fee(100, "1000000000000000"))!!).standing)
        // A nudge inside the band is the same fee.
        assertEquals(SigningAdvice.Standing.Current, standingOf(signing(fee = fee(100, "1100000000000000"))!!).standing)
        val moved = standingOf(signing(fee = fee(100, "3000000000000000"))!!)
        assertEquals(SigningAdvice.Standing.FeeChanged, moved.standing)
        assertEquals("0.001", moved.fee_when_given?.amount)
        val changed = standingOf(signing(fee = fee(100, "1000000000000000"), value = "0x2")!!)
        assertEquals(SigningAdvice.Standing.EarlierVersion, changed.standing)
    }

    /** The phone agent holds no opinion until one is given: no card, and never a crash. */
    @Test
    fun `the agent core gives no card before an opinion`() {
        val core = AgentCore(Shell(signing(fee = fee(100, "1000000000000000"))), "phone")
        try {
            assertNull(core.advice())
        } finally {
            core.close()
        }
    }

    // -- the rig ------------------------------------------------------------------

    private fun signing(
        fee: String? = null,
        simFacts: Pair<String, kotlinx.serialization.json.JsonElement>? = null,
        sim: SigningController.SimOutcome? = SigningController.SimOutcome.Pending,
        kind: String = "transaction",
        method: String = "eth_sendTransaction",
        value: String = "0x1",
    ): String? {
        val params = JSONArray().put(JSONObject().put("from", SAFE).put("to", TO).put("value", value)).toString()
        val view = JSONObject().put(
            "request",
            JSONObject().put("id", "r1").put("method", method).put("kind", kind).put("params_json", params)
                .put("origin", "https://app.example").put("chain_id", 100).put("signer_address", SAFE),
        )
        return SheetFacts.signing(view.toString(), CLEAR, null, fee, sim, simFacts, "https://data.example/v1")
    }

    private fun fee(chainId: Int, totalWei: String): String = JSONObject()
        .put("busy", false).put("stale", false).put("failed", JSONObject.NULL)
        .put("fee", JSONObject().put("chain_id", chainId).put("total_wei", totalWei).put("fee_asset", JSONObject().put("type", "native")))
        .toString()

    /** `vela_review` as the phone agent builds it: the request, and its `sheet` member beside it. */
    private fun review(handed: String): JSONObject {
        val signing = JSONObject(handed)
        val input = JSONObject().put("signing", signing).put("sheet", signing.get("sheet")).put("account", SAFE)
        return JSONObject(intentReview(input.toString()))
    }

    private class Shell(private val signing: String?) : AgentShell {
        override fun views(): String = "{}"
        override fun page(): String = "explore"
        override fun signing(): String? = signing
        override fun changed() = Unit
        override fun perform(action: String): String = "{}"
        override fun reveal(screen: String) = Unit
        override fun sendOpen(): AgentSendOpen? = null
        override fun sendAlert(): String? = null
        override fun sendStart(params: String): ULong? = null
        override fun sendDispatch(event: String) = Unit
        override fun sendClose() = Unit
        override fun account(): String = SAFE
        override fun balancesSettled(account: String): Boolean = true
        override fun holdings(): String = "[]"
        override fun tracker(): String = "[]"
        override fun networkId(chainId: UInt): String = "chain-$chainId"
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val TO = "0x1111111111111111111111111111111111111111"
        const val CLEAR = """{"surface":"clear"}"""
    }
}
