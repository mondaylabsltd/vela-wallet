package app.getvela.wallet

import app.getvela.wallet.feature.multisig.OtherSpace
import app.getvela.wallet.feature.settings.core.NetworkSpace
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * r13-1 (docs/multisig-machines.md §15.4, §17): where a multisig lives when
 * none of its networks is in the space in front is the core's rule
 * (`network_space::lives_elsewhere`, uniffi `network_lives_elsewhere`) — the
 * shared vector (`rust/crates/vela-core/tests/vectors/multisig/other-space.json`)
 * replayed through the one call the home makes ([OtherSpace.livesElsewhere]).
 * The shell keeps no copy of the rule.
 */
class OtherSpaceVectorsTest {

    private val vector = File(
        System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"),
        "rust/crates/vela-core/tests/vectors/multisig/other-space.json",
    )

    private fun ints(array: JSONArray): List<Int> = (0 until array.length()).map { array.getInt(it) }

    @Test
    fun the_other_space_vectors_hold() {
        val cases = JSONObject(vector.readText()).getJSONArray("cases")
        assertTrue(cases.length() >= 6)
        for (c in 0 until cases.length()) {
            val case = cases.getJSONObject(c)
            val name = case.getString("name")
            val got = OtherSpace.livesElsewhere(
                chainIds = ints(case.getJSONArray("chain_ids")),
                here = ints(case.getJSONArray("here")),
                other = ints(case.getJSONArray("other")),
                front = NetworkSpace.of(case.getString("front")),
            )
            val want = case.optJSONObject("expect")
            if (want == null) {
                assertNull(name, got)
                continue
            }
            requireNotNull(got) { "$name: the core said nothing" }
            assertEquals(name, ints(want.getJSONArray("chain_ids")), got.chain_ids)
            assertEquals(name, NetworkSpace.of(want.getString("space")), got.target)
            assertEquals(name, want.getString("line_key"), got.line_key)
            assertEquals(name, want.getString("heading_key"), got.heading_key)
            assertEquals(name, want.getString("switch_key"), got.switch_key)
        }
    }
}
