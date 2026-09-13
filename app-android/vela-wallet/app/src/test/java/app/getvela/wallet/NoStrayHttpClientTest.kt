package app.getvela.wallet

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * SC-107: **nothing but the pool talks to a chain.**
 *
 * The `rpc_pool` machine's entire value is that it decides which endpoint to
 * try, when to ban one and whether a 429 means "broken" or "busy". Every one of
 * those rules is bypassed by a single well-meaning `OkHttpClient()` somewhere
 * else — and nothing about that code would look wrong in review. It would work,
 * on a good network, on the reviewer's phone.
 *
 * So this test reads the source. A new HTTP client under `feature/` is a build
 * failure until it is either routed through the pool or added to the list below
 * with a reason.
 */
class NoStrayHttpClientTest {

    /**
     * The files allowed to construct a client, and why.
     *
     * `VelaHttp` is the one client the read path shares. `RpcPoolExecutor`
     * builds per-call clients from it to apply the core's own `timeout_ms`.
     * `RegistryClient` predates the pool, talks to one service (the passkey
     * index) rather than to a chain, and its own header explains its choice of
     * `HttpURLConnection`.
     */
    private val allowed = setOf(
        "core/net/VelaHttp.kt",
        "feature/wallet/core/RpcPoolExecutor.kt",
        "feature/onboarding/core/RegistryClient.kt",
        // The caBLE tunnel is a WebSocket to a FIDO relay, not a chain read.
        "feature/onboarding/core/CableTransports.kt",
        // Fetches a passkey provider's icon by URL. Not a chain, and not
        // routable through a pool that only knows about chains. Found by this
        // test on its first run, which is what it is for.
        "core/passkey/PasskeyDirectory.kt",
    )

    private val clientMarkers = listOf(
        "OkHttpClient(",
        "OkHttpClient.Builder",
        "HttpURLConnection",
        "URL(",
    )

    @Test
    fun onlyThePoolReachesTheNetwork() {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        val sources = File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet")
        assertTrue("Kotlin sources not found at ${sources.absolutePath}", sources.isDirectory)

        val offenders = sources.walkTopDown()
            .filter { it.isFile && it.extension == "kt" }
            .mapNotNull { file ->
                val relative = file.relativeTo(sources).path
                if (relative in allowed) return@mapNotNull null
                val text = file.readText()
                val hit = clientMarkers.firstOrNull { text.contains(it) }
                if (hit == null) null else "$relative (contains \"$hit\")"
            }
            .sorted()
            .toList()

        assertEquals(
            "these files reach the network without the pool — route them through " +
                "RpcPool, or add them to `allowed` with a reason:\n" +
                offenders.joinToString("\n"),
            emptyList<String>(),
            offenders,
        )
    }

    @Test
    fun theAllowListItselfStaysHonest() {
        // A list of exceptions is only useful while it is short and every entry
        // is real. An entry for a file that no longer exists is an exception
        // nobody is checking.
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set")
        val sources = File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet")
        val missing = allowed.filterNot { File(sources, it).isFile }
        assertEquals("allow-list entries that no longer exist", emptyList<String>(), missing)
    }
}
