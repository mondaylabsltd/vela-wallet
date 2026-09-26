package app.getvela.wallet.feature.send.core

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.getAndUpdate

/**
 * A payment request that arrives from outside Send — a `/pay` link, a payment
 * code scanned in 探索 — handed to Send exactly once (spec 078 round 3).
 *
 * The decision is made when the request arrives, not later:
 *
 * - **Send is closed:** the request is parked and Send is entered; the open
 *   takes it ([takeRequest] / [takeScan]) — atomically, so it is applied once.
 * - **Send is already open:** it is applied NOW, exactly as a closed Send would
 *   apply it (the locked request replaces the form), and nothing is parked.
 *
 * It used to be parked in both cases. With Send already open the open never
 * happened again, so the request sat there — and the NEXT time Send opened it
 * re-applied and overwrote the recipient the person had typed (device-found,
 * round 2). Nothing may be left behind to do that.
 */
class PaymentHandOff(
    private val parkedRequest: MutableStateFlow<SendOpenParams?>,
    private val parkedScan: MutableStateFlow<String?>,
) {
    /** A validated `/pay` request. */
    fun request(params: SendOpenParams, sendOpen: Boolean, openNow: (SendOpenParams) -> Unit, enterSend: () -> Unit) {
        if (sendOpen) {
            // A request parked earlier (never, by construction — but never
            // twice either) must not outlive this one.
            parkedRequest.value = null
            openNow(params)
        } else {
            parkedRequest.value = params
            enterSend()
        }
    }

    /** A payment code scanned outside Send; the send machine decides what it means. */
    fun scan(text: String, sendOpen: Boolean, scanNow: (String) -> Unit, enterScanner: () -> Unit) {
        if (sendOpen) {
            parkedScan.value = null
            scanNow(text)
        } else {
            parkedScan.value = text
            enterScanner()
        }
    }

    /** The open's share: the parked request, once. */
    fun takeRequest(): SendOpenParams? = parkedRequest.getAndUpdate { null }

    /** The open's share: the parked scan, once. */
    fun takeScan(): String? = parkedScan.getAndUpdate { null }
}
