package app.getvela.wallet.feature.signing.clearsigner

import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject
import uniffi.vela_core_uniffi.ClearSignerHandshake
import uniffi.vela_core_uniffi.ClearSignerSession
import uniffi.vela_core_uniffi.clearSignerKeyFingerprint
import uniffi.vela_core_uniffi.clearSignerRelayLink
import uniffi.vela_core_uniffi.clearSignerRelayRoom
import uniffi.vela_core_uniffi.clearSignerRelayRoomUrl
import java.security.SecureRandom

/**
 * The wallet's side of a cross-device pairing (spec 075, contracts/relay.md):
 * a blind relay carries sealed frames between this phone and a Clear Signer
 * page open on another device.
 *
 * Everything that decides anything is the core's. This class draws the room id
 * and the handshake's randomness, opens a socket, and moves frames; the ECDH,
 * the HKDF, the six digits, the AES-GCM and every verdict come from
 * `ClearSignerHandshake` / `ClearSignerSession` and the `clearSignerVerify…`
 * functions — the same Rust the page's own `lib/transport/secure.js` is pinned
 * against by shared vectors.
 *
 * Two checks make the relay's blindness enough:
 * - the pairing link carries `rk`, the fingerprint of THIS wallet's key, so a
 *   relay (or anyone who guessed the room) cannot stand in for the wallet —
 *   the page refuses a hello that does not hash to it;
 * - the six-digit code is shown on both screens and the person confirms it
 *   **before the wallet sends anything**, so a stolen link cannot substitute a
 *   page. That matters most for a create, where a substituted page would hand
 *   the wallet somebody else's key.
 */
