package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.signing.clearsigner.BleGatt
import app.getvela.wallet.feature.signing.clearsigner.BleReadiness
import app.getvela.wallet.feature.signing.clearsigner.BlePeripheral
import app.getvela.wallet.feature.signing.clearsigner.BlePeripheralEvents
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerAnswer
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerAsk
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerBleHost
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerBleWire
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerChannel
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.ClearSignerBleMessage
import uniffi.vela_core_uniffi.ClearSignerFramer
import uniffi.vela_core_uniffi.ClearSignerOutcome
import uniffi.vela_core_uniffi.ClearSignerReassembler
import uniffi.vela_core_uniffi.ClearSignerRefusal
import uniffi.vela_core_uniffi.clearSignerBleUuids
import java.io.File
import java.math.BigInteger
import java.security.AlgorithmParameters
import java.security.KeyFactory
import java.security.PrivateKey
import java.security.SecureRandom
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECParameterSpec
import java.security.spec.ECPoint
import java.security.spec.ECPrivateKeySpec
import java.security.spec.ECPublicKeySpec
import java.util.Base64
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import javax.crypto.Cipher
import javax.crypto.KeyAgreement
import javax.crypto.Mac
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * The wallet's side of a Clear Signer session over BLE (spec 075 T040,
 * PROTOCOL.md §1–4 and §11) — everything but the radio.
 *
 * Three things are on trial, and the radio is not one of them:
 *
 * - **The framing is the core's.** `rust/crates/vela-core/tests/clear-signer/ble-frames.json`
 *   pins `ClearSignerFramer` / `ClearSignerReassembler` against the page's own
 *   JavaScript; replaying it through the Kotlin bindings says the bridge
 *   carries it faithfully, so nothing here re-reads §2 for itself.
 * - **The session is the core's.** The page in these tests is a real P-256 +
 *   HKDF + AES-GCM central built on the JDK's own crypto, and it is pinned
 *   first against `secure-session.json`'s `ble-a` case — its six digits, its
 *   sealed bytes and the wallet's hello, byte for byte. A page that reproduces
 *   the vectors is the page the browser will be.
 * - **The glue.** Frames in order, shuffled, and with one dropped; a code the
 *   person will not confirm; several requests down one connection; `bye`.
 *
 * No radio pass is part of this: a `BluetoothGattServer` only exists on a
 * phone. What runs here is [ClearSignerBleWire] against a fake peripheral
 * whose callbacks are driven by hand, which is the whole reason
 * [BlePeripheral] is an interface.
 */
class ClearSignerBleTest {

    private val WORDS = ClearSignerChannel.Words(
        closed = "closed",
        refused = "refused",
        mismatch = "mismatch",
        timeout = "timeout",
        relayDown = "the relay is down",
        bluetoothNeeded = "Vela needs Bluetooth permission so the signing page can find this device.",
        bluetoothOff = "Turn Bluetooth on to pair this way.",
    )

    /** Every key this route renders, in the order the person meets them. */
    private val NEARBY_KEYS = listOf(
        "componentsUi.signing.clearSignerNearby",
        "componentsUi.signing.clearSignerNearbyHint",
        "componentsUi.signing.clearSignerNearbyName",
        "componentsUi.signing.clearSignerBluetoothNeeded",
        "componentsUi.signing.clearSignerBluetoothOff",
        "componentsUi.scanner.grantPermission",
    )

    private val repoRoot = File(
        System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"),
    )

    private val vector: JSONObject by lazy {
        val cases = JSONObject(
            File(repoRoot, "rust/crates/vela-core/tests/clear-signer/secure-session.json").readText(),
        ).getJSONArray("cases")
        (0 until cases.length()).map(cases::getJSONObject).first { it.getString("name") == "ble-a" }
    }

    private val frameCases: JSONArray by lazy {
        JSONObject(
            File(repoRoot, "rust/crates/vela-core/tests/clear-signer/ble-frames.json").readText(),
        ).getJSONArray("cases")
    }

