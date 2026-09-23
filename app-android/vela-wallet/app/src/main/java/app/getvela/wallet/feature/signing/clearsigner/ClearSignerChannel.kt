package app.getvela.wallet.feature.signing.clearsigner

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.send.core.ClearSigner
import app.getvela.wallet.feature.send.core.ClearSignerLabels
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.ClearSignerCeremonyOutcome
import uniffi.vela_core_uniffi.ClearSignerOutcome
import uniffi.vela_core_uniffi.ClearSignerRefusal
import uniffi.vela_core_uniffi.WalletKeyRecord
import java.security.SecureRandom

/**
 * The Clear Signer, as the rest of the app sees it (specs 071 and 075).
 *
 * It is the fourth passkey route: a page the person reads, which checks the
 * request and runs the WebAuthn ceremony itself. Everything that can ask for
 * one — a send, a dApp request, the key backup, and since 075 creating a
 * wallet, signing in and every proof inside those flows — comes through here.
 *
 * The page is a Custom Tab over the app, talking to a socket on this phone's
 * own loopback ([ClearSignerLoopback]). That is the only channel: the owner
 * retired the cross-device ones on 2026-09-23 ("客户端支持回环 + 蓝牙就够了"
 * and then "我确定砍掉蓝牙"), because only a page THIS device fetched can be
 * checked against what it is supposed to be.
 *
 * Two decisions live in this class and nowhere else:
 *
 * 1. **One page visit per flow.** A create mints a key and then confirms its
 *    membership; a recovery signs twice. Those are one conversation, kept open
 *    between requests and ended — `bye` — by [endFlow].
 * 2. **What a refusal means to the caller.** For a SIGNATURE, nothing signed
 *    is a cancelled ceremony whatever the reason (071 contract §5), so the
 *    request stays open and can be signed another way. For a CEREMONY inside
 *    create or sign-in, a closed page is a cancellation and everything else
 *    carries its own sentence, which is what the machines' failure shapes
 *    already distinguish.
 */
