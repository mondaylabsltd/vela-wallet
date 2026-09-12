package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.settings.CalloutTone
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.settings.SettingsPage
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.SettingsTone
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 023 gates: every settings key resolves through the real engine in all 15
 * locales, the state inventory covers every mock in `design/settings/`, each
 * state builds, and the numbers a reviewer would compare against the PNGs are
 * pinned.
 *
 * Same shape as `ContactsFixturesTest`, which is what keeps the four clients'
 * fixture canons from drifting: they are checked the same way.
 */
class SettingsFixturesTest {

    private val repoRoot = File(
        System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"),
    )

    /** The real engine over the real generated catalogs, same as the contacts test. */
    private fun strings(locale: String): I18nRuntime = I18nRuntime { tag ->
        File(repoRoot, "assets/i18n/$tag.json").readBytes()
    }.apply { initialize(locale) }

    @Test
    fun `every settings key resolves in every shipped locale`() {
        val keys = settingsKeys()
        assertTrue("expected the manifest to be non-trivial", keys.size > 100)
        for (locale in SHIPPED_LOCALES) {
            val s = strings(locale)
            for (key in keys) {
                val value = s.t(key)
                assertTrue("$key did not resolve in $locale", value != key && value.isNotBlank())
            }
        }
    }

    @Test
    fun `the state inventory covers ST1-ST16 and SR1-SR5`() {
        val states = SettingsScreenState.entries
        assertEquals(28, states.size)
        assertEquals(22, states.count { it.name.startsWith("ST") })
        assertEquals(6, states.count { it.name.startsWith("SR") })
    }

    @Test
    fun `every state builds and says which one it is`() {
        val s = strings("zh")
        for (state in SettingsScreenState.entries) {
            assertEquals(state, SettingsFixtures.buildState(state, s).state)
        }
    }

    @Test
    fun `each picker mock opens its own sheet`() {
        val s = strings("zh")
        val pairs = listOf(
            SettingsScreenState.ST2 to SettingsOverlay.Accounts,
            SettingsScreenState.ST3 to SettingsOverlay.SignOut,
            SettingsScreenState.ST4 to SettingsOverlay.Language,
            SettingsScreenState.ST5 to SettingsOverlay.Currency,
            SettingsScreenState.ST6 to SettingsOverlay.NumberFormat,
            SettingsScreenState.ST7 to SettingsOverlay.DateFormat,
            SettingsScreenState.ST8 to SettingsOverlay.TimeFormat,
            SettingsScreenState.ST13B to SettingsOverlay.ClearCaches,
            SettingsScreenState.ST15 to SettingsOverlay.Feedback,
            SettingsScreenState.ST16 to SettingsOverlay.EraseDevice,
        )
        for ((state, overlay) in pairs) {
            assertEquals(state.name, overlay, SettingsFixtures.buildState(state, s).overlay)
        }
    }

    @Test
    fun `each sub-page mock opens its own page`() {
        val s = strings("zh")
        val pairs = listOf(
            SettingsScreenState.ST9 to SettingsPage.Networks,
            SettingsScreenState.ST9B to SettingsPage.NetworkDetail,
            SettingsScreenState.ST10 to SettingsPage.AddNetwork,
            SettingsScreenState.ST11 to SettingsPage.RpcProviders,
            SettingsScreenState.ST12 to SettingsPage.Endpoints,
            SettingsScreenState.ST13 to SettingsPage.Storage,
            SettingsScreenState.ST14 to SettingsPage.About,
        )
        for ((state, page) in pairs) {
            assertEquals(state.name, page, SettingsFixtures.buildState(state, s).page)
        }
    }

    @Test
    fun `the rescue states sit on the wallet tab, not on settings`() {
        val s = strings("zh")
        for (state in SettingsScreenState.entries.filter { it.name.startsWith("SR") }) {
            assertEquals(state.name, "wallet", SettingsFixtures.buildState(state, s).selectedTab)
        }
    }

    @Test
    fun `ST9 lists eight networks with the custom one last and removable`() {
        val model = SettingsFixtures.buildState(SettingsScreenState.ST9, strings("zh"))
        assertEquals(8, model.networks.size)
        val last = model.networks.last()
        assertEquals("X Layer", last.name)
        assertTrue(last.removable)
        assertNull("a custom network has no latency to show", last.badge)
    }

    @Test
    fun `ST10b passes every check and ST10c fails all but EntryPoint`() {
        val s = strings("zh")
        val ok = SettingsFixtures.buildState(SettingsScreenState.ST10B, s).addNetwork
        val bad = SettingsFixtures.buildState(SettingsScreenState.ST10C, s).addNetwork
        assertEquals(listOf(true, true, true, true), ok.checks.map { it.ok })
        assertEquals(listOf(true, false, false, false), bad.checks.map { it.ok })
        // The failing state offers a way forward, not a greyed-out CTA.
        assertNotNull(ok.primary)
        assertNull(bad.primary)
        assertNotNull(bad.secondary)
        assertNotNull(bad.recheck)
    }

