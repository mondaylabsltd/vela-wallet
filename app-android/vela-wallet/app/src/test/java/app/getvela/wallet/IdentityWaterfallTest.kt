package app.getvela.wallet

import app.getvela.wallet.feature.contacts.core.IdentityResolver
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 043 T050: the identity waterfall's order and its cache — own accounts
 * before any network, the index before the name services, positives cached
 * for a day, misses never.
 */
class IdentityWaterfallTest {
    private val store = FakeStore()
    private val alice = "0x9F3c000000000000000000000000000000021aE0"
    private val calls = ArrayList<String>()
    private var registry: String? = null
    private var chainAnswers: (Int, String, String) -> String? = { _, _, _ -> null }
    private var clock = 1_000_000.0

    private fun resolver(own: List<Pair<String, String>> = emptyList()) = IdentityResolver(
        store = store,
        ownAccounts = { own },
        registryName = { calls += "index"; registry },
        ethCall = { chain, to, data -> calls += "call:$chain"; chainAnswers(chain, to, data) },
        now = { clock },
    )

    /** An ABI `string` return: offset, length, bytes. */
    private fun abiString(value: String): String {
        val bytes = value.toByteArray(Charsets.UTF_8)
        val hex = bytes.joinToString("") { "%02x".format(it) }.padEnd(((bytes.size + 31) / 32) * 64, '0')
        return "0x" + "20".padStart(64, '0') + bytes.size.toString(16).padStart(64, '0') + hex
    }

    @Test
    fun `the person's own account is named without a network call`() = runBlocking {
        val identity = resolver(own = listOf(alice.lowercase() to "Savings")).resolve(alice)
        assertEquals("Savings", identity!!.name)
        assertEquals("self", identity.source)
        assertTrue(calls.isEmpty())
    }

    @Test
    fun `the index answers before any name service is asked`() = runBlocking {
        registry = "alice.vela"
        val identity = resolver().resolve(alice)
        assertEquals("alice.vela" to "passkey", identity!!.name to identity.source)
        assertEquals(listOf("index"), calls)
        // Cached — the next look costs nothing.
        calls.clear()
        assertEquals("alice.vela", resolver().resolve(alice)!!.name)
        assertTrue(calls.isEmpty())
        assertTrue(store.values.getValue(IdentityResolver.CACHE_KEY).contains(alice.lowercase()))
    }

    @Test
    fun `a name service's reverse record is decoded and labelled by the service`() = runBlocking {
        val resolverAddr = "1111111111111111111111111111111111111111"
        chainAnswers = { chain, to, data ->
            when {
                chain != 1 -> null
                data.startsWith("0x" + IdentityResolver.SEL_RESOLVER) -> "0x" + resolverAddr.padStart(64, '0')
                data.startsWith("0x" + IdentityResolver.SEL_NAME) && to.equals("0x$resolverAddr", ignoreCase = true) -> abiString("alice.eth")
                else -> null
            }
        }
        val identity = resolver().resolve(alice)
        assertEquals("alice.eth", identity!!.name)
        assertEquals("ENS", identity.source)
        assertTrue("the index was asked first", calls.first() == "index")
        assertTrue("every service was asked", IdentityResolver.NAME_SERVICES.all { s -> calls.any { it == "call:${s.chainId}" } })
    }

    @Test
    fun `nobody knows this address, and that is not cached`() = runBlocking {
        assertNull(resolver().resolve(alice))
        assertNull(store.values[IdentityResolver.CACHE_KEY])
        // A name registered after the first look is seen on the next.
        registry = "late.vela"
        assertEquals("late.vela", resolver().resolve(alice)!!.name)
    }

    @Test
    fun `the cache expires after a day`() = runBlocking {
        registry = "alice.vela"
        resolver().resolve(alice)
        registry = "renamed.vela"
        clock += IdentityResolver.CACHE_TTL_MS + 1
        assertEquals("renamed.vela", resolver().resolve(alice)!!.name)
    }

    @Test
    fun `the zero address and malformed text are never asked about`() = runBlocking {
        assertNull(resolver().resolve("0x0000000000000000000000000000000000000000"))
        assertNull(resolver().resolve("0xabc"))
        assertTrue(calls.isEmpty())
    }

    @Test
    fun `namehash matches EIP-137`() {
        assertEquals("0x0000000000000000000000000000000000000000000000000000000000000000", IdentityResolver.namehash(""))
        assertEquals("0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae", IdentityResolver.namehash("eth"))
        assertEquals("0xde9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f", IdentityResolver.namehash("foo.eth"))
    }

    @Test
    fun `an over-long or empty name is refused`() {
        assertNull(IdentityResolver.decodeName(abiString("")))
        assertNull(IdentityResolver.decodeName(abiString("x".repeat(600))))
        assertEquals("alice.eth", IdentityResolver.decodeName(abiString("  alice.eth ")))
        val stored = JSONObject().put("name", "n").toString()
        assertTrue(stored.isNotEmpty())
    }
}