    // -- the core's framing, across the bridge --------------------------------

    @Test
    fun `the framing is the core's — every ble-frames vector, both ways`() {
        for (index in 0 until frameCases.length()) {
            val case = frameCases.getJSONObject(index)
            val name = case.getString("name")
            val sealed = case.getInt("flags") and 1 != 0
            val msgId = case.getInt("msgId").toUByte()
            val payload = unhex(case.getString("payload"))

            val framer = ClearSignerFramer()
            // The framer only ever gives ground, and only by halving — the same
            // walk down the chunk sizes an MTU of 23 forces on a real phone.
            while (framer.chunk().toInt() > case.getInt("chunk")) {
                assertTrue("$name: the chunk can still be halved", framer.halve())
            }
            val frames = framer.frames(msgId, payload, sealed)
            val expected = case.getJSONArray("frames")
            assertEquals("$name: frame count", expected.length(), frames.size)
            for (f in frames.indices) {
                assertEquals("$name[$f]", expected.getString(f), hex(frames[f]))
            }

            val reassembler = ClearSignerReassembler()
            var whole: ClearSignerBleMessage? = null
            for (frame in frames) whole = reassembler.accept(frame, 0uL) ?: whole
            val message = assertNotNull("$name: the frames completed the message", whole).let { whole!! }
            assertEquals("$name: msgId", msgId, message.msgId)
            assertEquals("$name: sealed", sealed, message.sealed)
            assertEquals("$name: payload", case.getString("payload"), hex(message.payload))
        }
    }

    @Test
    fun `the uuids are the core's, and the three of them differ`() {
        val uuids = clearSignerBleUuids()
        assertEquals(3, uuids.size)
        assertEquals(uuids[0], BleGatt.service.toString())
        assertEquals(uuids[1], BleGatt.c2p.toString())
        assertEquals(uuids[2], BleGatt.p2c.toString())
        assertEquals(3, uuids.toSet().size)
    }

    // -- the page in these tests is the page the vectors describe -------------

    @Test
    fun `the test page reproduces the ble-a vector — the code and every sealed byte`() {
        val page = page()
        page.complete(vector.getJSONObject("requester").getString("hello"))
        assertEquals(vector.getString("code"), page.code)

        val messages = vector.getJSONArray("messages")
        for (m in 0 until messages.length()) {
            val message = messages.getJSONObject(m)
            val msgId = message.getInt("msgId")
            val plaintext = message.getString("plaintext")
            if (message.getString("from") == "signer") {
                assertEquals(
                    "[$m]: sealed as the page",
                    message.getString("sealedHex"),
                    hex(page.seal(plaintext.toByteArray(Charsets.UTF_8), msgId)),
                )
            } else {
                assertEquals(
                    "[$m]: opened as the page",
                    plaintext,
                    String(page.open(unhex(message.getString("sealedHex")), msgId), Charsets.UTF_8),
                )
            }
        }
    }

    // -- a whole session ------------------------------------------------------

    @Test
    fun `a whole session — hello, the six digits, a sealed request and its answer`() = session { run ->
        assertEquals("the wallet advertises under this phone's own name", FakeRadio.NAME, run.advertisedAs())
        val intent = run.handshakeAndOpenFirstIntent()
        assertEquals("intent", intent.getString("t"))
        assertEquals(1, intent.getLong("n"))

        run.answer(intent.getString("id"), "user_rejected")
        val answer = run.answered()
        // The page declined; what matters is that its verdict came back through
        // the frames, the session and the core — not what the verdict was.
        val outcome = (answer as ClearSignerAnswer.Signed).outcome
        assertEquals(ClearSignerOutcome.Refused(ClearSignerRefusal.Declined), outcome)
    }

    @Test
    fun `frames that arrive out of order still make one message`() = session(shuffle = true) { run ->
        val intent = run.handshakeAndOpenFirstIntent()
        assertEquals("intent", intent.getString("t"))
        run.answer(intent.getString("id"), "user_rejected")
        assertTrue("$run", run.answered() is ClearSignerAnswer.Signed)
    }

