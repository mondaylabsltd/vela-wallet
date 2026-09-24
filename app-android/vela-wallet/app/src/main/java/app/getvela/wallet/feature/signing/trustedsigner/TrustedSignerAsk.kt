package app.getvela.wallet.feature.signing.trustedsigner

import uniffi.vela_core_uniffi.TrustedSignerCeremonyOutcome
import uniffi.vela_core_uniffi.TrustedSignerOutcome
import uniffi.vela_core_uniffi.WalletKeyRecord

/**
 * One request put to the Trusted Signer, and what came back — the vocabulary
 * every channel to the page speaks (spec 075).
 *
 * Two kinds of request, because the core judges them differently: a SIGNATURE
 * is an assertion over one digest by one of this wallet's keys (spec 071), and
 * a CEREMONY is a passkey operation the create/sign-in machines asked for,
 * judged against the operation's own rules. Both ride the same channels — the
 * loopback socket on this device — and both carry the
 * page they belong to, so a channel never has to know which flow it is in.
 */
sealed interface TrustedSignerAsk {
    /** The request JSON the page receives (`{intent, context}`). */
    val requestJson: String

    /** Spec 071: sign this digest with one of these keys. */
    data class Signature(
        override val requestJson: String,
        val digest: ByteArray,
        val keys: List<WalletKeyRecord>,
    ) : TrustedSignerAsk {
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
    ) : TrustedSignerAsk {
        override fun equals(other: Any?): Boolean = this === other
        override fun hashCode(): Int = System.identityHashCode(this)
    }
}

/** What one [TrustedSignerAsk] came back as. */
sealed interface TrustedSignerAnswer {
    data class Signed(val outcome: TrustedSignerOutcome) : TrustedSignerAnswer

    data class Ceremonial(val outcome: TrustedSignerCeremonyOutcome) : TrustedSignerAnswer

    /** The person closed the page, cancelled the sheet, or left the screen. */
    data object Cancelled : TrustedSignerAnswer

    data object TimedOut : TrustedSignerAnswer

    /** The channel itself could not be used. [detail] is for the log. */
    data class Unreachable(
        val detail: String,
        val why: Unreachability = Unreachability.Channel,
    ) : TrustedSignerAnswer
}

/** The shape of a channel failure — never its words. */
enum class Unreachability {
    /** It never opened: a socket that would not bind, a tab that would not open. */
    Channel,

    /** It opened and then the page left — a closed socket. */
    PeerGone,
}

/**
 * A live conversation with one Trusted Signer page.
 *
 * A flow — create then the member proof, a sign-in then recovery's two proofs
 * — is ONE conversation: the page is opened once, each request is put in turn,
 * and [end] says the flow is over (`bye`, then the socket closes). That is the
 * core's session (contract §1.5), and it is why this is an object with a life
 * rather than a function per ceremony.
 */
interface TrustedSignerWire {
    /** Put the next request and wait for its verdict. */
    suspend fun ask(ask: TrustedSignerAsk): TrustedSignerAnswer

    /** The flow is over: say `bye` and let the page go. Idempotent. */
    fun end()

    /** The waiting sheet's Cancel: the same as the person closing the page. */
    fun cancel()

    /** "Open the page again" — the same address, the same token. */
    fun reopen() = Unit
}
