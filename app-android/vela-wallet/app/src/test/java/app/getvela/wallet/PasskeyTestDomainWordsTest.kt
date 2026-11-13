package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.net.ProductionWriteGuard
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.OnboardingExecutor
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.PasskeyErrorKind
import uniffi.vela_core_uniffi.passkeyErrorWordsOn
import java.io.File

/**
 * R21-3 (§37): a TEST build asks for its own relying party (`localhost` on an
 * emulator, its `TEST_RP_DOMAIN` on a device), which no store app's
 * association covers. Its `[50152] RP ID cannot be validated` is then said as
 * the test domain not being set up — never the release words, which send a
 * tester after their connection. The flag is the one this build hands
 * `relying_party_id` ([ProductionWriteGuard.testBuild]); the rule is the core's.
 */
class PasskeyTestDomainWordsTest {
    private val root: String = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
    private val strings = I18nRuntime(
        readOnDemand = { namespace, tag -> File(root, "assets/" + I18nCatalogs.assetPath(tag, namespace)).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }
    private val association = "[50152] RP ID cannot be validated."
    private val testKey = "onboarding.common.passkeyError.testDomainNotAssociated"
    private val releaseKey = "onboarding.common.passkeyError.appNotAssociated"

    @Test
    fun `the core says a test build's association failure in the test domain's words, a release build's as before`() {
        val test = passkeyErrorWordsOn("android", "", null, association, true)
        val release = passkeyErrorWordsOn("android", "", null, association, false)
        assertEquals(PasskeyErrorKind.APP_NOT_ASSOCIATED, test.kind)
        assertEquals(testKey, test.key)
        assertEquals(releaseKey, release.key)
        assertEquals("the kind and the failure do not move", release.failure, test.failure)
        assertTrue("the sentence is in the corpus", strings.t(testKey) != testKey)
    }

    @Test
    fun `every passkey failure the onboarding machines hear carries this build's flag`() {
        val failure = PasskeyFailure(FailureKind.Other, association)
        assertEquals(true, OnboardingExecutor.errorFacts(failure, failure, testBuild = true).getBoolean("test_build"))
        assertEquals(false, OnboardingExecutor.errorFacts(failure, failure, testBuild = false).getBoolean("test_build"))
        // The executor's own answer: the flag `relying_party_id` is handed — a debug build's here.
        val answered = OnboardingExecutor.failureFor(JSONObject().put("type", "register_passkey"), failure)
        assertEquals("passkey_failed", answered.getString("type"))
        val facts = answered.getJSONObject("error")
        assertEquals(ProductionWriteGuard.testBuild, facts.getBoolean("test_build"))
        assertTrue("a unit test runs the debug build", facts.getBoolean("test_build"))
        assertEquals(association, facts.getString("message"))
    }

    @Test
    fun `a multisig action's passkey failure is worded for the build it runs in`() {
        val detail = MultisigShellFailure.Passkey("other", message = association).detail
        val onTest = MultisigWords(strings = strings, testBuild = true).failureText(detail)
        val onRelease = MultisigWords(strings = strings, testBuild = false).failureText(detail)
        assertEquals(strings.t(testKey), onTest)
        assertEquals(strings.t(releaseKey), onRelease)
        assertFalse(onTest == onRelease)
    }
}