    @Test
    fun `one dropped frame is a message that never arrives, and nothing is answered`() {
        val radio = FakeRadio()
        val shown = CompletableDeferred<String>()
        val page = page()
        runBlocking {
            val wire = wire(radio, timeoutMs = 1_500L) { code -> shown.complete(code); true }
            val asking = async(Dispatchers.Default) { wire.ask(signature()) }
            // The page's hello, small enough to need several frames, minus one
            // of the middle ones. The reassembler holds what it has and the
            // message never completes, so the wallet never answers a hello it
            // has not read.
            val frames = framesFor(page.hello, sealed = false, chunk = 20)
            assertTrue("the hello spans several frames", frames.size > 2)
            frames.filterIndexed { index, _ -> index != 1 }.forEach { radio.deliver(it) }

            assertNull("no hello went back out", radio.frames.poll(1_000, TimeUnit.MILLISECONDS))
            assertFalse("and no code was ever shown", shown.isCompleted)
            val answer = withTimeout(5_000L) { asking.await() }
            assertEquals(ClearSignerAnswer.TimedOut, answer)
            wire.end()
        }
    }

    // -- the code gate --------------------------------------------------------

    @Test
    fun `a code the person will not confirm sends nothing sealed`() {
        val radio = FakeRadio()
        val page = page()
        val shown = CompletableDeferred<String>()
        runBlocking {
            // The person looks at the two screens and says no.
            val wire = wire(radio, timeoutMs = 3_000L) { code ->
                shown.complete(code)
                false
            }
            val asking = async(Dispatchers.Default) { wire.ask(signature()) }
            framesFor(page.hello, sealed = false).forEach { radio.deliver(it) }
            val hello = run {
                val message = radio.await() ?: error("the wallet did not answer the hello")
                assertFalse("the hellos are in the clear", message.sealed)
                String(message.payload, Charsets.UTF_8)
            }
            page.complete(hello)
            assertEquals(
                "both ends derive the same six digits",
                page.code,
                withTimeout(4_000L) { shown.await() },
            )
            assertEquals(ClearSignerAnswer.Cancelled, withTimeout(5_000L) { asking.await() })
            // Only the hello left this phone, and then `bye`. The request
            // itself never did — that is the whole point of the code.
            val after = generateSequence { radio.await(500) }.toList()
            assertTrue("nothing but a bye followed the hello", after.size <= 1)
            assertTrue("the radio was stopped", radio.awaitStop())
        }
    }

    // -- several requests, and the end ----------------------------------------

    @Test
    fun `one connection carries several requests and ends with bye`() = session { run ->
        val first = run.handshakeAndOpenFirstIntent()
        run.answer(first.getString("id"), "user_rejected")
        assertTrue(run.answered() is ClearSignerAnswer.Signed)

        val second = run.ask()
        val next = run.openIntent()
        assertEquals("intent", next.getString("t"))
        assertTrue("`n` only ever rises", next.getLong("n") > first.getLong("n"))
        run.answer(next.getString("id"), "user_rejected")
        assertTrue(second.await() is ClearSignerAnswer.Signed)

        val bye = run.endAndReadBye()
        assertEquals("bye", bye.getString("t"))
        assertEquals("done", bye.getString("reason"))
        assertTrue("the radio was stopped", run.radio.awaitStop())
    }

    // -- the copy this route cannot work without ------------------------------

