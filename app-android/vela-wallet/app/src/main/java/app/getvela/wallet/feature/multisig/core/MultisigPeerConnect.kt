package app.getvela.wallet.feature.multisig.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.net.VelaHttp
import java.security.SecureRandom
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString
import uniffi.vela_core_uniffi.PeerConnectPeer
import uniffi.vela_core_uniffi.PeerOutput
import uniffi.vela_core_uniffi.PeerRoomRecord
import uniffi.vela_core_uniffi.peerconnectCreateRoom
import uniffi.vela_core_uniffi.peerconnectJoinRoom
import uniffi.vela_core_uniffi.peerconnectRoomRelay

/**
 * The relay and the token server, as a room reaches them: a WebSocket and
 * one POST. Tests hand their own; the app's is OkHttp ([OkHttpPeerNet]).
 */
interface PeerNet {
    /** Open a socket to [url]; its news goes to [listener]. */
    fun connect(url: String, listener: PeerSocketListener): PeerSocket

    /** POST [body] (JSON, or nothing): the status (`null`: no answer) and the text. */
    suspend fun post(url: String, body: String?): Pair<Int?, String>
}

interface PeerSocket {
    fun send(bytes: ByteArray)
    fun sendText(text: String)
    fun close()
}

interface PeerSocketListener {
    fun opened()
    fun text(text: String)
    fun binary(bytes: ByteArray)
    /** Closed or failed (the close code, when one came). Said once. */
    fun closed(code: Int?)
}

/**
 * The PeerConnect transport behind the multisig machines' `peer_*`
 * operations (native multisig M4) — the web's `shell/peer.ts`, the
 * desktop's `multisig/peer.rs`: rooms (Noise end to end; the relay reads
 * nothing) and the public relay's token. The protocol is PeerConnect's own
 * sans-I/O peer over uniffi (`PeerConnectPeer`, the vendored crate the web
 * runs as wasm); this file supplies the socket, the clock and a coroutine
 * per room. What is sent and what an arrival means are the core's
 * (`multisig_outside`); every relay judgement too (`relay.*` through the
 * link door: `needsToken`, `usable`, `tokenAnswer`, `refusals`,
 * `noTokenStep`, `onState`, `forgets`).
 *
 * A room is known to the core by a handle handed out here (`room-N`); what
 * the connection says of itself goes back to the machine that opened it as
 * its own events ([tell]): `peer_state`, `paired` (the record that reopens
 * it now) and `relay` (where getting onto it stands).
 */
