package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.settings.core.SignPrefEvent
import app.getvela.wallet.feature.settings.core.SignPrefExecutor
import app.getvela.wallet.feature.settings.core.SignPrefOperation
import app.getvela.wallet.feature.settings.core.SignPrefShellResult
import app.getvela.wallet.feature.settings.core.SignPrefView
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerAnswer
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerAsk
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerChannel
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerLoopback
import app.getvela.wallet.feature.signing.core.SigningController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import uniffi.vela_core_uniffi.ClearSignerOutcome
import uniffi.vela_core_uniffi.SignPrefCore
import uniffi.vela_core_uniffi.WalletKeyRecord
import java.io.InputStream
import java.math.BigInteger
import java.net.Socket
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.util.Base64
import kotlin.concurrent.thread

/**
 * The phones' Clear Signer channel (spec 071), end to end on a real loopback
 * socket: a test "page" does what the browser does — the RFC 6455 handshake
 * with its Origin, masked frames, the token, the intent, a WebAuthn answer
 * signed by a real P-256 key — and the channel, through the core's own
 * connection, decides.
 */
class ClearSignerChannelTest {

    private val words = ClearSignerChannel.Words(closed = "closed", refused = "refused", mismatch = "mismatch", timeout = "timeout")
    private val signer = keyPair()
    private val credential = byteArrayOf(0x11, 0x22, 0x33)
    private val digest = ByteArray(32) { 0xab.toByte() }
    private val keys = listOf(WalletKeyRecord(credentialId = hex(credential), publicKeyHex = uncompressed(signer)))
    private val request = """{"intent":{"method":"personal_sign","params":["0x68"],"origin":"https://app.example"},"context":{"chainId":100}}"""

    private fun channel(timeoutMs: Long = 20_000L, page: (port: Int, token: String) -> Unit) = ClearSignerChannel(
        signerUrl = { "https://sign.getvela.app/" },
        openPage = { url ->
            val fragment = url.substringAfter('#').split('&').associate { it.substringBefore('=') to it.substringAfter('=') }
            thread { page(fragment.getValue("p").toInt(), fragment.getValue("t")) }
            true
        },
        bringBack = {},
        words = { words },
        timeoutMs = timeoutMs,
    )

    /**
     * Spec 075 asks WHERE the signer is before anything opens. Every test that
     * drives a whole request answers "on this device" the moment the sheet is
     * up, which is what a person tapping the first row does.
     */
    private fun <T> onThisDevice(channel: ClearSignerChannel, block: suspend () -> T): T = runBlocking {
        val answering = launch(Dispatchers.Default) {
            channel.state.first { it is ClearSignerChannel.State.Where }
            channel.chooseWhere(thisDevice = true)
        }
        try {
            block()
        } finally {
            answering.cancel()
        }
    }

    @Test
    fun `a page that proves itself and signs this digest with this wallet's key is accepted`() {
        val channel = channel { port, token ->
            Page(port).use { page ->
                assertTrue(page.upgrade().startsWith("HTTP/1.1 101"))
                page.send("""{"v":1,"t":"hello","token":"$token"}""")
                val intent = JSONObject(page.receive())
                assertEquals("intent", intent.getString("t"))
                assertEquals("personal_sign", intent.getJSONObject("intent").getString("method"))
                page.send(JSONObject().put("v", 1).put("t", "result").put("id", intent.getString("id")).put("result", answer(signer)).toString())
                page.drain()
            }
        }
        val assertion = onThisDevice(channel) { channel.sign(request, digest, keys) }
        assertEquals("112233", assertion.credentialIdHex)
        assertTrue("DER, as the Safe envelope takes", assertion.signatureDerHex.startsWith("30"))
    }

    @Test
    fun `a stranger is turned away and the real page still gets through`() {
        val channel = channel { port, token ->
            Page(port).use { stranger -> assertTrue(stranger.upgrade(origin = "https://evil.example").startsWith("HTTP/1.1 403")) }
            Page(port).use { spent ->
                spent.upgrade()
                spent.send("""{"v":1,"t":"hello","token":"not-it"}""")
                spent.drain()
            }
            Page(port).use { page ->
                page.upgrade()
                page.send("""{"v":1,"t":"hello","token":"$token"}""")
                val id = JSONObject(page.receive()).getString("id")
                page.send(JSONObject().put("v", 1).put("t", "result").put("id", id).put("result", answer(signer)).toString())
                page.drain()
            }
        }
        val assertion = onThisDevice(channel) { channel.sign(request, digest, keys) }
        assertEquals("112233", assertion.credentialIdHex)
    }

