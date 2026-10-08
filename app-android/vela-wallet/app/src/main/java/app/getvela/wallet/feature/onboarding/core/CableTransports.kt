package app.getvela.wallet.feature.onboarding.core

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothSocket
import app.getvela.wallet.core.diagnostics.VelaLog
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString
import uniffi.vela_core_uniffi.CableFrameOutcome
import uniffi.vela_core_uniffi.CableFramePort
import java.io.DataInputStream
import java.io.IOException
import java.io.OutputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * The two caBLE data channels, as one message-oriented connection — a port of
 * the founder's proven demo (`apppasskeysdemo/transport/ble/CableConn.kt`).
 * Everything the frames MEAN (Noise, the transport cipher, CTAP) lives in
 * vela-core behind [CableFramePort]; these classes move bytes and nothing else.
 *
 *  * [L2capCableConn]     — CTAP 2.3 local BLE (L2CAP CoC), 4-byte big-endian
 *    length prefix per message. No tunnel server, no internet.
 *  * [WebSocketCableConn] — CTAP 2.2 WebSocket tunnel, one binary WS frame per
 *    message, subprotocol `fido.cable`.
 */
interface CableConn {
    val channel: String
    fun writeFrame(bytes: ByteArray)
    fun readFrame(): ByteArray
    fun close()
}

/** A [CableConn] as the Rust core's frame port. */
class CableConnPort(private val conn: CableConn) : CableFramePort {
    override fun writeFrame(frame: ByteArray): String? = try {
        conn.writeFrame(frame)
        null
    } catch (error: Exception) {
        error.message ?: "write failed"
    }

    override fun readFrame(): CableFrameOutcome = try {
        CableFrameOutcome.Frame(conn.readFrame())
    } catch (error: java.net.SocketTimeoutException) {
        CableFrameOutcome.TimedOut
    } catch (error: Exception) {
        CableFrameOutcome.Failed(error.message ?: "read failed")
    }

    override fun channel(): String = conn.channel
}

/** BLE L2CAP CoC connection; each message is a 4-byte big-endian length prefix + payload. */
class L2capCableConn private constructor(private val socket: BluetoothSocket) : CableConn {
    override val channel = "L2CAP"
    private val input = DataInputStream(socket.inputStream)
    private val output: OutputStream = socket.outputStream

    override fun writeFrame(bytes: ByteArray) {
        val header = byteArrayOf(
            (bytes.size ushr 24).toByte(),
            (bytes.size ushr 16).toByte(),
            (bytes.size ushr 8).toByte(),
            bytes.size.toByte(),
        )
        synchronized(output) {
            output.write(header)
            output.write(bytes)
            output.flush()
        }
    }

    override fun readFrame(): ByteArray {
        // A Bluetooth stream read has no SO_TIMEOUT; closing the socket is the
        // one way to unblock it. Without this, a responder that dies silently
        // would pin the Rust ceremony thread forever — the WebSocket tunnel and
        // both iOS channels already carry the same 130s ceremony-budget guard.
        val fired = java.util.concurrent.atomic.AtomicBoolean(false)
        val watchdog = WATCHDOG.schedule({
            fired.set(true)
            runCatching { socket.close() }
        }, READ_TIMEOUT_S, TimeUnit.SECONDS)
        try {
            val len = input.readInt()
            if (len < 0 || len > MAX_FRAME) throw IOException("bad L2CAP frame length $len")
            return ByteArray(len).also { input.readFully(it) }
        } catch (error: IOException) {
            if (fired.get()) throw java.net.SocketTimeoutException("the L2CAP channel went silent")
            throw error
        } finally {
            watchdog.cancel(false)
        }
    }

    override fun close() {
        runCatching { socket.close() }
    }