class ClearSignerRelayWire(
    /** The relay, as the preference holds it (`wss://…`). */
    private val relayUrl: String,
    /** The page the link opens — a key's own `signer_origin`, or Settings'. */
    private val signerUrl: String,
    private val sockets: RelaySockets,
    /** `vela-android/<version>`, for the page's own display. */
    private val appName: String,
    private val timeoutMs: Long,
    private val random: SecureRandom,
    /** The QR + copyable link, the moment there is one. */
    private val onLink: (link: String) -> Unit,
    /** Both ends derived this code; `false` when the person did not confirm it. */
    private val confirmCode: suspend (code: String) -> Boolean,
) : ClearSignerWire {

    private val handshake: ClearSignerHandshake
    private val room: String
    private val link: String
    private val roomUrl: String

    init {
        val secret = ByteArray(32).also(random::nextBytes)
        val nonce = ByteArray(16).also(random::nextBytes)
        handshake = ClearSignerHandshake(secret, nonce)
        room = clearSignerRelayRoom(ByteArray(16).also(random::nextBytes))
            ?: error("a 16-byte room id is always a room id")
        val rk = clearSignerKeyFingerprint(handshake.publicKey())
        link = clearSignerRelayLink(signerUrl, relayUrl, room, rk)
        roomUrl = clearSignerRelayRoomUrl(relayUrl, room)
    }

    private sealed interface Incoming {
        data class Text(val text: String) : Incoming
        data class Binary(val bytes: ByteArray) : Incoming {
            override fun equals(other: Any?) = this === other
            override fun hashCode() = System.identityHashCode(this)
        }

        data class Closed(val reason: String?) : Incoming
    }

    private val inbox = Channel<Incoming>(Channel.UNLIMITED)

    @Volatile
    private var socket: RelaySocket? = null

    @Volatile
    private var session: ClearSignerSession? = null

    @Volatile
    private var over = false

    @Volatile
    private var cancelled = false

    /** The page left, or the socket did: nothing was signed, and nothing will be. */
    @Volatile
    private var gone = false

    /** The session's sequence: own sent and peer received, as PROTOCOL §4 says. */
    private val envelopes = ClearSignerEnvelopes()

    override suspend fun ask(ask: ClearSignerAsk): ClearSignerAnswer {
        if (over) return ClearSignerAnswer.Cancelled
        if (session == null) pair()?.let { return it }
        val live = session ?: return ClearSignerAnswer.Cancelled
        val id = nextId()
        val envelope = envelopes.intent(id, ask.requestJson)
        if (!seal(live, envelope)) return ClearSignerAnswer.Cancelled
        val answer = awaitAnswer(live, id) ?: return timedOutOrCancelled()
        return judgeClearSignerAnswer(ask, answer, signerUrl)
    }

    override fun cancel() {
        cancelled = true
        // Waking the reader is enough: every wait treats a closed channel the
        // same as the page going away.
        inbox.trySend(Incoming.Closed("cancelled"))
    }

    override fun end() {
        if (over) return
        over = true
        session?.let { live -> runCatching { seal(live, envelopes.bye()) } }
        runCatching { socket?.close() }
        socket = null
        cancelled = true
        inbox.trySend(Incoming.Closed("ended"))
        inbox.close()
    }

    // -- pairing --------------------------------------------------------------

    /**
     * Connect, run the handshake, and wait on the code until the person says
     * the other device shows the same one. **Nothing of the request leaves
     * this device before that.** `null` means the pair succeeded; anything
     * else is the answer the caller owes its request.
     */
    private suspend fun pair(): ClearSignerAnswer? {
        onLink(link)
        val opened = runCatching {
            sockets.open(
                roomUrl,
                object : RelayListener {
                    override fun onText(text: String) {
                        inbox.trySend(Incoming.Text(text))
                    }

                    override fun onBinary(bytes: ByteArray) {
                        inbox.trySend(Incoming.Binary(bytes))
                    }

                    override fun onClosed(reason: String?) {
                        inbox.trySend(Incoming.Closed(reason))
                    }
                },
            )
        }.getOrElse { error ->
            return ClearSignerAnswer.Unreachable(error.message ?: "the relay could not be reached")
        }
        socket = opened

        // The relay says when both ends are in the room; only then does either
        // start its handshake (relay.md §1 — there is no buffering, so a frame
        // sent before that is simply dropped).
        var joined = false
        var peerHello: String? = null
        while (peerHello == null) {
            when (val next = receive() ?: return timedOutOrCancelled()) {
                is Incoming.Closed -> return if (cancelled) {
                    ClearSignerAnswer.Cancelled
                } else {
                    ClearSignerAnswer.Unreachable(next.reason ?: "closed")
                }
                is Incoming.Binary -> Unit // Nothing is sealed before the hellos.
                is Incoming.Text -> {
                    val message = runCatching { JSONObject(next.text) }.getOrNull() ?: continue
                    when {
                        message.has("relay") -> {
                            // `joined` / `left`: the relay's own, never an end's.
                            joined = message.optString("relay") == "joined"
                        }
                        joined && message.optString("t") == "hello" -> peerHello = next.text
                        else -> Unit
                    }
                }
            }
        }
        // Our own hello answers the page's, then both sides derive the same key
        // and the same six digits from the two nonces and the two keys.
        opened.send(handshake.hello(appName.ifEmpty { null }))
        val live = runCatching { handshake.complete(peerHello, true) }.getOrElse { error ->
            VelaLog.failure("clearsigner.relay", "handshake refused", error)
            return ClearSignerAnswer.Unreachable(error.message ?: "the handshake failed")
        }
        val confirmed = withTimeoutOrNull(timeoutMs) { confirmCode(live.code()) } ?: false
        if (!confirmed) {
            end()
            return ClearSignerAnswer.Cancelled
        }
        session = live
        return null
    }

    // -- frames ---------------------------------------------------------------

    private fun seal(live: ClearSignerSession, plaintext: String): Boolean {
        val out = socket ?: return false
        return runCatching {
            out.send(live.seal(plaintext.toByteArray(Charsets.UTF_8), null))
            true
        }.getOrDefault(false)
    }

    /** The page's next sealed message for [id] — opened, checked, as JSON. */
    private suspend fun awaitAnswer(live: ClearSignerSession, id: String): JSONObject? {
        while (true) {
            when (val next = receive() ?: return null) {
                is Incoming.Closed -> {
                    gone = true
                    return null
                }
                is Incoming.Text -> {
                    // After the hellos, an end's frames are binary. The relay's
                    // own `left` means the page went away.
                    val message = runCatching { JSONObject(next.text) }.getOrNull() ?: continue
                    if (message.optString("relay") == "left") {
                        gone = true
                        return null
                    }
                }
                is Incoming.Binary -> {
                    val opened = runCatching { live.open(next.bytes, null) }.getOrElse { error ->
                        // A replay, a reflection or a tampered frame. The
                        // session's counters never go backwards, so the only
                        // safe thing is to stop.
                        VelaLog.failure("clearsigner.relay", "sealed frame refused", error)
                        gone = true
                        return null
                    }
                    val message = runCatching { JSONObject(String(opened, Charsets.UTF_8)) }
                        .getOrNull() ?: continue
                    // PROTOCOL §4: `n` only ever rises.
                    if (!envelopes.accept(message.optLong("n", 0))) continue
                    when (message.optString("t")) {
                        "bye" -> {
                            gone = true
                            return null
                        }
                        "result", "error" -> if (message.optString("id") == id) return message
                        else -> Unit
                    }
                }
            }
        }
    }

    private suspend fun receive(): Incoming? =
        withTimeoutOrNull(timeoutMs) { runCatching { inbox.receive() }.getOrNull() }

    /**
     * Nothing came back. A person who cancelled, a page that left and a socket
     * that died are all "closed without signing"; only silence is a timeout.
     */
    private fun timedOutOrCancelled(): ClearSignerAnswer =
        if (cancelled || gone || over) ClearSignerAnswer.Cancelled else ClearSignerAnswer.TimedOut

    private fun nextId(): String =
        ByteArray(8).also(random::nextBytes).joinToString("") { "%02x".format(it) }
}
