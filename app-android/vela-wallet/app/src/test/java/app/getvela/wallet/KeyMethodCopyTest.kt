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
     * the Trusted Signer keeps the signing sheet's own two sentences (spec 075).
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
        assertEquals(
            strings.t("componentsUi.signing.trustedSignerBody"),
            methodCopy(KeyMethod.TrustedSigner, KeyChooser.SignIn, strings).second,
        )
        assertEquals(4, KeyMethod.entries.map { methodCopy(it, KeyChooser.SignIn, strings).first }.toSet().size)
    }
}
