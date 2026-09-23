package app.getvela.wallet.feature.signing.clearsigner

import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString
import java.util.concurrent.TimeUnit

/**
 * A WebSocket to the Clear Signer tunnel, as little of one as the requester
 * needs (spec 075, contracts/tunnel.md): text frames for the two hellos and the
 * tunnel's own `joined` / `left`, binary frames for everything after, and one
 * close.
 *
 * It is an interface because the tunnel half of the pairing has to be testable
 * without a network: the JVM suite runs the same requester against a fake
 * tunnel that implements tunnel.md §1 in-process.
 */
interface TunnelSockets {
    fun open(url: String, listener: TunnelListener): TunnelSocket
}

interface TunnelSocket {
    fun send(text: String)
    fun send(bytes: ByteArray)

    /** Normal close (1000). Idempotent. */
    fun close()
}

interface TunnelListener {
    fun onText(text: String)
    fun onBinary(bytes: ByteArray)

    /** The socket ended, for whatever reason — including one this end asked for. */
    fun onClosed(reason: String?)
}

/**
 * The app's own transport, on the OkHttp the wallet already ships.
 *
 * The tunnel pings every 30 s where its host lets it; this end pings too, so a
 * phone that dozes behind a NAT still finds out its socket is gone rather than
 * waiting out the five-minute clock on a connection nobody is on the other end
 * of.
 */
class OkHttpTunnelSockets(
    private val client: OkHttpClient = OkHttpClient.Builder()
        .pingInterval(30, TimeUnit.SECONDS)
        // A tunnel that cannot be reached must say so quickly: the person is
        // looking at a QR code with nothing happening.
        .connectTimeout(15, TimeUnit.SECONDS)
        // The socket is long-lived by design; the session's own clock ends it.
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .build(),
) : TunnelSockets {

    override fun open(url: String, listener: TunnelListener): TunnelSocket {
        val socket = client.newWebSocket(
            Request.Builder().url(url).build(),
            object : WebSocketListener() {
                override fun onMessage(webSocket: WebSocket, text: String) = listener.onText(text)

                override fun onMessage(webSocket: WebSocket, bytes: ByteString) =
                    listener.onBinary(bytes.toByteArray())

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) =
                    listener.onClosed("$code $reason")

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) =
                    listener.onClosed(t.message ?: response?.message)
            },
        )
        return object : TunnelSocket {
            override fun send(text: String) {
                socket.send(text)
            }

            override fun send(bytes: ByteArray) {
                socket.send(bytes.toByteString())
            }

            override fun close() {
                if (!socket.close(1000, null)) socket.cancel()
            }
        }
    }
}
