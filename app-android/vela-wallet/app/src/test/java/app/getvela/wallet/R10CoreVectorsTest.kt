package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.str
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.BalanceDashboardCore
import uniffi.vela_core_uniffi.incomingNextScanMs
import uniffi.vela_core_uniffi.incomingScanPlan
import uniffi.vela_core_uniffi.passkeyErrorWords

/**
 * Round 10's core rules (docs/multisig-machines.md §9), replayed on this
 * shell's binding from the shared vectors — the same files the core and
 * every other shell replay: names, the balance stamp, the incoming scan's
 * plan and cadence, the passkey error's words.
 */
class R10CoreVectorsTest {

    private val vectors = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"), "rust/crates/vela-core/tests/vectors")

    private fun read(path: String) = JSONObject(File(vectors, path).readText())

    private fun JSONObject.toMap(): Map<String, Any?> = keys().asSequence().associateWith { k -> opt(k).let { if (it == JSONObject.NULL) null else it } }

    /** §9.1: one name per address — the link door's `names.label`. */
    @Test
    fun names_vectors_hold_through_the_link_door() {
        val cases = read("multisig/names.json").getJSONArray("cases")
        assertTrue(cases.length() > 0)
        for (i in 0 until cases.length()) {
            val case = cases.getJSONObject(i)
            val got = MsRules.call("names.label", case.getJSONObject("args").toMap()).str
            assertEquals(case.getString("name"), case.getString("expect"), got)
        }
    }

    /** §9.7: the passkey error's words, never the system's English. */
    @Test
    fun passkey_error_vectors_hold() {
        val cases = read("rules/passkey-errors.json").getJSONArray("cases")
        for (i in 0 until cases.length()) {
            val case = cases.getJSONObject(i)
            val code = case.opt("code").let { if (it == JSONObject.NULL || it == null) null else (it as Number).toInt() }
            val got = passkeyErrorWords(case.getString("platform"), case.getString("domain"), code, case.getString("message"))
            val expect = case.getJSONObject("expect")
            assertEquals(case.getString("name"), expect.getString("key"), got.key)
            assertEquals(case.getString("name"), expect.getString("kind"), got.kind.name.lowercase())
        }
    }

    /** §9.4: the scan's networks and its cadence are the core's. */
    @Test
    fun incoming_scan_plan_and_cadence_vectors_hold() {
        val doc = read("incoming-scan.json")
        val plans = doc.getJSONArray("plans")
        for (i in 0 until plans.length()) {
            val case = plans.getJSONObject(i)
            val got = JSONObject(incomingScanPlan(case.getJSONObject("facts").toString()))
            val expect = case.getJSONObject("expect")
            assertEquals(case.getString("name"), expect.getJSONArray("scan_chain_ids").toString(), got.getJSONArray("scan_chain_ids").toString())
        }
        val cadence = doc.getJSONArray("cadence")
        for (i in 0 until cadence.length()) {
            val case = cadence.getJSONObject(i)
            assertEquals(
                case.getString("name"),
                case.getLong("expect_ms"),
                incomingNextScanMs(case.getBoolean("visible"), case.getDouble("since_interaction_ms")).toLong(),
            )
        }
    }

    /**
     * §9.3: the "Updated" stamp moves only when the networks the account
     * lives on answered — the machine's steps, driven through this binding
     * (an event is dispatched; an answer resolves the matching operation).
     */
    @Test
    fun balance_stamp_vectors_hold() {
        val cases = read("balance-stamp.json").getJSONArray("cases")
        assertTrue(cases.length() >= 8)
        for (c in 0 until cases.length()) {
            val case = cases.getJSONObject(c)
            val name = case.getString("name")
            val address = case.getString("address")
            val script = CoreScript(BalanceDashboardCore().asBridge()) { null }
            script.dispatch(JSONObject().put("type", "account_changed").put("address", address).toString())
            val steps: JSONArray = case.getJSONArray("steps")
            for (i in 0 until steps.length()) {
                val step = steps.getJSONObject(i)
                val event = step.optJSONObject("event")
                if (event != null) {
                    script.dispatch(event.toString())
                } else {
                    val answer = step.getJSONObject("answer")
                    val op = if (answer.getString("type") == "fetch_settled") "fetch_tokens" else "read_balance_cache"
                    assertTrue("$name, step $i: the core asked for $op (waiting ${script.waiting()})", script.resolve(op, answer.toString()))
                }
                if (step.has("stamp")) {
                    val view = JSONObject(script.viewJson())
                    val got = view.opt("last_refreshed_at_ms").let { if (it == null || it == JSONObject.NULL) null else (it as Number).toDouble() }
                    val want = step.opt("stamp").let { if (it == JSONObject.NULL) null else (it as Number).toDouble() }
                    assertEquals("$name, step $i", want, got)
                }
            }
        }
    }
}
