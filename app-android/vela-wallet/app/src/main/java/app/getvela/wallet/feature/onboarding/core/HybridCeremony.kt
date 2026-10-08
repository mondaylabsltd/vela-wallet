package app.getvela.wallet.feature.onboarding.core

import android.content.Context
import app.getvela.wallet.MainActivity
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.withContext
import uniffi.vela_core_uniffi.CableFramePort
import uniffi.vela_core_uniffi.CtapCeremonyHost
import uniffi.vela_core_uniffi.CtapCredentialChoice
import uniffi.vela_core_uniffi.CtapException
import uniffi.vela_core_uniffi.CtapPinRequest
import uniffi.vela_core_uniffi.ctapAssertCable
import uniffi.vela_core_uniffi.ctapRegisterCable
import uniffi.vela_core_uniffi.cableConnectUrl
import uniffi.vela_core_uniffi.cableQrPayload
import java.security.SecureRandom

/**
 * "Sign in with your phone" — the caBLE / hybrid transport as INITIATOR.
 *
 * The Noise handshake and CTAP framing are the core's (`vela_core::cable`, the
 * same code the desktop and the other phone's authenticator run). This class is
 * the Android side of the three seams:
 *
 *  * the QR the OTHER phone scans ([qrPayload]);
 *  * the transport — a BLE scan for the responder's advert, then either an
 *    L2CAP CoC (CTAP 2.3, no internet) or a WebSocket tunnel (CTAP 2.2), as one
 *    [CableFramePort] ([HybridCableScanner] + [CableTransports]); and
 *  * the person ([CtapCeremonyHost] — randomness, and the "look at your phone"
 *    touch prompt; a phone-resident passkey needs no PIN from us).
 *
 * Deliberately parallel to [UsbSecurityKeyCeremony]: same shape, different
 * route. The executor picks THIS one for [KeyMethod.Hybrid] (the scan method).
 *
 * A person who puts the code away ([Session.dismiss], issue #459) ends THIS
 * ceremony with [FailureKind.Cancelled] — the core's quiet cancel — at
 * whichever step it is in: the scan stops, a channel still opening is closed
 * when it opens, and an open one is closed under the exchange. Never a
 * coroutine cancellation: that is never answered, and the core would wait on
 * the ceremony forever.
 */
