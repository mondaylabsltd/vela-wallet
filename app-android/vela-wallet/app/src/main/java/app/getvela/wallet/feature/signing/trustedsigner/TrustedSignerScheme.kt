package app.getvela.wallet.feature.signing.trustedsigner

import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.TrustedSignerCallback
import uniffi.vela_core_uniffi.TrustedSignerCeremonyOutcome
import uniffi.vela_core_uniffi.TrustedSignerOutcome
import uniffi.vela_core_uniffi.TrustedSignerRefusal
import uniffi.vela_core_uniffi.trustedSignerCallbackToken
import uniffi.vela_core_uniffi.trustedSignerParseCallback
import uniffi.vela_core_uniffi.trustedSignerUrlLaunch
import uniffi.vela_core_uniffi.trustedSignerVerify
import uniffi.vela_core_uniffi.trustedSignerVerifyCeremony
import java.security.SecureRandom
import java.util.Base64
import java.util.concurrent.ConcurrentHashMap

/**
 * The phones' one channel to the Trusted Signer: the request goes out in a URL
 * the page is opened with, and the answer comes back as a
 * `velawallet://sign-result?…` the OS hands this app.
 *
 * **Why this and not a socket.** The published page carries `default-src
 * 'none'` inside the bytes that are hashed (spec 076), so a page whose hash
 * this wallet accepts cannot open a WebSocket at all — measured in a real
 * browser, the listening server saw no byte. A NAVIGATION is not governed by
 * that CSP, also measured. And a phone app is suspended the moment the browser
 * covers it, so it could not have accepted a loopback connection anyway. The
 * loopback channel this replaces was cut by the owner on 2026-09-23
 * (「回环 WebSocket 不做呀，现在就是纯 custom schema」).
 *
 * **What the transport is not.** Any app may register `velawallet://`, and
 * which one wins a collision is not defined. So this carries the answer and
 * authorises nothing: `trusted_signer_verify` accepts only a `webauthn.get`
 * over exactly the digest this wallet computed, by a credential it holds, so a
 * forged callback is refused. What an interception costs is the wallet waiting
 * (availability) and the interceptor learning the assertion (confidentiality),
 * and that is worth saying plainly rather than implying the scheme is a
 * boundary.
 *
 * **One visit per request, not per flow.** The socket channel kept one page
 * open for a whole flow, because the next request could go down the same
 * connection. A URL carries exactly one request, so a create's member proof is
 * a second visit. That is the honest cost of the transport, and [ask] never
 * pretends otherwise.
 */
