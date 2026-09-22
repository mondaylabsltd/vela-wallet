package app.getvela.wallet.feature.signing.clearsigner

import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject
import uniffi.vela_core_uniffi.ClearSignerBleMessage
import uniffi.vela_core_uniffi.ClearSignerFramer
import uniffi.vela_core_uniffi.ClearSignerHandshake
import uniffi.vela_core_uniffi.ClearSignerReassembler
import uniffi.vela_core_uniffi.ClearSignerSession
import java.security.SecureRandom

/**
 * The wallet's side of a BLE pairing (spec 075 T040, PROTOCOL.md §1–4 and
 * §11): this phone advertises as a GATT peripheral and a Clear Signer page
 * open in a nearby Chrome connects to it as the central.
 *
 * Everything that decides anything belongs to the core. This class draws the
 * handshake's randomness, drives the radio, and moves frames; the six-byte
 * header, the reassembly and the wrapping `msgId` are `ClearSignerFramer` /
 * `ClearSignerReassembler`, the ECDH, the HKDF, the six digits and the
 * AES-GCM are `ClearSignerHandshake` / `ClearSignerSession`, and every verdict
 * is `clearSignerVerify…` — the same Rust the page's own `lib/transport/ble.js`
 * and `secure.js` are pinned against by shared vectors.
 *
 * What makes proximity enough:
 *
 * - **There is no link to steal.** Unlike the relay there is no `rk` and no
 *   pairing URL; the attacker has to be inside radio range.
 * - **The six-digit code is confirmed before anything is sent.** Both ends
 *   derive it from the two public keys and the two nonces, so it agrees only
 *   when nobody is in the middle — and an attacker who completes ECDH with
 *   each end separately gets two different codes. PROTOCOL §3 puts it
 *   plainly: a BLE channel without the comparison code is a transfer sent in
 *   the clear.
 *
 * One connection carries several requests (§11) — a create then its member
 * proof, a recovery's two signatures — and [end] says `bye`.
 */
