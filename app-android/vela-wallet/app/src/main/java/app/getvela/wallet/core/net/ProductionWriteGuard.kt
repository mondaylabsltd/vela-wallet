package app.getvela.wallet.core.net

import app.getvela.wallet.BuildConfig
import app.getvela.wallet.core.diagnostics.VelaLog
import java.io.IOException
import okhttp3.Interceptor
import okhttp3.Request
import okhttp3.Response
import okio.Buffer
import uniffi.vela_core_uniffi.ServiceDefaults
import uniffi.vela_core_uniffi.isProductionHost
import uniffi.vela_core_uniffi.isTestBuild
import uniffi.vela_core_uniffi.productionWriteRefusal
import uniffi.vela_core_uniffi.productionWriteRefusedEnglish
import uniffi.vela_core_uniffi.productionWritesOptIn
import uniffi.vela_core_uniffi.serviceDefaults

/**
 * Every production service default — the core's (`service_defaults`, §28).
 * This app keeps no copy of its own: the key index, the multisig service,
 * the relay, the bug report and PeerConnect's addresses are read here.
 */
object CoreServiceDefaults {
    val value: ServiceDefaults by lazy { serviceDefaults() }
}

/**
 * §28: a test build never writes to production.
 *
 * The rule is the core's (`service_defaults::write_verdict`): in a TEST
 * build — debug (which is also the parallel space's dev-fixtures build), or
 * the `devicetest` flavour (`app.getvela.wallet.devicetest`) — a write
 * (POST/PUT/DELETE…; a JSON-RPC POST asking only reads is a read) to a
 * production host (`*.getvela.app`, the defaults' hosts) is refused before
 * it leaves the device, logged, and said honestly ([ProductionWriteRefused]'s
 * message). Reads stay allowed.
 *
 * Where it stands: [VelaHttp]'s client carries this interceptor in a test
 * build, so every client derived from it does (the pool, the relay, the
 * multisig service and its mailbox, PeerConnect's token, the bug report);
 * `RegistryClient` — a connection of its own — asks [refusal] before it
 * writes. A release build installs nothing ([installed] is false).
 *
 * The opt-in, off by default: `-PvelaAllowProductionWrites` at build time
 * (`BuildConfig.ALLOW_PRODUCTION_WRITES`), or the launch extra
 * `vela.allowProductionWrites=1` on a debug launch (`LaunchEntry`) —
 * `VELA_ALLOW_PRODUCTION_WRITES` by the core's name.
 */
object ProductionWriteGuard : Interceptor {

    /** This build is a test build (the core's `is_test_build`). */
    val testBuild: Boolean by lazy { testBuildFor(debug = BuildConfig.DEBUG, deviceTest = BuildConfig.DEVICE_TEST) }

    /**
     * A build with these facts is a test build: a debug build (on Android
     * also the dev-fixtures one: the parallel space is debug-only) or the
     * device-test flavour.
     */
    fun testBuildFor(debug: Boolean, deviceTest: Boolean): Boolean =
        isTestBuild(devFixtures = debug, deviceTest = deviceTest, debug = debug)

    /** Whether the guard is on the app's client: a test build only. */
    val installed: Boolean get() = testBuild

    @Volatile
    private var optedIn: Boolean = BuildConfig.ALLOW_PRODUCTION_WRITES

    /**
     * An APK signed with a certificate getvela.app trusts may write to
     * production even as a test build (owner, 2026-11-14): its passkeys are
     * getvela.app's (§35, revised), and creating and signing in must work as
     * they will ship — publishing to the key index included. Only the owner's
     * upload key is listed (`BuildConfig.PRODUCTION_CERT_SHA256`, from
     * app-links/assetlinks.json), so a debug-key build — CI's, any other
     * machine's, an emulator's — keeps refusing.
     */
    fun optInIfSignedForProduction(context: android.content.Context) {
        if (!testBuild || optedIn) return
        val signers = runCatching { signerDigests(context) }.getOrDefault(emptyList())
        if (signedForProduction(signers, BuildConfig.PRODUCTION_CERT_SHA256)) {
            optedIn = true
            VelaLog.event("guard", "production writes allowed: signed with getvela.app's certificate")
        }
    }

