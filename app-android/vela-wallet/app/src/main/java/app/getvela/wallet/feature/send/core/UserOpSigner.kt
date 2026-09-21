package app.getvela.wallet.feature.send.core

import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PasskeyExecutor

/**
 * The one seam the core cannot have: a challenge in, an assertion out.
 *
 * The send executor hands over the SafeOp hash (never a WebAuthn signing
 * hash — the core packs the assertion into the operation's signature and the
 * Safe verifies the client data it finds there). Outside the parallel space
 * the answer comes from the person's passkey through the ceremony onboarding
 * already owns; inside it, from the core's fixed keyset (research D2, D9).
 *
 * Exactly one call per attempt. The executor cancels the coroutine on
 * `cancel_passkey_sign`; a second prompt after a cancel is the bug spec 043
 * User Story 3 names, and the test for it counts calls on this interface.
 */
interface UserOpSigner {
    /**
     * @param challenge the 32-byte SafeOp hash.
     * @param credentialIdHex the pinned credential — the wallet's first key —
     *   or `null` for a discoverable ceremony.
     * @param transports the pinned key's stored transports (`"internal"`,
     *   `"hybrid,internal"`, `"usb,nfc"`…), load-bearing for routing.
     */
    suspend fun sign(
        challenge: ByteArray,
        credentialIdHex: String?,
        transports: String,
        method: KeyMethod,
    ): Assertion
}

/** The real thing: the same ceremony sign-in uses, with the same rpId. */
class PasskeyUserOpSigner(private val passkey: PasskeyExecutor) : UserOpSigner {
    override suspend fun sign(
        challenge: ByteArray,
        credentialIdHex: String?,
        transports: String,
        method: KeyMethod,
    ): Assertion = passkey.assert(
        challenge = challenge,
        credentialIdHex = credentialIdHex,
        transports = transports,
        method = method,
    )
}

/**
 * The fourth "Sign with" (spec 071): a separate page decodes the request from
 * the operation's own bytes, derives the digest itself and runs the ceremony.
 *
 * [requestJson] is the core's `clearSignerRequest`, [digest] the challenge the
 * passkey path would sign, [keys] the account's. The answer comes back already
 * judged by the core (this digest, one of these keys, a verified user), so the
 * caller packs it exactly as a passkey's. A person who closes the page gets a
 * [app.getvela.wallet.feature.onboarding.core.PasskeyFailure] of kind
 * `Cancelled`; every other refusal is one carrying the sentence to show.
 */
interface ClearSigner {
    suspend fun sign(requestJson: String, digest: ByteArray, keys: List<uniffi.vela_core_uniffi.WalletKeyRecord>): Assertion

    /**
     * What the page is told beside the request: the chain's name and coin
     * (the page prefers its own for chains it knows) and the account's name,
     * which points the person at a passkey.
     */
    fun describe(chainId: Int, account: String): ClearSignerLabels = ClearSignerLabels()
}

data class ClearSignerLabels(val chainName: String? = null, val nativeSymbol: String? = null, val accountName: String? = null)

/**
 * What a site asked for, as the Clear Signer shows it: the request's own
 * method and params, and the site's origin. The wallet's own send has none —
 * the core builds `wallet_sendCalls` from its calls.
 */
data class ClearSignerIntent(val method: String, val paramsJson: String, val origin: String)