    /**
     * Every sentence the Bluetooth route shows, through the real engine.
     *
     * `clearSignerNearbyName` is the one the flow cannot work without: it is
     * the only place the person is told WHICH line in the browser's device
     * list is their phone, and it carries `{{name}}`. A renamed variable would
     * not fail to compile, would not fail to resolve, and would put
     * "Pick {{name}} in the browser's device list" in front of somebody
     * choosing what to trust with a signature.
     */
    @Test
    fun `the Bluetooth route's own sentences resolve, and the device name lands in them`() {
        val runtime = app.getvela.wallet.core.i18n.I18nRuntime { tag ->
            File(repoRoot, "assets/i18n/$tag.json").readBytes()
        }
        for (language in listOf("en", "zh")) {
            runtime.initialize(language)
            for (key in NEARBY_KEYS) {
                val value = runtime.t(key)
                assertNotEquals("$language: $key echoes its own name", key, value)
                assertTrue("$language: $key is empty", value.isNotBlank())
            }
            val named = runtime.t(
                "componentsUi.signing.clearSignerNearbyName",
                mapOf("name" to FakeRadio.NAME),
            )
            assertTrue("$language: the name is in the sentence — $named", named.contains(FakeRadio.NAME))
            assertFalse("$language: nothing was left uninterpolated — $named", named.contains("{{"))
        }
    }

    // -- the route, and what happens when the radio says no -------------------

    @Test
    fun `the Bluetooth row is offered only where there is a radio to offer`() = runBlocking {
        assertFalse("no host attached", channel(host = null).offersNearby)
        val channel = channel(host = FakeHost(BleReadiness.Ready))
        assertTrue(channel.offersNearby)
        val asking = scope.async(Dispatchers.Default) { runCatching { channel.sign("{}", ByteArray(32), emptyList()) } }
        val where = withTimeout(4_000L) {
            channel.state.first { it is ClearSignerChannel.State.Where }
        } as ClearSignerChannel.State.Where
        assertTrue("the third row is on the sheet", where.nearby)
        channel.cancel()
        asking.await()
        Unit
    }

    @Test
    fun `refused permissions raise a card, and giving up says what was missing`() = runBlocking {
        val host = FakeHost(BleReadiness.Refused)
        val channel = channel(host = host)
        val asking = scope.async(Dispatchers.Default) {
            runCatching { channel.sign("{}", ByteArray(32), emptyList()) }
        }
        withTimeout(4_000L) { channel.state.first { it is ClearSignerChannel.State.Where } }
        channel.chooseWhere(ClearSignerChannel.Route.Nearby)
        // Not a wait that never ends: a card that says the permission is missing.
        withTimeout(4_000L) { channel.state.first { it is ClearSignerChannel.State.BluetoothRefused } }
        assertEquals(1, host.asked)
        channel.cancel()
        val failure = withTimeout(6_000L) { asking.await() }.exceptionOrNull()
        assertTrue("$failure", failure is PasskeyFailure)
        // The sentence is about Bluetooth, not about a page nobody ever opened.
        assertEquals(WORDS.bluetoothNeeded, channel.notice.value)
        assertEquals(WORDS.bluetoothNeeded, (failure as PasskeyFailure).message)
    }

    @Test
    fun `a radio that is off is told as a radio that is off, not as a permission`() = runBlocking {
        val channel = channel(host = FakeHost(BleReadiness.AdapterOff))
        val asking = scope.async(Dispatchers.Default) {
            runCatching { channel.sign("{}", ByteArray(32), emptyList()) }
        }
        withTimeout(4_000L) { channel.state.first { it is ClearSignerChannel.State.Where } }
        channel.chooseWhere(ClearSignerChannel.Route.Nearby)
        val failure = withTimeout(6_000L) { asking.await() }.exceptionOrNull()
        // The system's own dialog was already declined, so there is no second
        // card to show — but the sentence names the radio, not the permission
        // the person was never asked for.
        assertEquals(WORDS.bluetoothOff, channel.notice.value)
        assertEquals(WORDS.bluetoothOff, (failure as PasskeyFailure).message)
    }

