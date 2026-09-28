package app.getvela.wallet.feature.signing

import app.getvela.wallet.feature.signing.core.SignResponsePayload

/**
 * What a page's request ended as, held after the core closes its sheet (spec
 * 079). The core clears the signing sheet the moment it answers the page —
 * with the transaction hash when the operation landed inside the wait, with
 * the operation hash when the wait ran out, with the signature for a message —
 * and the sheet used to vanish at that instant: the person never saw it land.
 * This keeps the ending on screen: a tick that goes away by itself, or, for an
 * operation still on its way, the honest "not landed yet" until the person
 * closes it or the tracker sees it land.
 */
sealed class SigningAftercare {
    abstract val chainId: Int

    /** A message was signed. */
    data class Signed(override val chainId: Int) : SigningAftercare()

    /** The operation landed inside the wait; the page got this hash. */
    data class Landed(override val chainId: Int, val txHash: String) : SigningAftercare()

    /** The wait ran out; the page got the operation hash and the tracker keeps following it. */
    data class StillConfirming(override val chainId: Int, val userOpHash: String) : SigningAftercare()

    companion object {
        private val TRANSACTION_METHODS = setOf("eth_sendTransaction", "wallet_sendCalls")

        /**
         * The ending an answer stands for, or `null` when there is nothing to
         * show — a refusal (the page was told why; the sheet already said so)
         * or an answer with no result.
         */
        fun of(method: String, chainId: Int, payload: SignResponsePayload, submittedUserOp: String?): SigningAftercare? {
            val result = (payload as? SignResponsePayload.Ok)?.result?.takeIf { it.isNotBlank() } ?: return null
            if (method !in TRANSACTION_METHODS) return Signed(chainId)
            return if (submittedUserOp != null && result.equals(submittedUserOp, ignoreCase = true)) {
                StillConfirming(chainId, submittedUserOp)
            } else {
                Landed(chainId, result)
            }
        }
    }
}
