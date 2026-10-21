package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.agent.AgentConfig
import app.getvela.wallet.feature.agent.AgentOutcome
import app.getvela.wallet.feature.agent.AgentStore
import app.getvela.wallet.feature.agent.AgentWords
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsPage
import app.getvela.wallet.feature.settings.SettingsScreenState
import java.io.File
import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.intentCatalog

/**
 * The AI agent's phone half (spec 104 phase 7): what this app keeps of it —
 * the settings, the requests and calls it draws, the words it draws them in.
 * The agent itself (runner, transfers, tools) is `vela-agent`'s and tested
 * there; the road through this app's controllers is the live run's.
 */
class AgentTest {

    private val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")

    private fun strings(tag: String): VelaStrings =
        I18nRuntime { name -> File(root, "assets/i18n/$name.json").readBytes() }.apply { initialize(tag) }

    @Test
    fun theSettingsAreStoredAsTheWebAndTheDesktopStoreThem() {
        val config = AgentConfig(enabled = true, url = "wss://bridge.example.com/connect", token = "t0k")
        assertEquals(config, AgentConfig.from(config.json()))
        // Off unless the person turns it on; anything unreadable reads as off.
        assertEquals(AgentConfig(), AgentConfig.from(null))
        assertEquals(AgentConfig(), AgentConfig.from("not json"))
        assertFalse(AgentConfig().ready)
    }

    @Test
    fun aBridgeIsEncryptedUnlessItIsThisMachine() {
        assertTrue(AgentConfig(true, "wss://bridge.example.com/connect", "t").ready)
        assertFalse(AgentConfig(true, "ws://bridge.example.com/connect", "t").ready)
        assertTrue(AgentConfig(true, "ws://127.0.0.1:8787/connect", "t").ready)
        assertFalse("a token is needed", AgentConfig(true, "wss://bridge.example.com/connect", " ").ready)
        assertFalse("switched off", AgentConfig(false, "wss://bridge.example.com/connect", "t").ready)
        assertEquals("insecure", AgentConfig(true, "ws://bridge.example.com/connect", "t").urlProblem)
        assertNull("nothing entered is no problem yet", AgentConfig().urlProblem)
        assertTrue(AgentConfig(url = "wss://bridge.example.com/connect").endpoint!!.startsWith("https://bridge.example.com/"))
    }

    @Test
    fun theRequestsAndCallsAreReadAsTheCoreWritesThem() {
        val requests = AgentStore.requests(
            """[{"id":7,"action":{"type":"delete_group","id":"g1"},"risk":"manage","since":1.0}]""",
        )
        assertEquals(1, requests.size)
        assertEquals(7uL, requests[0].id)
        assertEquals("delete_group", requests[0].kind)
        assertEquals("g1", requests[0].action.optString("id"))

        val calls = AgentStore.calls(
            """[{"id":2,"action":"delete_group","stage":null,"outcome":{"status":"rejected","reason":{"code":"denied","detail":"no"}}},
                {"id":1,"action":"show","stage":"observing","outcome":null}]""",
        )
        assertEquals(listOf("delete_group", "show"), calls.map { it.kind })
        assertEquals("denied", calls[0].code)
        assertNull(calls[0].stage)
        assertEquals("observing", calls[1].stage)
        assertNull(calls[1].status)

        assertTrue(AgentOutcome.from("""{"status":"completed"}""").done)
        assertEquals("denied", AgentOutcome.from("""{"status":"rejected","reason":{"code":"denied","detail":"no"}}""").code)
        assertFalse(AgentOutcome.from("garbled").done)
    }

    @Test
    fun everyActionAnAgentMayAskForHasWords() {
        val catalog = JSONArray(intentCatalog())
        assertTrue(catalog.length() > 0)
        for (language in listOf("en", "zh")) {
            val strings = strings(language)
            for (index in 0 until catalog.length()) {
                val entry = catalog.getJSONObject(index)
                if (entry.optBoolean("person_only")) continue
                val kind = entry.getString("action")
                assertNotEquals("$language: $kind has no words", kind, AgentWords.action(kind, strings))
            }
        }
    }

    @Test
    fun settingsHasARowForTheAgentBeforeTheStorage() {
        val model = SettingsFixtures.buildState(SettingsScreenState.ST1, strings("en"))
        val ids = model.sections.flatMap { section -> section.rows.map { it.id } }
        val row = ids.indexOf(SettingsFixtures.AGENT_ROW)
        assertTrue("no AI agent row", row >= 0)
        assertEquals("storage", ids[row + 1])
        assertEquals("AI agent", model.agentTitle)
        assertEquals(SettingsPage.Home, SettingsPage.Agent.back)
    }
}