    @Test
    fun `try again asks once more, and a granted permission opens the radio`() = runBlocking {
        val host = FakeHost(BleReadiness.Refused, BleReadiness.Ready)
        val channel = channel(host = host)
        val asking = scope.async(Dispatchers.Default) {
            runCatching { channel.sign("{}", ByteArray(32), emptyList()) }
        }
        withTimeout(4_000L) { channel.state.first { it is ClearSignerChannel.State.Where } }
        channel.chooseWhere(ClearSignerChannel.Route.Nearby)
        withTimeout(4_000L) { channel.state.first { it is ClearSignerChannel.State.BluetoothRefused } }
        channel.retryBluetooth()
        // Granted the second time: the phone starts advertising, under a name
        // the person can recognise in the browser's chooser.
        val nearby = withTimeout(4_000L) {
            channel.state.first { it is ClearSignerChannel.State.Nearby }
        } as ClearSignerChannel.State.Nearby
        assertEquals(FakeRadio.NAME, nearby.deviceName)
        assertEquals(2, host.asked)
        assertTrue("the peripheral was started", host.radio.started)

        // And leaving the foreground stops the advert — PROTOCOL §1, and an
        // advert nobody is watching is one a stranger can still connect to.
        channel.leftForeground()
        assertTrue(host.radio.awaitStop())
        withTimeout(6_000L) { asking.await() }
        Unit
    }

    private fun channel(host: ClearSignerBleHost?) = ClearSignerChannel(
        signerUrl = { "https://sign.getvela.app/" },
        openPage = { false },
        bringBack = {},
        words = { WORDS },
        timeoutMs = 8_000L,
        bleHost = { host },
    )

    /** The platform, with a scripted answer (or two) about the radio. */
    private class FakeHost(vararg answers: BleReadiness) : ClearSignerBleHost {
        private val queue = ArrayDeque(answers.toList())
        val radio = FakeRadio()

        @Volatile
        var asked = 0
            private set

        override suspend fun ready(): BleReadiness {
            asked += 1
            return if (queue.size > 1) queue.removeFirst() else queue.first()
        }

        override fun peripheral(): BlePeripheral = radio
    }

    // -- the fake radio -------------------------------------------------------

    /**
     * A peripheral with no radio behind it: the frames the wallet pushes land
     * in a queue the test reads, and the frames the "page" writes are handed
     * to the events the wire registered — which is exactly what
     * `onCharacteristicWriteRequest` does on a phone.
     */
    private class FakeRadio(private val startable: Boolean = true) : BlePeripheral {
        override val deviceName = NAME
        val frames = LinkedBlockingQueue<ByteArray>()

        @Volatile
        var stopped = false
            private set

        @Volatile
        var started = false
            private set

        private var events: BlePeripheralEvents? = null

        override fun start(events: BlePeripheralEvents): Boolean {
            this.events = events
            started = startable
            return startable
        }

        override fun notify(frame: ByteArray): Boolean {
            frames.put(frame)
            return true
        }

        override fun stop() {
            stopped = true
        }

        /** One frame written on `c2p`, as the GATT callback would hand it over. */
        fun deliver(frame: ByteArray) {
            listening().onFrame(frame)
        }

        fun mtu(mtu: Int) {
            listening().onMtu(mtu)
        }

        /**
         * The wire starts advertising from the coroutine that took the
         * request, so a test writing frames can genuinely get there first —
         * which on a phone simply cannot happen, since nothing can connect to
         * a peripheral that has not started.
         */
        private fun listening(timeoutMs: Long = 4_000): BlePeripheralEvents {
            val deadline = System.currentTimeMillis() + timeoutMs
            while (System.currentTimeMillis() < deadline) {
                events?.let { return it }
                Thread.sleep(10)
            }
            error("the wire never started the peripheral")
        }

        /** The next WHOLE message the wallet pushed, reassembled by the core's own. */
        private val inbound = ClearSignerReassembler()

        fun await(timeoutMs: Long = 4_000): ClearSignerBleMessage? {
            val deadline = System.currentTimeMillis() + timeoutMs
            while (System.currentTimeMillis() < deadline) {
                val frame = frames.poll(deadline - System.currentTimeMillis(), TimeUnit.MILLISECONDS)
                    ?: return null
                inbound.accept(frame, 0uL)?.let { return it }
            }
            return null
        }

        fun awaitStop(timeoutMs: Long = 4_000): Boolean {
            val deadline = System.currentTimeMillis() + timeoutMs
            while (System.currentTimeMillis() < deadline) {
                if (stopped) return true
                Thread.sleep(20)
            }
            return stopped
        }

