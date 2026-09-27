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
     * @param credentialIdHex the pinned credential — the key the account signed
     *   in with (the first key for a record from before that) — or `null` for a
     *   discoverable ceremony.
     * @param transports the transports its route carries (`"internal"`,
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
 * The Trusted Signer (spec 071): a separate page decodes the request from
 * the operation's own bytes, derives the digest itself and runs the ceremony.
 *
 * [requestJson] is the core's `trustedSignerRequest`, [digest] the challenge the
 * passkey path would sign, [keys] the account's. The answer comes back already
 * judged by the core (this digest, one of these keys, a verified user), so the
 * caller packs it exactly as a passkey's. A person who closes the page gets a
 * [app.getvela.wallet.feature.onboarding.core.PasskeyFailure] of kind
 * `Cancelled`; every other refusal is one carrying the sentence to show.
 */
interface TrustedSigner {
    /**
     * [signerOrigin] is spec 075: a key minted or found through the Clear
     * Signer lives behind ONE page, and that is the page to open — empty means
     * the person's own page from Settings.
     */
    suspend fun sign(
        requestJson: String,
        digest: ByteArray,
        keys: List<uniffi.vela_core_uniffi.WalletKeyRecord>,
        signerOrigin: String = "",
    ): Assertion

    /**
     * What the page is told beside the request: the chain's name and coin
     * (the page prefers its own for chains it knows) and the account's name,
     * which points the person at a passkey.
     */
    fun describe(chainId: Int, account: String): TrustedSignerLabels = TrustedSignerLabels()
}

data class TrustedSignerLabels(val chainName: String? = null, val nativeSymbol: String? = null, val accountName: String? = null)

/**
 * What a site asked for, as the Trusted Signer shows it: the request's own
 * method and params, and the site's origin. The wallet's own send has none —
 * the core builds `wallet_sendCalls` from its calls.
 */
data class TrustedSignerIntent(val method: String, val paramsJson: String, val origin: String)
