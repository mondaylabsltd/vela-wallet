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
 * Issue #446: a sign-in with a phone whose link failed told the person to set
 * up Face ID. The core now flags a failed link (`phone_link`), and the sheet
 * says to scan again instead; an authenticator's own failure keeps its sheet.
 */
class PhoneLinkPromptTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    @Test
    fun `a failed phone link says to scan again, not to set up biometrics`() {
        val link = promptCopy(PromptKind("sign_in_failed", "caBLE transport: closed", phoneLink = true), strings)
        assertEquals(strings.t(I18nKeys.Flow.PHONE_LINK_FAILED), link.message)
        assertFalse(link.message.contains("Face ID"))

        val create = promptCopy(PromptKind("create_failed", "caBLE transport: closed", phoneLink = true), strings)
        assertEquals(strings.t(I18nKeys.Flow.PHONE_LINK_FAILED), create.message)

        val other = promptCopy(PromptKind("sign_in_failed", "boom"), strings)
        assertTrue(other.message.contains("boom"))
    }

    @Test
    fun `the flag is read from the core's JSON, and absent is false`() {
        assertTrue(PromptKind.from(JSONObject("""{"type":"sign_in_failed","detail":"x","phone_link":true}""")).phoneLink)
        assertFalse(PromptKind.from(JSONObject("""{"type":"sign_in_failed","detail":"x"}""")).phoneLink)
    }
}