class HybridCeremony(
    private val context: Context,
    private val prompts: UsbSecurityKeyCeremony.Prompts,
) {
    private val scanner = HybridCableScanner(context)
    private val secureRandom = SecureRandom()

    /**
     * Fresh per-ceremony secrets: the QR both encodes and the handshake keys
     * off — and the ceremony's own dismissal, so a code put away cannot end
     * the next ceremony (a retry, or recovery's second signature).
     */
    class Session(val staticSeed: ByteArray, val qrSecret: ByteArray) {
        private val dismissal = CompletableDeferred<Unit>()

        /** The channel the ceremony is talking over, once one is open. */
        @Volatile
        private var conn: CableConn? = null

        /** The person put the code (or the phone's prompt) away. */
        val dismissed: Boolean get() = dismissal.isCompleted

        /**
         * End this ceremony: the scan stops, and closing the channel is what
         * unblocks an exchange already running (it is a blocking call).
         */
        fun dismiss() {
            if (!dismissal.complete(Unit)) return
            VelaLog.event("cable", "dismissed — ending this ceremony")
            conn?.let { runCatching { it.close() } }
        }

        /** [work]'s answer, or [FailureKind.Cancelled] the moment the person dismisses. */
        internal suspend fun <T> unlessDismissed(work: Deferred<T>): T = select {
            work.onAwait { it }
            dismissal.onAwait { throw dismissedFailure() }
        }

        /** Holds [opened] so a dismissal closes it; refuses it if the dismissal came first. */
        internal fun hold(opened: CableConn) {
            conn = opened
            // The other order is covered too: [dismiss] reads the channel
            // after it marks the dismissal, so one of the two closes it.
            if (dismissed) {
                runCatching { opened.close() }
                throw dismissedFailure()
            }
        }

        internal fun dismissedFailure() = PasskeyFailure(FailureKind.Cancelled, "The code was dismissed")
    }

    fun newSession(): Session = Session(
        staticSeed = ByteArray(32).also(secureRandom::nextBytes),
        qrSecret = ByteArray(16).also(secureRandom::nextBytes),
    )

    /**
     * The `FIDO:/…` QR to render, or null if the fresh secrets were malformed
     * (the caller retries with a new [Session]). The payload is exactly
     * Chrome's shape — the BLE channel is chosen by the AUTHENTICATOR's advert
     * (its PSM suffix), never offered in the QR, because GMS's caBLE-v2.1
     * parser hard-rejects any QR whose key 6 is not its legacy bool
     * (device-found 2026-08-28).
     */
    fun qrPayload(session: Session, forGet: Boolean): String? = cableQrPayload(
        staticSeed = session.staticSeed,
        qrSecret = session.qrSecret,
        epochSeconds = System.currentTimeMillis() / 1000,
        forGet = forGet,
    )

    suspend fun register(
        session: Session,
        name: String,
        excludeCredentialIds: List<String>,
    ): Registration = runCeremony(session) { port, plaintext, host ->
        ctapRegisterCable(
            port, host, session.staticSeed, session.qrSecret, plaintext,
            HYBRID_PRODUCT, name, excludeCredentialIds,
        ).toRegistration()
    }

    suspend fun assert(
        session: Session,
        challenge: ByteArray,
        credentialIdHex: String?,
    ): Assertion = runCeremony(session) { port, plaintext, host ->
        ctapAssertCable(
            port, host, session.staticSeed, session.qrSecret, plaintext,
            HYBRID_PRODUCT, challenge, credentialIdHex ?: "",
        ).toAssertion()
    }

    /**
     * Grant Bluetooth, scan for the phone that scanned our QR, open the channel
     * the advert chose, and run [body] on the IO dispatcher. The ceremony call
     * blocks (Rust drives the callbacks), exactly like the USB path.
     */
    private suspend fun <T> runCeremony(
        session: Session,
        body: (CableFramePort, ByteArray, CtapCeremonyHost) -> T,
    ): T {
        // The code is on screen through every step below, so it may be put
        // away during any of them: each ends the ceremony there.
        if (session.dismissed) throw session.dismissedFailure()
        val granted = (context as? MainActivity)?.requestBluetoothPermission() ?: false
        if (!granted) {
            throw PasskeyFailure(FailureKind.Cancelled, "Bluetooth permission was declined")
        }
        if (session.dismissed) throw session.dismissedFailure()
        if (!scanner.bluetoothReady()) {
            // A radio that is merely OFF is not "not supported" — NotSupported
            // renders as the biometrics alert, a sentence about the wrong
            // subject (device-found on a OnePlus 5T, 2026-08-28). Ask the
            // system to turn it on (its dialog, its localization); declining is
            // a cancel, exactly like declining the permission above.
            val enabled = (context as? MainActivity)?.requestEnableBluetooth() ?: false
            if (!enabled || !awaitBluetoothReady()) {
                throw PasskeyFailure(FailureKind.Cancelled, "Bluetooth stayed off")
            }
        }
        if (!locationReadyForScan()) {
            // API ≤30 gates BLE SCAN RESULTS on location services — off means a
            // silent empty scan, which would misreport as "no phone answered"
            // after the full window. Explain first (an unannounced jump into
            // system settings reads as broken), then send the person to the
            // system toggle — there is no in-place enable dialog without GMS —
            // and continue when they return with it on. Declining either step
            // is a cancel, exactly like declining the permission above.
            val agreed = withContext(Dispatchers.IO) { prompts.askEnableLocation() }
            if (!agreed) {
                throw PasskeyFailure(FailureKind.Cancelled, "Location explainer declined")
            }
            (context as? MainActivity)?.openLocationSettings()
            if (!locationReadyForScan()) {
                throw PasskeyFailure(FailureKind.Cancelled, "Location stayed off")
            }
        }
        if (session.dismissed) throw session.dismissedFailure()

        return withContext(Dispatchers.IO) {
            // The scan is cancellable: a dismissal stops it at once.
            val hit = coroutineScope {
                session.unlessDismissed(async { scanner.findResponder(session.qrSecret, SCAN_TIMEOUT_MS) })
            } ?: throw PasskeyFailure(
                FailureKind.Other,
                "No phone answered the code. Scan it with the other device and try again.",
            )

            val conn = openChannel(session, hit)
            val port = CableConnPort(conn)
            val host = HostBridge(prompts, secureRandom)
            try {
                body(port, hit.advert.plaintext, host).also {
                    // Put away as the phone answered: the person said no, and a
                    // signature they took back must not go on to sign a send.
                    if (session.dismissed) throw session.dismissedFailure()
                }
            } catch (error: CtapException) {
                // The channel closed under the exchange because the person
                // dismissed it — their cancel, not a failed link.
                if (session.dismissed) throw session.dismissedFailure()
                throw error.toPasskeyFailure()
            } finally {
                runCatching { conn.close() }
                prompts.touchWaiting(null, "")
            }
        }
    }

    /**
     * Open the channel the advert chose. The connect blocks and nothing
     * interrupts it, so it runs outside this ceremony's scope: a dismissal
     * answers at once, and a channel that opens afterwards is closed unused.
     */
    private suspend fun openChannel(session: Session, hit: HybridCableScanner.AdvertHit): CableConn {
        val opening = OPENER.async {
            val opened = if (hit.advert.psm != null) {
                VelaLog.event("cable", "advert offers BLE (PSM ${hit.advert.psm}) — L2CAP CoC, no tunnel")
                L2capCableConn.connect(hit.device, hit.advert.psm!!.toInt())
            } else {
                val url = cableConnectUrl(session.staticSeed, session.qrSecret, hit.advert.plaintext)
                    ?: throw PasskeyFailure(FailureKind.Other, "the phone's advertisement named an unknown tunnel")
                VelaLog.event("cable", "advert has no PSM — WebSocket tunnel")
                WebSocketCableConn.connect(url, timeoutMs = TUNNEL_CONNECT_MS)
            }
            // Dismissed while it was opening: nobody is waiting for it.
            opened.also { if (session.dismissed) runCatching { it.close() } }
        }
        return session.unlessDismissed(opening).also(session::hold)
    }

    /**
     * The enable dialog answers OK when the adapter has ACCEPTED the turn-on,
     * which can be a beat before it reports enabled — poll briefly rather than
     * failing a scan the person just approved.
     */
    private suspend fun awaitBluetoothReady(): Boolean {
        repeat(20) {
            if (scanner.bluetoothReady()) return true
            kotlinx.coroutines.delay(100)
        }
        return scanner.bluetoothReady()
    }

    /**
     * On API ≤30 the platform withholds BLE scan results unless location
     * services are on (the runtime permission alone is not enough); 31+ with
     * BLUETOOTH_SCAN `neverForLocation` has no such gate.
     */
    private fun locationReadyForScan(): Boolean {
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S) return true
        val manager = context.getSystemService(Context.LOCATION_SERVICE)
            as? android.location.LocationManager ?: return false
        return androidx.core.location.LocationManagerCompat.isLocationEnabled(manager)
    }

    /** The [CtapCeremonyHost] for a phone-resident credential: no PIN, no picker
     *  (the phone runs its own), just randomness and the "look at your phone"
     *  prompt. */
    private class HostBridge(
        private val prompts: UsbSecurityKeyCeremony.Prompts,
        private val secureRandom: SecureRandom,
    ) : CtapCeremonyHost {
        override fun pin(request: CtapPinRequest): String? = null
        override fun pick(choices: List<CtapCredentialChoice>): UInt? = 0u
        override fun random(len: UInt): ByteArray =
            ByteArray(len.toInt()).also(secureRandom::nextBytes)
        override fun note(line: String) = VelaLog.event("cable.ctap", line)
        override fun touch(kind: String, product: String) = prompts.touchWaiting(kind, product)
    }

    private companion object {
        /**
         * Where a channel opens: not a child of the ceremony, so a dismissal
         * never waits on a connect that cannot be interrupted.
         */
        val OPENER = CoroutineScope(kotlinx.coroutines.SupervisorJob() + Dispatchers.IO)

        /** The person picks up the other phone, unlocks it, approves the prompt. */
        const val SCAN_TIMEOUT_MS = 90_000L
        const val TUNNEL_CONNECT_MS = 15_000L
    }
}

/**
 * What the touch prompt names while the OTHER phone shows its sheet — how the
 * prompt knows the approval waits on the phone, not on a key in hand.
 */
const val HYBRID_PRODUCT = "your phone"
