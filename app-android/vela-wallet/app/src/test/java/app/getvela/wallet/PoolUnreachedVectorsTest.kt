package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.wallet.core.BalanceEvent
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.BalanceDashboardCore

/**
 * r12-2 / r12-3 (docs/multisig-machines.md §13.2): a network that does not
 * answer is never a confident zero — the shared vectors
 * (`rust/crates/vela-core/tests/vectors/balance-pool-unreached.json`, the
 * core's `the_pool_unreached_vectors_hold`) replayed on this shell's binding.
 *
 * Every event goes through THIS shell's wire first ([BalanceEvent], encoded
 * the way `WalletController` sends it), so a `pool_unreached` the Kotlin
 * side spells differently from the core fails here, not on a phone.
 */
class PoolUnreachedVectorsTest {

    private val vectors = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"), "rust/crates/vela-core/tests/vectors")

    /** The vector's event, decoded as this shell's [BalanceEvent] and encoded back — what `balanceHost.dispatch` sends. */
    private fun throughOurWire(event: JSONObject): String {
        val ours = Wire.json.decodeFromString(BalanceEvent.serializer(), event.toString())
        return Wire.json.encodeToString(BalanceEvent.serializer(), ours)
    }

    @Test
    fun the_pool_unreached_event_is_spelled_as_the_core_reads_it() {
        val sent = JSONObject(Wire.json.encodeToString(BalanceEvent.serializer(), BalanceEvent.PoolUnreached(listOf(130, 1868))))
        assertEquals("pool_unreached", sent.getString("type"))
        assertEquals("[130,1868]", sent.getJSONArray("chain_ids").toString())
    }

    @Test
    fun the_pool_unreached_vectors_hold() {
        val cases = JSONObject(File(vectors, "balance-pool-unreached.json").readText()).getJSONArray("cases")
        assertTrue(cases.length() >= 7)
        for (c in 0 until cases.length()) {
            val case = cases.getJSONObject(c)
            val name = case.getString("name")
            val address = case.getString("address")
            val script = CoreScript(BalanceDashboardCore().asBridge()) { null }
            script.dispatch(throughOurWire(JSONObject().put("type", "account_changed").put("address", address)))
            val steps: JSONArray = case.getJSONArray("steps")
            for (i in 0 until steps.length()) {
                val step = steps.getJSONObject(i)
                val event = step.optJSONObject("event")
                if (event != null) {
                    script.dispatch(throughOurWire(event))
                } else {
                    val answer = step.getJSONObject("answer")
                    val op = if (answer.getString("type") == "fetch_settled") "fetch_tokens" else "read_balance_cache"
                    assertTrue("$name, step $i: the core asked for $op (waiting ${script.waiting()})", script.resolve(op, answer.toString()))
                }
                val expect = step.optJSONObject("expect") ?: continue
                if (expect.has("fetch")) {
                    val out = script.waitingOperations().firstOrNull { it.optString("type") == "fetch_tokens" }
                    val want = expect.optJSONObject("fetch")?.getBoolean("force")
                    assertEquals("$name, step $i: fetch", want, out?.getBoolean("force"))
                }
                val view = JSONObject(script.viewJson())
                if (expect.has("unreachable")) {
                    val networks = view.optJSONArray("unreachable_networks") ?: JSONArray()
                    val got = (0 until networks.length()).map { networks.getJSONObject(it).getLong("chain_id") }
                    val want = expect.getJSONArray("unreachable").let { ids -> (0 until ids.length()).map { ids.getLong(it) } }
                    assertEquals("$name, step $i: unreachable", want, got)
                }
                if (expect.has("live")) {
                    val live = view.opt("live_key").let { it != null && it != JSONObject.NULL }
                    assertEquals("$name, step $i: live", expect.getBoolean("live"), live)
                }
            }
        }
    }
}
