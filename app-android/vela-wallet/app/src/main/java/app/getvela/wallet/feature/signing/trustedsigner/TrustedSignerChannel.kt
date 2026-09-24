package app.getvela.wallet.feature.signing.trustedsigner

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.send.core.TrustedSigner
import app.getvela.wallet.feature.send.core.TrustedSignerLabels
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.TrustedSignerCeremonyOutcome
import uniffi.vela_core_uniffi.TrustedSignerOutcome
import uniffi.vela_core_uniffi.TrustedSignerRefusal
import uniffi.vela_core_uniffi.WalletKeyRecord
import java.security.SecureRandom

/**
 * The Trusted Signer, as the rest of the app sees it (specs 071 and 075).
 *
 * It is the fourth passkey route: a page the person reads, which checks the
 * request and runs the WebAuthn ceremony itself. Everything that can ask for
 * one — a send, a dApp request, the key backup, and since 075 creating a
 * wallet, signing in and every proof inside those flows — comes through here.
 *
 * The page is a Custom Tab over the app, answering through the custom scheme
 * ([TrustedSignerScheme]). That is the only channel: the owner retired the
 * cross-device ones on 2026-09-23 ("客户端支持回环 + 蓝牙就够了" and then
 * "我确定砍掉蓝牙"), because only a page THIS device fetched can be checked
 * against what it is supposed to be — and then the loopback socket with them
 * (「回环 WebSocket 不做呀，现在就是纯 custom schema」), because the published
 * page's own `default-src 'none'` makes a socket from inside it impossible.
 *
 * Two decisions live in this class and nowhere else:
 *
 * 1. **Who owns the visit.** A create mints a key and then confirms its
 *    membership; a recovery signs twice. Those are one FLOW, and [endFlow]
 *    ends it — though on the custom-scheme channel each request is its own
 *    page visit, because a URL carries exactly one request.
 * 2. **What a refusal means to the caller.** For a SIGNATURE, nothing signed
 *    is a cancelled ceremony whatever the reason (071 contract §5), so the
 *    request stays open and can be signed another way. For a CEREMONY inside
 *    create or sign-in, a closed page is a cancellation and everything else
 *    carries its own sentence, which is what the machines' failure shapes
 *    already distinguish.
 */
