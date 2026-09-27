package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.settings.CommunityLinks
import app.getvela.wallet.feature.settings.RowTrailing
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsScreenState
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Settings → Community (founder, 2026-09-27): the exact official links, pinned
 * so the four shells cannot drift — and the group sits directly above the
 * last one (About / Send feedback), every row leaving the app.
 */
class CommunityLinksTest {

    @Test
    fun `the three official links, exactly`() {
        assertEquals(
            listOf(
                Triple("X (Twitter)", "@realvelawallet", "https://x.com/realvelawallet"),
                Triple("Telegram", "@velawallet", "https://t.me/velawallet"),
                Triple("Discord", "discord.gg/23gWrtaYSa", "https://discord.gg/23gWrtaYSa"),
            ),
            CommunityLinks.ALL.map { Triple(it.title, it.handle, it.url) },
        )
        assertEquals("https://discord.gg/23gWrtaYSa", CommunityLinks.urlFor("community-discord"))
        assertNull(CommunityLinks.urlFor("about"))
    }

    @Test
    fun `the group sits above about and feedback, and its rows leave the app`() {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        val strings = I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
        val sections = SettingsFixtures.buildState(SettingsScreenState.ST1, strings).sections
        val community = sections[sections.size - 2]
        assertEquals("Community", community.label)
        assertEquals(CommunityLinks.ALL.map { it.id }, community.rows.map { it.id })
        assertEquals(CommunityLinks.ALL.map { it.handle }, community.rows.map { it.subtitle })
        community.rows.forEach { assertEquals(RowTrailing.External, it.trailing) }
        assertEquals(listOf("about", "feedback"), sections.last().rows.map { it.id })
    }
}
