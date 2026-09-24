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
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerAnswer
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerAsk
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerCallbacks
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerChannel
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerScheme
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
import uniffi.vela_core_uniffi.TrustedSignerOutcome
import uniffi.vela_core_uniffi.SignPrefCore
import uniffi.vela_core_uniffi.WalletKeyRecord
import java.math.BigInteger
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.util.Base64
import java.util.zip.Inflater
import kotlin.concurrent.thread

/**
 * The phones' Trusted Signer channel (specs 071 and 076), end to end over the
 * channel the app actually has: the request goes out in the launch URL's
 * fragment, and a test "page" answers it the way the real page does — by
 * navigating to `velawallet://sign-result?t=…&result=…` with a WebAuthn
 * assertion signed by a real P-256 key. The core does the judging.
 *
 * The loopback socket these tests used to drive was cut by the owner on
 * 2026-09-23 (「回环 WebSocket 不做呀，现在就是纯 custom schema」), and it
 * could not have survived spec 076 anyway: the published page carries
 * `default-src 'none'` in its own hashed bytes and cannot open a socket.
 */
class TrustedSignerChannelTest {

    private val words = TrustedSignerChannel.Words(closed = "closed", refused = "refused", mismatch = "mismatch", timeout = "timeout")
    private val signer = keyPair()
    private val credential = byteArrayOf(0x11, 0x22, 0x33)
    private val digest = ByteArray(32) { 0xab.toByte() }
    private val keys = listOf(WalletKeyRecord(credentialId = hex(credential), publicKeyHex = uncompressed(signer)))
    private val request = """{"intent":{"method":"personal_sign","params":["0x68"],"origin":"https://app.example"},"context":{"chainId":100}}"""

    /** How many times the wallet was brought back over the page. */
    private val broughtBack = java.util.concurrent.atomic.AtomicInteger(0)

    private fun channel(timeoutMs: Long = 20_000L, page: (Visit) -> Unit) = TrustedSignerChannel(
        signerUrl = { "https://sign.getvela.app/" },
        openPage = { url ->
            thread { page(Visit(url)) }
            true
        },
        bringBack = { broughtBack.incrementAndGet() },
        words = { words },
        timeoutMs = timeoutMs,
    )

    /**
     * There is ONE channel now (owner, 2026-09-23: the tunnel, Bluetooth and
     * then the loopback socket all went), so nothing is asked before a request
     * opens — the page is a Custom Tab answering by custom scheme or it is
     * nothing. Kept as a wrapper so the tests below still say where they are
     * running.
     */
    private fun <T> onThisDevice(block: suspend () -> T): T = runBlocking { block() }

    @Test
    fun `a page that signs this digest with this wallet's key is accepted`() {
        val channel = channel { visit ->
            // What the page receives is what the wallet meant to ask for.
            assertEquals("personal_sign", visit.request.getJSONObject("intent").getString("method"))
            visit.answer(answer(signer))
        }
        val assertion = onThisDevice { channel.sign(request, digest, keys) }
        assertEquals("112233", assertion.credentialIdHex)
        assertTrue("DER, as the Safe envelope takes", assertion.signatureDerHex.startsWith("30"))
    }

    @Test
    fun `an answered request brings the wallet back over the page`() {
        // A visit ends with its answer on this channel: there is no session for
        // the page to hold, and the next request opens it again. Leaving the tab
        // in front is what the socket channel did, and it stranded the person on
        // the page's own "handed back to the wallet" screen while the wallet,
        // which had the answer, sat behind it (owner, 2026-09-24).
        val channel = channel { visit -> visit.answer(answer(signer)) }
        onThisDevice { channel.sign(request, digest, keys) }
        assertTrue("the wallet was never brought back", broughtBack.get() > 0)
    }

    @Test
    fun `the request rides in the fragment, where no server sees it`() {
        // The whole point of the transport: the page is fetched from a server
        // that must not learn what is being signed. A query would be in that
        // server's log; a fragment is never sent.
        val seen = java.util.concurrent.LinkedBlockingQueue<String>()
        val channel = channel { visit ->
            seen += visit.url
            visit.answer(answer(signer))
        }
        onThisDevice { channel.sign(request, digest, keys) }
        val url = seen.take()
        assertTrue("the page is asked for by URL: $url", url.contains("ch=url"))
        assertTrue("the request is in the fragment: $url", url.contains("#i="))
        assertEquals("nothing of the request may be in the query", -1, url.substringBefore('#').indexOf("i="))
    }

