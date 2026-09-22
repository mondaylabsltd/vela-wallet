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

    /**
     * `addr(bytes32)` — the FORWARD record, which the core asks for once a
     * reverse record has claimed a name (spec 081, FR-010).
     */
    private val selAddr = "3b3b57de"
    private val resolverAddr = "1111111111111111111111111111111111111111"

    /**
     * An ENS registry that reverse-resolves `alice` to `claimed`, and forward-
     * resolves `claimed` to whatever `forwardsTo` says — `null` for a name with
     * no forward record at all.
     */
    private fun serveEns(claimed: String, forwardsTo: String?) {
        chainAnswers = { chain, to, data ->
            when {
                chain != 1 -> null
                data.startsWith("0x" + IdentityResolver.SEL_RESOLVER) -> "0x" + resolverAddr.padStart(64, '0')
                !to.equals("0x$resolverAddr", ignoreCase = true) -> null
                data.startsWith("0x" + IdentityResolver.SEL_NAME) -> abiString(claimed)
                data.startsWith("0x$selAddr") ->
                    forwardsTo?.let { "0x" + it.removePrefix("0x").lowercase().padStart(64, '0') }
                        ?: ("0x" + "0".repeat(64))
                else -> null
            }
        }
    }

    @Test
    fun `a name service's reverse record is decoded and labelled by the service`() = runBlocking {
        serveEns(claimed = "alice.eth", forwardsTo = alice)
        val identity = resolver().resolve(alice)
        assertEquals("alice.eth", identity!!.name)
        assertEquals("ENS", identity.source)
        assertTrue("the index was asked first", calls.first() == "index")
        assertTrue("every service was asked", IdentityResolver.NAME_SERVICES.all { s -> calls.any { it == "call:${s.chainId}" } })
    }

    /**
     * The attack this shell now refuses: anyone can write their own
     * `addr.reverse`, so an address is named `vitalik.eth` and the wallet used
     * to draw it. Resolving that name forward lands on somebody else, so no
     * name is shown — and none is cached either.
     */
    @Test
    fun `a reverse record that resolves elsewhere is not shown`() = runBlocking {
        serveEns(claimed = "vitalik.eth", forwardsTo = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")
        assertNull(resolver().resolve(alice))
        assertNull(store.values[IdentityResolver.CACHE_KEY])
    }

    /** A claimed name with no forward record at all names nobody. */
    @Test
    fun `a reverse record with no forward record is not shown`() = runBlocking {
        serveEns(claimed = "alice.eth", forwardsTo = null)
        assertNull(resolver().resolve(alice))
    }

    /**
     * The forward lookup could not be made. The name is still not shown:
     * failing open here would let whoever poisons a record pick the moment.
     */
    @Test
    fun `a name is not shown when the forward lookup cannot be made`() = runBlocking {
        // The reverse direction answers; the forward one is unreachable.
        val reverseNode = IdentityResolver.namehash("${alice.drop(2).lowercase()}.addr.reverse").removePrefix("0x")
        chainAnswers = { chain, to, data ->
            when {
                chain != 1 -> null
                data == "0x" + IdentityResolver.SEL_RESOLVER + reverseNode -> "0x" + resolverAddr.padStart(64, '0')
                data.startsWith("0x" + IdentityResolver.SEL_NAME) -> abiString("alice.eth")
                else -> null
            }
        }
        assertNull(resolver().resolve(alice))
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
