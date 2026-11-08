package app.getvela.wallet.feature.onboarding.core

import android.os.Build
import app.getvela.wallet.BuildConfig
import app.getvela.wallet.core.net.ProductionWriteGuard
import uniffi.vela_core_uniffi.relyingPartyId

/**
 * §35 (review 0-1 follow-up): the relying party this build's PLATFORM passkeys
 * live under — the core's rule (`service_defaults::relying_party_id`), never
 * this shell's. getvela.app's association files list only the store apps, so:
 *
 * - a release build → `getvela.app` (the test domain is never read);
 * - a test build (debug — the parallel space's build too — or the device-test
 *   flavour: [ProductionWriteGuard.testBuild], the core's `is_test_build`) on
 *   an emulator → `localhost`;
 * - a test build on a device → its `TEST_RP_DOMAIN`
 *   (`-PvelaTestRpDomain=…` or env `TEST_RP_DOMAIN` → [BuildConfig.TEST_RP_DOMAIN],
 *   empty = unset), and with none the core's error, said as the ceremony's
 *   failure — never a quiet fall-back to `getvela.app`.
 *
 * The app-owned CTAP routes (USB, caBLE) and the GMS security-key route keep
 * `getvela.app` ([PasskeyExecutor.RELYING_PARTY]); the parallel space's
 * fixture keys sign without a platform ceremony and never ask.
 */
object RelyingParty {
    /** What the core answered: the rpId, or why this build has none. */
    sealed class Answer {
        data class Id(val rpId: String) : Answer()

        /** The core's sentence (`TEST_RP_DOMAIN_UNSET`, or why the domain set is not one). */
        data class Unavailable(val message: String) : Answer()

        /** The rpId, or the ceremony's failure in the core's words. */
        fun orFail(): String = when (this) {
            is Id -> rpId
            is Unavailable -> throw PasskeyFailure(FailureKind.Other, message)
        }
    }

    /** The core's answer for these build facts. [testDomain] empty = unset. */
    fun answer(testBuild: Boolean, emulator: Boolean, testDomain: String): Answer =
        runCatching { relyingPartyId(testBuild, emulator, testDomain.takeIf { it.isNotBlank() }) }
            .fold(
                onSuccess = { Answer.Id(it) },
                onFailure = { Answer.Unavailable(it.message ?: "This build has no passkey domain.") },
            )

    /** This build on this phone. */
    val current: Answer by lazy {
        answer(testBuild = ProductionWriteGuard.testBuild, emulator = isEmulator(), testDomain = BuildConfig.TEST_RP_DOMAIN)
    }

    /**
     * Is this an Android emulator — the "simulator" the core's rule asks
     * about. The emulator's own hardware names (`ranchu`, `goldfish`), its
     * SDK images' product and fingerprint; a phone answers none of them.
     */
    fun isEmulator(
        // `?: ""`: a JVM test's android.jar has no values here.
        hardware: String = Build.HARDWARE ?: "",
        product: String = Build.PRODUCT ?: "",
        fingerprint: String = Build.FINGERPRINT ?: "",
        model: String = Build.MODEL ?: "",
    ): Boolean {
        val hw = hardware.lowercase()
        if (hw == "ranchu" || hw == "goldfish" || hw.startsWith("cutf")) return true
        val prod = product.lowercase()
        if (prod.startsWith("sdk") || prod.contains("_sdk_") || prod.endsWith("_sdk") || prod.contains("emulator") || prod.contains("simulator")) return true
        val fp = fingerprint.lowercase()
        if (fp.startsWith("generic") || fp.contains("/sdk_gphone") || fp.contains(":emulator") || fp.contains("emulator/")) return true
        return model.contains("Android SDK built for", ignoreCase = true) || model.startsWith("sdk_gphone", ignoreCase = true)
    }
}