    @Test
    fun `a page closed after the intent arrived was closed without signing`() {
        val channel = channel { port, token ->
            Page(port).use { page ->
                page.upgrade()
                page.send("""{"v":1,"t":"hello","token":"$token"}""")
                page.receive()
            }
        }
        val failure = failureOf(channel) { channel.sign(request, digest, keys) }
        assertEquals(FailureKind.Cancelled, failure.kind)
        assertEquals("closed", channel.notice.value)
    }

    @Test
    fun `a signature by a key the wallet does not hold is refused and nothing is returned`() {
        val stranger = keyPair()
        val channel = channel { port, token ->
            Page(port).use { page ->
                page.upgrade()
                page.send("""{"v":1,"t":"hello","token":"$token"}""")
                val id = JSONObject(page.receive()).getString("id")
                page.send(JSONObject().put("v", 1).put("t", "result").put("id", id).put("result", answer(stranger)).toString())
                page.drain()
            }
        }
        val failure = failureOf(channel) { channel.sign(request, digest, keys) }
        // The request stays open (contract §5): a cancelled ceremony, the reason on the notice.
        assertEquals(FailureKind.Cancelled, failure.kind)
        assertEquals("mismatch", channel.notice.value)
    }

    @Test
    fun `the page's own refusal is told as the page's`() {
        val channel = channel { port, token ->
            Page(port).use { page ->
                page.upgrade()
                page.send("""{"v":1,"t":"hello","token":"$token"}""")
                val id = JSONObject(page.receive()).getString("id")
                page.send("""{"v":1,"t":"error","id":"$id","code":"refused"}""")
                page.drain()
            }
        }
        assertEquals("refused", failureOf(channel) { channel.sign(request, digest, keys) }.message)
    }

    @Test
    fun `cancel and the five-minute clock both end the wait`() {
        lateinit var cancelling: ClearSignerChannel
        cancelling = channel { _, _ -> Thread.sleep(200); cancelling.cancel() }
        assertEquals(FailureKind.Cancelled, failureOf(cancelling) { cancelling.sign(request, digest, keys) }.kind)
        assertEquals(ClearSignerChannel.State.Idle, cancelling.state.value)

        val waiting = channel(timeoutMs = 300L) { _, _ -> }
        assertEquals("timeout", failureOf(waiting) { waiting.sign(request, digest, keys) }.message)
    }

    // --- spec 075: one page visit, several requests ---------------------------

    @Test
    fun `a flow puts several requests down one socket and ends it with bye`() {
        val second = ByteArray(32) { 0xcd.toByte() }
        val seen = java.util.concurrent.CopyOnWriteArrayList<String>()
        lateinit var wire: ClearSignerLoopback
        wire = ClearSignerLoopback(
            base = "https://sign.getvela.app/",
            openPage = { url ->
                val fragment = url.substringAfter('#').split('&').associate { it.substringBefore('=') to it.substringAfter('=') }
                thread {
                    Page(fragment.getValue("p").toInt()).use { page ->
                        page.upgrade()
                        page.send("""{"v":1,"t":"hello","token":"${fragment.getValue("t")}"}""")
                        repeat(2) { index ->
                            val intent = JSONObject(page.receive())
                            seen += intent.getString("id")
                            val over = if (index == 0) digest else second
                            page.send(
                                JSONObject().put("v", 1).put("t", "result")
                                    .put("id", intent.getString("id"))
                                    .put("result", answer(signer, over)).toString(),
                            )
                        }
                        // The flow's end, on the same socket.
                        seen += JSONObject(page.receive()).getString("t")
                        page.drain()
                    }
                }
                true
            },
            timeoutMs = 20_000L,
            random = SecureRandom(),
        )
        val outcomes = runBlocking {
            val first = wire.ask(ClearSignerAsk.Signature(request, digest, keys))
            val next = wire.ask(ClearSignerAsk.Signature(request, second, keys))
            first to next
        }
        wire.end()
        listOf(outcomes.first, outcomes.second).forEach { answer ->
            val outcome = (answer as ClearSignerAnswer.Signed).outcome
            assertTrue("both requests were answered on the one session", outcome is ClearSignerOutcome.Accepted)
        }
        // Two distinct request ids, then the session's own goodbye — one
        // listener, one page visit (contract §1.5).
        runBlocking { withTimeout(5_000L) { while (seen.size < 3) kotlinx.coroutines.delay(20) } }
        assertEquals(3, seen.size)
        assertTrue("each request has its own id", seen[0] != seen[1])
        assertEquals("bye", seen[2])
    }

