package app.getvela.wallet

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.format.ShortAddress
import app.getvela.wallet.core.i18n.I18nRuntime
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
 * §31 on a device, through the app's OWN core library (the .so this APK
 * ships): a site's request that would make this account act for a multisig
 * it is a member of is refused by the signing machine, never signed, and
 * answered `-32603` `parent_safe_blocked`; a Safe not on the device's list is
 * read first (`read_safe_owner`), and one nobody could read is refused
 * "couldn't check". The sheet says it in the catalog's words, from the
 * catalog this APK carries. (The JVM proofs of the entry points are
 * `ParentSafeEntryPointsTest`; this pins that the phone's library is the same.)
 */
@RunWith(AndroidJUnit4::class)
class ParentSafeGuardDeviceTest {
    private val signer = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    private val parent = "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"
    private val unknown = "0x7777777777777777777777777777777777777777"
    private val chain = 8453

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

    /** The signing machine booted as the app boots it per request: networks, the one signer, the device's multisigs. */
    private fun machine(multisigs: List<String>): CoreScript {
        val script = CoreScript(SignRequestCore().asBridge()) { null }
        script.dispatch(Wire.json.encodeToString(SignEvent.serializer(), SignEvent.NetworksChanged(listOf(chain))))
        script.dispatch(Wire.json.encodeToString(SignEvent.serializer(), SignEvent.AccountsChanged(listOf(SignAccountRef(address = signer, credential_id = "00")), 0)))
        script.dispatch(Wire.json.encodeToString(SignEvent.serializer(), SignEvent.MultisigsChanged(multisigs)))
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
        val response = script.waitingOperations().first { it.optString("type") == "send_response" }
        val payload = response.getJSONObject("payload")
        assertEquals(-32603, payload.getInt("code"))
        assertEquals("parent_safe_blocked", payload.getString("kind"))
        assertFalse("nothing signed", script.waiting().contains("sign_and_submit"))
    }

    private fun sheetWords(view: SignView) =
        SigningLive.statusBlocks(view, strings).filterIsInstance<SigningBlock.Warning>().single().text

    @Test
    fun the_takeover_of_a_multisig_on_the_list_is_refused_by_this_phones_core() {
        val verdict = JSONObject(
            parentSafeCheck(
                JSONObject().put("method", "eth_sendTransaction").put("params", tx(parent, takeoverData)).put("signer", signer)
                    .put("chain_id", chain).put("known", JSONArray().put(parent)).put("answers", JSONArray()).put("at_submit", false).toString(),
            ),
        )
        assertEquals("refuse", verdict.getString("verdict"))
        assertEquals("member", verdict.getString("reason"))

        val script = machine(listOf(parent))
        arrive(script, tx(parent, takeoverData))
        val view = view(script)
        val blocked = view.blocked!!
        assertEquals(parent, blocked.parent)
        assertEquals(ParentReason.Member, blocked.reason)
        assertEquals("execTransaction", blocked.function)
        assertFalse(view.confirm_gate_open)
        assertFalse("no read for a multisig on the list", script.waiting().contains("read_safe_owner"))
        assertEquals(
            strings.t("componentsUi.signing.parentSafeBlocked", mapOf("function" to "execTransaction", "safe" to ShortAddress.of(parent))),
            sheetWords(view),
        )
        assertAnsweredRefused(script)
    }

    @Test
    fun a_safe_nobody_could_read_is_refused_couldnt_check() {
        val script = machine(listOf(parent))
        arrive(script, tx(unknown, "0xd4d9bdcd" + "cd".repeat(32)))
        val read = script.waitingOperations().first { it.optString("type") == "read_safe_owner" }
        assertEquals(unknown, read.getString("safe"))
        assertTrue(read.getString("data").startsWith("0x2f54bf6e"))
        assertFalse("the confirm is shut while the read is out", view(script).confirm_gate_open)
        val answer = SignShellResult.SafeOwnerRead(read.getString("id"), read.getInt("chain_id"), read.getString("safe"), SafeOwnerAnswer.Unreachable("offline"))
        assertTrue(script.resolve("read_safe_owner", Wire.json.encodeToString(SignShellResult.serializer(), answer)))
        val view = view(script)
        assertEquals(unknown, view.blocked!!.parent)
        assertEquals(ParentReason.Unchecked, view.blocked!!.reason)
        assertEquals(
            strings.t("componentsUi.signing.parentSafeUnchecked", mapOf("function" to "approveHash", "safe" to ShortAddress.of(unknown))),
            sheetWords(view),
        )
        assertAnsweredRefused(script)
    }
}
