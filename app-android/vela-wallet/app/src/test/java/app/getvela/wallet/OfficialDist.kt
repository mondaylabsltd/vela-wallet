package app.getvela.wallet

import app.getvela.wallet.feature.signing.trustedsigner.SignerPageChecks
import java.io.File
import java.util.concurrent.atomic.AtomicInteger

/**
 * The official signing page's deployment, served from the repo's committed
 * `app-web/trusted-signer/dist/` — the very bytes `sign.getvela.app` serves —
 * so a test can run the phone's integrity check (spec 102 R6) end to end
 * against the real core without reaching the internet. `index.json` lists the
 * published versions; `/b/<sha>/sign` is the content-addressed page, which the
 * official host serves without the `.html` (the core's launch URL).
 */
object OfficialDist {
    private val root: File by lazy {
        val repo = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        File(repo, "app-web/trusted-signer/dist")
    }

    /** The version this build launches today (BUILD_ALLOWED[0], listed by the index). */
    val VERSION: String get() = uniffi.vela_core_uniffi.signerPageAllowed().first()

    /** The one URL that is fetched to be checked and opened. */
    val LAUNCH_URL: String get() = "https://sign.getvela.app/b/$VERSION/sign"

    val fetches = AtomicInteger(0)

    /**
     * Any deployment of `dist/` — the official host, or the same bytes copied
     * to another (a self-hoster's, or the fixture keyset's `getvela.app`).
     */
    suspend fun fetch(url: String): SignerPageChecks.Fetched {
        fetches.incrementAndGet()
        val path = url.substringAfter("://").substringAfter('/')
        val file = when {
            path.startsWith("b/") && path.endsWith("/sign") -> File(root, "$path.html")
            else -> File(root, path)
        }
        return if (file.isFile) {
            SignerPageChecks.Fetched.Body(200, file.readBytes())
        } else {
            SignerPageChecks.Fetched.Body(404, ByteArray(0))
        }
    }

    fun checks(clock: () -> Long = System::currentTimeMillis, store: FakeStore = FakeStore()) =
        SignerPageChecks(fetch = ::fetch, store = store, clock = clock)
}
