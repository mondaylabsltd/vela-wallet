package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.flow.ANDROID_UNLOCK
import app.getvela.wallet.feature.onboarding.flow.KeyChooser
import app.getvela.wallet.feature.onboarding.flow.methodCopy
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The key-method rows' words (087 F01, F02): the core decides them
 * (`keyMethodWords`) for the create picker, the sign-in sheet and the QR card.
 */
class KeyMethodCopyTest {

    private fun strings(tag: String): VelaStrings {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        return I18nRuntime { lang -> File(root, "assets/i18n/$lang.json").readBytes() }
            .apply { initialize(tag) }
    }

    /**
     * 087 F01: 这台设备 on an Android phone read "Touch ID 或 Windows Hello" — a
     * Mac's and a PC's authenticators. It names the family Android unlocks a
     * passkey with instead, and never another platform's product.
     */
    @Test
    fun `this device names what unlocks an Android phone`() {
        assertEquals("other", ANDROID_UNLOCK)
        for (tag in listOf("zh", "en")) {
            val strings = strings(tag)
            for (chooser in KeyChooser.entries) {
                val (title, line) = methodCopy(KeyMethod.Platform, chooser, strings)
                assertEquals(strings.t("onboarding.create.methodPlatformTitle"), title)
                assertEquals(strings.t("onboarding.create.methodPlatformBody"), line)
                assertFalse(line, line.contains("Windows Hello"))
                assertFalse(line, line.contains("Touch ID"))
            }
        }
        assertEquals("指纹、面容或屏幕锁", methodCopy(KeyMethod.Platform, KeyChooser.SignIn, strings("zh")).second)
    }

    /** 087 F02: the sign-in sheet's 手机或平板 scans; only the create picker creates. */
    @Test
    fun `the sign-in phone row never says create`() {
        val strings = strings("zh")
        val signIn = methodCopy(KeyMethod.Hybrid, KeyChooser.SignIn, strings).second
        val create = methodCopy(KeyMethod.Hybrid, KeyChooser.Create, strings).second
        assertEquals(strings.t("explore.scan"), signIn)
        assertEquals(strings.t("onboarding.create.methodHybridBody"), create)
        assertFalse(signIn, signIn.contains("创建"))
        assertNotEquals(create, signIn)
    }

    /**
     * Every row resolves to words in both choosers — never a bare key — and
     * there are three places, each its own words (spec 102: no fourth row).
     */
    @Test
    fun `every row has words in both choosers`() {
        val strings = strings("zh")
        for (method in KeyMethod.entries) {
            for (chooser in KeyChooser.entries) {
                val (title, line) = methodCopy(method, chooser, strings)
                for (said in listOf(title, line)) {
                    assertTrue("$method $chooser", said.isNotBlank())
                    assertFalse("$method $chooser drew a key: $said", said.startsWith("onboarding.") || said.startsWith("componentsUi."))
                }
            }
        }
        assertEquals(3, KeyMethod.entries.map { methodCopy(it, KeyChooser.SignIn, strings).first }.toSet().size)
        // The fourth method's words are gone from the core: nothing can draw it.
        assertNull(uniffi.vela_core_uniffi.keyMethodWords("trusted_signer", "sign_in", "other"))
    }

    /**
     * Spec 102 D6: "Use a trusted signing page" has words of its own — the
     * venue's, not a key place's; it lists Vela's official page too, so it is
     * never "my own" — and so do the two venues of "Where you review and
     * sign", which read alike (D-19), in every chooser that draws them.
     */
    @Test
    fun `the signing page entry and the venues have their own words`() {
        val strings = strings("en")
        val (title, line) = app.getvela.wallet.feature.onboarding.flow.OwnPageModel.entry(strings)
        assertEquals("Use a trusted signing page", title)
        // Issue #475: one line under the title, on a phone, in every language.
        assertEquals("Advanced: Vela's page or your own", line)
        assertEquals("Review and sign in Vela", strings.t(uniffi.vela_core_uniffi.venueWords("in_vela")!!.titleKey))
        assertEquals("Review and sign on a trusted signing page", strings.t(uniffi.vela_core_uniffi.venueWords("page")!!.titleKey))
        val zh = strings("zh")
        assertEquals("使用可信签名页", app.getvela.wallet.feature.onboarding.flow.OwnPageModel.entry(zh).first)
        assertEquals("高级：Vela 官方页或自建页", app.getvela.wallet.feature.onboarding.flow.OwnPageModel.entry(zh).second)
        assertEquals("在 Vela 里预览并签名", zh.t(uniffi.vela_core_uniffi.venueWords("in_vela")!!.titleKey))
        assertEquals("在可信签名页预览并签名", zh.t(uniffi.vela_core_uniffi.venueWords("page")!!.titleKey))
        // "My own signing page" is nowhere (D6).
        for (s in listOf(strings, zh)) {
            assertFalse(app.getvela.wallet.feature.onboarding.flow.OwnPageModel.entry(s).first.contains("own", ignoreCase = true))
            assertFalse(app.getvela.wallet.feature.onboarding.flow.OwnPageModel.entry(s).first.contains("自己"))
        }
    }
}
