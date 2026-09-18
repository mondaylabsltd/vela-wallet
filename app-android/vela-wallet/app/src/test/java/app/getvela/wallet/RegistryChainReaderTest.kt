package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.RegistryChainReader
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.RegistryFailure
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Spec 062: signing in when the index service is gone. The REAL core reads the
 * REAL contract bytes (the fixture web recorded from Gnosis and Ethereum), so
 * what is pinned is the whole native path: plan → `eth_call` → index-shaped
 * JSON → the client's own guards. Unit ids are per deployment, which is the
 * mistake this is most likely to make: Gnosis's unit 10 is Ethereum's unit 0.
 */
class RegistryChainReaderTest {
    private val fixture: JSONObject by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        JSONObject(File(root, "app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json").readText())
    }
    private val key: String get() = fixture.getString("publicKey")

    private val asked = ArrayList<Int>()

    /** The recorded chains, minus the ones that are "down". */
    private fun reader(vararg down: Int) = RegistryChainReader(ethCall = { chainId, _, data ->
        asked += chainId
        if (chainId in down) null
        else fixture.getJSONObject("answers").optJSONObject(chainId.toString())?.optString(data)?.takeIf { it.isNotEmpty() }
    })

    /** An index nobody can reach: a closed local port refuses at once. */
    private fun client(reader: RegistryChainReader?) = RegistryClient("http://127.0.0.1:9", reader)

    @Test
    fun `index unreachable - the key and its unit are read from the contract on Gnosis`() = runBlocking {
        val client = client(reader())
        val status = client.queryByPublicKey(key)
        assertTrue(status.registered)
        assertEquals(listOf(12L, 10L, 8L), status.unitIds)

        val unit = client.queryUnit(10)
        assertEquals(3, unit.members.size)
        assertEquals(key, unit.members.first().publicKeyHex)
        assertTrue(unit.members.all { it.credentialIdHex.isNotEmpty() })
        assertTrue("Ethereum is not asked while Gnosis answers", asked.all { it == 100 })
    }

    @Test
    fun `Gnosis silent too - Ethereum's backup answers, under ITS unit ids`() = runBlocking {
        val client = client(reader(100))
        val status = client.queryByPublicKey(key)
        assertEquals(listOf(0L), status.unitIds)

        asked.clear()
        val unit = client.queryUnit(0)
        assertEquals(3, unit.members.size)
        assertEquals(key, unit.members.first().publicKeyHex)
        // A chain listing is continued on that chain and nowhere else.
        assertEquals(setOf(1), asked.toSet())
    }

    @Test
    fun `nobody answers - the index's own failure is what is reported`() {
        val gone = assertThrows(RegistryFailure::class.java) { runBlocking { client(reader(100, 1)).queryByPublicKey(key) } }
        assertTrue(gone.network)
        // And with no reader at all the client is exactly what it was.
        assertThrows(RegistryFailure::class.java) { runBlocking { client(null).queryByPublicKey(key) } }
    }

    @Test
    fun `a key this build cannot read plans nothing`() = runBlocking {
        assertNull(reader().keyProfile("not a key"))
        assertTrue(asked.isEmpty())
    }

    @Test
    fun `only an absent index is replaced - a refusal is an answer`() {
        assertTrue(RegistryFailure("Query failed: timeout", network = true).indexIsGone)
        assertTrue(RegistryFailure("Query failed: 503", network = false).indexIsGone)
        assertTrue(!RegistryFailure("Query failed: 400", network = false).indexIsGone)
        assertTrue(!RegistryFailure("Query failed: 404", network = false).indexIsGone)
        assertTrue(!RegistryFailure("Query failed: malformed response", network = false).indexIsGone)
    }
}
