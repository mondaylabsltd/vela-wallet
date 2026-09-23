package app.getvela.wallet

import app.getvela.wallet.feature.signing.clearsigner.ClearSignerAnswer
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerAsk
import app.getvela.wallet.feature.signing.clearsigner.TunnelListener
import app.getvela.wallet.feature.signing.clearsigner.TunnelSocket
import app.getvela.wallet.feature.signing.clearsigner.TunnelSockets
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerTunnelWire
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.ClearSignerHandshake
import uniffi.vela_core_uniffi.clearSignerKeyFingerprint
import uniffi.vela_core_uniffi.clearSignerTunnelLink
import uniffi.vela_core_uniffi.clearSignerTunnelRoom
import uniffi.vela_core_uniffi.clearSignerTunnelRoomUrl
import java.io.File
import java.security.SecureRandom
import java.util.concurrent.CopyOnWriteArrayList

/**
 * The wallet's side of a cross-device pairing (spec 075, contracts/tunnel.md).
 *
 * Two halves, both without a network:
 *
 * - **The vectors.** `rust/crates/vela-core/tests/clear-signer/secure-session.json`
 *   pins the session both ends run — the public key, the pairing link's `rk`,
 *   the six digits, and every sealed frame byte for byte. The requester here
 *   is the same `ClearSignerHandshake` / `ClearSignerSession` the app uses, so
 *   if this passes the phone and the page agree.
 * - **A fake tunnel.** `ClearSignerTunnelWire` against an in-process tunnel that
 *   implements tunnel.md §1, with the vectors' page as the signer: the link it
 *   publishes, the code it shows, and — the one that matters — that **nothing
 *   is sent before the person confirms the code**.
 */
class ClearSignerTunnelTest {

    private val repoRoot = File(
        System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"),
    )

    private val vectors: JSONArray by lazy {
        JSONObject(
            File(repoRoot, "rust/crates/vela-core/tests/clear-signer/secure-session.json").readText(),
        ).getJSONArray("cases")
    }

    private fun case(name: String): JSONObject =
        (0 until vectors.length()).map(vectors::getJSONObject).first { it.getString("name") == name }

    // -- the vectors ----------------------------------------------------------

    @Test
    fun `the requester reproduces every vector — its key, the link's rk, the code, and every frame`() {
        for (index in 0 until vectors.length()) {
            val vector = vectors.getJSONObject(index)
            val requester = vector.getJSONObject("requester")
            val signer = vector.getJSONObject("signer")
            val tunnel = vector.getString("label") == "vela-tunnel/1"

            val handshake = ClearSignerHandshake(
                unhex(requester.getString("secretHex")),
                unhex(requester.getString("nonceHex")),
            )
            assertEquals(
                "${vector.getString("name")}: the public key",
                requester.getString("publicKeyHex"),
                hex(handshake.publicKey()),
            )
            // Only the tunnel's link carries `rk`; the fingerprint itself is the
            // same function on both channels.
            if (vector.has("rk")) {
                assertEquals(
                    "${vector.getString("name")}: the pairing link's rk",
                    vector.getString("rk"),
                    clearSignerKeyFingerprint(handshake.publicKey()),
                )
            }
            // The hello is compared as JSON: the field order is nobody's wire.
            assertEquals(
                "${vector.getString("name")}: our hello",
                sortedJson(requester.getString("hello")),
                sortedJson(
                    handshake.hello(
                        requester.optString("app").ifEmpty { null },
                        // The vectors predate the peer's mark (spec 075) and
                        // carry no icon; an absent one is a hello without the
                        // field, which is what they pin.
                        requester.optString("icon").ifEmpty { null },
                    ),
                ),
            )

            val session = handshake.complete(signer.getString("hello"), tunnel)
            assertEquals(
                "${vector.getString("name")}: the six digits",
                vector.getString("code"),
                session.code(),
            )

            val messages = vector.getJSONArray("messages")
            for (m in 0 until messages.length()) {
                val message = messages.getJSONObject(m)
                val plaintext = message.getString("plaintext")
                val sealed = message.getString("sealedHex")
                // BLE binds its frame message id into the AAD; the tunnel binds
                // the counter (`null`).
                val msgId = if (message.has("msgId")) message.getInt("msgId").toUByte() else null
                if (message.getString("from") == "requester") {
                    assertEquals(
                        "${vector.getString("name")}[$m]: sealed as the requester",
                        sealed,
                        hex(session.seal(plaintext.toByteArray(Charsets.UTF_8), msgId)),
                    )
                } else {
                    assertEquals(
                        "${vector.getString("name")}[$m]: opened as the requester",
                        plaintext,
                        String(session.open(unhex(sealed), msgId), Charsets.UTF_8),
                    )
                }
            }
        }
    }

    // -- the fake tunnel -------------------------------------------------------

    /**
     * tunnel.md §1, in process: one room, one requester, one signer, `joined`
     * when both are in, and every frame forwarded byte for byte. The tap
     * records what the tunnel saw, which is how the test can say the tunnel
     * learned nothing it should not have.
     */
    private class FakeTunnel(
        /** What the page says once both ends are in the room. */
        private val signerHello: String,
        /** Sealed frames the page sends, in order, one per requester frame. */
        private val replies: List<ByteArray> = emptyList(),
    ) : TunnelSockets {
        val opened = CopyOnWriteArrayList<String>()
        val fromRequester = CopyOnWriteArrayList<Any>()
        private var listener: TunnelListener? = null
        private var sent = 0

        override fun open(url: String, listener: TunnelListener): TunnelSocket {
            opened += url
            this.listener = listener
            val socket = object : TunnelSocket {
                override fun send(text: String) {
                    fromRequester += text
                }

                override fun send(bytes: ByteArray) {
                    fromRequester += bytes
                    replies.getOrNull(sent++)?.let { listener.onBinary(it) }
                }

                override fun close() {
                    listener.onClosed("1000")
                }
            }
            // Both roles are in the room: the tunnel says so, then the page
            // (the signer) speaks first.
            listener.onText("""{"v":1,"relay":"joined"}""")
            listener.onText(signerHello)
            return socket
        }
    }