        companion object {
            const val NAME = "Vela test phone"
        }
    }

    // -- a session, driven from the page's side -------------------------------

    /**
     * One live conversation: the wire on one side, a real (if scripted) Clear
     * Signer page on the other, and nothing in between but frames.
     */
    private inner class Run(
        val radio: FakeRadio,
        val wire: ClearSignerBleWire,
        val page: Page,
        val shownCode: CompletableDeferred<String>,
        val confirm: CompletableDeferred<Boolean>,
        val advertised: CompletableDeferred<String>,
        var pending: Deferred<ClearSignerAnswer>,
    ) {
        /**
         * The page's framer, walked down to the floor so that every message
         * this side writes spans a dozen frames. At 244 bytes a hello is one
         * frame and reassembly would only be tested in name.
         */
        private val framer = ClearSignerFramer().also {
            while (it.chunk().toInt() > 20) it.halve()
        }
        private var shuffle = false

        fun shuffled(on: Boolean) {
            shuffle = on
        }

        suspend fun advertisedAs(): String = withTimeout(4_000L) { advertised.await() }

        /** The hellos, the code, the confirmation, and the first sealed intent. */
        suspend fun handshakeAndOpenFirstIntent(): JSONObject {
            write(page.hello, sealed = false)
            val hello = radio.await() ?: error("the wallet did not answer the hello")
            assertFalse("the hellos travel in the clear", hello.sealed)
            page.complete(String(hello.payload, Charsets.UTF_8))
            assertEquals(
                "the wallet's hello is the vector's, field for field",
                sortedJson(vector.getJSONObject("requester").getString("hello")),
                sortedJson(String(hello.payload, Charsets.UTF_8)),
            )
            assertEquals("both ends derive the same six digits", page.code, withTimeout(4_000L) { shownCode.await() })
            confirm.complete(true)
            return openIntent()
        }

        /** The next sealed message from the wallet, opened and parsed. */
        fun openIntent(): JSONObject {
            val sealed = radio.await() ?: error("no sealed message arrived")
            assertTrue("everything after the hellos is sealed", sealed.sealed)
            return JSONObject(String(page.open(sealed.payload, sealed.msgId.toInt()), Charsets.UTF_8))
        }

        /** Put another request down the same connection (§11). */
        fun ask(): Deferred<ClearSignerAnswer> {
            pending = scope.async(Dispatchers.Default) { wire.ask(signature()) }
            return pending
        }

        suspend fun answered(): ClearSignerAnswer = withTimeout(6_000L) { pending.await() }

        /** The page's verdict, sealed and framed back to the wallet. */
        fun answer(id: String, code: String) {
            val body = JSONObject()
                .put("v", 1)
                .put("t", "error")
                .put("n", page.nextN())
                .put("id", id)
                .put("code", code)
                .toString()
            writeSealed(body)
        }

        /** End the flow and read the `bye` the wallet owes the page. */
        suspend fun endAndReadBye(): JSONObject {
            wire.end()
            val sealed = radio.await() ?: error("the wallet said no goodbye")
            return JSONObject(String(page.open(sealed.payload, sealed.msgId.toInt()), Charsets.UTF_8))
        }

        private fun writeSealed(json: String) {
            val msgId = framer.nextId()
            // The id is taken before sealing on this side too: the page's own
            // AAD binds the very frames it is about to write.
            write(page.seal(json.toByteArray(Charsets.UTF_8), msgId.toInt()), sealed = true, msgId = msgId)
        }

        private fun write(text: String, sealed: Boolean) =
            write(text.toByteArray(Charsets.UTF_8), sealed, framer.nextId())

        private fun write(payload: ByteArray, sealed: Boolean, msgId: UByte) {
            val frames = framer.frames(msgId, payload, sealed)
            assertTrue("a message worth reassembling", frames.size > 2)
            // Backwards is the harshest order a radio can deliver in, and the
            // one that shows the reassembler is not quietly appending.
            val order = if (shuffle) frames.reversed() else frames
            order.forEach(radio::deliver)
        }
    }

