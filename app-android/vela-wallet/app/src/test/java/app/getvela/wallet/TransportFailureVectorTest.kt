package app.getvela.wallet

import app.getvela.wallet.feature.wallet.core.OkHttpTransport
import app.getvela.wallet.feature.wallet.core.RpcTransportOutcome
import java.io.File
import java.io.IOException
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * r11-9: "not sent" versus "may have been sent" is the core's rule, from the
 * failure CLASS the shell passes (`transport_outcome_of`). The round-11 send
 * failed in 4–6 ms against the relay behind `adb reverse` and said "It may
 * have been sent… don't send it again" though nothing had left the device.
 * The shared vector `tests/vectors/rules/transport-failures.json`, replayed
 * through this shell's classifier: Android's errors, at the stage okhttp's
 * event listener saw them, land in the class the vector names.
 */
class TransportFailureVectorTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val vector = JSONObject(File(repoRoot, "rust/crates/vela-core/tests/vectors/rules/transport-failures.json").readText())

    private fun outcomeType(outcome: RpcTransportOutcome) = when (outcome) {
        RpcTransportOutcome.NotConnected -> "not_connected"
        RpcTransportOutcome.Timeout -> "timeout"
        RpcTransportOutcome.Network -> "network"
        else -> outcome.toString()
    }

    @Test
    fun every_class_means_what_the_core_says() {
        val cases = vector.getJSONArray("cases")
        for (i in 0 until cases.length()) {
            val case = cases.getJSONObject(i)
            val failure = case.getString("failure")
            assertEquals(failure, case.getJSONObject("outcome").getString("type"), outcomeType(OkHttpTransport.outcomeOfClass(failure)))
        }
    }

    private fun stages(connected: Boolean, written: Boolean, reused: Boolean = false) =
        OkHttpTransport.CallStages().apply {
            this.connected = connected
            this.bodyWritten = written
            this.reused = reused
        }

    private fun classOf(failure: IOException, connected: Boolean, written: Boolean, reused: Boolean = false) =
        OkHttpTransport.failureClass(failure, stages(connected, written, reused))

    /** The vector's `platform.android` guidance, as this shell's errors meet it. */
    @Test
    fun android_errors_land_in_their_class() {
        assertEquals("dns", classOf(java.net.UnknownHostException("relay.test"), connected = false, written = false))
        assertEquals("connect_refused", classOf(java.net.ConnectException("Failed to connect to /127.0.0.1:12490"), connected = false, written = false))
        assertEquals("connect_refused", classOf(java.net.NoRouteToHostException("no route"), connected = false, written = false))
        assertEquals("connect_timeout", classOf(java.net.SocketTimeoutException("connect timed out"), connected = false, written = false))
        assertEquals("tls", classOf(javax.net.ssl.SSLHandshakeException("handshake"), connected = false, written = false))
        // Broken while the request was being written: it never left whole.
        assertEquals("write_failed", classOf(java.net.SocketException("Broken pipe"), connected = true, written = false))
        // Broken after it was written whole: it may have been acted on.
        assertEquals("after_write", classOf(IOException("unexpected end of stream on http://127.0.0.1:12490/..."), connected = true, written = true, reused = true))
        assertEquals("after_write", classOf(java.net.SocketException("Connection reset"), connected = true, written = true))
        assertEquals("read_timeout", classOf(java.net.SocketTimeoutException("timeout"), connected = true, written = true))
        // okhttp's call timeout, at the stage it struck.
        assertEquals("connect_timeout", classOf(java.io.InterruptedIOException("timeout"), connected = false, written = false))
        assertEquals("read_timeout", classOf(java.io.InterruptedIOException("timeout"), connected = true, written = true))

        // Sent / not sent, the core's words for each.
        assertEquals(RpcTransportOutcome.NotConnected, OkHttpTransport.outcomeOfClass("write_failed"))
        assertEquals(RpcTransportOutcome.Network, OkHttpTransport.outcomeOfClass("after_write"))
        assertEquals(RpcTransportOutcome.Timeout, OkHttpTransport.outcomeOfClass("read_timeout"))
    }
}
