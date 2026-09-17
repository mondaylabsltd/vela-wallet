package app.getvela.wallet

import app.getvela.wallet.feature.contacts.core.RegistryNameLookup
import app.getvela.wallet.feature.contacts.core.RegistryNameLookup.IndexAnswer
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Issue 191: the Android transport for `vela_core::registry_lookup`. The walk
 * itself is the core's and is tested there (`registry_lookup.rs`); what is
 * pinned here is that this shell carries requests and answers faithfully — a
 * bare `0x` is an answer, a `null` is a silence, a 404 is not a failure — and
 * keeps a verdict exactly as long as the core says.
 *
 * The step function is scripted, so this needs no native library.
 */
class RegistryNameLookupTest {
    private val store = FakeStore()
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private var clock = 1_000_000.0
    private val transcripts = ArrayList<JSONArray>()
    private val performed = ArrayList<String>()

    private fun ask(vararg requests: JSONObject) =
        JSONObject().put("type", "ask").put("requests", JSONArray(requests.toList())).toString()

    private fun call(chain: Int) = JSONObject()
        .put("type", "eth_call").put("id", "cfg:$chain").put("chain_id", chain).put("to", "0xsigner").put("data", "0xdata")

    private fun get(id: String, path: String) = JSONObject().put("type", "index_get").put("id", id).put("path", path)

    private fun done(name: String?, remember: String) = JSONObject()
        .put("type", "done")
        .put("found", name?.let { JSONObject().put("name", it).put("public_key", "04ab") } ?: JSONObject.NULL)
        .put("remember", remember)
        .toString()

    private fun lookup(
        script: List<String>,
        chain: (Int) -> String? = { "0x" },
        index: (String) -> IndexAnswer = { IndexAnswer.NotFound },
    ): RegistryNameLookup {
        var round = 0
        return RegistryNameLookup(
            store = store,
            ethCall = { chainId, _, _ -> performed += "call:$chainId"; chain(chainId) },
            indexGet = { path -> performed += "get:$path"; index(path) },
            step = { _, answers -> transcripts += JSONArray(answers); script[round++] },
            now = { clock },
        )
    }

    @Test
    fun `requests are carried out and their answers handed back verbatim`() = runBlocking {
        val name = lookup(
            script = listOf(ask(call(100)), ask(call(8453), call(1)), ask(get("key", "/api/query?publicKey=04ab")), done("Interleave", "forever")),
            chain = { if (it == 100) "0x" else if (it == 8453) "0xkey" else null },
            index = { IndexAnswer.Ok("""{"groups":{"unitIds":[10]}}""") },
        ).nameFor(safe)

        assertEquals("Interleave", name)
        assertEquals(listOf("call:100", "call:8453", "call:1", "get:/api/query?publicKey=04ab"), performed)
        val last = transcripts.last()
        val byId = (0 until last.length()).associate { last.getJSONObject(it).getString("id") to last.getJSONObject(it) }
        // A bare `0x` is a chain ANSWERING; `null` is nobody answering.
        assertEquals("ok" to "0x", byId.getValue("cfg:100").let { it.getString("outcome") to it.getString("body") })
        assertEquals("ok", byId.getValue("cfg:8453").getString("outcome"))
        assertEquals("failed", byId.getValue("cfg:1").getString("outcome"))
        assertTrue(byId.getValue("cfg:1").isNull("body"))
        assertEquals("""{"groups":{"unitIds":[10]}}""", byId.getValue("key").getString("body"))
    }

    @Test
    fun `a hit is kept for good, with its key`() = runBlocking {
        lookup(listOf(done("Interleave", "forever"))).nameFor(safe)
        performed.clear()
        clock += 365.0 * 24 * 60 * 60 * 1000
        // A fresh instance, an empty script: the answer can only come from disk.
        assertEquals("Interleave", lookup(emptyList()).nameFor(safe.lowercase()))
        assertTrue(performed.isEmpty())
        assertTrue(store.values.getValue(RegistryNameLookup.CACHE_KEY).contains("\"publicKey\":\"04ab\""))
    }

    @Test
    fun `a miss is kept briefly, and a silence not at all`() = runBlocking {
        assertNull(lookup(listOf(done(null, "briefly"))).nameFor(safe))
        assertNull(lookup(emptyList()).nameFor(safe)) // inside the window: nothing is asked
        clock += RegistryNameLookup.MISS_TTL_MS + 1
        assertEquals("Late", lookup(listOf(done("Late", "forever"))).nameFor(safe))

        val other = "0x" + "11".repeat(20)
        assertNull(lookup(listOf(done(null, "no"))).nameFor(other))
        assertEquals("Now", lookup(listOf(done("Now", "forever"))).nameFor(other))
    }

    @Test
    fun `a 404 from the index is an answer, anything else is a silence`() = runBlocking {
        lookup(
            script = listOf(ask(get("a", "/a"), get("b", "/b")), done(null, "no")),
            index = { if (it == "/a") IndexAnswer.NotFound else IndexAnswer.Failed },
        ).nameFor(safe)
        val sent = transcripts.last()
        assertEquals(listOf("not_found", "failed"), (0 until sent.length()).map { sent.getJSONObject(it).getString("outcome") })
    }

    @Test
    fun `a malformed address costs nothing`() = runBlocking {
        assertNull(lookup(emptyList()).nameFor("nope"))
        assertTrue(transcripts.isEmpty())
    }
}
