package app.getvela.wallet

import app.getvela.wallet.core.net.ProductionWriteGuard
import app.getvela.wallet.feature.onboarding.core.RelyingParty
import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.relyingPartyJson

/**
 * §35 (review 0-1 follow-up) on Android: the relying party this build's
 * platform passkeys live under is the core's rule (`relying_party_id`) —
 * replayed here from the shared vector through `relyingPartyJson`, and through
 * the shell's own mapping ([RelyingParty.answer]): release → getvela.app
 * (its test domain never read); a test build on an emulator → localhost; a
 * test build on a device → its `TEST_RP_DOMAIN`, else the core's error, said
 * as the ceremony's failure — never getvela.app.
 */
class RelyingPartyTest {
    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle"))
    private val vector = JSONObject(File(repoRoot, "rust/crates/vela-core/tests/vectors/rules/rp-id.json").readText())
    private val native = vector.getJSONArray("native").let { cases -> (0 until cases.length()).map(cases::getJSONObject) }

    @Test
    fun `the native vector, through the core's door`() {
        assertEquals("the vector's 14 native cases", 14, native.size)
        for (case in native) {
            val name = case.getString("name")
            val answer = JSONObject(relyingPartyJson(case.getJSONObject("ask").toString()))
            if (case.has("rpId")) {
                assertEquals(name, case.getString("rpId"), answer.optString("rpId"))
            } else {
                assertEquals(name, case.getString("error"), answer.getJSONObject("error").getString("kind"))
            }
        }
    }

    @Test
    fun `the native vector, through the shell's mapping`() {
        for (case in native) {
            val name = case.getString("name")
            val ask = case.getJSONObject("ask")
            val answer = RelyingParty.answer(
                testBuild = ask.getBoolean("testBuild"),
                emulator = ask.getBoolean("simulator"),
                // The build's setting: empty is unset (BuildConfig.TEST_RP_DOMAIN).
                testDomain = if (ask.isNull("testDomain")) "" else ask.getString("testDomain"),
            )
            if (case.has("rpId")) {
                assertEquals(name, RelyingParty.Answer.Id(case.getString("rpId")), answer)
            } else {
                assertTrue("$name: $answer", answer is RelyingParty.Answer.Unavailable)
                // The core's words, never a fall-back.
                assertFalse(name, (answer as RelyingParty.Answer.Unavailable).message.isBlank())
            }
        }
    }

    @Test
    fun `this shell's builds ask the core as their facts say`() {
        val release = ProductionWriteGuard.testBuildFor(debug = false, deviceTest = false)
        val debug = ProductionWriteGuard.testBuildFor(debug = true, deviceTest = false)
        val deviceTest = ProductionWriteGuard.testBuildFor(debug = false, deviceTest = true)
        assertFalse("a store release is not a test build", release)
        assertTrue(debug)
        assertTrue("a device-test release is a test build", deviceTest)

        // A store release: getvela.app on a phone and an emulator, whatever domain leaked in.
        assertEquals(RelyingParty.Answer.Id("getvela.app"), RelyingParty.answer(release, emulator = false, testDomain = ""))
        assertEquals(RelyingParty.Answer.Id("getvela.app"), RelyingParty.answer(release, emulator = true, testDomain = "vela-test.example"))
        // Debug on an emulator: localhost.
        assertEquals(RelyingParty.Answer.Id("localhost"), RelyingParty.answer(debug, emulator = true, testDomain = ""))
        // A device test with its domain: that domain.
        assertEquals(RelyingParty.Answer.Id("vela-test.example"), RelyingParty.answer(deviceTest, emulator = false, testDomain = "vela-test.example"))
        // A device test without one (debug or devicetest): getvela.app, as the
        // store app (§35, revised 2026-11-14). Whether it may use it is
        // getvela.app's assetlinks.json's answer: the release certificate only.
        assertEquals(RelyingParty.Answer.Id("getvela.app"), RelyingParty.answer(deviceTest, emulator = false, testDomain = ""))
        assertEquals(RelyingParty.Answer.Id("getvela.app"), RelyingParty.answer(debug, emulator = false, testDomain = ""))
        assertEquals("localhost", RelyingParty.answer(debug, emulator = true, testDomain = "").orFail())
    }

    @Test
    fun `an emulator is told from a phone by its own build facts`() {
        // The SDK images this repo's AVDs run (API 34 google_apis arm64, API 30 x86_64).
        assertTrue(RelyingParty.isEmulator("ranchu", "sdk_gphone64_arm64", "google/sdk_gphone64_arm64/emu64a:14/UE1A.230829.036/1:userdebug/dev-keys", "sdk_gphone64_arm64"))
        assertTrue(RelyingParty.isEmulator("goldfish", "sdk_google_phone_x86", "generic/sdk_google_phone_x86/generic_x86:7.0/NYC/1:userdebug/test-keys", "Android SDK built for x86"))
        assertTrue(RelyingParty.isEmulator("", "sdk_gphone_x86_64", "", ""))
        // Phones.
        assertFalse(RelyingParty.isEmulator("husky", "husky", "google/husky/husky:14/AP1A.240305.019.A1/11445699:user/release-keys", "Pixel 8 Pro"))
        assertFalse(RelyingParty.isEmulator("qcom", "b0qxxx", "samsung/b0qxxx/b0q:14/UP1A.231005.007/S908BXXU5DWK1:user/release-keys", "SM-S908B"))
        assertFalse(RelyingParty.isEmulator("qcom", "OnePlus5T", "OnePlus/OnePlus5T/OnePlus5T:10/QKQ1.191014.012/2010292059:user/release-keys", "ONEPLUS A5010"))
        // A JVM test's android.jar has no values: not an emulator, and no crash.
        assertFalse(RelyingParty.isEmulator())
    }
}
