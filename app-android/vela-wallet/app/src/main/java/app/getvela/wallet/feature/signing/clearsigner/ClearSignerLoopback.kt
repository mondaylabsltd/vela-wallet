package app.getvela.wallet.feature.signing.clearsigner

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.ClearSignerConnection
import uniffi.vela_core_uniffi.ClearSignerStep
import uniffi.vela_core_uniffi.clearSignerWsLaunch
import java.io.IOException
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.security.SecureRandom
import java.util.Base64
import java.util.concurrent.ConcurrentLinkedQueue

/**
 * The phones' channel to a Clear Signer on THIS device (spec 071 contract §2,
 * extended to sessions by spec 075): a socket on the wallet's own loopback,
 * and the page in a Custom Tab over the app.
 *
 * This class owns I/O and nothing else. Who may connect (the page's origin,
 * the one-time token), what the bytes mean (RFC 6455) and whether an answer is
 * acceptable (this digest, one of these keys, this ceremony's own rules) are
 * all `ClearSignerConnection`'s — the core's, the same code the page's own
 * suite runs over wasm. A connection that never proves itself changes nothing;
 * the listener keeps listening until one has an outcome, the person cancels,
 * or five minutes pass.
 *
 * **One listener per flow, not per request.** A create mints a key and then
 * asks the same page to confirm its membership; a recovery asks for two
 * signatures. Opening the page again between them would lose the WebAuthn
 * context the person is looking at and make them re-approve a tab they never
 * left — so after an answer the socket stays up and the next request goes down
 * it ([ask] again), until [end].
 */
