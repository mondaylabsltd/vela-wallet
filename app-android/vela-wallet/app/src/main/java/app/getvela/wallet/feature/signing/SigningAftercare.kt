package app.getvela.wallet.feature.signing

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.send.core.TrackEntryView
import app.getvela.wallet.feature.signing.core.SignEnding
import app.getvela.wallet.feature.signing.core.SignEndingState
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import uniffi.vela_core_uniffi.signEndingOf
import uniffi.vela_core_uniffi.signEndingState

/**
 * What a page's request ended as, held after the core closes its sheet (spec
 * 079). The core clears the signing sheet the moment it answers the page —
 * with the transaction hash when the operation landed inside the wait, with
 * the operation hash when the wait ran out, with the signature for a message —
 * and the sheet used to vanish at that instant: the person never saw it land.
 * This keeps the ending on screen: a tick that goes away by itself, or, for an
 * operation still on its way, the honest "not landed yet" until the person
 * closes it or the tracker sees it land.
 *
 * Spec 082 RA8: both halves are the core's. [ending] is `sign_ending_of`'s
 * reading of the answer, [state] is `sign_ending_state` once the tracker has
 * had its say — a landed operation is drawn confirmed only when the tracker
 * says so (it alone judges a revert, a Safe `ExecutionFailure` included), and
 * one it never saw reads "not sent".
 */
data class SigningAftercare(val chainId: Int, val ending: SignEnding) {

    /** The operation the tracker follows for this ending, if it has one. */
    val userOpHash: String?
        get() = when (val e = ending) {
            is SignEnding.Landed -> e.user_op_hash
            is SignEnding.StillConfirming -> e.user_op_hash
            SignEnding.Signed -> null
        }

    /** What the ending stands for now, given the tracker's entry for its operation (`null`: not taken yet). */
    fun state(track: TrackEntryView?): SignEndingState {
        val entry = track?.takeIf { entry -> userOpHash?.equals(entry.user_op_hash, ignoreCase = true) == true }
        val endingJson = Wire.json.encodeToString(SignEnding.serializer(), ending)
        val entryJson = entry?.let { Wire.json.encodeToString(TrackEntryView.serializer(), it) }
        return runCatching {
            Wire.json.decodeFromString(SignEndingState.serializer(), signEndingState(endingJson, entryJson))
        }.getOrElse { error ->
            // Nothing the core could read: never "confirmed" on a guess — the
            // operation is still being followed.
            VelaLog.failure("sign.ending", "the ending's state could not be read", error)
            if (ending is SignEnding.Signed) {
                SignEndingState.Signed
            } else {
                SignEndingState.Following(userOpHash.orEmpty(), app.getvela.wallet.feature.send.core.TrackOutcome.Landing)
            }
        }
    }

    companion object {
        /**
         * The ending an answer stands for, or `null` when there is nothing to
         * show — a refusal (the page was told why; the sheet already said so)
         * or an answer with no result. The core's reading (`sign_ending_of`).
         */
        fun of(method: String, chainId: Int, payload: SignResponsePayload, submittedUserOp: String?): SigningAftercare? {
            val payloadJson = Wire.json.encodeToString(SignResponsePayload.serializer(), payload)
            val endingJson = runCatching { signEndingOf(method, payloadJson, submittedUserOp) }
                .onFailure { VelaLog.failure("sign.ending", "the answer could not be read", it) }
                .getOrNull() ?: return null
            val ending = runCatching { Wire.json.decodeFromString(SignEnding.serializer(), endingJson) }
                .onFailure { VelaLog.failure("sign.ending", "an ending this app cannot decode", it) }
                .getOrNull() ?: return null
            return SigningAftercare(chainId, ending)
        }
    }
}