    @Test
    fun `ST13 accounts for 2_4 MB over three groups`() {
        val model = SettingsFixtures.buildState(SettingsScreenState.ST13, strings("zh")).storage
        assertEquals("2.4", model.amount)
        assertEquals("MB", model.unit)
        assertTrue(model.summary.contains("216"))
        assertEquals(listOf(4, 3, 1), model.groups.map { it.items.size })
        // Only the cache group offers a clear-them-all action.
        assertEquals(listOf(false, true, false), model.groups.map { it.action != null })
        // User data and connections clear destructively; caches do not.
        assertTrue(model.groups[0].items.all { it.destructive })
        assertTrue(model.groups[1].items.none { it.destructive })
        assertEquals(1.0f, model.segments.sumOf { it.fraction.toDouble() }.toFloat(), 0.0001f)
    }

    @Test
    fun `SR2 is offline with a way to fix it and SR2b is restored and done`() {
        val s = strings("zh")
        val failing = SettingsFixtures.buildState(SettingsScreenState.SR2, s).rpcFix
        val restored = SettingsFixtures.buildState(SettingsScreenState.SR2B, s).rpcFix
        assertEquals(SettingsTone.Error, failing.badge.tone)
        assertEquals(CalloutTone.Warning, failing.callout.tone)
        assertEquals(4, failing.providers.size)
        assertEquals(SettingsTone.Ok, restored.badge.tone)
        assertEquals(CalloutTone.Success, restored.callout.tone)
        // Nothing left to go and get once it works.
        assertTrue(restored.providers.isEmpty())
        assertNull(restored.report)
    }

    @Test
    fun `SR3 tells rate-limiting from a dead RPC`() {
        val detail = SettingsFixtures.buildState(SettingsScreenState.SR3, strings("zh")).balanceDetail
        // Quiet, resolves itself, no button.
        assertEquals(SettingsTone.Neutral, detail.pending[0].tone)
        assertNull(detail.pending[0].action)
        // Loud, does not resolve itself, offers 立即重试.
        assertEquals(SettingsTone.Error, detail.pending[1].tone)
        assertNotNull(detail.pending[1].action)
    }

    @Test
    fun `ST3b adds the pending-upload warning that ST3 has no reason to show`() {
        val s = strings("zh")
        assertNull(SettingsFixtures.buildState(SettingsScreenState.ST3, s).signOutSheet.callout)
        assertEquals(
            CalloutTone.Warning,
            SettingsFixtures.buildState(SettingsScreenState.ST3B, s).signOutSheet.callout?.tone,
        )
    }

    @Test
    fun `every destructive confirm is red and every reversible one is not`() {
        val model = SettingsFixtures.buildState(SettingsScreenState.ST1, strings("zh"))
        assertTrue(model.signOutSheet.danger)
        assertTrue(model.eraseSheet.danger)
        assertTrue("clearing a cache is reversible", !model.clearCachesSheet.danger)
    }

    @Test
    fun `language endonyms read the same in every locale — they are data`() {
        val zh = SettingsFixtures.buildState(SettingsScreenState.ST4, strings("zh"))
        val en = SettingsFixtures.buildState(SettingsScreenState.ST4, strings("en"))
        // Row 0 is 跟随系统 and IS translated; the rest are endonyms.
        assertEquals(
            zh.languageSheet.rows.drop(1).map { it.label },
            en.languageSheet.rows.drop(1).map { it.label },
        )
        assertEquals(15, SettingsFixtures.LOCALE_ENDONYMS.size)
    }

    @Test
    fun `the signed-in identity replaces only the active account`() {
        val model = SettingsFixtures.buildState(SettingsScreenState.ST2, strings("zh"))
            .withIdentity("kimik3", "0xABC", "0xABC…def")
        assertEquals("kimik3", model.account.name)
        assertEquals("kimik3", model.accountsSheet.rows[0].name)
        // The other two stay fixtures: the core exposes no account list yet, and
        // inventing one would be the screen lying about how many wallets exist.
        assertEquals("旅行基金", model.accountsSheet.rows[1].name)
    }

    private companion object {
        val SHIPPED_LOCALES = listOf(
            "en", "zh", "zh-TW", "zh-HK", "ja", "ko", "vi", "id",
            "tr", "es-MX", "pt-BR", "fr", "de", "ru", "it",
        )

        /** Every constant on [I18nKeys.SettingsUi], read reflectively. */
        fun settingsKeys(): List<String> =
            I18nKeys.SettingsUi::class.java.declaredFields
                .filter { it.type == String::class.java }
                .map {
                    it.isAccessible = true
                    it.get(I18nKeys.SettingsUi) as String
                }
    }
}