    private val scope = kotlinx.coroutines.CoroutineScope(
        kotlinx.coroutines.SupervisorJob() + Dispatchers.Default,
    )

    private fun session(shuffle: Boolean = false, body: suspend (Run) -> Unit) = runBlocking {
        val radio = FakeRadio()
        val shown = CompletableDeferred<String>()
        val confirm = CompletableDeferred<Boolean>()
        val advertised = CompletableDeferred<String>()
        val wire = wire(
            radio,
            onAdvertising = { name -> advertised.complete(name) },
        ) { code ->
            shown.complete(code)
            confirm.await()
        }
        val asking = scope.async(Dispatchers.Default) { wire.ask(signature()) }
        val run = Run(radio, wire, page(), shown, confirm, advertised, asking)
        run.shuffled(shuffle)
        // A small MTU, so a hello and an intent both span several frames and
        // reassembly is genuinely on trial rather than nominally.
        radio.mtu(23)
        try {
            body(run)
        } finally {
            wire.end()
        }
    }

    // -- helpers --------------------------------------------------------------

    private fun wire(
        radio: BlePeripheral,
        timeoutMs: Long = 6_000L,
        onAdvertising: (String) -> Unit = {},
        confirmCode: suspend (String) -> Boolean,
    ): ClearSignerBleWire {
        val requester = vector.getJSONObject("requester")
        return ClearSignerBleWire(
            radio = radio,
            signerUrl = "https://sign.getvela.app/",
            appName = requester.getString("app"),
            timeoutMs = timeoutMs,
            random = scripted(
                unhex(requester.getString("secretHex")),
                unhex(requester.getString("nonceHex")),
            ),
            onAdvertising = onAdvertising,
            confirmCode = confirmCode,
        )
    }

    private fun signature() = ClearSignerAsk.Signature(
        """{"intent":{"method":"personal_sign","params":["0x68"],"origin":""},"context":{"chainId":100}}""",
        ByteArray(32) { 0xab.toByte() },
        emptyList(),
    )

    private fun page() = Page(
        secret = unhex(vector.getJSONObject("signer").getString("secretHex")),
        nonce = unhex(vector.getJSONObject("signer").getString("nonceHex")),
        hello = vector.getJSONObject("signer").getString("hello"),
    )

    /** The core's framer, at a chosen chunk, for frames the PAGE writes. */
    private fun framesFor(text: String, sealed: Boolean, chunk: Int = 244): List<ByteArray> {
        val framer = ClearSignerFramer()
        while (framer.chunk().toInt() > chunk) framer.halve()
        return framer.frames(framer.nextId(), text.toByteArray(Charsets.UTF_8), sealed)
    }

    /**
     * The Clear Signer page's half of PROTOCOL §3, on the JDK's own crypto —
     * P-256 ECDH, HKDF-SHA256, AES-256-GCM — and pinned against `ble-a` before
     * it is trusted to stand in for a browser.
     *
     * The public key is not derived here: the JDK exposes no scalar
     * multiplication, and the vector's recorded hello carries the key that
     * belongs to this secret anyway.
     */
    private class Page(secret: ByteArray, private val nonce: ByteArray, val hello: String) {
        private val params: ECParameterSpec = AlgorithmParameters.getInstance("EC").run {
            init(ECGenParameterSpec("secp256r1"))
            getParameterSpec(ECParameterSpec::class.java)
        }
        private val privateKey: PrivateKey = KeyFactory.getInstance("EC")
            .generatePrivate(ECPrivateKeySpec(BigInteger(1, secret), params))

        private lateinit var key: SecretKeySpec

        /** The six digits this page shows beside the wallet's. */
        lateinit var code: String
            private set

        private var out = 0L
        private var seen = 0L
        private var n = 0L

