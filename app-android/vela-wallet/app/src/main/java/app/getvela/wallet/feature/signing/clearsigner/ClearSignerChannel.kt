package app.getvela.wallet.feature.signing.clearsigner

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.send.core.ClearSigner
import app.getvela.wallet.feature.send.core.ClearSignerLabels
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import uniffi.vela_core_uniffi.ClearSignerCeremonyOutcome
import uniffi.vela_core_uniffi.ClearSignerOutcome
import uniffi.vela_core_uniffi.ClearSignerRefusal
import uniffi.vela_core_uniffi.WalletKeyRecord
import uniffi.vela_core_uniffi.clearSignerDefaultRelay
import java.security.SecureRandom

/**
 * The Clear Signer, as the rest of the app sees it (specs 071 and 075).
 *
 * It is the fourth passkey route: a page the person reads, which checks the
 * request and runs the WebAuthn ceremony itself. Everything that can ask for
 * one — a send, a dApp request, the key backup, and since 075 creating a
 * wallet, signing in and every proof inside those flows — comes through here.
 *
 * Three decisions live in this class and nowhere else:
 *
 * 1. **Where the signer is.** On this device the page is a Custom Tab over the
 *    app on a loopback socket ([ClearSignerLoopback]); on another device it is
 *    reached through a blind relay ([ClearSignerRelayWire]). The person is
 *    asked once per flow.
 * 2. **One page visit per flow.** A create mints a key and then confirms its
 *    membership; a recovery signs twice. Those are one conversation, kept open
 *    between requests and ended — `bye` — by [endFlow].
 * 3. **What a refusal means to the caller.** For a SIGNATURE, nothing signed
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
    /** Spec 075: the relay a cross-device pairing goes through (`sign_pref`'s). */
    private val relayUrl: () -> String = { "" },
    /** The chain's name and coin, the account's name — from the wallet's own lists. */
    private val labels: (chainId: Int, account: String) -> ClearSignerLabels = { _, _ -> ClearSignerLabels() },
    private val timeoutMs: Long = 5 * 60_000L,
    private val random: SecureRandom = SecureRandom(),
    /** `vela-android/<version>` — what the page shows as the requester. */
    private val appName: String = "vela-android",
    /** The relay transport; a fake one in tests. */
    private val sockets: RelaySockets = OkHttpRelaySockets(),
) : ClearSigner {

    /** The sentences a refusal is told in (`componentsUi.signing.clearSigner*`). */
    data class Words(
        val closed: String,
        val refused: String,
        val mismatch: String,
        val timeout: String,
        /** Spec 075: the relay could not be reached. */
        val relayDown: String = timeout,
    )

    sealed interface State {
        data object Idle : State

        /** Spec 075: "where is your Clear Signer?" — this device, or another. */
        data object Where : State

        /** The page is open (or being opened) at [url] and the socket is listening. */
        data class Waiting(val url: String) : State

        /** Spec 075: the pairing sheet — the QR of [link], the link, and waiting. */
        data class Pairing(val link: String) : State

        /** Spec 075: both ends derived [code]; the person confirms it here. */
        data class Code(val code: String) : State

        /** Spec 075: paired and confirmed — the other device is being asked. */
        data object Paired : State
    }

    private val _state = MutableStateFlow<State>(State.Idle)
    val state: StateFlow<State> = _state

    /** Why the last attempt did not sign — shown until the next one starts. */
    val notice = MutableStateFlow<String?>(null)

    private val one = Mutex()

    /** The flow's conversation with one page; `null` between flows. */
    @Volatile
    private var wire: ClearSignerWire? = null

    /** Where this flow's signer is, once the person has said. */
    @Volatile
    private var onThisDevice: Boolean? = null

    @Volatile
    private var whereAnswer: CompletableDeferred<Boolean?>? = null

    @Volatile
    private var codeAnswer: CompletableDeferred<Boolean>? = null

    override fun describe(chainId: Int, account: String): ClearSignerLabels = labels(chainId, account)

    // -- what the screens drive -----------------------------------------------

    /** "On this device" / "On another device" on the where sheet. */
    fun chooseWhere(thisDevice: Boolean) {
        whereAnswer?.complete(thisDevice)
    }

    /** The codes match. */
    fun confirmCode() {
        codeAnswer?.complete(true)
    }

    /** Cancel, on any of the sheets: the same as closing the page. */
    fun cancel() {
        whereAnswer?.complete(null)
        codeAnswer?.complete(false)
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
        val hadTab = onThisDevice == true
        wire = null
        onThisDevice = null
        cancel()
        live?.end()
        _state.value = State.Idle
        // Only a Custom Tab over this app needs closing; a page on another
        // device is somebody else's screen.
        if (hadTab) bringBack()
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
        val standalone = wire == null
        try {
            val answer = put(ClearSignerAsk.Signature(requestJson, digest, keys), signerOrigin)
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
            if (standalone) endFlow()
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
        )
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

    private suspend fun put(ask: ClearSignerAsk, signerOrigin: String): ClearSignerAnswer =
        one.withLock {
            notice.value = null
            val live = wire ?: open(signerOrigin) ?: return@withLock ClearSignerAnswer.Cancelled
            val answer = live.ask(ask)
            if (answer !is ClearSignerAnswer.Signed && answer !is ClearSignerAnswer.Ceremonial) {
                // A flow that did not answer has no session left to reuse.
                endFlow()
            } else if (_state.value is State.Code || _state.value is State.Pairing) {
                _state.value = State.Paired
            }
            answer
        }

    /**
     * Open the flow's conversation: ask where the signer is (once per flow),
     * then build the channel that reaches it.
     */
    private suspend fun open(signerOrigin: String): ClearSignerWire? {
        val page = signerOrigin.ifEmpty { signerUrl() }
        val here = onThisDevice ?: askWhere() ?: return null
        onThisDevice = here
        val built = if (here) {
            ClearSignerLoopback(
                base = page,
                openPage = openPage,
                timeoutMs = timeoutMs,
                random = random,
                onOpened = { url -> _state.value = State.Waiting(url) },
            )
        } else {
            ClearSignerRelayWire(
                relayUrl = relayUrl().ifEmpty { clearSignerDefaultRelay() },
                signerUrl = page,
                sockets = sockets,
                appName = appName,
                timeoutMs = timeoutMs,
                random = random,
                onLink = { link -> _state.value = State.Pairing(link) },
                confirmCode = { code -> awaitCode(code) },
            )
        }
        wire = built
        return built
    }

    /** "Where is your Clear Signer?" — asked once per flow. */
    private suspend fun askWhere(): Boolean? {
        val answer = CompletableDeferred<Boolean?>()
        whereAnswer = answer
        _state.value = State.Where
        val here = answer.await()
        whereAnswer = null
        if (here == null) _state.value = State.Idle
        return here
    }

    /**
     * The six digits, on both screens, with a button. **Nothing of the request
     * is sent until this returns true** — that is what stops a stolen link
     * from putting somebody else's page in the wallet's conversation.
     */
    private suspend fun awaitCode(code: String): Boolean {
        val answer = CompletableDeferred<Boolean>()
        codeAnswer = answer
        _state.value = State.Code(code)
        val confirmed = answer.await()
        codeAnswer = null
        if (confirmed) _state.value = State.Paired
        return confirmed
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
                VelaLog.event("clearsigner", "relay unreachable", "detail" to answer.detail)
                w.relayDown
            }
            is ClearSignerAnswer.Ceremonial -> told(
                (answer.outcome as? ClearSignerCeremonyOutcome.Refused)?.refusal
                    ?: ClearSignerRefusal.Declined,
            )
            is ClearSignerAnswer.Signed -> told(
                (answer.outcome as? ClearSignerOutcome.Refused)?.refusal
                    ?: ClearSignerRefusal.Declined,
            )
            ClearSignerAnswer.Cancelled -> w.closed
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