    @Test
    fun `an answer carrying another attempt's token reaches nothing`() {
        val channel = channel(timeoutMs = 1_500L) { visit ->
            // Another app that registered the scheme, replaying or guessing.
            assertTrue(
                "a callback for a token nobody awaits settles nothing",
                !TrustedSignerCallbacks.deliver(callback("some-other-token", answer(signer))),
            )
        }
        assertEquals("timeout", failureOf { channel.sign(request, digest, keys) }.message)
    }

    @Test
    fun `a page closed without answering was closed without signing`() {
        val channel = channel { visit -> visit.refuse("user_rejected") }
        val failure = failureOf { channel.sign(request, digest, keys) }
        assertEquals(FailureKind.Cancelled, failure.kind)
        assertEquals("closed", channel.notice.value)
    }

    @Test
    fun `a signature by a key the wallet does not hold is refused and nothing is returned`() {
        val stranger = keyPair()
        val channel = channel { visit -> visit.answer(answer(stranger)) }
        val failure = failureOf { channel.sign(request, digest, keys) }
        // The request stays open (contract §5): a cancelled ceremony, the reason on the notice.
        assertEquals(FailureKind.Cancelled, failure.kind)
        assertEquals("mismatch", channel.notice.value)
    }

    @Test
    fun `a signature over some other digest is refused`() {
        // The one property that survives an intercepted or forged callback:
        // only an assertion over exactly the digest THIS wallet computed is
        // taken, so the transport carries the answer and authorises nothing.
        val channel = channel { visit -> visit.answer(answer(signer, over = ByteArray(32) { 0x01 })) }
        assertEquals(FailureKind.Cancelled, failureOf { channel.sign(request, digest, keys) }.kind)
        assertEquals("mismatch", channel.notice.value)
    }

    @Test
    fun `the page's own refusal is told as the page's`() {
        val channel = channel { visit -> visit.refuse("refused") }
        assertEquals("refused", failureOf { channel.sign(request, digest, keys) }.message)
    }

    @Test
    fun `cancel and the five-minute clock both end the wait`() {
        lateinit var cancelling: TrustedSignerChannel
        cancelling = channel { Thread.sleep(200); cancelling.cancel() }
        assertEquals(FailureKind.Cancelled, failureOf { cancelling.sign(request, digest, keys) }.kind)
        assertEquals(TrustedSignerChannel.State.Idle, cancelling.state.value)

        val waiting = channel(timeoutMs = 300L) { }
        assertEquals("timeout", failureOf { waiting.sign(request, digest, keys) }.message)
    }

    // --- spec 075 over spec 076's transport: a flow of several requests -------

    @Test
    fun `each request of a flow is its own visit, with its own one-time token`() {
        val second = ByteArray(32) { 0xcd.toByte() }
        val tokens = java.util.concurrent.CopyOnWriteArrayList<String>()
        var over = digest
        val wire = TrustedSignerScheme(
            base = "https://sign.getvela.app/",
            openPage = { url ->
                val visit = Visit(url)
                tokens += visit.token
                thread { visit.answer(answer(signer, over)) }
                true
            },
            timeoutMs = 20_000L,
            random = SecureRandom(),
        )
        val outcomes = runBlocking {
            val first = wire.ask(TrustedSignerAsk.Signature(request, digest, keys))
            over = second
            val next = wire.ask(TrustedSignerAsk.Signature(request, second, keys))
            first to next
        }
        wire.end()
        listOf(outcomes.first, outcomes.second).forEach { answer ->
            val outcome = (answer as TrustedSignerAnswer.Signed).outcome
            assertTrue("both requests were answered", outcome is TrustedSignerOutcome.Accepted)
        }
        // A URL carries one request, so a flow of two is two visits — and the
        // token is per request, which is what makes "one-time" literal.
        assertEquals(2, tokens.size)
        assertTrue("each visit has its own token", tokens[0] != tokens[1])
    }

    @Test
    fun `an answer that arrives after its request gave up settles nothing`() {
        val late = java.util.concurrent.LinkedBlockingQueue<Visit>()
        val wire = TrustedSignerScheme(
            base = "https://sign.getvela.app/",
            openPage = { url -> late += Visit(url); true },
            timeoutMs = 300L,
            random = SecureRandom(),
        )
        val timedOut = runBlocking { wire.ask(TrustedSignerAsk.Signature(request, digest, keys)) }
        assertEquals(TrustedSignerAnswer.TimedOut, timedOut)
        // The page finally navigates — a minute late, or from a tab the person
        // came back to. Nothing is waiting, so nothing happens.
        assertTrue(
            "a callback for nobody changes nothing",
            !TrustedSignerCallbacks.deliver(late.take().callbackFor(answer(signer))),
        )
    }