    @Test
    fun `a page that closes between requests ends the flow rather than hanging the next one`() {
        lateinit var wire: ClearSignerLoopback
        wire = ClearSignerLoopback(
            base = "https://sign.getvela.app/",
            openPage = { url ->
                val fragment = url.substringAfter('#').split('&').associate { it.substringBefore('=') to it.substringAfter('=') }
                thread {
                    Page(fragment.getValue("p").toInt()).use { page ->
                        page.upgrade()
                        page.send("""{"v":1,"t":"hello","token":"${fragment.getValue("t")}"}""")
                        val intent = JSONObject(page.receive())
                        page.send(
                            JSONObject().put("v", 1).put("t", "result")
                                .put("id", intent.getString("id"))
                                .put("result", answer(signer)).toString(),
                        )
                    }
                }
                true
            },
            timeoutMs = 20_000L,
            random = SecureRandom(),
        )
        val second = runBlocking {
            wire.ask(ClearSignerAsk.Signature(request, digest, keys))
            // The page is gone; the next request must not wait out the clock.
            withTimeout(20_000L) { wire.ask(ClearSignerAsk.Signature(request, digest, keys)) }
        }
        assertEquals(ClearSignerAnswer.Cancelled, second)
    }

    @Test
    fun `a cancel that arrives before the request is waiting still ends it`() {
        var opened = 0
        val wire = ClearSignerLoopback(
            base = "https://sign.getvela.app/",
            openPage = { opened += 1; true },
            timeoutMs = 20_000L,
            random = SecureRandom(),
        )
        // The Where sheet's Cancel lands in the breath between the wire being
        // built and the first request waiting on anything. Before this was
        // sticky, it was dropped and the person watched a dead sheet for five
        // minutes.
        wire.cancel()
        val answer = runBlocking {
            withTimeout(5_000L) { wire.ask(ClearSignerAsk.Signature(request, digest, keys)) }
        }
        assertEquals(ClearSignerAnswer.Cancelled, answer)
        assertEquals("no page was opened", 0, opened)
    }

    @Test
    fun `nothing opens until the person says where the signer is`() {
        var opened = 0
        val channel = channel { _, _ -> opened += 1 }
        val failure = runBlocking {
            val cancelling = launch(Dispatchers.Default) {
                channel.state.first { it is ClearSignerChannel.State.Where }
                channel.cancel()
            }
            try {
                runCatching { channel.sign(request, digest, keys) }.exceptionOrNull()
            } finally {
                cancelling.cancel()
            }
        }
        assertEquals(0, opened)
        assertEquals(FailureKind.Cancelled, (failure as PasskeyFailure).kind)
        assertEquals(ClearSignerChannel.State.Idle, channel.state.value)
    }

    // --- the preference ------------------------------------------------------

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun tearDown() = scopes.forEach { it.cancel() }