    companion object {
        /** The most a frame may claim, so a corrupt prefix cannot allocate the world. */
        private const val MAX_FRAME = 1 shl 20

        /** One read may wait this long — the CTAP user-presence budget and then
         *  some, matching the tunnel's. */
        private const val READ_TIMEOUT_S = 130L

        /** Daemon timer shared by every L2CAP read; a fired task only closes a
         *  socket, so one thread serves all ceremonies. */
        private val WATCHDOG =
            java.util.concurrent.Executors.newSingleThreadScheduledExecutor { runnable ->
                Thread(runnable, "l2cap-read-watchdog").apply { isDaemon = true }
            }

        @SuppressLint("MissingPermission")
        fun connect(device: BluetoothDevice, psm: Int): L2capCableConn {
            val socket = device.createInsecureL2capChannel(psm)
            socket.connect()
            VelaLog.event("cable.ble", "L2CAP CoC connected", "psm" to psm)
            return L2capCableConn(socket)
        }
    }
}

/** WebSocket tunnel connection: one binary WS frame per message. */
class WebSocketCableConn private constructor(
    private val ws: WebSocket,
    private val client: OkHttpClient,
    private val inbound: LinkedBlockingQueue<Any>,
) : CableConn {
    override val channel = "WebSocket"

    override fun writeFrame(bytes: ByteArray) {
        if (!ws.send(bytes.toByteString())) throw IOException("the tunnel has closed")
    }

    override fun readFrame(): ByteArray {
        // Long-lived: the person may take a while approving on the other phone.
        val item = inbound.poll(READ_TIMEOUT_S, TimeUnit.SECONDS)
            ?: throw java.net.SocketTimeoutException("the tunnel went silent")
        return when (item) {
            is ByteArray -> item
            else -> throw IOException("the tunnel closed")
        }
    }

    override fun close() {
        runCatching { ws.close(1000, null) }
        runCatching { client.dispatcher.executorService.shutdown() }
        // A read blocked on the queue ends now, not when the relay
        // acknowledges the close: closing is how a dismissal (issue #459)
        // unblocks an exchange that is waiting on the other phone.
        inbound.offer(CLOSED)
    }

    companion object {
        private val CLOSED = Any()
        private const val SUBPROTOCOL = "fido.cable"

        /** How long one read may wait — the CTAP user-presence budget and then some. */
        private const val READ_TIMEOUT_S = 130L

        fun connect(url: String, timeoutMs: Long): WebSocketCableConn {
            val client = OkHttpClient.Builder()
                .connectTimeout(timeoutMs, TimeUnit.MILLISECONDS)
                .readTimeout(0, TimeUnit.MILLISECONDS) // long-lived tunnel
                // The relay keeps the pair alive with pings and kills BOTH legs
                // when one stops answering.
                .pingInterval(30, TimeUnit.SECONDS)
                .followRedirects(true)
                .build()
            val request = Request.Builder()
                .url(url)
                .header("Sec-WebSocket-Protocol", SUBPROTOCOL)
                .build()

            val inbound = LinkedBlockingQueue<Any>()
            val openLatch = CountDownLatch(1)
            val failure = AtomicReference<Throwable?>()

            val listener = object : WebSocketListener() {
                override fun onOpen(webSocket: WebSocket, response: Response) = openLatch.countDown()
                override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                    inbound.put(bytes.toByteArray())
                }
                // Only binary frames are caBLE; text is tunnel housekeeping.
                override fun onMessage(webSocket: WebSocket, text: String) {}
                override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                    inbound.put(CLOSED)
                }
                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    inbound.put(CLOSED)
                }
                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    failure.set(t)
                    openLatch.countDown()
                    inbound.put(CLOSED)
                }
            }

            val ws = client.newWebSocket(request, listener)
            if (!openLatch.await(timeoutMs, TimeUnit.MILLISECONDS)) {
                ws.cancel()
                throw IOException("the tunnel would not open in time")
            }
            failure.get()?.let {
                ws.cancel()
                throw IOException("the tunnel would not open: ${it.message}", it)
            }
            VelaLog.event("cable.tunnel", "WebSocket tunnel established")
            return WebSocketCableConn(ws, client, inbound)
        }
    }
}