    /** Does any of [signers] (SHA-256, any case or separator) appear in [trusted] (comma-separated)? */
    fun signedForProduction(signers: List<String>, trusted: String): Boolean {
        fun norm(s: String) = s.filter { it.isLetterOrDigit() }.uppercase()
        val known = trusted.split(',').map(::norm).filter { it.length == 64 }.toSet()
        return signers.any { norm(it) in known }
    }

    @Suppress("DEPRECATION")
    private fun signerDigests(context: android.content.Context): List<String> {
        val info = context.packageManager.getPackageInfo(
            context.packageName,
            android.content.pm.PackageManager.GET_SIGNING_CERTIFICATES,
        )
        val signing = info.signingInfo ?: return emptyList()
        val certs = if (signing.hasMultipleSigners()) signing.apkContentsSigners else signing.signingCertificateHistory
        return certs.map { cert ->
            java.security.MessageDigest.getInstance("SHA-256").digest(cert.toByteArray())
                .joinToString("") { "%02X".format(it) }
        }
    }

    /** A debug launch's `vela.allowProductionWrites` (the core reads the value). */
    fun optIn(value: String?) {
        if (testBuild && productionWritesOptIn(value)) {
            optedIn = true
            VelaLog.event("guard", "production writes allowed by launch input")
        }
    }

    /**
     * The production host a request is refused for, or `null` — it may
     * leave. Always `null` in a release build. [body] is read only for a
     * POST to a production host (a JSON-RPC read is allowed).
     */
    fun refusal(method: String, url: String, body: () -> String?): String? =
        refusalFor(testBuild, optedIn, method, url, body)

    /** [refusal] for a given build — the seam the release-build test uses. */
    fun refusalFor(testBuild: Boolean, allow: Boolean, method: String, url: String, body: () -> String?): String? {
        if (!testBuild || allow) return null
        val candidate = method.equals("POST", ignoreCase = true) &&
            (runCatching { java.net.URI(url).host }.getOrNull()?.let(::isProductionHost) == true)
        return productionWriteRefusal(testBuild, allow, method, url, if (candidate) body() else null)
    }

    override fun intercept(chain: Interceptor.Chain): Response = interceptorFor(testBuild) { optedIn }.intercept(chain)

    /** The interceptor for a given build — the seam the tests use. */
    fun interceptorFor(testBuild: Boolean, allow: () -> Boolean): Interceptor = Interceptor { chain ->
        val request = chain.request()
        val url = request.url.toString()
        val host = refusalFor(testBuild, allow(), request.method, url) { bodyText(request) }
        if (host != null) throw refused(request.method, url, host)
        chain.proceed(request)
    }

    /** The refusal as the error a transport throws, logged. */
    fun refused(method: String, url: String, host: String): ProductionWriteRefused {
        VelaLog.event("guard", "refused a test build's write to production", "method" to method, "host" to host, "url" to url)
        return ProductionWriteRefused(host)
    }

    private fun bodyText(request: Request): String? = runCatching {
        val body = request.body ?: return@runCatching null
        if (body.isOneShot()) return@runCatching null
        Buffer().also(body::writeTo).readUtf8()
    }.getOrNull()
}

/**
 * A test build's write to a production service, refused before it left
 * ("This test build won't write to the live service; point it at a local
 * one." — `common.testBuild.productionWriteRefused`). Never sent, so never
 * "may have been sent".
 */
class ProductionWriteRefused(val host: String) : IOException("${productionWriteRefusedEnglish()} ($host)")

/**
 * r18-8 (§32): whether an error — or anything that caused it — is this test
 * build's guard refusing a write before it left. The shells say it to the
 * core as `refused: true`: never sent, never the network's answer.
 */
object Refusals {
    fun Throwable.refusal(): Boolean =
        generateSequence(this) { it.cause }.take(8).any {
            it is ProductionWriteRefused || (it as? app.getvela.wallet.feature.onboarding.core.RegistryFailure)?.refused == true
        }

    /** [refusal] of [error], for a caller with no receiver in hand. */
    fun of(error: Throwable): Boolean = error.refusal()
}