    private fun prefHost(store: KeyValueStore): CoreHost<SignPrefView> {
        val executor = SignPrefExecutor(store)
        return CoreHost(
            bridge = SignPrefCore().asBridge(),
            scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it },
            initial = SignPrefView(),
            serializer = SignPrefView.serializer(),
            perform = JsonShell.perform(SignPrefOperation.serializer(), SignPrefShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(
                SignPrefOperation.serializer(),
                SignPrefShellResult.serializer(),
                fallback = SignPrefShellResult.Stored(),
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
    }

    private fun <V : Any> CoreHost<V>.settle(predicate: (V) -> Boolean): V =
        runBlocking { withTimeout(10_000L) { view.first(predicate) } }

    @Test
    fun `the picker's methods are the core's, in the core's order`() {
        val host = prefHost(FakeStore())
        host.start()
        host.dispatch(SignPrefEvent.Refresh, SignPrefEvent.serializer())
        val view = host.settle { true }
        assertEquals(SigningController.SIGN_METHODS, view.offered)
        assertEquals(SigningController.SIGN_METHODS, SignPrefView().offered)
    }

    @Test
    fun `the default method and the signer page reach the store, and a page a browser would not sign on does not`() {
        val store = FakeStore()
        val host = prefHost(store)
        host.start()
        host.dispatch(SignPrefEvent.Refresh, SignPrefEvent.serializer())
        host.settle { !it.method_committed }
        host.dispatch(SignPrefEvent.MethodChosen("clear_signer"), SignPrefEvent.serializer())
        host.settle { it.method == "clear_signer" }
        host.dispatch(SignPrefEvent.SignerUrlSubmitted("http://192.168.1.4/"), SignPrefEvent.serializer())
        assertEquals("insecure", host.settle { it.signer_url_error != null }.signer_url_error)
        host.dispatch(SignPrefEvent.SignerUrlSubmitted("http://127.0.0.1:8140"), SignPrefEvent.serializer())
        val chosen = host.settle { !it.signer_url_is_default }
        assertEquals("http://127.0.0.1:8140/", chosen.signer_url)
        assertEquals(false, chosen.signer_uses_wallet_passkeys)
        runBlocking { withTimeout(10_000L) { while (store.values[KeyValueStore.Keys.CLEAR_SIGNER_URL] == null) kotlinx.coroutines.delay(20) } }
        assertEquals("clear_signer", store.values[KeyValueStore.Keys.SIGN_METHOD])
        host.dispatch(SignPrefEvent.SignerUrlReset, SignPrefEvent.serializer())
        host.settle { it.signer_url_is_default }
        runBlocking { withTimeout(10_000L) { while (store.values.containsKey(KeyValueStore.Keys.CLEAR_SIGNER_URL)) kotlinx.coroutines.delay(20) } }
    }

    // --- the test page ------------------------------------------------------

    /** The browser's side of the socket. */
    private class Page(port: Int) : AutoCloseable {
        private val socket = Socket("127.0.0.1", port).apply { soTimeout = 10_000 }
        private val input: InputStream = socket.getInputStream()

        fun upgrade(origin: String = "https://sign.getvela.app"): String {
            socket.getOutputStream().write(
                ("GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n" +
                    "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: $origin\r\n\r\n").toByteArray(),
            )
            val head = StringBuilder()
            while (!head.endsWith("\r\n\r\n")) {
                val byte = input.read()
                if (byte < 0) break
                head.append(byte.toChar())
            }
            return head.toString()
        }

        /** A masked text frame, as a browser sends one. */
        fun send(text: String) {
            val payload = text.toByteArray()
            val mask = byteArrayOf(0x37, 0xfa.toByte(), 0x21, 0x3d)
            val frame = java.io.ByteArrayOutputStream()
            frame.write(0x81)
            if (payload.size < 126) {
                frame.write(0x80 or payload.size)
            } else {
                frame.write(0x80 or 126)
                frame.write(payload.size shr 8)
                frame.write(payload.size and 0xff)
            }
            frame.write(mask)
            payload.forEachIndexed { i, b -> frame.write(b.toInt() xor mask[i % 4].toInt()) }
            socket.getOutputStream().write(frame.toByteArray())
        }

        /** The next text frame from the wallet (never masked). */
        fun receive(): String {
            while (true) {
                val first = input.read()
                var length = input.read() and 0x7f
                if (length == 126) length = (input.read() shl 8) or input.read()
                val payload = input.readNBytes(length)
                if (first and 0x0f == 0x1) return String(payload)
            }
        }

        /** Read until the wallet closes. */
        fun drain() {
            runCatching { while (input.read() >= 0) Unit }
        }

        override fun close() = socket.close()
    }

    // --- WebAuthn, by hand ----------------------------------------------------

    private fun answer(pair: KeyPair, over: ByteArray = digest): JSONObject {
        val authenticatorData = sha256("getvela.app".toByteArray()) + byteArrayOf(0x05, 0, 0, 0, 7)
        val client = """{"type":"webauthn.get","challenge":"${b64url(over)}","origin":"https://sign.getvela.app","crossOrigin":false}"""
        val der = Signature.getInstance("SHA256withECDSA").run {
            initSign(pair.private)
            update(authenticatorData + sha256(client.toByteArray()))
            sign()
        }
        return JSONObject()
            .put("credentialId", b64url(credential))
            .put("signature", "0x" + hex(rawOf(der)))
            .put("authenticatorData", "0x" + hex(authenticatorData))
            .put("clientDataJSON", "0x" + hex(client.toByteArray()))
    }

    /** DER `SEQUENCE { r, s }` → `r‖s`, 32 bytes each (the page sends it so). */
    private fun rawOf(der: ByteArray): ByteArray {
        var at = 2
        fun integer(): ByteArray {
            val length = der[at + 1].toInt()
            val value = der.copyOfRange(at + 2, at + 2 + length)
            at += 2 + length
            return BigInteger(1, value).toByteArray().let { bytes ->
                val trimmed = if (bytes.size > 32) bytes.copyOfRange(bytes.size - 32, bytes.size) else bytes
                ByteArray(32 - trimmed.size) + trimmed
            }
        }
        return integer() + integer()
    }

    private fun failureOf(channel: ClearSignerChannel, block: suspend () -> Unit): PasskeyFailure {
        try {
            onThisDevice(channel) { block() }
        } catch (failure: PasskeyFailure) {
            return failure
        }
        fail("expected a refusal")
        throw IllegalStateException()
    }

    private companion object {
        fun keyPair(): KeyPair = KeyPairGenerator.getInstance("EC").run {
            initialize(ECGenParameterSpec("secp256r1"))
            generateKeyPair()
        }

        fun uncompressed(pair: KeyPair): String {
            val point = (pair.public as ECPublicKey).w
            fun coordinate(value: BigInteger) = value.toByteArray().let { bytes ->
                val trimmed = if (bytes.size > 32) bytes.copyOfRange(bytes.size - 32, bytes.size) else bytes
                ByteArray(32 - trimmed.size) + trimmed
            }
            return "04" + hex(coordinate(point.affineX)) + hex(coordinate(point.affineY))
        }

        fun sha256(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)
        fun b64url(bytes: ByteArray): String = Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)
        fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }
    }
}
