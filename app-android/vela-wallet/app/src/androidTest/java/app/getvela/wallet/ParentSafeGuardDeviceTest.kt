package app.getvela.wallet

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.core.ParentReason
import app.getvela.wallet.feature.signing.core.SafeOwnerAnswer
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignEvent
import app.getvela.wallet.feature.signing.core.SignShellResult
import app.getvela.wallet.feature.signing.core.SignView
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.vela_core_uniffi.SignRequestCore
import uniffi.vela_core_uniffi.parentSafeCheck

/**
 * VELA-2026-002 on a device, through the app's OWN core library (the .so
 * this APK ships): a site's request that would make this account act for a
 * Safe it owns is read first (`read_safe_owner`), refused by the signing
 * machine once the read says "owner", never signed, and answered `-32603`
 * `parent_safe_blocked`; one nobody could read is refused "couldn't check".
 * The sheet says it in the catalog's words, from the catalog this APK
 * carries. (The JVM proofs of the entry points are `ParentSafeEntryPointsTest`;
 * this pins that the phone's library is the same.)
 */
@RunWith(AndroidJUnit4::class)
class ParentSafeGuardDeviceTest {
    private val signer = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    private val parent = "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"
    private val unknown = "0x7777777777777777777777777777777777777777"
    private val chain = 8453
    private val owned = "0x" + "0".repeat(63) + "1"

