package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.RowTrailing
import app.getvela.wallet.feature.settings.core.RegistryBackup
import java.io.File
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 062: the Android transport for `vela_core::registry_backup`, and the
 * settings row it feeds. The walk is the core's and is tested there; pinned
 * here is that requests are carried faithfully (a bare `0x` is an ANSWER, a
 * `null` a silence), that a call is only ever offered with "not backed up",
 * and that the row is a button only while there is something to do.
 */
class RegistryBackupTest {
    private val transcripts = ArrayList<JSONArray>()

    private fun ask(id: String, chain: Int) = JSONObject().put("type", "ask").put(
        "requests",
        JSONArray().put(JSONObject().put("type", "eth_call").put("id", id).put("chain_id", chain).put("to", "0xreg").put("data", "0xdata")),
    ).toString()

    private fun done(state: String, withCall: Boolean = false) = JSONObject()
        .put("type", "done").put("state", state)
        .put("call", if (withCall) JSONObject().put("chain_id", 1).put("to", "0xreg").put("value", "0").put("data", "0xcd438f9b") else JSONObject.NULL)
        .put("unit_id", 10).toString()

    private fun backup(script: List<String>, chain: (Int) -> String? = { "0x" }): RegistryBackup {
        var round = 0
        return RegistryBackup(
            ethCall = { chainId, _, _ -> chain(chainId) },
            step = { _, _, answers, _ -> transcripts += JSONArray(answers); script[round++] },
        )
    }

    @Test
    fun `answers are handed back verbatim - 0x is an answer, null a silence`() = runBlocking {
        val check = backup(listOf(ask("target", 1), ask("groups", 100), done("not_backed_up", withCall = true))) { if (it == 1) "0x" else null }
            .check("0xsafe", "04ab")
        assertEquals(RegistryBackup.State.NotBackedUp, check.state)
        assertEquals(RegistryBackup.Call(1, "0xreg", "0xcd438f9b"), check.call)
        val last = transcripts.last()
        assertEquals("ok" to "0x", last.getJSONObject(0).let { it.getString("outcome") to it.getString("body") })
        assertEquals("failed", last.getJSONObject(1).getString("outcome"))
        assertTrue(last.getJSONObject(1).isNull("body"))
    }

    @Test
    fun `every state maps, and a call without its state is not believed`() = runBlocking {
        for ((wire, state) in listOf(
            "unavailable" to RegistryBackup.State.Unavailable,
            "not_registered" to RegistryBackup.State.NotRegistered,
            "backed_up" to RegistryBackup.State.BackedUp,
            "could_not_check" to RegistryBackup.State.CouldNotCheck,
            "something new" to RegistryBackup.State.CouldNotCheck,
        )) {
            val check = backup(listOf(done(wire))).check("0xsafe", "04ab")
            assertEquals(state, check.state)
            assertNull(check.call)
        }
        // "Not backed up" with nothing to send is not something to show a fee for.
        assertEquals(RegistryBackup.State.CouldNotCheck, backup(listOf(done("not_backed_up"))).check("0xsafe", "04ab").state)
    }

    @Test
    fun `the row - three states, a chevron only when there is something to do, nothing when dark`() {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        val strings: VelaStrings = I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
        val model = SettingsFixtures.buildState(SettingsScreenState.ST1, strings)
        fun row(state: RegistryBackup.State?) =
            SettingsLive.withEthereumBackup(model, state, strings).sections.first().rows.first()

        assertSame(model, SettingsLive.withEthereumBackup(model, RegistryBackup.State.Unavailable, strings))
        assertSame(model, SettingsLive.withEthereumBackup(model, RegistryBackup.State.NotRegistered, strings))

        assertEquals(SettingsLive.ETHEREUM_BACKUP_ROW, row(null).id)
        assertEquals("Back up keys to Ethereum", row(null).title)
        assertEquals("Checking…" to RowTrailing.None, row(null).let { it.subtitle to it.trailing })
        assertEquals("Backed up on Ethereum" to RowTrailing.None, row(RegistryBackup.State.BackedUp).let { it.subtitle to it.trailing })
        assertEquals("Not backed up yet" to RowTrailing.Chevron, row(RegistryBackup.State.NotBackedUp).let { it.subtitle to it.trailing })
        assertEquals("Could not check" to RowTrailing.None, row(RegistryBackup.State.CouldNotCheck).let { it.subtitle to it.trailing })
    }
}
