package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.flow.promptCopy
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Device pass 2026-10-09: a failed sign-in on Android said 「请确保您的设备已设置
 * Face ID、Touch ID 或指纹」 — an iPhone's words. The sheet keeps the
 * platform's own words and puts Android's advice under them (the corpus's
 * `onboarding.common.openBiometricSettings`), in every language the app ships.
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

    @Test
    fun `the sign-in failure names no Apple biometric in any language`() {
        assertTrue("the shipped languages are there to read: $tags", tags.containsAll(listOf("en", "zh")))
        tags.forEach { tag ->
            val strings = strings(tag)
            val body = promptCopy(PromptKind("sign_in_failed", "boom"), strings).message
            assertFalse("$tag: $body", body.contains("Face ID") || body.contains("Touch ID"))
            assertEquals("$tag: the platform's words, then the advice", "boom\n\n" + strings.t(I18nKeys.Flow.OPEN_BIOMETRIC_SETTINGS), body)
        }
    }

    @Test
    fun `the Chinese sheet is the platform's words and the system-settings advice`() {
        val body = promptCopy(PromptKind("sign_in_failed", "boom"), strings("zh")).message
        assertEquals("boom\n\n前往系统设置开启生物识别", body)
    }

    @Test
    fun `with no words from the platform, the advice alone`() {
        val strings = strings("en")
        assertEquals(strings.t(I18nKeys.Flow.OPEN_BIOMETRIC_SETTINGS), promptCopy(PromptKind("sign_in_failed", null), strings).message)
        assertEquals(strings.t(I18nKeys.Flow.OPEN_BIOMETRIC_SETTINGS), promptCopy(PromptKind("sign_in_failed", " "), strings).message)
    }
}
