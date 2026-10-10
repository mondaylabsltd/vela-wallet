package app.getvela.wallet

import app.getvela.wallet.feature.wallet.core.ChainData
import app.getvela.wallet.feature.wallet.core.ChainDoc
import java.io.IOException
import java.net.SocketTimeoutException
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.runBlocking
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * PR 2 polish: the chain registry document has THREE outcomes — there
 * (`Doc`), definitively not there (`Absent`, a 404 from a server that
 * answered), and no answer to go on (`Unread`: the network failed, the read
 * timed out, 5xx, 429 or any other non-2xx, an unreadable body). The first
 * two are cached; an unread one never is, so the next round asks again.
 *
 * Every answer here comes through the real OkHttp call path ([ChainData]'s
 * own `fetch`), from an interceptor standing in for the server.
 */
class ChainDataTest {

    private val doc = """
        {"nativeCurrency":{"name":"USD","symbol":"USD","decimals":18},
         "stables":[{"symbol":"pathUSD","type":"native","contract":"0x20C0000000000000000000000000000000000000"}]}
    """.trimIndent()

    /** A server answering every request with [answer]; [asked] counts the requests that reached it. */
    private class Server(private val answer: (Interceptor.Chain) -> Response) {
        val asked = AtomicInteger()
        val client: OkHttpClient = OkHttpClient.Builder()
            .addInterceptor { chain -> asked.incrementAndGet(); answer(chain) }
            .build()
    }

    private fun respond(chain: Interceptor.Chain, code: Int, body: String = "") = Response.Builder()
        .request(chain.request())
        .protocol(Protocol.HTTP_1_1)
        .code(code)
        .message("status $code")
        .body(body.toResponseBody("application/json".toMediaType()))
        .build()

    private fun chainData(server: Server, endpoint: String = "https://data.test") =
        ChainData(endpoint = { endpoint }, http = { server.client })

    @Test
    fun `a document read is a doc, and is cached`() = runBlocking<Unit> {
        val server = Server { respond(it, 200, doc) }
        val data = chainData(server)
        val first = data.read(4217)
        assertTrue("$first", first is ChainDoc.Doc)
        assertEquals("pathUSD", (first as ChainDoc.Doc).info.stables.single().symbol)
        assertEquals(first, data.read(4217))
        assertEquals("cached: asked once", 1, server.asked.get())
        assertEquals(first.info, data.forChain(4217))
    }

    @Test
    fun `a 404 from a server that answered is absent, and is cached`() = runBlocking<Unit> {
        val server = Server { respond(it, 404, "not found") }
        val data = chainData(server)
        assertEquals(ChainDoc.Absent, data.read(4217))
        assertEquals(ChainDoc.Absent, data.read(4217))
        assertEquals("cached: asked once", 1, server.asked.get())
        assertEquals(null, data.forChain(4217))
    }

    @Test
    fun `a 5xx, a 429 or any other non-2xx is unread, and never cached`() = runBlocking<Unit> {
        for (code in listOf(500, 502, 503, 429, 403, 301)) {
            val server = Server { respond(it, code) }
            val data = chainData(server)
            assertEquals("HTTP $code", ChainDoc.Unread, data.read(4217))
            assertEquals("HTTP $code", ChainDoc.Unread, data.read(4217))
            assertEquals("HTTP $code: asked again, never cached", 2, server.asked.get())
        }
    }

    @Test
    fun `a network failure or a timeout is unread, and never cached`() = runBlocking<Unit> {
        for (failure in listOf(IOException("connection reset"), SocketTimeoutException("timeout"))) {
            val server = Server { throw failure }
            val data = chainData(server)
            assertEquals(failure.toString(), ChainDoc.Unread, data.read(4217))
            assertEquals(failure.toString(), ChainDoc.Unread, data.read(4217))
            assertEquals("asked again", 2, server.asked.get())
        }
    }

    @Test
    fun `an unreadable body is unread`() = runBlocking<Unit> {
        val server = Server { respond(it, 200, "<html>captive portal</html>") }
        val data = chainData(server)
        assertEquals(ChainDoc.Unread, data.read(4217))
        assertEquals(ChainDoc.Unread, data.read(4217))
        assertEquals("never cached", 2, server.asked.get())
    }

    @Test
    fun `an unread answer is replaced by the next good one`() = runBlocking<Unit> {
        var down = true
        val server = Server { if (down) respond(it, 503) else respond(it, 200, doc) }
        val data = chainData(server)
        assertEquals(ChainDoc.Unread, data.read(4217))
        down = false
        assertTrue(data.read(4217) is ChainDoc.Doc)
    }

    @Test
    fun `no endpoint yet is unread — nothing was asked`() = runBlocking<Unit> {
        val server = Server { respond(it, 200, doc) }
        assertEquals(ChainDoc.Unread, chainData(server, endpoint = "").read(4217))
        assertEquals(0, server.asked.get())
    }
}
