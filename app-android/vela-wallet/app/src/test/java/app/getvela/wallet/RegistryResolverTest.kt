package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.RegistryFailure
import app.getvela.wallet.feature.onboarding.core.RegistryResolver
import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.net.InetSocketAddress
import java.util.Collections

/**
 * The three layers (067), through [RegistryClient], with the REAL core over the
 * REAL bytes both chains answered (the fixture web recorded) and a real local
 * HTTP index that says whatever the test tells it to.
 *
 * The rules are `vela_core::registry_resolve` and are tested there. Pinned HERE
 * is that this shell really hands its reads to that walk: an index answer is
 * PROVED rather than believed, a forged one is thrown away, and a listing's
 * unit ids are asked of whoever listed them (Gnosis's unit 10 is Ethereum's 0).
 */
class RegistryResolverTest {
    private val fixture: JSONObject by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        JSONObject(File(root, "app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json").readText())
    }
    private val key: String get() = fixture.getString("publicKey")
    private val asked: MutableList<String> = Collections.synchronizedList(ArrayList())

    /** What the index says: `null` = 503, else the body. */
    private var listing: String? = null
    private var unit: String? = null
    private var server: HttpServer? = null

    @After
    fun stop() { server?.stop(0) }

    private fun chains(vararg down: Int): suspend (Int, String, String) -> String? = { chainId, _, data ->
        asked += "chain:$chainId"
        if (chainId in down) null else fixture.getJSONObject("answers").optJSONObject(chainId.toString())?.optString(data)?.takeIf { it.isNotEmpty() }
    }

    /** A client whose index is a real local server (or a closed port when `serve` is false). */
    private fun client(serve: Boolean, vararg down: Int): RegistryClient {
        if (!serve) return RegistryClient("http://127.0.0.1:9", RegistryResolver(chains(*down)))
        val http = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        http.createContext("/api/query") { exchange ->
            asked += "index"
            val body = if (exchange.requestURI.query.contains("publicKey=")) listing else unit
            val bytes = (body ?: "{}").toByteArray()
            exchange.sendResponseHeaders(if (body == null) 503 else 200, bytes.size.toLong())
            exchange.responseBody.use { it.write(bytes) }
        }
        http.start()
        server = http
        return RegistryClient("http://127.0.0.1:${http.address.port}", RegistryResolver(chains(*down)))
    }

    /** Gnosis unit 10 in the index's shape, built from the CHAIN's own bytes — what an honest index returns. */
    private fun honestUnit(): JSONObject {
        val calls = JSONObject(uniffi.vela_core_uniffi.registryChainUnitPlan(10u)!!).getJSONArray("calls")
        val at = { i: Int -> fixture.getJSONObject("answers").getJSONObject("100").getString(calls.getJSONObject(i).getString("data")) }
        return JSONObject(uniffi.vela_core_uniffi.registryChainUnit(10u, at(0), at(1))!!)
    }

    @Test
    fun `an honest index is proved by ONE call to Gnosis`() = runBlocking {
        listing = """{"entry":{},"groups":{"total":1,"unitIds":[10]}}"""
        unit = honestUnit().toString()
        val client = client(serve = true)
        assertEquals(listOf(10L), client.queryByPublicKey(key).unitIds)
        asked.clear()
        assertEquals(3, client.queryUnit(10).members.size)
        assertEquals(listOf("index", "chain:100"), asked.toList())
    }

    @Test
    fun `a FORGED member is caught and the chain's founding set is returned`() = runBlocking {
        val honest = honestUnit()
        val forged = JSONObject(honest.toString())
        val items = forged.getJSONObject("members").getJSONArray("items")
        items.getJSONObject(2).put("publicKey", items.getJSONObject(1).getString("publicKey"))
        listing = """{"entry":{},"groups":{"total":1,"unitIds":[10]}}"""
        unit = forged.toString()
        val client = client(serve = true)
        client.queryByPublicKey(key)
        val members = client.queryUnit(10).members.map { it.publicKeyHex }
        val real = honest.getJSONObject("members").getJSONArray("items").let { a -> (0 until a.length()).map { a.getJSONObject(it).getString("publicKey") } }
        assertEquals(real, members)
        assertEquals(3, members.toSet().size)
    }

    @Test
    fun `no chain reachable - the person still signs in on the index's word`() = runBlocking {
        listing = """{"entry":{},"groups":{"total":1,"unitIds":[10]}}"""
        unit = honestUnit().toString()
        val client = client(serve = true, 100, 1)
        client.queryByPublicKey(key)
        assertEquals(3, client.queryUnit(10).members.size)
    }

    @Test
    fun `index unreachable - Gnosis answers, and Ethereum is not asked`() = runBlocking {
        val client = client(serve = false)
        val status = client.queryByPublicKey(key)
        assertTrue(status.registered)
        assertEquals(listOf(12L, 10L, 8L), status.unitIds)
        val unit = client.queryUnit(10)
        assertEquals(key, unit.members.first().publicKeyHex)
        assertFalse(asked.contains("chain:1"))
    }

    @Test
    fun `an index that fails or says no such wallet is checked with the contract first`() = runBlocking {
        listing = null // 503
        assertEquals(listOf(12L, 10L, 8L), client(serve = true).queryByPublicKey(key).unitIds)
        server?.stop(0)
        listing = """{"entry":null,"groups":{"total":0,"unitIds":[]}}"""
        assertEquals(listOf(12L, 10L, 8L), client(serve = true).queryByPublicKey(key).unitIds)
    }

    @Test
    fun `Gnosis silent too - Ethereum's backup answers under ITS unit ids, and only it is asked about them`() = runBlocking {
        val client = client(serve = false, 100)
        assertEquals(listOf(0L), client.queryByPublicKey(key).unitIds)
        asked.clear()
        assertEquals(key, client.queryUnit(0).members.first().publicKeyHex)
        assertEquals(setOf("chain:1"), asked.toSet())
    }

    @Test
    fun `nobody at all - the index's own failure is what is reported`() {
        val gone = assertThrows(RegistryFailure::class.java) { runBlocking { client(serve = false, 100, 1).queryByPublicKey(key) } }
        assertTrue(gone.network)
    }
}
