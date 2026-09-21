package app.getvela.wallet.feature.signing.clearsigner

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.send.core.ClearSigner
import app.getvela.wallet.feature.send.core.ClearSignerLabels
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.ClearSignerConnection
import uniffi.vela_core_uniffi.ClearSignerOutcome
import uniffi.vela_core_uniffi.ClearSignerRefusal
import uniffi.vela_core_uniffi.WalletKeyRecord
import uniffi.vela_core_uniffi.clearSignerWsLaunch
import java.io.IOException
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.security.SecureRandom
import java.util.Base64
import java.util.concurrent.ConcurrentLinkedQueue

/**
 * The phones' channel to the Clear Signer (spec 071, contract §2): a socket on
 * this device's own loopback, and the page in a Custom Tab over the app.
 *
 * This class owns I/O and nothing else. Who may connect (the page's origin,
 * the one-time token), what the bytes mean (RFC 6455) and whether the answer
 * is acceptable (this digest, one of these keys, a verified user) are all
 * `ClearSignerConnection`'s — the core's, the same code the page's own suite
 * runs over wasm. A connection that never proves itself changes nothing; the
 * listener keeps listening until one has an outcome, the person cancels, or
 * five minutes pass.
 */
class ClearSignerChannel(
    /** The page the wallet opens (`sign_pref`'s, always usable). */
    private val signerUrl: () -> String,
    /** Open [url] in a Custom Tab over the app; `false` when nothing is on screen to open it from. */
    private val openPage: (url: String) -> Boolean,
    /** Put the app back over the tab. */
    private val bringBack: () -> Unit,
    private val words: () -> Words,
    /** The chain's name and coin, the account's name — from the wallet's own lists. */
    private val labels: (chainId: Int, account: String) -> ClearSignerLabels = { _, _ -> ClearSignerLabels() },
    private val timeoutMs: Long = 5 * 60_000L,
    private val random: SecureRandom = SecureRandom(),
) : ClearSigner {

    /** The sentences a refusal is told in (`componentsUi.signing.clearSigner*`). */
    data class Words(val closed: String, val refused: String, val mismatch: String, val timeout: String)

    sealed interface State {
        data object Idle : State

        /** The page is open (or being opened) at [url] and the socket is listening. */
        data class Waiting(val url: String) : State
    }

    private val _state = MutableStateFlow<State>(State.Idle)
    val state: StateFlow<State> = _state

    /** Why the last attempt did not sign — shown until the next one starts. */
    val notice = MutableStateFlow<String?>(null)

    private val one = Mutex()

    @Volatile
    private var finish: CompletableDeferred<Verdict>? = null

    private sealed interface Verdict {
        data class Answered(val outcome: ClearSignerOutcome) : Verdict
        data object Cancelled : Verdict
        data object TimedOut : Verdict
    }

    override fun describe(chainId: Int, account: String): ClearSignerLabels = labels(chainId, account)

    /** The waiting sheet's Cancel: the same as closing the page. */
    fun cancel() {
        finish?.complete(Verdict.Cancelled)
    }

    /** "Open the page again" — same port, same token: the core lets a new connection prove itself. */
    fun reopen() {
        (state.value as? State.Waiting)?.let { openPage(it.url) }
    }

    override suspend fun sign(requestJson: String, digest: ByteArray, keys: List<WalletKeyRecord>): Assertion = one.withLock {
        notice.value = null
        val base = signerUrl()
        val server = withContext(Dispatchers.IO) {
            ServerSocket(0, 8, InetAddress.getByAddress(byteArrayOf(127, 0, 0, 1)))
        }
        val token = Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(16).also(random::nextBytes))
        val id = ByteArray(8).also(random::nextBytes).joinToString("") { "%02x".format(it) }
        val url = clearSignerWsLaunch(base, server.localPort.toUShort(), token)
        val done = CompletableDeferred<Verdict>()
        val sockets = ConcurrentLinkedQueue<Socket>()
        finish = done
        val verdict = try {
            coroutineScope {
                val acceptor = launch(Dispatchers.IO) {
                    while (isActive) {
                        val socket = try {
                            server.accept()
                        } catch (_: IOException) {
                            break
                        }
                        sockets += socket
                        launch { serve(socket, ClearSignerConnection(base, token, id, requestJson, digest, keys), done) }
                    }
                }
                _state.value = State.Waiting(url)
                if (!openPage(url)) done.complete(Verdict.Cancelled)
                val verdict = withTimeoutOrNull(timeoutMs) { done.await() } ?: Verdict.TimedOut
                // A blocked accept/read only lets go when its socket closes.
                runCatching { server.close() }
                sockets.forEach { runCatching { it.close() } }
                acceptor.cancel()
                verdict
            }
        } finally {
            runCatching { server.close() }
            finish = null
            _state.value = State.Idle
        }
        bringBack()
        judged(verdict)
    }

    /** One accepted connection, byte for byte through the core. */
    private fun serve(socket: Socket, connection: ClearSignerConnection, done: CompletableDeferred<Verdict>) {
        connection.use { conn ->
            try {
                socket.soTimeout = 0
                val input = socket.getInputStream()
                val output = socket.getOutputStream()
                val buffer = ByteArray(16 * 1024)
                while (true) {
                    val read = input.read(buffer)
                    if (read < 0) break
                    val step = conn.feed(buffer.copyOf(read))
                    if (step.write.isNotEmpty()) {
                        output.write(step.write)
                        output.flush()
                    }
                    step.outcome?.let { done.complete(Verdict.Answered(it)) }
                    if (step.close) break
                }
            } catch (_: IOException) {
                // The page went away mid-frame, or we closed it: `closed()` says which it means.
            } finally {
                conn.closed()?.let { done.complete(Verdict.Answered(ClearSignerOutcome.Refused(it))) }
                runCatching { socket.close() }
            }
        }
    }

    private fun judged(verdict: Verdict): Assertion {
        val words = words()
        val outcome = when (verdict) {
            Verdict.Cancelled -> refuse(FailureKind.Cancelled, words.closed)
            Verdict.TimedOut -> refuse(FailureKind.Other, words.timeout)
            is Verdict.Answered -> verdict.outcome
        }
        return when (outcome) {
            is ClearSignerOutcome.Accepted -> Assertion(
                credentialIdHex = outcome.credentialIdHex,
                signatureDerHex = hex(outcome.assertion.signatureDer),
                authenticatorDataHex = hex(outcome.assertion.authenticatorData),
                clientDataJsonHex = hex(outcome.assertion.clientDataJson),
                userIdHex = null,
                authenticatorAttachment = "",
            )
            is ClearSignerOutcome.Refused -> when (val refusal = outcome.refusal) {
                ClearSignerRefusal.Declined -> refuse(FailureKind.Cancelled, words.closed)
                is ClearSignerRefusal.PageRefused -> refuse(FailureKind.Other, words.refused)
                else -> {
                    // Signed, but not what this wallet asked for — logged, never submitted.
                    VelaLog.event("clearsigner", "answer refused", "refusal" to refusal.toString())
                    refuse(FailureKind.Other, words.mismatch)
                }
            }
        }
    }

    /**
     * Nothing was signed, whatever the reason: to the signing paths that is a
     * cancelled ceremony, so the request stays open to be signed another way
     * (contract §5) — a site is never answered with an error for a page's
     * refusal or a timeout. The sentence rides on [notice].
     */
    private fun refuse(@Suppress("UNUSED_PARAMETER") kind: FailureKind, sentence: String): Nothing {
        notice.value = sentence
        throw PasskeyFailure(FailureKind.Cancelled, sentence)
    }

    private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }
}
