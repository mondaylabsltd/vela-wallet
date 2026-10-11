package app.getvela.wallet

import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Device walk 2026-10-11, item 9: the feedback sheet's facts read 「无法连接的
 * RPC  1625」 — which reads as a count of RPCs. 1625 was a chain id, of a
 * network the wallet does not have, with no name on this device.
 *
 * The label is "Unreachable networks" now (the core's corpus), the core no
 * longer lists a chain the wallet has no endpoint for, and this shell names
 * each network it is told is out of reach: by its NAME, and one it has no
 * name for as the corpus's "Chain 1625" — never a bare number.
 */
class FeedbackNetworksTest {

    private fun strings(tag: String): VelaStrings {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        return I18nRuntime { name -> File(root, "assets/i18n/$name.json").readBytes() }.apply { initialize(tag) }
    }

    private val names = mapOf(1 to "Ethereum", 100 to "Gnosis", 5042002 to "Arc Testnet", 7 to "  ")

    @Test
    fun `each network is said by its name, and one without a name as Chain N`() {
        val zh = strings("zh")
        assertEquals(listOf("Gnosis", "链 1625", "Arc Testnet"), SettingsLive.unreachableNetworks(listOf(100, 1625, 5042002), names, zh))
        assertEquals(listOf("Chain 1625"), SettingsLive.unreachableNetworks(listOf(1625), names, strings("en")))
        // A name that is only spaces is no name.
        assertEquals(listOf("链 7"), SettingsLive.unreachableNetworks(listOf(7), names, zh))
        assertEquals(emptyList<String>(), SettingsLive.unreachableNetworks(emptyList(), names, zh))
    }

    /** The line as the feedback sheet prints it, and as the report carries it: the walk's own case. */
    @Test
    fun `the feedback line never prints a bare chain id`() {
        val zh = strings("zh")
        fun line(chains: List<Int>): String {
            val facts = BugReport.DeviceFacts(
                version = "0.9.7", commit = "d3b80df", platform = "Android 13", language = "zh",
                unreachable = SettingsLive.unreachableNetworks(chains, names, zh),
                failures = emptyList(),
            )
            val model = SettingsLive.withFeedback(SettingsFixtures.buildState(SettingsScreenState.ST1, zh), facts, zh)
            return model.feedback.previewLines.single { it.startsWith(zh.t("componentsUi.bugReport.previewRpc")) }
        }
        assertEquals("连不上的网络: 链 1625", line(listOf(1625)))
        assertEquals("连不上的网络: Gnosis, 链 1625", line(listOf(100, 1625)))
        assertEquals("连不上的网络: ${zh.t("componentsUi.bugReport.previewNone")}", line(emptyList()))
        // Whatever the ids: after the label, no item of the list is a number alone.
        for (chains in listOf(listOf(1625), listOf(1, 1625, 100), listOf(999999, 31337))) {
            val items = line(chains).substringAfter(": ").split(", ")
            assertEquals(chains.size, items.size)
            assertFalse("a bare id in ${line(chains)}", items.any { item -> item.all(Char::isDigit) })
            assertTrue(items.all { it.isNotBlank() })
        }
    }
}