    /** The vector's case (a): `parent.execTransaction(swapOwner(…, signer → 0xbad…))`, signature `{r = signer, v = 1}`. */
    /** Copied from `tests/vectors/rules/parent-safe-guard.json` case (a) — the test APK carries no vectors. */
    private val takeoverData = "0x6a761202" + listOf(
        "0000000000000000000000005afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000140",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "00000000000000000000000000000000000000000000000000000000000001e0",
        "0000000000000000000000000000000000000000000000000000000000000064",
        "e318b52b00000000000000000000000000000000000000000000000000000000",
        "00000001000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "aaaaaaaa000000000000000000000000badbadbadbadbadbadbadbadbadbadba",
        "dbadbad000000000000000000000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000041",
        "000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0100000000000000000000000000000000000000000000000000000000000000",
    ).joinToString("")

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply { initialize("en") }
    }

    private fun tx(to: String, data: String) = JSONArray().put(JSONObject().put("to", to).put("data", data).put("value", "0x0"))

    /** Every operation the machine asked for, in order. */
    private val asked = ArrayList<JSONObject>()

    /**
     * The signing machine booted as the app boots it per request: networks
     * and the one signer. Every `read_safe_owner` is answered [readAnswer];
     * every page answer is acknowledged; anything else is left out.
     */
    private fun machine(readAnswer: (JSONObject) -> SafeOwnerAnswer): CoreScript {
        val script = CoreScript(SignRequestCore().asBridge()) { op ->
            asked += op
            when (op.optString("type")) {
                "read_safe_owner" -> Wire.json.encodeToString(
                    SignShellResult.serializer(),
                    SignShellResult.SafeOwnerRead(op.getString("id"), op.getInt("chain_id"), op.getString("safe"), readAnswer(op)),
                )
                "send_response" -> Wire.json.encodeToString(SignShellResult.serializer(), SignShellResult.Responded)
                else -> null
            }
        }
        script.dispatch(Wire.json.encodeToString(SignEvent.serializer(), SignEvent.NetworksChanged(listOf(chain))))
        script.dispatch(Wire.json.encodeToString(SignEvent.serializer(), SignEvent.AccountsChanged(listOf(SignAccountRef(address = signer, credential_id = "00")), 0)))
        return script
    }

    private fun arrive(script: CoreScript, params: JSONArray) = script.dispatch(
        Wire.json.encodeToString(
            SignEvent.serializer(),
            SignEvent.RequestArrived(
                id = "rid-1", method = "eth_sendTransaction", params_json = params.toString(), origin = "https://app.example",
                transport_id = "tab-1", dedicated_transport = true, per_request_chain = chain, granted_address = signer, now_ms = 1.0,
            ),
        ),
    )

    private fun view(script: CoreScript): SignView = Wire.json.decodeFromString(SignView.serializer(), script.viewJson())

    /** Closed by the person: the page is answered "the wallet refused" (-32603, `parent_safe_blocked`), never 4001. */
    private fun assertAnsweredRefused(script: CoreScript) {
        script.dispatch(Wire.json.encodeToString(SignEvent.serializer(), SignEvent.RejectTapped))
        val response = asked.last { it.optString("type") == "send_response" }
        val payload = response.getJSONObject("payload")
        assertEquals(-32603, payload.getInt("code"))
        assertEquals("parent_safe_blocked", payload.getString("kind"))
        assertFalse("nothing signed", asked.any { it.optString("type") == "sign_and_submit" })
    }

    private fun sheetWords(view: SignView) =
        SigningLive.statusBlocks(view, strings).filterIsInstance<SigningBlock.Warning>().single().text

    @Test
    fun the_takeover_of_an_owned_safe_is_refused_by_this_phones_core() {
        val verdict = JSONObject(
            parentSafeCheck(
                JSONObject().put("method", "eth_sendTransaction").put("params", tx(parent, takeoverData)).put("signer", signer)
                    .put("chain_id", chain)
                    .put("answers", JSONArray().put(JSONObject().put("chain_id", chain).put("safe", parent).put("answer", JSONObject().put("type", "returned").put("data", owned))))
                    .put("at_submit", false).toString(),
            ),
        )
        assertEquals("refuse", verdict.getString("verdict"))
        assertEquals("owner", verdict.getString("reason"))

        val script = machine { SafeOwnerAnswer.Returned(owned) }
        arrive(script, tx(parent, takeoverData))
        val read = asked.single { it.optString("type") == "read_safe_owner" }
        assertEquals(parent, read.getString("safe"))
        assertTrue(read.getString("data").startsWith("0x2f54bf6e"))
        val view = view(script)
        val blocked = view.blocked!!
        assertEquals(parent, blocked.parent)
        assertEquals(ParentReason.Owner, blocked.reason)
        assertEquals("execTransaction", blocked.function)
        assertFalse(view.confirm_gate_open)
        assertEquals(
            strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to "execTransaction", "safe" to ExploreLive.shortAddress(parent))),
            sheetWords(view),
        )
        assertAnsweredRefused(script)
    }

    /**
     * 0.9.10: the same takeover with the Safe's address written without its
     * `0x` — the encoders read it as the Safe, 0.9.9's guard did not — is
     * answered -32602 at arrival by this phone's core: no read, no sheet,
     * nothing signed.
     */
    @Test
    fun the_bare_hex_takeover_is_answered_invalid_params_by_this_phones_core() {
        val verdict = JSONObject(
            parentSafeCheck(
                JSONObject().put("method", "eth_sendTransaction").put("params", tx(parent.removePrefix("0x"), takeoverData))
                    .put("signer", signer).put("chain_id", chain).put("answers", JSONArray()).put("at_submit", false)
                    .put("at_arrival", true).toString(),
            ),
        )
        assertEquals("invalid", verdict.getString("verdict"))
        assertEquals("Invalid transaction `to`: expected 0x followed by 40 hex characters", verdict.getString("message"))

        for (to in listOf(parent.removePrefix("0x"), "0X" + parent.removePrefix("0x"), parent.dropLast(2))) {
            asked.clear()
            val script = machine { SafeOwnerAnswer.Returned(owned) }
            arrive(script, tx(to, takeoverData))
            assertFalse("$to: never read", asked.any { it.optString("type") == "read_safe_owner" })
            val payload = asked.single { it.optString("type") == "send_response" }.getJSONObject("payload")
            assertEquals(to, -32602, payload.getInt("code"))
            assertEquals(to, "invalid_params", payload.getString("kind"))
            assertEquals(to, null, view(script).blocked)
            assertFalse("$to: nothing signed", asked.any { it.optString("type") == "sign_and_submit" })
        }
    }

    /** 0.9.10: a site's SafeOp for a Safe this account owns (under a Safe v1.5.0, its approval) — read, refused. */
    @Test
    fun a_safe_op_for_an_owned_safe_is_refused_by_this_phones_core() {
        val module = "0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226"
        val doc = JSONObject()
            .put("types", JSONObject()
                .put("EIP712Domain", JSONArray().put(JSONObject().put("name", "chainId").put("type", "uint256")).put(JSONObject().put("name", "verifyingContract").put("type", "address")))
                .put("SafeOp", JSONArray().put(JSONObject().put("name", "safe").put("type", "address")).put(JSONObject().put("name", "nonce").put("type", "uint256")).put(JSONObject().put("name", "callData").put("type", "bytes"))))
            .put("primaryType", "SafeOp")
            .put("domain", JSONObject().put("chainId", chain).put("verifyingContract", module))
            .put("message", JSONObject().put("safe", parent).put("nonce", "0").put("callData", "0x"))
        val script = machine { SafeOwnerAnswer.Returned(owned) }
        script.dispatch(
            Wire.json.encodeToString(
                SignEvent.serializer(),
                SignEvent.RequestArrived(
                    id = "rid-1", method = "eth_signTypedData_v4", params_json = JSONArray().put(signer).put(doc.toString()).toString(),
                    origin = "https://app.example", transport_id = "tab-1", dedicated_transport = true, per_request_chain = chain,
                    granted_address = signer, now_ms = 1.0,
                ),
            ),
        )
        assertEquals("the Safe is read, never the module", listOf(parent), asked.filter { it.optString("type") == "read_safe_owner" }.map { it.getString("safe") })
        val view = view(script)
        assertEquals(parent, view.blocked!!.parent)
        assertEquals(ParentReason.Owner, view.blocked!!.reason)
        assertEquals("SafeOp", view.blocked!!.function)
        assertEquals(
            strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to "SafeOp", "safe" to ExploreLive.shortAddress(parent))),
            sheetWords(view),
        )
        assertAnsweredRefused(script)
    }

    @Test
    fun a_safe_nobody_could_read_is_refused_couldnt_check() {
        val script = machine { SafeOwnerAnswer.Unreachable("offline") }
        arrive(script, tx(unknown, "0xd4d9bdcd" + "cd".repeat(32)))
        assertEquals(unknown, asked.single { it.optString("type") == "read_safe_owner" }.getString("safe"))
        val view = view(script)
        assertEquals(unknown, view.blocked!!.parent)
        assertEquals(ParentReason.Unchecked, view.blocked!!.reason)
        assertEquals(
            strings.t("componentsUi.signing.parentSafeUnchecked", mapOf("function" to "approveHash", "safe" to ExploreLive.shortAddress(unknown))),
            sheetWords(view),
        )
        assertAnsweredRefused(script)
    }
}
