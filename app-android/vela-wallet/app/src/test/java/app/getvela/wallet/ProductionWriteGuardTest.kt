package app.getvela.wallet

import app.getvela.wallet.core.net.CoreServiceDefaults
import app.getvela.wallet.core.net.ProductionWriteGuard
import app.getvela.wallet.core.net.ProductionWriteRefused
import app.getvela.wallet.core.net.VelaHttp
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.wallet.core.OkHttpTransport
import java.io.File
import java.io.IOException
import java.net.InetAddress
import java.net.ServerSocket
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread
import okhttp3.Dns
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test

/**
 * §28: a test build never writes to production.
 *
 * Proven with a local server and the production host names mapped to it
 * (okhttp's `Dns` answers loopback for every name): nothing here can reach a
 * production service. The vector `tests/vectors/rules/production-writes.json`
 * is replayed through this shell's door to the core's rule.
 */
class ProductionWriteGuardTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val vector = JSONObject(File(repoRoot, "rust/crates/vela-core/tests/vectors/rules/production-writes.json").readText())

    private lateinit var server: ServerSocket
    private val seen = LinkedBlockingQueue<String>()

    @Before
    fun up() {
        server = ServerSocket(0, 50, InetAddress.getLoopbackAddress())
        thread(isDaemon = true) {
            while (!server.isClosed) {
                val socket = runCatching { server.accept() }.getOrNull() ?: break
                thread(isDaemon = true) {
                    socket.use { s ->
                        val input = s.getInputStream().bufferedReader()
                        val line = input.readLine() ?: return@thread
                        var length = 0
                        while (true) {
                            val header = input.readLine() ?: break
                            if (header.isEmpty()) break
                            if (header.lowercase().startsWith("content-length:")) length = header.substringAfter(':').trim().toInt()
                        }
                        repeat(length) { input.read() }
                        seen.add(line)
                        val answer = "{\"ok\":true}"
                        s.getOutputStream().write(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: ${answer.length}\r\nconnection: close\r\n\r\n$answer".toByteArray(),
                        )
                    }
                }
            }
        }
    }

    @After
    fun down() = server.close()

    /** A client whose every name is this machine — the production names mapped to loopback. */
    private fun client(guard: Interceptor?): OkHttpClient = OkHttpClient.Builder()
        .proxy(java.net.Proxy.NO_PROXY)
        .dns(object : Dns { override fun lookup(hostname: String) = listOf(InetAddress.getLoopbackAddress()) })
        .apply { if (guard != null) addInterceptor(guard) }
        .callTimeout(5, TimeUnit.SECONDS)
        .build()

    private fun post(client: OkHttpClient, url: String, body: String) =
        client.newCall(Request.Builder().url(url).post(body.toRequestBody("application/json".toMediaType())).build()).execute().use { it.code }

    private val register get() = "http://p256-index-v2.getvela.app:${server.localPort}/api/register"

    @Test
    fun a_test_build_refuses_a_production_write_before_it_leaves() {
        val guard = ProductionWriteGuard.interceptorFor(testBuild = true) { false }
        try {
            post(client(guard), register, "{\"members\":[]}")
            fail("a test build's write to the key index went out")
        } catch (refused: ProductionWriteRefused) {
            assertEquals("p256-index-v2.getvela.app", refused.host)
            assertTrue(refused.message!!.contains("This test build won't write to the live service"))
        }
        assertNull("nothing reached the (loopback) index", seen.poll(300, TimeUnit.MILLISECONDS))
        // Never sent: the core's class for it is "refused" — not "may have been sent".
        assertEquals("refused", OkHttpTransport.failureClass(ProductionWriteRefused("x"), null))
    }

    @Test
    fun a_test_build_still_reads_production_and_writes_locally() {
        val guarded = client(ProductionWriteGuard.interceptorFor(testBuild = true) { false })
        guarded.newCall(Request.Builder().url("http://p256-index-v2.getvela.app:${server.localPort}/api/health").build()).execute().close()
        assertEquals("GET /api/health HTTP/1.1", seen.poll(5, TimeUnit.SECONDS))
        // A JSON-RPC read to the relay is a read.
        post(guarded, "http://vela-relay-cf.getvela.app:${server.localPort}/100", """{"jsonrpc":"2.0","id":1,"method":"eth_estimateUserOperationGas","params":[]}""")
        assertEquals("POST /100 HTTP/1.1", seen.poll(5, TimeUnit.SECONDS))
        // A submit to it is a write.
        try {
            post(guarded, "http://vela-relay-cf.getvela.app:${server.localPort}/100", """{"jsonrpc":"2.0","id":2,"method":"eth_sendUserOperation","params":[]}""")
            fail("a submit went out")
        } catch (_: ProductionWriteRefused) {
        }
        // A local stand-in takes writes.
        post(guarded, "http://127.0.0.1:${server.localPort}/api/register", "{}")
        assertEquals("POST /api/register HTTP/1.1", seen.poll(5, TimeUnit.SECONDS))
    }

    @Test
    fun the_opt_in_lets_a_test_build_write() {
        post(client(ProductionWriteGuard.interceptorFor(testBuild = true) { true }), register, "{}")
        assertEquals("POST /api/register HTTP/1.1", seen.poll(5, TimeUnit.SECONDS))
    }

    /** A release build installs no guard, and its write leaves as before (to the loopback stand-in). */
    @Test
    fun a_release_build_is_unaffected() {
        assertFalse(ProductionWriteGuard.testBuildFor(debug = false, deviceTest = false))
        assertTrue(ProductionWriteGuard.testBuildFor(debug = false, deviceTest = true))
        assertTrue(ProductionWriteGuard.testBuildFor(debug = true, deviceTest = false))
        assertNull(ProductionWriteGuard.refusalFor(false, false, "POST", register) { "{}" })
        // Even were the interceptor there, a release build's answer is "send it".
        post(client(ProductionWriteGuard.interceptorFor(testBuild = false) { false }), register, "{\"members\":[]}")
        assertEquals("POST /api/register HTTP/1.1", seen.poll(5, TimeUnit.SECONDS))
        post(client(null), register, "{}")
        assertEquals("POST /api/register HTTP/1.1", seen.poll(5, TimeUnit.SECONDS))
    }

    /** This (debug) unit-test build is a test build, and the app's client carries the guard. */
    @Test
    fun this_debug_build_carries_the_guard() {
        assertTrue(BuildConfig.DEBUG)
        assertTrue(ProductionWriteGuard.installed)
        assertTrue(VelaHttp.client.interceptors.contains(ProductionWriteGuard))
        assertFalse("the opt-in is off by default", BuildConfig.ALLOW_PRODUCTION_WRITES)
    }

    /** RegistryClient's own connection asks the guard before it writes. */
    @Test
    fun the_key_index_client_refuses_before_it_connects() {
        val client = RegistryClient("http://p256-index-v2.getvela.app:${server.localPort}")
        val failure = runCatching { kotlinx.coroutines.runBlocking { client.groupChallenge("0x", "0x04", emptyList(), "getvela.app") } }.exceptionOrNull()
        assertTrue("$failure", failure?.message?.contains("This test build won't write to the live service") == true)
        assertNull(seen.poll(300, TimeUnit.MILLISECONDS))
    }

    @Test
    fun the_defaults_are_the_cores() {
        val defaults = vector.getJSONObject("defaults")
        val core = CoreServiceDefaults.value
        assertEquals(defaults.getString("passkeyIndexUrl"), core.passkeyIndexUrl)
        assertEquals(defaults.getString("genesisServiceUrl"), core.genesisServiceUrl)
        assertEquals(defaults.getString("bundlerServiceUrl"), core.bundlerServiceUrl)
        assertEquals(defaults.getString("bugReportUrl"), core.bugReportUrl)
        assertEquals(core.passkeyIndexUrl, RegistryClient.DEFAULT_REGISTRY_URL)
        assertEquals(core.genesisServiceUrl, app.getvela.wallet.feature.multisig.MultisigWiring.DEFAULT_SERVICE)
        assertEquals(core.bugReportUrl, app.getvela.wallet.core.diagnostics.BugReport.ENDPOINT)
    }

    @Test
    fun every_vector_case_answers_as_the_core_says() {
        val cases = vector.getJSONArray("cases")
        for (i in 0 until cases.length()) {
            val case = cases.getJSONObject(i)
            val refused = ProductionWriteGuard.refusalFor(
                case.getBoolean("testBuild"),
                case.getBoolean("allowProductionWrites"),
                case.getString("method"),
                case.getString("url"),
            ) { case.optString("body").takeIf { case.has("body") } }
            assertEquals(case.getString("about"), case.optString("host").takeIf { case.has("host") }, refused)
        }
    }

    @Suppress("unused")
    private fun ioOf(error: Throwable) = error as? IOException
}
