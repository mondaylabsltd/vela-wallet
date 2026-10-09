package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.flow.promptCopy
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Device pass 2026-10-09: a failed sign-in on Android said 「请确保您的设备已设置
 * Face ID、Touch ID 或指纹」 — an iPhone's words. The sheet keeps the
 * platform's own words and puts Android's own sentence under them
 * (`onboarding.login.alertSignInFailedBodyAndroid`: a screen lock or a
 * fingerprint), in every language the app ships — not the button label
 * (`onboarding.common.openBiometricSettings`) it borrowed for a day.
 */
class SignInFailedCopyTest {

    private val root: File by lazy {
        File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"))
    }

    private fun strings(tag: String): VelaStrings =
        I18nRuntime { file -> File(root, "assets/i18n/$file.json").readBytes() }.apply { initialize(tag) }

    private val tags: List<String> by lazy {
        File(root, "assets/i18n").listFiles { file -> file.extension == "json" }!!.map { it.nameWithoutExtension }.sorted()
    }

    /** What the sheet says under the platform's words, in [strings]' language. */
    private fun advice(strings: VelaStrings) =
        strings.t(I18nKeys.Login.ALERT_SIGN_IN_FAILED_BODY_ANDROID, mapOf("message" to "")).trimStart()

    @Test
    fun `the sign-in failure names no Apple biometric in any language`() {
        assertTrue("the shipped languages are there to read: $tags", tags.containsAll(listOf("en", "zh")))
        tags.forEach { tag ->
            val strings = strings(tag)
            val body = promptCopy(PromptKind("sign_in_failed", "boom"), strings).message
            assertFalse("$tag: $body", body.contains("Face ID") || body.contains("Touch ID"))
            assertEquals("$tag: the platform's words, then the advice", "boom\n\n" + advice(strings), body)
        }
    }

    /** A sentence that ends as one — not the settings button's label it borrowed. */
    @Test
    fun `the advice is Android's own sentence, not a button label`() {
        tags.forEach { tag ->
            val strings = strings(tag)
            val advice = advice(strings)
            assertNotEquals("$tag: the button's label", strings.t(I18nKeys.Flow.OPEN_BIOMETRIC_SETTINGS), advice)
            assertTrue("$tag ends without a full stop: $advice", advice.last() in ".。")
            assertFalse("$tag: no placeholder left: $advice", advice.contains("{{"))
        }
    }

    @Test
    fun `the Chinese and English sheets`() {
        assertEquals(
            "boom\n\n请确保这台设备已设置屏幕锁或指纹，然后重试。",
            promptCopy(PromptKind("sign_in_failed", "boom"), strings("zh")).message,
        )
        assertEquals(
            "boom\n\nMake sure this device has a screen lock or fingerprint set up and try again.",
            promptCopy(PromptKind("sign_in_failed", "boom"), strings("en")).message,
        )
    }

    @Test
    fun `with no words from the platform, the advice alone`() {
        val strings = strings("en")
        val alone = "Make sure this device has a screen lock or fingerprint set up and try again."
        assertEquals(alone, promptCopy(PromptKind("sign_in_failed", null), strings).message)
        assertEquals(alone, promptCopy(PromptKind("sign_in_failed", " "), strings).message)
    }
}
