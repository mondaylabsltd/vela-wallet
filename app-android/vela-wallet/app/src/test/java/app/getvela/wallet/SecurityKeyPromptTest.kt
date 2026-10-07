package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.flow.promptCopy
import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Issue #450: a security key this device could not use said "Biometric
 * authentication is not available on this device". The core now flags the
 * route (`security_key`), and the sheet talks about the key; a passkey on this
 * device keeps the biometrics sheet.
 */
class SecurityKeyPromptTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    @Test
    fun `a security key that cannot run is about the key, not biometrics`() {
        for (type in listOf("not_supported_login", "not_supported_create")) {
            val key = promptCopy(PromptKind(type, null, securityKey = true), strings)
            assertEquals(strings.t(I18nKeys.Flow.KEY_UNAVAILABLE_TITLE), key.title)
            assertEquals(strings.t(I18nKeys.Flow.KEY_UNAVAILABLE_BODY), key.message)
            assertFalse(key.message.contains("Biometric"))
        }
        val device = promptCopy(PromptKind("not_supported_login", null), strings)
        assertEquals(strings.t(I18nKeys.Login.ALERT_NOT_SUPPORTED_BODY), device.message)
    }

    @Test
    fun `the flag is read from the core's JSON, and absent is false`() {
        assertTrue(PromptKind.from(JSONObject("""{"type":"not_supported_login","security_key":true}""")).securityKey)
        assertFalse(PromptKind.from(JSONObject("""{"type":"not_supported_login"}""")).securityKey)
    }
}