class ClearSignerChannel(
    /** The page the wallet opens by default (`sign_pref`'s, always usable). */
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
    /** This app's own mark, inline PNG, for the page to draw beside the name. */
    private val appIcon: () -> String = { "" },
    /** `Vela Wallet <version>` — what the page shows as the requester, marked there as self-reported. */
    private val appName: String = "vela-android",
) : ClearSigner {

    /** The sentences a refusal is told in (`componentsUi.signing.clearSigner*`). */
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
    private var wire: ClearSignerWire? = null

    /**
     * Why this flow's channel could not be opened at all, when there is a
     * better sentence than "the page was closed".
     *
     * A request that never opened a page comes back as [ClearSignerAnswer.Cancelled]
     * like any other, and the caller is told "the Clear Signer was closed
     * without signing" — which is true of a dismissed sheet and a lie about a
     * radio the person was never allowed to turn on. This carries the real
     * reason as far as the sentence, and is cleared at the start of every
     * request so it can never outlive the attempt that set it.
     */
    @Volatile
    private var unopenable: String? = null

    override fun describe(chainId: Int, account: String): ClearSignerLabels = labels(chainId, account)

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
        val put = put(ClearSignerAsk.Signature(requestJson, digest, keys), signerOrigin)
        try {
            val answer = put.answer
            val outcome = (answer as? ClearSignerAnswer.Signed)?.outcome
                // Nothing signed: a cancelled ceremony, so the request stays open.
                ?: refuse(FailureKind.Cancelled, sentenceFor(answer))
            return when (outcome) {
                is ClearSignerOutcome.Accepted -> Assertion(
                    credentialIdHex = outcome.credentialIdHex,
                    signatureDerHex = hex(outcome.assertion.signatureDer),
                    authenticatorDataHex = hex(outcome.assertion.authenticatorData),
                    clientDataJsonHex = hex(outcome.assertion.clientDataJson),
                    userIdHex = null,
                    authenticatorAttachment = "",
                )
                is ClearSignerOutcome.Refused -> refuse(FailureKind.Cancelled, told(outcome.refusal))
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
    ): ClearSignerCeremonyOutcome {
        val answer = put(
            ClearSignerAsk.Ceremony(requestJson, operationJson, expectedMemberChallenge),
            signerOrigin,
        ).answer
        val outcome = (answer as? ClearSignerAnswer.Ceremonial)?.outcome
            ?: refuse(kindOf(answer), sentenceFor(answer))
        if (outcome is ClearSignerCeremonyOutcome.Refused) {
            refuse(
                if (outcome.refusal == ClearSignerRefusal.Declined) {
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
    private class Put(val answer: ClearSignerAnswer, val ownsFlow: Boolean)

    private suspend fun put(ask: ClearSignerAsk, signerOrigin: String): Put =
        one.withLock {
            notice.value = null
            unopenable = null
            val existing = wire
            val mine = existing == null
            val live = existing ?: open(signerOrigin) ?: return@withLock Put(ClearSignerAnswer.Cancelled, true)
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
            if (answer !is ClearSignerAnswer.Signed && answer !is ClearSignerAnswer.Ceremonial) {
                // A flow that did not answer has no session left to reuse.
                endFlow()
            } else {
                // Answered — for now. The page stays open for the flow's next
                // request, but the wallet has nothing pending on it, so no sheet
                // of ours may sit over whatever the flow shows next. (A refused
                // ceremony lands here too: its own alert is what the person
                // needs to read, not our waiting card.)
                _state.value = State.Idle
            }
            Put(answer, mine)
        }

    /** Open the flow's conversation: a Custom Tab on this phone's loopback. */
    private fun open(signerOrigin: String): ClearSignerWire? {
        val page = signerOrigin.ifEmpty { signerUrl() }
        // A channel that cannot even be built — a port nothing will bind — is
        // an unopened page, not an exception on its way through the signing
        // paths.
        val built = runCatching {
            ClearSignerLoopback(
                base = page,
                openPage = openPage,
                timeoutMs = timeoutMs,
                random = random,
                onOpened = { url -> _state.value = State.Waiting(url) },
            )
        }.getOrElse { error ->
            VelaLog.failure("clearsigner", "the channel could not be opened", error)
            _state.value = State.Idle
            return null
        }
        wire = built
        return built
    }

    private fun kindOf(answer: ClearSignerAnswer): FailureKind = when (answer) {
        ClearSignerAnswer.Cancelled -> FailureKind.Cancelled
        else -> FailureKind.Other
    }

    /** Why a request came back with nothing, in the person's language. */
    private fun sentenceFor(answer: ClearSignerAnswer): String {
        val w = words()
        return when (answer) {
            ClearSignerAnswer.TimedOut -> w.timeout
            is ClearSignerAnswer.Unreachable -> {
                VelaLog.event(
                    "clearsigner",
                    "the channel could not carry the request",
                    "why" to answer.why.name,
                    "detail" to answer.detail,
                )
                w.timeout
            }
            is ClearSignerAnswer.Ceremonial -> told(
                (answer.outcome as? ClearSignerCeremonyOutcome.Refused)?.refusal
                    ?: ClearSignerRefusal.Declined,
            )
            is ClearSignerAnswer.Signed -> told(
                (answer.outcome as? ClearSignerOutcome.Refused)?.refusal
                    ?: ClearSignerRefusal.Declined,
            )
            // A channel that never opened says why; everything else that
            // comes back with nothing is a page the person closed.
            ClearSignerAnswer.Cancelled -> unopenable ?: w.closed
        }
    }

    private fun told(refusal: ClearSignerRefusal): String {
        val w = words()
        return when (refusal) {
            ClearSignerRefusal.Declined -> w.closed
            is ClearSignerRefusal.PageRefused -> w.refused
            else -> {
                // Signed, but not what this wallet asked for — logged, never used.
                VelaLog.event("clearsigner", "answer refused", "refusal" to refusal.toString())
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