class ClearSignerBleWire(
    private val radio: BlePeripheral,
    /** The page the answer must have come from — a key's own, or Settings'. */
    private val signerUrl: String,
    /** `vela-android/<version>`, for the page's own display. */
    private val appName: String,
    private val timeoutMs: Long,
    random: SecureRandom,
    private val now: () -> Long = System::currentTimeMillis,
    /** Advertising has started under this name; the card tells the person to look for it. */
    private val onAdvertising: (deviceName: String) -> Unit,
    /** Both ends derived this code; `false` when the person did not confirm it. */
    private val confirmCode: suspend (code: String) -> Boolean,
) : ClearSignerWire {

    private val handshake: ClearSignerHandshake
    private val framer = ClearSignerFramer()
    private val reassembler = ClearSignerReassembler()
    private val envelopes = ClearSignerEnvelopes()
    private val ids = random

    init {
        val secret = ByteArray(32).also(random::nextBytes)
        val nonce = ByteArray(16).also(random::nextBytes)
        handshake = ClearSignerHandshake(secret, nonce)
        // Until the central negotiates, 23 is the only MTU BLE promises. The
        // hello can go out before `onMtuChanged` arrives, and a hello the link
        // truncated is a handshake that never happens — so the frames start
        // small and grow, rather than starting at a size nothing guarantees.
        framer.fitToMtu(DEFAULT_MTU)
    }

    private sealed interface Incoming {
        class Frame(val bytes: ByteArray) : Incoming
        class Mtu(val mtu: Int) : Incoming
        class Gone(val reason: String) : Incoming
    }

    private val inbox = Channel<Incoming>(Channel.UNLIMITED)

    private val events = object : BlePeripheralEvents {
        override fun onFrame(bytes: ByteArray) {
            inbox.trySend(Incoming.Frame(bytes))
        }

        override fun onMtu(mtu: Int) {
            inbox.trySend(Incoming.Mtu(mtu))
        }

        override fun onGone(reason: String) {
            inbox.trySend(Incoming.Gone(reason))
        }
    }

    @Volatile
    private var session: ClearSignerSession? = null

    @Volatile
    private var over = false

    @Volatile
    private var cancelled = false

    /** The page left, or the radio did: nothing was signed, and nothing will be. */
    @Volatile
    private var gone = false

    override suspend fun ask(ask: ClearSignerAsk): ClearSignerAnswer {
        if (over) return ClearSignerAnswer.Cancelled
        if (session == null) pair()?.let { return it }
        val live = session ?: return ClearSignerAnswer.Cancelled
        val id = nextRequestId()
        if (!seal(live, envelopes.intent(id, ask.requestJson))) return ClearSignerAnswer.Cancelled
        val answer = awaitAnswer(live, id) ?: return timedOutOrCancelled()
        return judgeClearSignerAnswer(ask, answer, signerUrl)
    }

    override fun cancel() {
        cancelled = true
        // Waking the reader is enough: every wait treats the page going away
        // and the person cancelling the same way.
        inbox.trySend(Incoming.Gone("cancelled"))
    }

    override fun end() {
        if (over) return
        over = true
        cancelled = true
        val live = session
        // `bye` is a courtesy to a page that may already be gone, and one
        // notification can take a moment to leave the stack. The caller — a
        // screen the person just left, as often as not — does not wait for it.
        Thread {
            runCatching { runBlocking { live?.let { seal(it, envelopes.bye()) } } }
            runCatching { radio.stop() }
        }.apply { isDaemon = true }.start()
        inbox.trySend(Incoming.Gone("ended"))
        inbox.close()
    }

    // -- pairing --------------------------------------------------------------

    /**
     * Advertise, answer the page's hello, and wait on the code until the
     * person says the browser shows the same six digits. **Nothing of the
     * request leaves this phone before that.** `null` means the pair
     * succeeded; anything else is the answer the caller owes its request.
     */
    private suspend fun pair(): ClearSignerAnswer? {
        val advertising = runCatching { radio.start(events) }.getOrElse { error ->
            VelaLog.failure("clearsigner.ble", "the peripheral would not start", error)
            false
        }
        if (!advertising) {
            return ClearSignerAnswer.Unreachable("this phone would not advertise")
        }
        onAdvertising(radio.deviceName)

        // PROTOCOL §3: the central speaks first, in the clear.
        var peerHello: String? = null
        while (peerHello == null) {
            when (val next = receive() ?: return timedOutOrCancelled()) {
                is Incoming.Gone -> return if (cancelled) {
                    ClearSignerAnswer.Cancelled
                } else {
                    ClearSignerAnswer.Unreachable(next.reason)
                }
                is Incoming.Mtu -> fitChunk(next.mtu)
                is Incoming.Frame -> {
                    val message = take(next.bytes) ?: continue
                    // Nothing is sealed before the hellos. A sealed message
                    // here is a stray from a session this end knows nothing
                    // about, and there is no key to open it with anyway.
                    if (message.sealed) continue
                    val text = String(message.payload, Charsets.UTF_8)
                    val json = runCatching { JSONObject(text) }.getOrNull() ?: continue
                    if (json.optString("t") == "hello") peerHello = text
                }
            }
        }

        if (!sendPlain(handshake.hello(appName.ifEmpty { null }))) {
            return ClearSignerAnswer.Unreachable("the wallet's hello would not go out")
        }
        // `relay = false` picks the BLE label (`vela-ble/1`), whose AAD binds
        // each message's frame id rather than the session's counter.
        val live = runCatching { handshake.complete(peerHello, false) }.getOrElse { error ->
            VelaLog.failure("clearsigner.ble", "handshake refused", error)
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

    /**
     * Size the frames for the link the central negotiated.
     *
     * The arithmetic is the core's (`Framer::fit_to_mtu`), and it is not the
     * obvious one: the default chunk of 244 is what the PAGE writes with, and
     * 244 plus the six-byte header is a 250-byte value an MTU-247 link cannot
     * carry. A central survives that because the OS turns an oversized write
     * into a long write. **A notify cannot be split** —
     * `notifyCharacteristicChanged` hands the stack the whole value and the
     * link truncates it — and every answer this route carries goes out over
     * notify, so a frame that does not fit is a signature that never arrives.
     */
    private fun fitChunk(mtu: Int) {
        val fitted = runCatching { framer.fitToMtu(mtu.toUInt()) }.getOrNull() ?: return
        VelaLog.event("clearsigner.ble", "the link was sized", "mtu" to mtu.toString(), "chunk" to fitted.toString())
    }

    /** One frame into the core's reassembler; non-null when a message is whole. */
    private fun take(frame: ByteArray): ClearSignerBleMessage? = runCatching {
        // A message whose remaining frames never arrived is dropped rather
        // than waited out, and said out loud — a half-arrived intent that sat
        // in memory would come back as somebody else's `msgId` later.
        for (id in reassembler.sweep(now().toULong())) {
            VelaLog.event("clearsigner.ble", "a message never completed", "msgId" to (id.toInt() and 0xff).toString())
        }
        reassembler.accept(frame, now().toULong())
    }.getOrNull()

    /**
     * One whole message out on `p2c`.
     *
     * The size is [fitChunk]'s business, not this one's: by the time a frame
     * is built it already fits the negotiated link. [halve] here is the
     * fallback for the case the MTU did not predict — a stack that refuses
     * the value anyway — and it sends the WHOLE message again, because the
     * central's reassembler (like ours) starts a message over when its
     * `total` disagrees with what it already holds. Down to the core's floor,
     * and then it is a failure rather than a loop.
     */
    private suspend fun push(msgId: UByte, payload: ByteArray, sealed: Boolean): Boolean =
        withContext(Dispatchers.IO) {
            var delivered = false
            while (!delivered) {
                val frames = runCatching { framer.frames(msgId, payload, sealed) }.getOrNull() ?: break
                delivered = frames.all { radio.notify(it) }
                if (!delivered && !runCatching { framer.halve() }.getOrDefault(false)) {
                    VelaLog.event("clearsigner.ble", "a frame would not go out at the smallest chunk")
                    break
                }
            }
            delivered
        }

    /** A handshake hello, in the clear (`flags` bit 0 = 0). */
    private suspend fun sendPlain(text: String): Boolean =
        push(framer.nextId(), text.toByteArray(Charsets.UTF_8), sealed = false)

    /**
     * One sealed message. The id is taken BEFORE sealing: the session binds it
     * into the AAD (`secure::Tail::MsgId`) and the frames must carry the very
     * same one, or the page will refuse to open what it reassembles.
     */
    private suspend fun seal(live: ClearSignerSession, plaintext: String): Boolean {
        val msgId = runCatching { framer.nextId() }.getOrNull() ?: return false
        val body = runCatching { live.seal(plaintext.toByteArray(Charsets.UTF_8), msgId) }.getOrNull()
            ?: return false
        return push(msgId, body, sealed = true)
    }

    /** The page's next sealed message for [id] — opened, checked, as JSON. */
    private suspend fun awaitAnswer(live: ClearSignerSession, id: String): JSONObject? {
        while (true) {
            when (val next = receive() ?: return null) {
                is Incoming.Gone -> {
                    gone = true
                    return null
                }
                is Incoming.Mtu -> fitChunk(next.mtu)
                is Incoming.Frame -> {
                    val message = take(next.bytes) ?: continue
                    // The hellos are over; a plaintext message now is noise.
                    if (!message.sealed) continue
                    val opened = runCatching { live.open(message.payload, message.msgId) }.getOrElse { error ->
                        // A replay, a reflection or a tampered frame. The
                        // session's counters never go backwards, so the only
                        // safe thing left is to stop.
                        VelaLog.failure("clearsigner.ble", "sealed frame refused", error)
                        gone = true
                        return null
                    }
                    val json = runCatching { JSONObject(String(opened, Charsets.UTF_8)) }.getOrNull() ?: continue
                    // PROTOCOL §4: `n` only ever rises.
                    if (!envelopes.accept(json.optLong("n", 0))) continue
                    when (json.optString("t")) {
                        "bye" -> {
                            gone = true
                            return null
                        }
                        "result", "error" -> if (json.optString("id") == id) return json
                        else -> Unit
                    }
                }
            }
        }
    }

    private suspend fun receive(): Incoming? =
        withTimeoutOrNull(timeoutMs) { runCatching { inbox.receive() }.getOrNull() }

    /**
     * Nothing came back. A person who cancelled, a page that left and a radio
     * that died are all "closed without signing"; only silence is a timeout.
     */
    private fun timedOutOrCancelled(): ClearSignerAnswer =
        if (cancelled || gone || over) ClearSignerAnswer.Cancelled else ClearSignerAnswer.TimedOut

    private fun nextRequestId(): String =
        ByteArray(8).also(ids::nextBytes).joinToString("") { "%02x".format(it) }

    private companion object {
        /** What BLE promises before a central asks for more. */
        const val DEFAULT_MTU = 23u
    }
}