class MultisigPeerConnect(
    private val scope: CoroutineScope,
    private val store: KeyValueStore,
    private val net: PeerNet = OkHttpPeerNet,
) : MultisigPeerRooms {

    @Volatile
    override var tell: ((JsonObject) -> Unit)? = null

    private val rooms = ConcurrentHashMap<String, Room>()
    private val next = AtomicLong(0)

    override suspend fun open(relay: String?, invite: String?, pairing: String?): PeerOpened {
        val made = when {
            pairing != null -> PeerRoomRecord(pairing, null, peerconnectRoomRelay(pairing))
            invite != null -> peerconnectJoinRoom(invite)
            else -> peerconnectCreateRoom(relay ?: relayUrl())
        }
        val handle = "room-${next.incrementAndGet()}"
        val needsToken = MsRules.call("relay.needsToken", mapOf("relay" to made.relay)).bool ?: false
        val room = Room(handle, made.record, made.relay, needsToken)
        rooms[handle] = room
        room.start()
        return PeerOpened(handle, if (pairing == null && invite == null) made.invite else null, made.record)
    }

    override suspend fun send(room: String, message: JsonElement) {
        rooms[room]?.send(message)
    }

    override suspend fun receive(room: String): PeerArrival = rooms[room]?.receive() ?: PeerArrival.Closed("closed")

    /** `walletpair_cancel` is the core's message, sent before this (`cancel`). */
    override suspend fun close(room: String, cancel: Boolean, end: Boolean) {
        rooms.remove(room)?.stop(end)
    }

    override fun retryAll() {
        rooms.values.forEach { it.retry() }
    }

    override fun leaveAll() {
        rooms.values.forEach { it.stop(end = false) }
        rooms.clear()
    }

    /** The relay new rooms go through: the one the device names, else PeerConnect's. */
    private suspend fun relayUrl(): String =
        store.read(RELAY_KEY)?.trim()?.ifEmpty { null } ?: DEFAULT_RELAY

    private fun say(event: JsonObject) {
        runCatching { tell?.invoke(event) }.onFailure { VelaLog.failure("multisig.peer", "tell", it) }
    }

    /** What a room's loop is told: the socket's news, the app's asks. */
    private sealed interface Input {
        data class Opened(val attempt: Long) : Input
        data class Closed(val code: Int?, val attempt: Long) : Input
        data class Text(val text: String, val attempt: Long) : Input
        class Binary(val bytes: ByteArray, val attempt: Long) : Input
        class Send(val bytes: ByteArray) : Input
        data class Token(val token: String) : Input
        data class Unreachable(val generation: Long) : Input
        data class Stop(val end: Boolean) : Input
        data object Retry : Input
    }

    /** One room: its peer, its socket, its inbox, its loop, its way onto the relay. */
    private inner class Room(val handle: String, record: String, val relay: String, val needsToken: Boolean) {
        private val started = System.nanoTime()

        /** The clock moved on by a Retry (the JS SDK's `reconnectNow`). */
        private var ahead = 0L
        private fun now(): ULong = ((System.nanoTime() - started) / 1_000_000 + ahead).toULong()

        private val peer = PeerConnectPeer(record, needsToken, now())
        private val inputs = Channel<Input>(Channel.UNLIMITED)
        private val inbox = Channel<JsonElement>(Channel.UNLIMITED)
        private val closed = CompletableDeferred<String>()
        private val receiving = Mutex()
        private val watch = Watch(handle, relay)
        private var socket: PeerSocket? = null
        private var attempt = 0L
        private var connecting = false
        private var state = "offline"
        private var unreachable: Job? = null
        private var unreachableGeneration = 0L

        fun start() {
            scope.launch(Dispatchers.IO) { loop() }
            if (needsToken) fetchToken()
        }

        fun send(message: JsonElement) {
            val text = if (message is JsonPrimitive && message.isString) message.content else message.toString()
            inputs.trySend(Input.Send(text.toByteArray(Charsets.UTF_8)))
        }

        suspend fun receive(): PeerArrival = receiving.withLock {
            inbox.tryReceive().getOrNull()?.let { return@withLock PeerArrival.Message(it) }
            if (closed.isCompleted) return@withLock PeerArrival.Closed(closed.getCompleted())
            select {
                inbox.onReceiveCatching { got ->
                    got.getOrNull()?.let { PeerArrival.Message(it) } ?: PeerArrival.Closed(closed.await())
                }
                closed.onAwait { PeerArrival.Closed(it) }
            }
        }

        fun stop(end: Boolean) {
            watch.end()
            closed.complete(if (end) "closed" else "left")
            inputs.trySend(Input.Stop(end))
        }

        /** The person's Retry: a wait cut short, a stop undone, the relay tried now. */
        fun retry() {
            watch.wake()
            inputs.trySend(Input.Retry)
        }

        private suspend fun loop() {
            pump()
            while (true) {
                val wait = peer.pollTimeout()?.let { (it.toLong() - now().toLong()).coerceAtLeast(0) }
                val input = if (wait == null) inputs.receive() else withTimeoutOrNull(wait) { inputs.receive() }
                val t = now()
                try {
                    when (input) {
                        null -> peer.handleTimeout(t)
                        is Input.Opened -> if (input.attempt == attempt && connecting) {
                            connecting = false
                            peer.handleOpen(t)
                        }
                        is Input.Closed -> if (input.attempt == attempt && (connecting || socket != null)) {
                            connecting = false
                            socket = null
                            peer.handleClose(t, input.code?.toUShort())
                        }
                        is Input.Text -> if (input.attempt == attempt) peer.handleText(t, input.text)
                        is Input.Binary -> if (input.attempt == attempt) peer.handleBinary(t, input.bytes)
                        is Input.Send -> peer.send(input.bytes)
                        is Input.Token -> peer.setToken(t, input.token)
                        is Input.Unreachable -> if (input.generation == unreachableGeneration) {
                            watch.onState(state, UNREACHABLE_AFTER_MS, needsToken)
                        }
                        Input.Retry -> reconnectNow()
                        is Input.Stop -> {
                            if (input.end) peer.end() else peer.leave()
                            pump()
                            socket?.close()
                            socket = null
                            unreachable?.cancel()
                            inbox.close()
                            peer.destroy()
                            return
                        }
                    }
                } catch (e: Exception) {
                    VelaLog.failure("multisig.peer", "room input", e, "room" to handle)
                }
                pump()
            }
        }

        /** Offline and waiting to try the relay again: tried now, the clock moved on to that moment. */
        private fun reconnectNow() {
            if (state != "offline" || socket != null || connecting) return
            val at = peer.pollTimeout()?.toLong() ?: return
            val n = now().toLong()
            if (at > n) ahead += at - n
            peer.handleTimeout(now())
        }

        /** Everything the peer asks, done in order. */
        private fun pump() {
            while (true) {
                when (val out = peer.pollOutput() ?: return) {
                    is PeerOutput.Connect -> connect(out.url)
                    is PeerOutput.Send -> socket?.send(out.bytes)
                    PeerOutput.Ping -> socket?.sendText("ping")
                    PeerOutput.Disconnect -> {
                        socket?.close()
                        socket = null
                        connecting = false
                        attempt += 1
                    }
                    is PeerOutput.State -> {
                        state = out.state
                        checkState()
                        say(ev("peer_state", "room" to handle, "state" to out.state))
                        if (out.state == "closed") closed.complete(out.reason ?: "closed")
                    }
                    is PeerOutput.Message -> {
                        val text = out.bytes.toString(Charsets.UTF_8)
                        inbox.trySend(runCatching { Wire.json.parseToJsonElement(text) }.getOrElse { JsonPrimitive(text) })
                    }
                    is PeerOutput.Delivered, PeerOutput.Replaced -> Unit
                    is PeerOutput.Paired -> say(ev("paired", "room" to handle, "pairing" to out.record))
                    PeerOutput.Unauthorized -> if (needsToken) {
                        // Before the next token is asked for: that one must not be the one just refused.
                        val refused = watch.unauthorized()
                        scope.launch(Dispatchers.IO) {
                            forgetToken(refused)
                            fetchToken()
                        }
                    }
                }
            }
        }

        /** Where the way onto the relay stands, as the core reads the state; offline for the core's while, said as the relay not answering. */
        private fun checkState() {
            unreachable?.cancel()
            val generation = ++unreachableGeneration
            val accepted = watch.onState(state, 0.0, needsToken)
            if (accepted || state == "closed") return
            unreachable = scope.launch {
                delay(UNREACHABLE_AFTER_MS.toLong())
                inputs.trySend(Input.Unreachable(generation))
            }
        }

        private fun connect(url: String) {
            attempt += 1
            val mine = attempt
            connecting = true
            socket = net.connect(
                url,
                object : PeerSocketListener {
                    override fun opened() {
                        inputs.trySend(Input.Opened(mine))
                    }

                    override fun text(text: String) {
                        inputs.trySend(Input.Text(text, mine))
                    }

                    override fun binary(bytes: ByteArray) {
                        inputs.trySend(Input.Binary(bytes, mine))
                    }

                    override fun closed(code: Int?) {
                        inputs.trySend(Input.Closed(code, mine))
                    }
                },
            )
        }

        /** A token for the peer, on a coroutine of its own (one at a time). */
        fun fetchToken() {
            if (!watch.beginFetch()) return
            scope.launch(Dispatchers.IO) {
                val token = try {
                    tokenFor()
                } catch (e: Exception) {
                    VelaLog.failure("multisig.peer", "token", e, "room" to handle)
                    null
                } finally {
                    watch.endFetch()
                }
                if (token != null) inputs.trySend(Input.Token(token))
            }
        }

        /**
         * The token for the peer, as the core schedules it: a refusal
         * waited out (or stopped, until Retry), then the kept one or a new
         * one, with the waits and the stop when none comes. `null`: the
         * room is gone.
         */
        private suspend fun tokenFor(): String? {
            var before = watch.refusals("beforeToken")
            while (before.kind != "go") {
                if (before.kind == "wait") {
                    if (!watch.waitOut("refused", "HTTP 4403", before["ms"].long ?: 2_000)) return null
                } else {
                    if (!watch.stopUntilRetry("refused", "HTTP 4403")) return null
                    watch.refusals("retried")
                }
                before = watch.refusals("beforeToken")
            }
            // The last chance before stopping is the trial's, once.
            val trial = before["trial"].bool ?: false
            var tries = 0
            while (true) {
                if (watch.over) return null
                watch.set(jo("kind" to "connecting"))
                when (val got = relayToken(trial)) {
                    is Got.Token -> {
                        watch.issued(got.token, got.source)
                        return got.token
                    }
                    is Got.Nothing -> {
                        val step = MsRules.call("relay.noTokenStep", mapOf("tries" to tries))
                        if (step.kind == "wait") {
                            if (!watch.waitOut("noToken", got.detail, step["ms"].long ?: 2_000)) return null
                            tries += 1
                        } else {
                            if (!watch.stopUntilRetry("noToken", got.detail)) return null
                            tries = 0
                        }
                    }
                }
            }
        }
    }

    /** One room's way onto the relay, as the screen says it, with the refusals counted and the waits a Retry cuts short. */
    private inner class Watch(val handle: String, val relay: String) {
        private var status: JsonElement = jo("kind" to "connecting")
        private var refusals: JsonElement? = null
        private var last: String? = null
        private var fetching = false
        private val wakes = MutableStateFlow(0)

        @Volatile
        var over = false
            private set

        @Synchronized
        fun set(next: JsonElement) {
            if (next == status) return
            status = next
            say(ev("relay", "room" to handle, "status" to next))
        }

        @Synchronized
        fun refusals(on: String, source: String? = null): JsonElement? {
            val answer = MsRules.call("relay.refusals", mapOf("state" to refusals, "on" to on, "source" to source))
            answer["state"]?.let { refusals = it }
            return answer["before"]
        }

        /** The relay refused the token in use: counted; the token handed back. */
        @Synchronized
        fun unauthorized(): String? {
            refusals("unauthorized")
            return last
        }

        @Synchronized
        fun issued(token: String, source: String) {
            last = token
            refusals("issued", source)
        }

        @Synchronized
        fun beginFetch(): Boolean {
            if (fetching || over) return false
            fetching = true
            return true
        }

        @Synchronized
        fun endFetch() {
            fetching = false
        }

        /** The core's reading of the connection's state: a new status said; on the relay, the refusals forgotten. Whether it is on the relay. */
        @Synchronized
        fun onState(state: String, offlineMs: Double, token: Boolean): Boolean {
            val answer = MsRules.call(
                "relay.onState",
                mapOf("status" to status, "state" to state, "offlineMs" to offlineMs, "relay" to relay),
            )
            val accepted = answer["accepted"].bool ?: false
            if (accepted && token) refusals("accepted")
            answer["status"]?.let(::set)
            return accepted
        }

        /** Wait [ms], said as retrying for [why] — less, if Retry is tapped. `false`: the room is gone. */
        suspend fun waitOut(why: String, detail: String, ms: Long): Boolean {
            val from = wakes.value
            set(jo("kind" to "retrying", "why" to why, "detail" to detail, "at" to (System.currentTimeMillis() + ms).toDouble()))
            withTimeoutOrNull(ms) { wakes.first { it != from } }
            return !over
        }

        /** Stopped, said as failed for [why], until Retry. `false`: the room is gone. */
        suspend fun stopUntilRetry(why: String, detail: String): Boolean {
            val from = wakes.value
            set(jo("kind" to "failed", "why" to why, "detail" to detail))
            wakes.first { it != from }
            return !over
        }

        fun wake() {
            wakes.value += 1
        }

        fun end() {
            over = true
            wakes.value += 1
        }
    }

    private sealed interface Got {
        data class Token(val token: String, val source: String) : Got
        data class Nothing(val detail: String) : Got
    }

    /** Two rooms opening at once share one request. */
    private val asking = Mutex()

    /**
     * A token for the public relay: the one kept, if the core finds it still
     * good, else a new one from Vela's site under this device's ID, else a
     * trial one (`trial`: the trial's, unless the one kept is the trial's
     * already). [Got.Nothing]: what each place answered, in the core's words.
     */
    private suspend fun relayToken(trial: Boolean): Got {
        usable(trial)?.let { return it }
        return asking.withLock {
            // Another room may have just asked.
            usable(trial)?.let { return@withLock it }
            val endpoint = store.read(ENDPOINT_KEY)?.trim()?.ifEmpty { null } ?: TOKEN_ENDPOINT
            val site = if (trial) null else ask(endpoint, jo("device" to deviceId()).toString())
            val siteOk = site["ok"]
            val (fresh, source) = if (siteOk != null) {
                siteOk to "site"
            } else {
                val tried = ask(TRIAL_ENDPOINT, null)
                val trialOk = tried["ok"] ?: return@withLock Got.Nothing(
                    MsRules.call(
                        "relay.noTokenDetail",
                        mapOf("endpoint" to endpoint, "site" to site["err"].str, "trial" to (tried["err"].str ?: "no answer")),
                    ).str ?: "no token",
                )
                trialOk to "trial"
            }
            val kept = JsonObject(fresh.obj + ("source" to JsonPrimitive(source)))
            store.write(TOKEN_KEY, kept.toString())
            val token = kept["token"].str ?: return@withLock Got.Nothing("no token")
            Got.Token(token, source)
        }
    }

    private suspend fun usable(trial: Boolean): Got.Token? {
        val answer = MsRules.call(
            "relay.usable",
            mapOf("kept" to keptToken(), "nowS" to System.currentTimeMillis() / 1000.0, "trial" to trial),
        )
        val token = answer["token"].str ?: return null
        return Got.Token(token, answer["source"].str ?: "site")
    }

    /** One endpoint asked; what it answered read by the core (`{ok}` or `{err}`). */
    private suspend fun ask(url: String, body: String?): JsonElement? {
        val (status, text) = net.post(url, body)
        return MsRules.call("relay.tokenAnswer", mapOf("status" to status, "body" to text))
    }

    private suspend fun keptToken(): JsonElement? =
        store.read(TOKEN_KEY)?.let { runCatching { Wire.json.parseToJsonElement(it) }.getOrNull() }

    /** The relay refused [refused]: the kept token goes when it is that one (the core's `forgets`). */
    private suspend fun forgetToken(refused: String?) {
        val forgets = MsRules.call("relay.forgets", mapOf("kept" to keptToken(), "refused" to refused)).bool ?: false
        if (forgets) store.remove(TOKEN_KEY)
    }

    /** This device's ID at Vela's site, made once. */
    private suspend fun deviceId(): String {
        store.read(DEVICE_KEY)?.trim()?.takeIf { it.length >= 16 }?.let { return it }
        val made = ByteArray(16).also(SecureRandom()::nextBytes).joinToString("") { "%02x".format(it) }
        store.write(DEVICE_KEY, made)
        return made
    }

    companion object {
        /** The relay new rooms go through, when the device names one (the web's key). */
        const val RELAY_KEY = "vela.peerconnectRelay"
        /** Where tokens are asked for, when the device names a place. */
        const val ENDPOINT_KEY = "vela.peerconnectTokenEndpoint"
        const val DEVICE_KEY = "vela.peerconnectDevice"
        const val TOKEN_KEY = "vela.peerconnectToken.v1"

        private val constants by lazy { MsRules.call("relay.constants") }
        val DEFAULT_RELAY: String get() = constants["defaultRelay"].str ?: "wss://relay.peerconnect.dev"
        val TOKEN_ENDPOINT: String get() = constants["tokenEndpoint"].str ?: "https://getvela.app/api/peerconnect-token"
        val TRIAL_ENDPOINT: String get() = constants["trialEndpoint"].str ?: "https://peerconnect.dev/api/trial"
        val UNREACHABLE_AFTER_MS: Double get() = constants["unreachableAfterMs"].long?.toDouble() ?: 10_000.0
    }
}