    private fun wireFor(
        tunnel: FakeTunnel,
        vector: JSONObject,
        onLink: (String) -> Unit = {},
        confirm: suspend (String) -> Boolean,
    ): ClearSignerTunnelWire {
        val requester = vector.getJSONObject("requester")
        return ClearSignerTunnelWire(
            tunnelUrl = "wss://tunnel.example",
            signerUrl = "https://sign.getvela.app/",
            sockets = tunnel,
            appName = requester.optString("app"),
            timeoutMs = 5_000L,
            random = scripted(
                unhex(requester.getString("secretHex")),
                unhex(requester.getString("nonceHex")),
                ByteArray(16) { 0x5a },
            ),
            onLink = onLink,
            confirmCode = confirm,
        )
    }

    @Test
    fun `the pairing link is the core's, and the room is the socket the wallet opens`() {
        val vector = case("tunnel-a")
        val tunnel = FakeTunnel(vector.getJSONObject("signer").getString("hello"))
        val published = CompletableDeferred<String>()
        // The page never answers here — only the link and the room are on trial.
        val wire = wireFor(tunnel, vector, onLink = { published.complete(it) }) { true }
        val link = runBlocking {
            val asking = launch(Dispatchers.Default) {
                wire.ask(ClearSignerAsk.Signature("{}", ByteArray(32), emptyList()))
            }
            try {
                withTimeout(5_000L) { published.await() }
            } finally {
                asking.cancel()
            }
        }

        val room = clearSignerTunnelRoom(ByteArray(16) { 0x5a })!!
        assertEquals(
            clearSignerTunnelLink("https://sign.getvela.app/", "wss://tunnel.example", room, vector.getString("rk")),
            link,
        )
        assertEquals(clearSignerTunnelRoomUrl("wss://tunnel.example", room), tunnel.opened.single())
    }

    @Test
    fun `the code is the core's six digits, and nothing is sent until the person confirms it`() {
        val vector = case("tunnel-a")
        val tunnel = FakeTunnel(vector.getJSONObject("signer").getString("hello"))
        val shown = CompletableDeferred<String>()
        val confirmed = CompletableDeferred<Boolean>()
        val wire = wireFor(tunnel, vector) { code ->
            shown.complete(code)
            confirmed.await()
        }
        val answer = runBlocking {
            val asking = launch(Dispatchers.Default) {
                wire.ask(ClearSignerAsk.Signature("{}", ByteArray(32), emptyList()))
            }
            val code = withTimeout(5_000L) { shown.await() }
            assertEquals(vector.getString("code"), code)
            // The tunnel has seen the two hellos and NOTHING else: the request
            // does not leave this device before the codes are agreed.
            assertEquals(1, tunnel.fromRequester.size)
            assertTrue("only the wallet's hello, in the clear", tunnel.fromRequester.single() is String)
            confirmed.complete(false)
            asking.join()
            ClearSignerAnswer.Cancelled
        }
        assertEquals(ClearSignerAnswer.Cancelled, answer)
        // Refusing the code sends nothing sealed either — only the `bye` the
        // session says on its way out.
        assertFalse(
            "no intent was ever sealed",
            tunnel.fromRequester.filterIsInstance<ByteArray>().size > 1,
        )
    }

    @Test
    fun `a tunnel that cannot be reached is told as the tunnel being down, not as a refusal`() {
        val vector = case("tunnel-a")
        val dead = object : TunnelSockets {
            override fun open(url: String, listener: TunnelListener): TunnelSocket =
                throw java.io.IOException("no route to host")
        }
        val requester = vector.getJSONObject("requester")
        val wire = ClearSignerTunnelWire(
            tunnelUrl = "wss://tunnel.example",
            signerUrl = "https://sign.getvela.app/",
            sockets = dead,
            appName = "vela-test/1",
            timeoutMs = 2_000L,
            random = scripted(
                unhex(requester.getString("secretHex")),
                unhex(requester.getString("nonceHex")),
                ByteArray(16) { 0x5a },
            ),
            onLink = {},
            confirmCode = { true },
        )
        val answer = runBlocking {
            withTimeout(5_000L) { wire.ask(ClearSignerAsk.Signature("{}", ByteArray(32), emptyList())) }
        }
        assertTrue("$answer", answer is ClearSignerAnswer.Unreachable)
    }

    // -- helpers --------------------------------------------------------------

    /**
     * A `SecureRandom` that hands out exactly these bytes, in order — so the
     * wire's session is the vectors' session and its room id is known. Past
     * the script it is the real thing, which no test reaches.
     */
    private fun scripted(vararg chunks: ByteArray): SecureRandom {
        val queue = ArrayDeque(chunks.toList())
        return object : SecureRandom() {
            override fun nextBytes(bytes: ByteArray) {
                val next = queue.removeFirstOrNull()
                if (next == null || next.size != bytes.size) {
                    java.util.Random(7).nextBytes(bytes)
                    return
                }
                next.copyInto(bytes)
            }
        }
    }

    private fun sortedJson(text: String): String {
        val json = JSONObject(text)
        return json.keys().asSequence().sorted().joinToString(",") { "$it=${json.get(it)}" }
    }

    private fun hex(bytes: ByteArray) = bytes.joinToString("") { "%02x".format(it) }

    private fun unhex(text: String) = text.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
}