        /** The wallet's hello: derive the shared key and the comparison code. */
        fun complete(peerHello: String) {
            val peer = JSONObject(peerHello)
            val raw = b64url(peer.getString("pk"))
            val point = ECPoint(
                BigInteger(1, raw.copyOfRange(1, 33)),
                BigInteger(1, raw.copyOfRange(33, 65)),
            )
            val peerKey = KeyFactory.getInstance("EC").generatePublic(ECPublicKeySpec(point, params))
            val shared = KeyAgreement.getInstance("ECDH").run {
                init(privateKey)
                doPhase(peerKey, true)
                generateSecret()
            }
            // PROTOCOL §3: the central's nonce first — the central is this page.
            val salt = nonce + b64url(peer.getString("nonce"))
            key = SecretKeySpec(hkdf(shared, salt, "$LABEL key", 32), "AES")
            val digits = hkdf(shared, salt, "$LABEL code", 4)
            val value = java.nio.ByteBuffer.wrap(digits).int.toLong() and 0xffffffffL
            code = "%06d".format(value % 1_000_000L)
        }

        /** The next `n` for a message this page sends (§4). */
        fun nextN(): Long {
            n = maxOf(n, seen) + 1
            return n
        }

        fun seal(plaintext: ByteArray, msgId: Int): ByteArray {
            out += 1
            val iv = iv("C2P.", out)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key, GCMParameterSpec(128, iv))
            cipher.updateAAD("$LABEL|c2p|$msgId".toByteArray(Charsets.UTF_8))
            return iv + cipher.doFinal(plaintext)
        }

        fun open(sealed: ByteArray, msgId: Int): ByteArray {
            val iv = sealed.copyOfRange(0, 12)
            assertEquals("the wallet's direction tag", "P2C.", String(iv.copyOfRange(0, 4), Charsets.UTF_8))
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, iv))
            cipher.updateAAD("$LABEL|p2c|$msgId".toByteArray(Charsets.UTF_8))
            val plain = cipher.doFinal(sealed, 12, sealed.size - 12)
            runCatching { JSONObject(String(plain, Charsets.UTF_8)).optLong("n", 0) }
                .getOrNull()?.let { if (it > seen) seen = it }
            return plain
        }

        private fun iv(tag: String, counter: Long): ByteArray {
            val iv = ByteArray(12)
            tag.toByteArray(Charsets.UTF_8).copyInto(iv)
            for (i in 0 until 8) iv[11 - i] = ((counter shr (8 * i)) and 0xff).toByte()
            return iv
        }

        private fun hkdf(ikm: ByteArray, salt: ByteArray, info: String, length: Int): ByteArray {
            val prk = Mac.getInstance("HmacSHA256").apply {
                init(SecretKeySpec(salt, "HmacSHA256"))
            }.doFinal(ikm)
            val mac = Mac.getInstance("HmacSHA256").apply { init(SecretKeySpec(prk, "HmacSHA256")) }
            val out = ByteArray(length)
            var block = ByteArray(0)
            var at = 0
            var counter = 1
            while (at < length) {
                mac.reset()
                mac.update(block)
                mac.update(info.toByteArray(Charsets.UTF_8))
                mac.update(counter.toByte())
                block = mac.doFinal()
                val take = minOf(block.size, length - at)
                block.copyInto(out, at, 0, take)
                at += take
                counter += 1
            }
            return out
        }

        private fun b64url(text: String): ByteArray =
            Base64.getUrlDecoder().decode(text.trimEnd('='))

        private companion object {
            const val LABEL = "vela-ble/1"
        }
    }

    /**
     * A `SecureRandom` that hands out exactly these bytes, in order — so the
     * wire's session is the vectors' session. Past the script it is
     * deterministic rather than random, which only the request ids reach.
     */
    private fun scripted(vararg chunks: ByteArray): SecureRandom {
        val queue = ArrayDeque(chunks.toList())
        return object : SecureRandom() {
            override fun nextBytes(bytes: ByteArray) {
                val next = queue.removeFirstOrNull()
                if (next == null || next.size != bytes.size) {
                    java.util.Random(11).nextBytes(bytes)
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