class ClearSignerLoopback(
    /** The page to open — a `signer_origin`, or the person's own from Settings. */
    private val base: String,
    /** Open [url] in a Custom Tab over the app; `false` when nothing is on screen to open it from. */
    private val openPage: (url: String) -> Boolean,
    private val timeoutMs: Long,
    private val random: SecureRandom,
    /** Called whenever the page's address changes (the waiting sheet's "open again"). */
    private val onOpened: (url: String) -> Unit = {},
) : ClearSignerWire {

    private val server: ServerSocket =
        ServerSocket(0, 8, InetAddress.getByAddress(byteArrayOf(127, 0, 0, 1)))
    private val token: String =
        Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(16).also(random::nextBytes))

    /** The page's address: same port, same token, for as long as the flow lasts. */
    val url: String = clearSignerWsLaunch(base, server.localPort.toUShort(), token)

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val sockets = ConcurrentLinkedQueue<Socket>()

    /** The connection (and its socket) that proved itself and answered. */
    @Volatile
    private var held: Held? = null

    @Volatile
    private var pending: CompletableDeferred<ClearSignerAnswer>? = null

    @Volatile
    private var over = false

    private var opened = false

    private class Held(val connection: ClearSignerConnection, val socket: Socket)

    override suspend fun ask(ask: ClearSignerAsk): ClearSignerAnswer {
        if (over) return ClearSignerAnswer.Cancelled
        val done = CompletableDeferred<ClearSignerAnswer>()
        pending = done
        val live = held
        if (live == null) {
            // The first request: listen, then send the person to the page.
            if (!opened) {
                opened = true
                scope.launch { accept(ask) }
                onOpened(url)
                if (!openPage(url)) {
                    end()
                    return ClearSignerAnswer.Cancelled
                }
            }
        } else {
            // A later request of the same flow, down the socket that answered.
            val id = nextId()
            val step = runCatching {
                when (ask) {
                    is ClearSignerAsk.Signature ->
                        live.connection.sendSignature(id, ask.requestJson, ask.digest, ask.keys)
                    is ClearSignerAsk.Ceremony -> live.connection.sendCeremony(
                        id,
                        ask.requestJson,
                        ask.operationJson,
                        ask.expectedMemberChallenge,
                    )
                }
            }.getOrElse {
                end()
                return ClearSignerAnswer.Cancelled
            }
            // An empty step means the connection was not waiting for one —
            // the page went away between requests. Nothing to wait for.
            if (step.write.isEmpty()) {
                end()
                return ClearSignerAnswer.Cancelled
            }
            if (!write(live.socket, step)) {
                end()
                return ClearSignerAnswer.Cancelled
            }
        }
        val answer = withTimeoutOrNull(timeoutMs) { done.await() } ?: ClearSignerAnswer.TimedOut
        pending = null
        if (answer is ClearSignerAnswer.TimedOut || answer is ClearSignerAnswer.Cancelled) end()
        return answer
    }

    override fun cancel() {
        pending?.complete(ClearSignerAnswer.Cancelled)
    }

    override fun reopen() {
        if (!over) openPage(url)
    }

    override fun end() {
        if (over) return
        over = true
        held?.let { live ->
            runCatching { write(live.socket, live.connection.end()) }
            runCatching { live.connection.close() }
        }
        // A blocked accept/read only lets go when its socket closes.
        runCatching { server.close() }
        sockets.forEach { runCatching { it.close() } }
        pending?.complete(ClearSignerAnswer.Cancelled)
        scope.cancel()
    }

    /** Every connection until one has an outcome; then this flow's own. */
    private fun accept(first: ClearSignerAsk) {
        // Until one connection has an outcome, any page that proves itself may
        // take the flow ("open the page again"). After that the session belongs
        // to the one that answered, and nothing else is let in.
        while (scope.isActive && held == null) {
            val socket = try {
                server.accept()
            } catch (_: IOException) {
                break
            }
            sockets += socket
            val connection = runCatching { connectionFor(first) }.getOrNull()
            if (connection == null) {
                runCatching { socket.close() }
                break
            }
            scope.launch { serve(socket, connection) }
        }
    }

    private fun connectionFor(ask: ClearSignerAsk): ClearSignerConnection = when (ask) {
        is ClearSignerAsk.Signature ->
            ClearSignerConnection(base, token, nextId(), ask.requestJson, ask.digest, ask.keys)
        is ClearSignerAsk.Ceremony -> ClearSignerConnection.newCeremony(
            base,
            token,
            nextId(),
            ask.requestJson,
            ask.operationJson,
            ask.expectedMemberChallenge,
        )
    }

    /**
     * One accepted connection, byte for byte through the core, for as long as
     * the flow lasts. A step that carries a verdict settles whatever request is
     * waiting and the loop keeps reading: the next request of the same flow
     * arrives on this socket.
     */
    private fun serve(socket: Socket, connection: ClearSignerConnection) {
        try {
            socket.soTimeout = 0
            val input = socket.getInputStream()
            val buffer = ByteArray(16 * 1024)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                val step = connection.feed(buffer.copyOf(read))
                if (!write(socket, step)) break
                val verdict = step.outcome?.let { ClearSignerAnswer.Signed(it) }
                    ?: step.ceremony?.let { ClearSignerAnswer.Ceremonial(it) }
                if (verdict != null) {
                    // Whichever connection answers first IS the session's.
                    if (held == null) held = Held(connection, socket)
                    pending?.complete(verdict)
                }
                if (step.close) break
            }
        } catch (_: IOException) {
            // The page went away mid-frame, or we closed it: `closed()` says
            // which it means.
        } finally {
            // `closed()` is `Declined` for a page that HELD the request and went
            // away, and nothing for one that never proved itself. Either way a
            // page that leaves mid-flow leaves nothing signed, which is the
            // cancelled ceremony the signing paths already know how to tell.
            val declined = runCatching { connection.closed() }.getOrNull() != null
            if (declined || held?.socket === socket) {
                pending?.complete(ClearSignerAnswer.Cancelled)
            }
            if (held?.socket !== socket) runCatching { connection.close() }
            runCatching { socket.close() }
        }
    }

    private fun write(socket: Socket, step: ClearSignerStep): Boolean = runCatching {
        if (step.write.isNotEmpty()) {
            val output = socket.getOutputStream()
            output.write(step.write)
            output.flush()
        }
        true
    }.getOrDefault(false)

    private fun nextId(): String =
        ByteArray(8).also(random::nextBytes).joinToString("") { "%02x".format(it) }
}