class TrustedSignerChannel(
    /** The page the wallet opens by default (`sign_pref`'s, always usable). */
    private val signerUrl: () -> String,
    /** Open [url] in a Custom Tab over the app; `false` when nothing is on screen to open it from. */
    private val openPage: (url: String) -> Boolean,
    /** Put the app back over the tab. */
    private val bringBack: () -> Unit,
    private val words: () -> Words,
    /** The chain's name and coin, the account's name — from the wallet's own lists. */
    private val labels: (chainId: Int, account: String) -> TrustedSignerLabels = { _, _ -> TrustedSignerLabels() },
    private val timeoutMs: Long = 5 * 60_000L,
    private val random: SecureRandom = SecureRandom(),
    /** This app's own mark, inline PNG, for the page to draw beside the name. */
    private val appIcon: () -> String = { "" },
    /** `Vela Wallet <version>` — what the page shows as the requester, marked there as self-reported. */
    private val appName: String = "vela-android",
) : TrustedSigner {

    /** The sentences a refusal is told in (`componentsUi.signing.trustedSigner*`). */
    data class Words(
        val closed: String,
        val refused: String,
        val mismatch: String,
        val timeout: String,
    )

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

    /** The flow's conversation with one page; `null` between flows. */
    @Volatile
    private var wire: TrustedSignerWire? = null

    /**
     * Why this flow's channel could not be opened at all, when there is a
     * better sentence than "the page was closed".
     *
     * A request that never opened a page comes back as [TrustedSignerAnswer.Cancelled]
     * like any other, and the caller is told "the Trusted Signer was closed
     * without signing" — which is true of a dismissed sheet and a lie about a
     * radio the person was never allowed to turn on. This carries the real
     * reason as far as the sentence, and is cleared at the start of every
     * request so it can never outlive the attempt that set it.
     */
    @Volatile
    private var unopenable: String? = null

    override fun describe(chainId: Int, account: String): TrustedSignerLabels = labels(chainId, account)

    // -- what the screens drive -----------------------------------------------

    /** Cancel, on the waiting sheet: the same as closing the page. */
    fun cancel() {
        wire?.cancel()
    }

    /** "Open the page again" — same port, same token. */
    fun reopen() {
        wire?.reopen()
    }

    /**
     * The flow is over — a create finished or failed, a sign-in settled, the
     * person left the screen. Says `bye`, lets the page go and brings the app
     * back. Safe to call when no flow is open.
     */
    fun endFlow() {
        val live = wire
        wire = null
        cancel()
        live?.end()
        _state.value = State.Idle
        bringBack()
    }

    // -- the two things a caller can ask for ----------------------------------

    /**
     * Spec 071: sign [digest] with one of [keys]. [signerOrigin] is the page
     * the key lives behind, empty for the person's own page.
     *
     * A standalone signature is a flow of one: the page opens, signs and goes.
     * Inside a create or a sign-in the conversation is already open, and this
     * simply puts the next request down it.
     */
    override suspend fun sign(
        requestJson: String,
        digest: ByteArray,
        keys: List<WalletKeyRecord>,
        signerOrigin: String,
    ): Assertion {
        // A signature asked for on its own opens and closes its own flow; one
        // asked for inside a create or a sign-in belongs to that flow's visit.
        // WHICH it is can only be decided under the lock — two callers reading
        // `wire` from outside it would both think they owned the flow, or
        // neither would.
        val put = put(TrustedSignerAsk.Signature(requestJson, digest, keys), signerOrigin)
        try {
            val answer = put.answer
            val outcome = (answer as? TrustedSignerAnswer.Signed)?.outcome
                // Nothing signed: a cancelled ceremony, so the request stays open.
                ?: refuse(FailureKind.Cancelled, sentenceFor(answer))
            return when (outcome) {
                is TrustedSignerOutcome.Accepted -> Assertion(
                    credentialIdHex = outcome.credentialIdHex,
                    signatureDerHex = hex(outcome.assertion.signatureDer),
                    authenticatorDataHex = hex(outcome.assertion.authenticatorData),
                    clientDataJsonHex = hex(outcome.assertion.clientDataJson),
                    userIdHex = null,
                    authenticatorAttachment = "",
                )
                is TrustedSignerOutcome.Refused -> refuse(FailureKind.Cancelled, told(outcome.refusal))
            }
        } finally {
            if (put.ownsFlow) endFlow()
        }
    }

    /**
     * Spec 075: run a passkey ceremony on the page. [operationJson] is the
     * machine operation's own wire JSON; the answer comes back already judged
     * by the core, ready to be reported as the platform ceremony's result.
     *
     * The conversation STAYS OPEN: a create's member proof and a recovery's
     * second signature are the next request of the same visit. [endFlow] ends
     * it.
     *
     * A refusal throws a [PasskeyFailure] — the vocabulary the executor already
     * answers every ceremony's failure in.
     */
    suspend fun ceremony(
        requestJson: String,
        operationJson: String,
        expectedMemberChallenge: ByteArray? = null,
        signerOrigin: String = "",
    ): TrustedSignerCeremonyOutcome {
        val answer = put(
            TrustedSignerAsk.Ceremony(requestJson, operationJson, expectedMemberChallenge),
            signerOrigin,
        ).answer
        val outcome = (answer as? TrustedSignerAnswer.Ceremonial)?.outcome
            ?: refuse(kindOf(answer), sentenceFor(answer))
        if (outcome is TrustedSignerCeremonyOutcome.Refused) {
            refuse(
                if (outcome.refusal == TrustedSignerRefusal.Declined) {
                    FailureKind.Cancelled
                } else {
                    FailureKind.Other
                },
                told(outcome.refusal),
            )
        }
        return outcome
    }

    // -- the machinery --------------------------------------------------------

    /** One request's verdict, and whether this caller opened the flow it rode. */
    private class Put(val answer: TrustedSignerAnswer, val ownsFlow: Boolean)

    private suspend fun put(ask: TrustedSignerAsk, signerOrigin: String): Put =
        one.withLock {
            notice.value = null
            unopenable = null
            val existing = wire
            val mine = existing == null
            val live = existing ?: open(signerOrigin) ?: return@withLock Put(TrustedSignerAnswer.Cancelled, true)
            val answer = try {
                live.ask(ask)
            } catch (cancellation: CancellationException) {
                // The caller went away mid-request: a screen left, a ViewModel
                // cleared, a scope torn down. Nothing was signed, and a page
                // left open would hold a bound socket and a tab in front of an
                // app that is no longer asking for anything.
                endFlow()
                throw cancellation
            }
            if (answer !is TrustedSignerAnswer.Signed && answer !is TrustedSignerAnswer.Ceremonial) {
                // A flow that did not answer has no session left to reuse.
                endFlow()
            } else {
                // Answered. On the custom-scheme channel a visit ENDS with its
                // answer — there is no session to keep, and the next request
                // opens the page again — so the person must be brought back to
                // the wallet now.
                //
                // Leaving the tab up is what the socket channel did, because
                // there the next request arrived on the same connection and the
                // page itself moved on. Here nothing moves the page, and the
                // owner found exactly that: 「签完名后，会卡在 完成，结果已交回
                // 钱包」 — the page says it handed the answer back and the
                // wallet, which had it, was behind the tab.
                _state.value = State.Idle
                bringBack()
            }
            Put(answer, mine)
        }

    /** Open the flow's conversation: a Custom Tab, answering by custom scheme. */
    private fun open(signerOrigin: String): TrustedSignerWire? {
        val page = signerOrigin.ifEmpty { signerUrl() }
        // A channel that cannot even be built is an unopened page, not an
        // exception on its way through the signing paths.
        val built = runCatching {
            TrustedSignerScheme(
                base = page,
                openPage = openPage,
                timeoutMs = timeoutMs,
                random = random,
                onOpened = { url -> _state.value = State.Waiting(url) },
            )
        }.getOrElse { error ->
            VelaLog.failure("trustedsigner", "the channel could not be opened", error)
            _state.value = State.Idle
            return null
        }
        wire = built
        return built
    }

    private fun kindOf(answer: TrustedSignerAnswer): FailureKind = when (answer) {
        TrustedSignerAnswer.Cancelled -> FailureKind.Cancelled
        else -> FailureKind.Other
    }

    /** Why a request came back with nothing, in the person's language. */
    private fun sentenceFor(answer: TrustedSignerAnswer): String {
        val w = words()
        return when (answer) {
            TrustedSignerAnswer.TimedOut -> w.timeout
            is TrustedSignerAnswer.Unreachable -> {
                VelaLog.event(
                    "trustedsigner",
                    "the channel could not carry the request",
                    "why" to answer.why.name,
                    "detail" to answer.detail,
                )
                w.timeout
            }
            is TrustedSignerAnswer.Ceremonial -> told(
                (answer.outcome as? TrustedSignerCeremonyOutcome.Refused)?.refusal
                    ?: TrustedSignerRefusal.Declined,
            )
            is TrustedSignerAnswer.Signed -> told(
                (answer.outcome as? TrustedSignerOutcome.Refused)?.refusal
                    ?: TrustedSignerRefusal.Declined,
            )
            // A channel that never opened says why; everything else that
            // comes back with nothing is a page the person closed.
            TrustedSignerAnswer.Cancelled -> unopenable ?: w.closed
        }
    }

    private fun told(refusal: TrustedSignerRefusal): String {
        val w = words()
        return when (refusal) {
            TrustedSignerRefusal.Declined -> w.closed
            is TrustedSignerRefusal.PageRefused -> w.refused
            else -> {
                // Signed, but not what this wallet asked for — logged, never used.
                VelaLog.event("trustedsigner", "answer refused", "refusal" to refusal.toString())
                w.mismatch
            }
        }
    }

    /**
     * Nothing was signed. The sentence rides on [notice] for the signing
     * sheet, and the failure itself is the vocabulary every caller already
     * classifies on.
     */
    private fun refuse(kind: FailureKind, sentence: String): Nothing {
        notice.value = sentence
        throw PasskeyFailure(kind, sentence)
    }

    private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }
}