    @Test
    fun `a cancel that arrives before the request is waiting still ends it`() {
        var opened = 0
        val wire = TrustedSignerScheme(
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
            withTimeout(5_000L) { wire.ask(TrustedSignerAsk.Signature(request, digest, keys)) }
        }
        assertEquals(TrustedSignerAnswer.Cancelled, answer)
        assertEquals("no page was opened", 0, opened)
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
        host.dispatch(SignPrefEvent.MethodChosen("trusted_signer"), SignPrefEvent.serializer())
        host.settle { it.method == "trusted_signer" }
        host.dispatch(SignPrefEvent.SignerUrlSubmitted("http://192.168.1.4/"), SignPrefEvent.serializer())
        assertEquals("insecure", host.settle { it.signer_url_error != null }.signer_url_error)
        host.dispatch(SignPrefEvent.SignerUrlSubmitted("http://127.0.0.1:8140"), SignPrefEvent.serializer())
        val chosen = host.settle { !it.signer_url_is_default }
        assertEquals("http://127.0.0.1:8140/", chosen.signer_url)
        assertEquals(false, chosen.signer_uses_wallet_passkeys)
        runBlocking { withTimeout(10_000L) { while (store.values[KeyValueStore.Keys.TRUSTED_SIGNER_URL] == null) kotlinx.coroutines.delay(20) } }
        assertEquals("trusted_signer", store.values[KeyValueStore.Keys.SIGN_METHOD])
        host.dispatch(SignPrefEvent.SignerUrlReset, SignPrefEvent.serializer())
        host.settle { it.signer_url_is_default }
        runBlocking { withTimeout(10_000L) { while (store.values.containsKey(KeyValueStore.Keys.TRUSTED_SIGNER_URL)) kotlinx.coroutines.delay(20) } }
    }

    // --- the test page ------------------------------------------------------

    /**
     * The browser's side of one visit: it reads the request out of the launch
     * URL exactly as `intake.js` does — the fragment, base64url, raw DEFLATE —
     * and answers by handing the app a `velawallet://sign-result?…`, which is
     * what a navigation to that scheme amounts to from this side.
     */
    private inner class Visit(val url: String) {
        private val fragment = url.substringAfter('#').split('&')
            .associate { it.substringBefore('=') to it.substringAfter('=') }

        val token: String = java.net.URLDecoder.decode(fragment.getValue("t"), "UTF-8")

        /** Where the page was told to send the answer (`cb`, base64url). */
        val callbackUrl: String = String(b64urlBytes(fragment.getValue("cb")))

        /** The request the wallet put in the fragment. */
        val request: JSONObject = JSONObject(
            if (fragment["z"] == "1") {
                inflateRaw(b64urlBytes(fragment.getValue("i")))
            } else {
                String(b64urlBytes(fragment.getValue("i")))
            },
        )

        fun callbackFor(result: JSONObject): String =
            callbackUrl + "?t=" + java.net.URLEncoder.encode(token, "UTF-8") +
                "&result=" + b64url(result.toString().toByteArray())

        /** The page signed: it navigates to the callback. */
        fun answer(result: JSONObject) {
            TrustedSignerCallbacks.deliver(callbackFor(result))
        }

        /** The page (or the person) said no. */
        fun refuse(code: String) {
            TrustedSignerCallbacks.deliver(
                callbackUrl + "?t=" + java.net.URLEncoder.encode(token, "UTF-8") + "&error=" + code,
            )
        }
    }

    /** A callback naming some other attempt — nothing in this process awaits it. */
    private fun callback(token: String, result: JSONObject): String =
        "velawallet://sign-result?t=" + java.net.URLEncoder.encode(token, "UTF-8") +
            "&result=" + b64url(result.toString().toByteArray())

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

    private fun failureOf(block: suspend () -> Unit): PasskeyFailure {
        try {
            onThisDevice { block() }
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
        fun b64urlBytes(text: String): ByteArray = Base64.getUrlDecoder().decode(text.trimEnd('='))

        /** `DecompressionStream('deflate-raw')`, which is what the page uses. */
        fun inflateRaw(bytes: ByteArray): String {
            val inflater = Inflater(true)
            inflater.setInput(bytes)
            val out = java.io.ByteArrayOutputStream()
            val buffer = ByteArray(8 * 1024)
            while (!inflater.finished()) {
                val read = inflater.inflate(buffer)
                if (read == 0) break
                out.write(buffer, 0, read)
            }
            inflater.end()
            return out.toString("UTF-8")
        }
        fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }
    }
}
