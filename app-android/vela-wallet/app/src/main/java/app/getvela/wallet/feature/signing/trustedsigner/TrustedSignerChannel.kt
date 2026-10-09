package app.getvela.wallet.feature.signing.trustedsigner

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.send.core.TrustedSigner
import app.getvela.wallet.feature.send.core.TrustedSignerLabels
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.TrustedSignerCeremonyOutcome
import uniffi.vela_core_uniffi.TrustedSignerOutcome
import uniffi.vela_core_uniffi.TrustedSignerRefusal
import uniffi.vela_core_uniffi.WalletKeyRecord
import uniffi.vela_core_uniffi.trustedSignerCeremonyTitleKey
import java.security.SecureRandom

/**
 * The signing page, as the rest of the app sees it (specs 071, 075 and 102).
 *
 * Spec 102: the page is where a person REVIEWS AND SIGNS — an account's
 * signing venue — not a fourth place a key lives. Every signature of an
 * account whose venue is a page comes through here (a send, a dApp request,
 * the key backup), and so does every key ceremony of a wallet on a custom
 * domain (R3: only its page can mint or use those keys). Which page is always
 * the caller's — the plan's venue, or the op's `page` — never a setting.
 *
 * Nothing opens unless its integrity check admitted it ([SignerPageChecks],
 * R6): the version opened is the version checked.
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
    /** Spec 102 R6: the phone's integrity checks — the only way a page opens. */
    val checks: SignerPageChecks,
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
    /**
     * Spec 079: whether the page's address answers at all (any HTTP status
     * counts) — asked only when the person is back with no answer.
     */
    private val reachable: suspend (url: String) -> Boolean = { true },
    /** The language the app shows, for the page's launch (`lang=`, spec 102). */
    private val lang: () -> String = { "" },
) : TrustedSigner {

    /** The sentences a refusal is told in (`componentsUi.signing.trustedSigner*`). */
    data class Words(
        val closed: String,
        val refused: String,
        val mismatch: String,
        val timeout: String,
        /** A page that was not opened: its integrity line, in words. */
        val unchecked: (uniffi.vela_core_uniffi.SignerIntegrityLine) -> String = { it.key },
    )

    sealed interface State {
        data object Idle : State

        /**
         * Spec 102 D4: the hand-off card — review and sign on [page], with the
         * key row [key] ("Confirm with | the key's name, or its place"). The
         * integrity line is [checks]'s for [page]; Open goes on only when it
         * opens. Raised for a signature no signing sheet is showing (a send
         * the person started); the dApp sheet draws this card itself.
         */
        data class Handoff(val page: String, val key: SigningPlan.KeyLabel?) : State

        /**
         * The page is open (or being opened) at [url] and the answer is awaited.
         * [unreachable]: the person came back without one and the page's address
         * does not answer (spec 079) — the card says so and offers a retry.
         * [title]: a key ceremony's own title (the corpus key the core names —
         * create, sign in, confirm; spec 102), `null` for a signature. [key]: that
         * ceremony's key row (the core's `Ceremony::key_label` — "New key on |
         * Phone or tablet", "Confirm with | This device"), `null` for a signature.
         */
        data class Waiting(
            val url: String,
            val unreachable: Boolean = false,
            val title: String? = null,
            val key: SigningPlan.KeyLabel? = null,
        ) : State
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

    /**
     * The request in flight's own title, when it is a key ceremony: the
     * core's `trustedSignerCeremonyTitleKey` — "Create a key on the
     * signing page", "Sign in on…", "Confirm with the key on…" — instead of
     * a signature's wait. Set per request: one flow can be a create and then
     * its member proof.
     */
    @Volatile
    private var ceremonyTitle: String? = null

    /** The request in flight's key row, when it is a key ceremony (`trustedSignerCeremonyKeyLabel`). */
    @Volatile
    private var ceremonyKey: SigningPlan.KeyLabel? = null

    override fun describe(chainId: Int, account: String): TrustedSignerLabels = labels(chainId, account)

    // -- what the screens drive -----------------------------------------------

    /** The hand-off card's answer, while it is up: Open (`true`) or Cancel. */
    @Volatile
    private var gate: CompletableDeferred<Boolean>? = null

    /** Cancel, on the hand-off card or the waiting sheet: the same as closing the page. */
    fun cancel() {
        gate?.complete(false)
        wire?.cancel()
    }

    /** The hand-off card's Open (spec 102 D4). Ignored when no card is up. */
    fun open() {
        gate?.complete(true)
    }

    /** "Open the page again" — same port, same token. */
    fun reopen() {
        (_state.value as? State.Waiting)?.takeIf { it.unreachable }?.let { _state.value = it.copy(unreachable = false) }
        wire?.reopen()
    }

    /**
     * Spec 079: the person is back in the wallet and the page has not
     * answered. A closed tab does not say why — a page that never loaded looks
     * the same as a mind changed — so ask whether the page's address answers
     * at all; when it does not, the waiting card says the page could not open.
     * The request stays open either way: retry, or close the sheet.
     */
    suspend fun personReturned() {
        val waiting = _state.value as? State.Waiting ?: return
        if (waiting.unreachable || reachable(waiting.url)) return
        // Still the same visit, still unanswered.
        if (_state.value == waiting) _state.value = waiting.copy(unreachable = true)
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
        gate = null
        live?.end()
        _state.value = State.Idle
        bringBack()
    }

    // -- the two things a caller can ask for ----------------------------------

    /**
     * Sign [digest] with one of [keys], on [page] — the account's venue (spec
     * 102 R4). [key] is the hand-off card's key row ("Confirm with | …");
     * [askFirst] raises that card ([State.Handoff]) and waits for its Open —
     * `false` when the caller's own sheet was the card.
     *
     * A standalone signature is a flow of one: the page opens, signs and goes.
     */
    override suspend fun sign(
        requestJson: String,
        digest: ByteArray,
        keys: List<WalletKeyRecord>,
        page: String,
        key: SigningPlan.KeyLabel?,
        askFirst: Boolean,
    ): Assertion {
        // A signature asked for on its own opens and closes its own flow; one
        // asked for inside a create or a sign-in belongs to that flow's visit.
        // WHICH it is can only be decided under the lock — two callers reading
        // `wire` from outside it would both think they owned the flow, or
        // neither would.
        val put = put(
            TrustedSignerAsk.Signature(requestJson, digest, keys),
            page,
            handoff = if (askFirst) State.Handoff(page, key) else null,
        )
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
        page: String,
    ): TrustedSignerCeremonyOutcome {
        val answer = put(
            TrustedSignerAsk.Ceremony(requestJson, operationJson, expectedMemberChallenge),
            page,
            handoff = null,
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

    private suspend fun put(ask: TrustedSignerAsk, page: String, handoff: State.Handoff?): Put =
        one.withLock {
            notice.value = null
            unopenable = null
            ceremonyTitle = (ask as? TrustedSignerAsk.Ceremony)
                ?.let { runCatching { trustedSignerCeremonyTitleKey(it.operationJson) }.getOrNull() }
            ceremonyKey = (ask as? TrustedSignerAsk.Ceremony)?.let { SigningPlan.KeyLabel.ofCeremony(it.operationJson) }
            if (handoff != null && !handedOff(handoff)) {
                _state.value = State.Idle
                return@withLock Put(TrustedSignerAnswer.Cancelled, wire == null)
            }
            val existing = wire
            val mine = existing == null
            val live = existing ?: open(page) ?: return@withLock Put(TrustedSignerAnswer.Cancelled, true)
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
                // opens the page again.
                //
                // Bringing the wallet back is NOT done from here, even though
                // this is where the answer arrives: by now this app is in the
                // background, and a background app cannot move its task in
                // front of the browser's (measured — see
                // `SignResultActivity.bringTheWalletBack`). The callback's own
                // activity does it, because the system started that one itself.
                _state.value = State.Idle
            }
            Put(answer, mine)
        }

    /**
     * Spec 102 D4: raise the hand-off card for [card] and wait for the
     * person's Open (`true`) or Cancel. The page's check starts now, beside
     * the card, so its line is there by the time they read it.
     */
    private suspend fun handedOff(card: State.Handoff): Boolean = coroutineScope {
        val answer = CompletableDeferred<Boolean>()
        gate = answer
        _state.value = card
        val checking = launch { runCatching { checks.ensure(card.page) } }
        try {
            answer.await()
        } finally {
            checking.cancel()
            if (gate === answer) gate = null
        }
    }

    /** Open the flow's conversation: a Custom Tab, answering by custom scheme. */
    private fun open(page: String): TrustedSignerWire? {
        // A channel that cannot even be built is an unopened page, not an
        // exception on its way through the signing paths.
        val built = runCatching {
            TrustedSignerScheme(
                base = page,
                admit = { checks.ensure(page) },
                refusal = { checks.line(page) },
                openPage = openPage,
                timeoutMs = timeoutMs,
                random = random,
                onOpened = { url -> _state.value = State.Waiting(url, title = ceremonyTitle, key = ceremonyKey) },
                lang = lang,
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
            // R6: not opened — the check's own line says why.
            is TrustedSignerAnswer.Unchecked -> w.unchecked(answer.line)
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
