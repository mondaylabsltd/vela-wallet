package app.getvela.wallet

import app.getvela.wallet.feature.wallet.core.OkHttpTransport
import app.getvela.wallet.feature.wallet.core.RpcTransportOutcome
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.atomic.AtomicInteger
import kotlin.concurrent.thread
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * r11-9 (round 11, live): every relay failure on the emulator was okhttp's
 * "unexpected end of stream" on a REUSED connection — the relay (behind
 * `adb reverse`) had closed it after its answer, okhttp kept it pooled, and
 * the next POST met a dead socket: an estimate "couldn't be checked", a
 * submit "may have been sent". Here a server that answers one request per
 * connection and drops the next one unanswered stands in for it.
 */
class StaleConnectionTest {

    private val server = ServerSocket(0, 50, InetAddress.getLoopbackAddress())
    private val connections = AtomicInteger()
    private val answered = AtomicInteger()

    init {
        thread(isDaemon = true) {
            while (!server.isClosed) {
                val socket = runCatching { server.accept() }.getOrNull() ?: break
                connections.incrementAndGet()
                thread(isDaemon = true) { serve(socket) }
            }
        }
    }

    /** One answer, kept alive; the next request on the same connection read and dropped. */
    private fun serve(socket: Socket) = socket.use {
        val input = BufferedReader(InputStreamReader(it.getInputStream()))
        for (n in 0..1) {
            var length = 0
            while (true) {
                val line = input.readLine() ?: return
                if (line.isEmpty()) break
                if (line.startsWith("content-length:", ignoreCase = true)) length = line.substringAfter(':').trim().toInt()
            }
            val body = CharArray(length)
            var read = 0
            while (read < length) read += input.read(body, read, length - read).takeIf { r -> r > 0 } ?: return
            if (n == 1) return // the connection dies under the next request
            val answer = """{"jsonrpc":"2.0","id":1,"result":"0x64"}"""
            it.getOutputStream().write("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: ${answer.length}\r\n\r\n$answer".toByteArray())
            it.getOutputStream().flush()
            answered.incrementAndGet()
        }
    }

    @After
    fun stop() = server.close()

    private val url get() = "http://127.0.0.1:${server.localPort}/100"

    @Test
    fun a_read_that_meets_a_dead_pooled_connection_is_asked_again_on_a_new_one() = runBlocking {
        val transport = OkHttpTransport()
        assertTrue(transport.post(url, "eth_chainId", emptyList(), null, 5_000).outcome is RpcTransportOutcome.Response)
        // The pooled connection is reused, dies under the request: asked again, answered.
        val second = transport.post(url, "eth_estimateUserOperationGas", emptyList(), null, 5_000)
        assertTrue("a stale connection is not an unreachable relay: ${second.outcome}", second.outcome is RpcTransportOutcome.Response)
        assertEquals(2, answered.get())
    }

    @Test
    fun a_submit_goes_on_a_connection_of_its_own() = runBlocking {
        val transport = OkHttpTransport()
        assertTrue(transport.post(url, "eth_chainId", emptyList(), null, 5_000).outcome is RpcTransportOutcome.Response)
        val before = connections.get()
        val submit = transport.post(url, "eth_sendUserOperation", emptyList(), null, 5_000)
        assertTrue("never the dead pooled one: ${submit.outcome}", submit.outcome is RpcTransportOutcome.Response)
        assertEquals("a new connection, asked once", before + 1, connections.get())
        assertTrue(OkHttpTransport.readOnly("eth_call") && OkHttpTransport.readOnly("pimlico_getUserOperationGasPrice"))
        assertTrue(!OkHttpTransport.readOnly("eth_sendUserOperation") && !OkHttpTransport.readOnly("eth_sendRawTransaction"))
    }
}
