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
 *    reached through a blind relay ([ClearSignerRelayWire]) or, when the other
 *    device is in the room, over Bluetooth with this phone as the GATT
 *    peripheral ([ClearSignerBleWire]). The person is asked once per flow.
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
    /**
     * Spec 075 T040: the Bluetooth route's platform side — the activity's, and
     * `null` where there is none (a process with no activity attached, a test
     * that is not about the radio). The route is offered only when it is here.
     *
     * A provider rather than a value: this channel is built once per process,
     * and the activity that owns the permission launchers comes and goes.
     */
    private val bleHost: () -> ClearSignerBleHost? = { null },
) : ClearSigner {

    /** The sentences a refusal is told in (`componentsUi.signing.clearSigner*`). */
    data class Words(
        val closed: String,
        val refused: String,
        val mismatch: String,
        val timeout: String,
        /** Spec 075: the relay could not be reached. */
        val relayDown: String = timeout,
        /**
         * Spec 075 T040: the Bluetooth permission was refused, or this phone
         * cannot advertise at all (`clearSignerBluetoothNeeded`).
         */
        val bluetoothNeeded: String = relayDown,
        /** Spec 075 T040: the radio is off (`clearSignerBluetoothOff`). */
        val bluetoothOff: String = bluetoothNeeded,
        /**
         * Spec 075 T040: this phone cannot pair this way at all
         * (`clearSignerBluetoothUnsupported`). Also what a radio that would
         * not advertise is told as — the person's options are the same.
         */
        val bluetoothUnsupported: String = bluetoothNeeded,
        /**
         * Spec 075 T040: the page was there and then it was not — the link
         * dropped mid-flow. Distinct from every other Bluetooth sentence,
         * because the pairing DID work and the advice is to try it again.
         */
        val nearbyLost: String = bluetoothUnsupported,
    )

    /** Where this flow's Clear Signer page is — asked once, on the first request. */
    enum class Route {
        /** A Custom Tab over the app, on the wallet's own loopback. */
        ThisDevice,

        /** Another device entirely, reached through the blind relay. */
        OtherDevice,

        /** A browser in the room: this phone advertises and it connects. */
        Nearby,
    }

    sealed interface State {
        data object Idle : State

        /**
         * Spec 075: "where is your Clear Signer?" — this device, another one
         * through the relay, or, when this phone can advertise, one nearby
         * over Bluetooth.
         */
        data class Where(val nearby: Boolean = false) : State

        /** The page is open (or being opened) at [url] and the socket is listening. */
        data class Waiting(val url: String) : State

        /** Spec 075: the pairing sheet — the QR of [link], the link, and waiting. */
        data class Pairing(val link: String) : State

        /** Spec 075: both ends derived [code]; the person confirms it here. */
        data class Code(val code: String) : State

        /** Spec 075: paired and confirmed — the other device is being asked. */
        data object Paired : State

        /**
         * Spec 075 T040: this phone is advertising under [deviceName], which
         * is the name the browser's chooser will show. Saying it here is the
         * difference between picking your own phone and picking a stranger's.
         */
        data class Nearby(val deviceName: String) : State

        /**
         * Spec 075 T040: the Android 12 Bluetooth permissions were refused.
         *
         * It is a card with a way to grant them, not a wait that quietly never
         * ends — and it is the ONLY readiness that gets one. An adapter that
         * is off raises the system's own dialog, and a phone that cannot
         * advertise cannot be argued with; both of those end the attempt with
         * a sentence on [notice] instead of a card asking for something the
         * person has already refused or cannot give.
         */
        data object BluetoothRefused : State
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
    private var route: Route? = null

    @Volatile
    private var whereAnswer: CompletableDeferred<Route?>? = null

    @Volatile
    private var codeAnswer: CompletableDeferred<Boolean>? = null

    /** The Bluetooth card's "try again", or its Cancel. */
    @Volatile
    private var bluetoothAnswer: CompletableDeferred<Boolean>? = null

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

    /**
     * The route the request being judged rode on.
     *
     * [route] itself cannot answer this: a request that came back with nothing
     * ends its flow, and [endFlow] clears [route] BEFORE the caller turns the
     * answer into a sentence. Reading it there gave every failed Bluetooth
     * request the relay's words (T043 defect 2) — right up to the point where
     * a test finally asked.
     */
    @Volatile
    private var judgedRoute: Route? = null

    /** Is the Bluetooth route on offer at all on this build and this phone? */
    val offersNearby: Boolean get() = bleHost() != null

    override fun describe(chainId: Int, account: String): ClearSignerLabels = labels(chainId, account)

    // -- what the screens drive -----------------------------------------------

    /** A row on the where sheet. */
    fun chooseWhere(route: Route) {
        whereAnswer?.complete(route)
    }

    /** "On this device" / "On another device" — the two-way question, unchanged. */
    fun chooseWhere(thisDevice: Boolean) {
        chooseWhere(if (thisDevice) Route.ThisDevice else Route.OtherDevice)
    }

    /** The codes match. */
    fun confirmCode() {
        codeAnswer?.complete(true)
    }

    /** "Try again" on the Bluetooth card — ask for whatever is still missing. */
    fun retryBluetooth() {
        bluetoothAnswer?.complete(true)
    }

    /**
     * The app went to the background. PROTOCOL §1 wants the peripheral in the
     * foreground while a session is running, and an advert this app forgot
     * about is one a stranger can still connect to — so a Bluetooth flow ends
     * here. The loopback route does NOT: its page is a Custom Tab, so being
     * backgrounded is the normal, expected state of that flow.
     */
    fun leftForeground() {
        if (wire is ClearSignerBleWire) endFlow()
    }

    /** Cancel, on any of the sheets: the same as closing the page. */
    fun cancel() {
        whereAnswer?.complete(null)
        codeAnswer?.complete(false)
        bluetoothAnswer?.complete(false)
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
        val hadTab = route == Route.ThisDevice
        wire = null
        route = null
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
            judgedRoute = null
            val existing = wire
            val mine = existing == null
            val live = existing ?: open(signerOrigin) ?: return@withLock Put(ClearSignerAnswer.Cancelled, true)
            // Remembered while the flow is still alive: the answer is turned
            // into words after `endFlow` has forgotten where it was going.
            judgedRoute = route
            // A later request of the same flow raises no sheet of its own. The
            // loopback names its page again from inside `ask` (its sheet offers
            // "open it again"); a page on another device — relayed or nearby —
            // has no page of ours to name, so the wait is said here.
            if (!mine && route != null && route != Route.ThisDevice) _state.value = State.Paired
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

    /**
     * Open the flow's conversation: ask where the signer is (once per flow),
     * then build the channel that reaches it.
     */
    private suspend fun open(signerOrigin: String): ClearSignerWire? {
        val page = signerOrigin.ifEmpty { signerUrl() }
        val chosen = route ?: askWhere() ?: return null
        // The Bluetooth route is the only one that can be refused by something
        // other than the person — no radio, no permission, an adapter that is
        // off. Finding that out here means a card that says so, while the
        // flow is still unopened and another route is still available.
        val radio = if (chosen == Route.Nearby) awaitRadio() ?: return null else null
        route = chosen
        // A channel that cannot even be built — a port nothing will bind, a
        // 2⁻³² secret that is not a P-256 scalar — is an unopened page, not an
        // exception on its way through the signing paths.
        val built = runCatching {
            when (chosen) {
                Route.ThisDevice -> ClearSignerLoopback(
                    base = page,
                    openPage = openPage,
                    timeoutMs = timeoutMs,
                    random = random,
                    onOpened = { url -> _state.value = State.Waiting(url) },
                )
                Route.OtherDevice -> ClearSignerRelayWire(
                    relayUrl = relayUrl().ifEmpty { clearSignerDefaultRelay() },
                    signerUrl = page,
                    sockets = sockets,
                    appName = appName,
                    timeoutMs = timeoutMs,
                    random = random,
                    onLink = { link -> _state.value = State.Pairing(link) },
                    confirmCode = { code -> awaitCode(code) },
                )
                Route.Nearby -> ClearSignerBleWire(
                    radio = radio ?: error("the Bluetooth route was taken without a radio"),
                    signerUrl = page,
                    appName = appName,
                    timeoutMs = timeoutMs,
                    random = random,
                    onAdvertising = { name -> _state.value = State.Nearby(name) },
                    confirmCode = { code -> awaitCode(code) },
                )
            }
        }.getOrElse { error ->
            VelaLog.failure("clearsigner", "the channel could not be opened", error)
            // A radio handed out and then not used keeps a GATT server and an
            // advert up with nobody behind them.
            radio?.let { runCatching { it.stop() } }
            route = null
            _state.value = State.Idle
            return null
        }
        wire = built
        return built
    }

    /**
     * "Where is your Clear Signer?" — asked once per flow, and under the same
     * five-minute clock as everything else: a question left on screen must not
     * hold the request open forever.
     */
    private suspend fun askWhere(): Route? {
        val answer = CompletableDeferred<Route?>()
        whereAnswer = answer
        _state.value = State.Where(nearby = offersNearby)
        val chosen = withTimeoutOrNull(timeoutMs) { answer.await() }
        whereAnswer = null
        if (chosen == null) _state.value = State.Idle
        return chosen
    }

    /**
     * Hardware, permissions and the adapter, with a card for each way it can
     * say no and a "try again" that asks once more. `null` means the person
     * gave up (or was never going to be able to) — the flow does not open, and
     * the sentence rides on [notice] so whatever raised the request says why.
     */
    private suspend fun awaitRadio(): BlePeripheral? {
        val host = bleHost() ?: return null
        while (true) {
            val readiness = runCatching { host.ready() }.getOrElse { error ->
                VelaLog.failure("clearsigner.ble", "the radio could not be asked about", error)
                BleReadiness.Unsupported
            }
            when (readiness) {
                BleReadiness.Ready -> return runCatching { host.peripheral() }.getOrElse { error ->
                    VelaLog.failure("clearsigner.ble", "the peripheral could not be built", error)
                    blocked(readiness)
                }
                // The permissions are the one refusal worth a second ask: the
                // person may have tapped Deny without reading, and there is
                // something for them to do about it.
                BleReadiness.Refused -> {
                    val answer = CompletableDeferred<Boolean>()
                    bluetoothAnswer = answer
                    _state.value = State.BluetoothRefused
                    val again = withTimeoutOrNull(timeoutMs) { answer.await() } ?: false
                    bluetoothAnswer = null
                    if (!again) return blocked(readiness)
                }
                // The adapter's own dialog was declined, or this phone cannot
                // advertise at all. Neither is a question worth asking twice.
                BleReadiness.AdapterOff, BleReadiness.Unsupported -> {
                    VelaLog.event("clearsigner.ble", "the radio is not usable", "why" to readiness.toString())
                    return blocked(readiness)
                }
            }
        }
    }

    /**
     * No radio: say WHICH way, where the flow's own screen will read it, and
     * stop. A person who turned Bluetooth down and a person whose phone cannot
     * advertise need different sentences, and neither of them is "the Clear
     * Signer was closed".
     */
    private fun blocked(why: BleReadiness): BlePeripheral? {
        val w = words()
        unopenable = when (why) {
            BleReadiness.AdapterOff -> w.bluetoothOff
            BleReadiness.Unsupported -> w.bluetoothUnsupported
            else -> w.bluetoothNeeded
        }
        notice.value = unopenable
        _state.value = State.Idle
        return null
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
        val confirmed = withTimeoutOrNull(timeoutMs) { answer.await() } ?: false
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
                // The route picks the words. Telling somebody who chose
                // Bluetooth that the relay could not be reached — and advising
                // them to try a route they did not choose — names the wrong
                // channel and offers the wrong remedy (T043 radio pass).
                VelaLog.event(
                    "clearsigner",
                    "the channel could not carry the request",
                    "route" to (judgedRoute?.name ?: "unknown"),
                    "why" to answer.why.name,
                    "detail" to answer.detail,
                )
                when {
                    judgedRoute != Route.Nearby -> w.relayDown
                    answer.why == Unreachability.PeerGone -> w.nearbyLost
                    else -> w.bluetoothUnsupported
                }
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