/** The app's socket and POST: OkHttp, the app's client (no read timeout on a socket). */
object OkHttpPeerNet : PeerNet {
    private val sockets: OkHttpClient by lazy {
        VelaHttp.client.newBuilder().readTimeout(0, TimeUnit.MILLISECONDS).build()
    }
    private val posts: OkHttpClient by lazy {
        VelaHttp.client.newBuilder().callTimeout(15, TimeUnit.SECONDS).build()
    }

    override fun connect(url: String, listener: PeerSocketListener): PeerSocket {
        val said = AtomicBoolean(false)
        fun closedOnce(code: Int?) {
            if (said.compareAndSet(false, true)) listener.closed(code)
        }
        val socket = sockets.newWebSocket(
            Request.Builder().url(url).build(),
            object : WebSocketListener() {
                override fun onOpen(webSocket: WebSocket, response: Response) = listener.opened()
                override fun onMessage(webSocket: WebSocket, text: String) = listener.text(text)
                override fun onMessage(webSocket: WebSocket, bytes: ByteString) = listener.binary(bytes.toByteArray())
                override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                    webSocket.close(1000, null)
                    closedOnce(code)
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) = closedOnce(code)
                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) = closedOnce(null)
            },
        )
        return object : PeerSocket {
            override fun send(bytes: ByteArray) {
                socket.send(bytes.toByteString())
            }

            override fun sendText(text: String) {
                socket.send(text)
            }

            override fun close() {
                socket.close(1000, null)
            }
        }
    }

    override suspend fun post(url: String, body: String?): Pair<Int?, String> = withContext(Dispatchers.IO) {
        runCatching {
            val request = Request.Builder().url(url)
                .post((body ?: "").toRequestBody("application/json".toMediaType())).build()
            posts.newCall(request).execute().use { it.code to (it.body?.string().orEmpty()) }
        }.getOrElse { null to (it.message ?: it.javaClass.simpleName) }
    }
}
