package app.getvela.wallet.feature.signing.clearsigner

import uniffi.vela_core_uniffi.ClearSignerCeremonyOutcome
import uniffi.vela_core_uniffi.ClearSignerOutcome
import uniffi.vela_core_uniffi.WalletKeyRecord

/**
 * One request put to the Clear Signer, and what came back — the vocabulary
 * every channel to the page speaks (spec 075).
 *
 * Two kinds of request, because the core judges them differently: a SIGNATURE
 * is an assertion over one digest by one of this wallet's keys (spec 071), and
 * a CEREMONY is a passkey operation the create/sign-in machines asked for,
 * judged against the operation's own rules. Both ride the same channels — the
 * loopback socket on this device, the relay to another — and both carry the
 * page they belong to, so a channel never has to know which flow it is in.
 */
sealed interface ClearSignerAsk {
    /** The request JSON the page receives (`{intent, context}`). */
    val requestJson: String

    /** Spec 071: sign this digest with one of these keys. */
    data class Signature(
        override val requestJson: String,
        val digest: ByteArray,
        val keys: List<WalletKeyRecord>,
    ) : ClearSignerAsk {
        // Data classes over a ByteArray: identity, not content — nothing
        // compares two asks, and the generated `equals` would be a trap.
        override fun equals(other: Any?): Boolean = this === other
        override fun hashCode(): Int = System.identityHashCode(this)
    }

    /**
     * Spec 075: a passkey ceremony. [operationJson] is the machine operation's
     * own wire JSON — exactly what the executor received — and
     * [expectedMemberChallenge] the registry challenge the WALLET fetched, for
     * a member proof.
     */
    data class Ceremony(
        override val requestJson: String,
        val operationJson: String,
        val expectedMemberChallenge: ByteArray?,
    ) : ClearSignerAsk {
        override fun equals(other: Any?): Boolean = this === other
        override fun hashCode(): Int = System.identityHashCode(this)
    }
}

/** What one [ClearSignerAsk] came back as. */
sealed interface ClearSignerAnswer {
    data class Signed(val outcome: ClearSignerOutcome) : ClearSignerAnswer

    data class Ceremonial(val outcome: ClearSignerCeremonyOutcome) : ClearSignerAnswer

    /** The person closed the page, cancelled the sheet, or left the screen. */
    data object Cancelled : ClearSignerAnswer

    data object TimedOut : ClearSignerAnswer

    /** The channel itself could not be used (the relay was unreachable). */
    data class Unreachable(val detail: String) : ClearSignerAnswer
}

/**
 * A live conversation with one Clear Signer page.
 *
 * A flow — create then the member proof, a sign-in then recovery's two proofs
 * — is ONE conversation: the page is opened once, each request is put in turn,
 * and [end] says the flow is over (`bye`, then the socket closes). That is the
 * core's session (contract §1.5), and it is why this is an object with a life
 * rather than a function per ceremony.
 */
interface ClearSignerWire {
    /** Put the next request and wait for its verdict. */
    suspend fun ask(ask: ClearSignerAsk): ClearSignerAnswer

    /** The flow is over: say `bye` and let the page go. Idempotent. */
    fun end()

    /** The waiting sheet's Cancel: the same as the person closing the page. */
    fun cancel()

    /** "Open the page again" — the same address, the same token. */
    fun reopen() = Unit
}