class TrustedSignerScheme(
    /** The page to open — a `signer_origin`, or the person's own from Settings. */
    private val base: String,
    /** Open [url] in a Custom Tab over the app; `false` when nothing is on screen to open it from. */
    private val openPage: (url: String) -> Boolean,
    private val timeoutMs: Long,
    private val random: SecureRandom,
    /** Called whenever the page's address changes (the waiting sheet's "open again"). */
    private val onOpened: (url: String) -> Unit = {},
    /** The page the answer must have come from, for a ceremony's origin check. */
    private val signerOrigin: String = base,
) : TrustedSignerWire {

    /** The request in flight: its one-time token and the URL that carries it. */
    private class Visit(val token: String, val url: String)

    @Volatile
    private var visit: Visit? = null

    @Volatile
    private var over = false

    /**
     * Cancel is sticky. It can arrive in the breath between this wire being
     * built and the first request waiting on anything — the Where sheet's
     * Cancel does exactly that — and a cancel dropped there left the person
     * looking at a sheet that answered nothing for five minutes.
     */
    @Volatile
    private var stopped = false

    override suspend fun ask(ask: TrustedSignerAsk): TrustedSignerAnswer {
        if (over) return TrustedSignerAnswer.Cancelled
        if (stopped) {
            end()
            return TrustedSignerAnswer.Cancelled
        }
        // A token per REQUEST, not per flow: "one-time" is then literally true,
        // and no answer to the first request can be read as the second's.
        val token = Base64.getUrlEncoder().withoutPadding()
            .encodeToString(ByteArray(16).also(random::nextBytes))
        val url = runCatching { trustedSignerUrlLaunch(base, ask.requestJson, token) }
            .getOrElse { error ->
                VelaLog.failure("trustedsigner", "the request could not be put in a URL", error)
                return TrustedSignerAnswer.Unreachable(error.toString())
            }
        // `null` is how a cancel settles the wait, so it stays one shape.
        val waiting = CompletableDeferred<String?>()
        TrustedSignerCallbacks.await(token, waiting)
        visit = Visit(token, url)
        try {
            onOpened(url)
            if (!openPage(url)) {
                return TrustedSignerAnswer.Unreachable("no screen to open the page from")
            }
            val callbackUrl = withTimeoutOrNull(timeoutMs) { waiting.await() }
                ?: return if (stopped) TrustedSignerAnswer.Cancelled else TrustedSignerAnswer.TimedOut
            if (callbackUrl == null) return TrustedSignerAnswer.Cancelled
            return judge(ask, token, callbackUrl)
        } finally {
            TrustedSignerCallbacks.forget(token)
            visit = null
        }
    }

    /**
     * The core reads the callback, and the core judges what it carried.
     *
     * The answer goes to the judge **unwrapped**, because that is what the URL
     * channel carries: the page sends a signature's `result` object and a
     * ceremony's own envelope (`intake.js`, `body.result !== undefined ?
     * body.result : body`). Putting a `result` wrapper back round both would
     * make every ceremony unreadable.
     */
    private fun judge(ask: TrustedSignerAsk, token: String, callbackUrl: String): TrustedSignerAnswer =
        when (val callback = trustedSignerParseCallback(callbackUrl, token)) {
            is TrustedSignerCallback.Answered -> when (ask) {
                is TrustedSignerAsk.Signature -> TrustedSignerAnswer.Signed(
                    trustedSignerVerify(callback.answerJson, ask.digest, ask.keys),
                )

                is TrustedSignerAsk.Ceremony -> TrustedSignerAnswer.Ceremonial(
                    trustedSignerVerifyCeremony(
                        ask.operationJson,
                        callback.answerJson,
                        signerOrigin,
                        ask.expectedMemberChallenge,
                    ),
                )
            }

            is TrustedSignerCallback.Refused -> refused(ask, callback.refusal)
        }

    private fun refused(ask: TrustedSignerAsk, refusal: TrustedSignerRefusal): TrustedSignerAnswer =
        when (ask) {
            is TrustedSignerAsk.Signature ->
                TrustedSignerAnswer.Signed(TrustedSignerOutcome.Refused(refusal))

            is TrustedSignerAsk.Ceremony ->
                TrustedSignerAnswer.Ceremonial(TrustedSignerCeremonyOutcome.Refused(refusal))
        }

    override fun cancel() {
        stopped = true
        visit?.let { TrustedSignerCallbacks.give(it.token, null) }
    }

    override fun reopen() {
        if (!over) visit?.let { openPage(it.url) }
    }

    override fun end() {
        if (over) return
        over = true
        visit?.let { TrustedSignerCallbacks.give(it.token, null) }
    }
}

/**
 * The callbacks this process is waiting for, by the one-time token they name.
 *
 * Process-wide rather than held by the activity, because the two ends are in
 * different places: a request waits inside whatever coroutine asked for a
 * signature, and the callback arrives in `MainActivity.onNewIntent`, which may
 * not even be the instance the flow started on.
 *
 * Keyed by the token, and only the token: it is what says WHICH request a
 * callback belongs to. A callback for a token nobody is waiting on reaches
 * nothing and is dropped in silence — no route change, no state change, no
 * error a person cannot act on.
 */
object TrustedSignerCallbacks {

    private val waiting = ConcurrentHashMap<String, CompletableDeferred<String?>>()

    internal fun await(token: String, sink: CompletableDeferred<String?>) {
        waiting[token] = sink
    }

    internal fun forget(token: String) {
        waiting.remove(token)
    }

    /** Settle [token]'s request with [callbackUrl], or cancel it with `null`. */
    internal fun give(token: String, callbackUrl: String?) {
        waiting.remove(token)?.complete(callbackUrl)
    }

    /**
     * A `velawallet://…` the OS handed this app.
     *
     * **It is an event for a pending request, not a navigation.** `true` when
     * it settled one, and `false` for every other URL — the wallet's own `pay`
     * and `open` links included — which the caller then routes as it always
     * did. Nothing here touches the screen either way.
     */
    fun deliver(url: String): Boolean {
        val token = runCatching { trustedSignerCallbackToken(url) }.getOrNull() ?: return false
        val sink = waiting.remove(token)
        if (sink == null) {
            // Late, replayed, or another app's: there is nothing to tell the
            // person, and nothing they could do about it.
            VelaLog.event("trustedsigner", "a callback arrived for no pending request")
            return false
        }
        sink.complete(url)
        return true
    }
}
