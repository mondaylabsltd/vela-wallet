package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import org.json.JSONObject
import uniffi.vela_core_uniffi.PasskeyCeremonyEnd
import uniffi.vela_core_uniffi.RelayRejection
import uniffi.vela_core_uniffi.passkeyCeremonyEnd
import uniffi.vela_core_uniffi.passkeySignTimeoutMs
import uniffi.vela_core_uniffi.WalletKeyRecord
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.UserOpDraft
import uniffi.vela_core_uniffi.userOpCallsToMeasure
import uniffi.vela_core_uniffi.userOpRaiseCallGas
import uniffi.vela_core_uniffi.UserOpFeeMode
import uniffi.vela_core_uniffi.WebAuthnAssertion
import uniffi.vela_core_uniffi.TrustedSignerInput
import uniffi.vela_core_uniffi.trustedSignerRequest
import app.getvela.wallet.feature.signing.trustedsigner.PageSignFailure
import app.getvela.wallet.feature.signing.trustedsigner.SigningPlan
import app.getvela.wallet.feature.signing.trustedsigner.SigningRoute
import app.getvela.wallet.feature.signing.trustedsigner.VenueBlock
import app.getvela.wallet.feature.signing.trustedsigner.VenueWords
import uniffi.vela_core_uniffi.eip1271Signature
import uniffi.vela_core_uniffi.safeMessageHash
import uniffi.vela_core_uniffi.isChainWithoutNativeCoin
import uniffi.vela_core_uniffi.quotedFeeUsable
import uniffi.vela_core_uniffi.userOpHash
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.vela_core_uniffi.userOpApplyEstimate
import uniffi.vela_core_uniffi.userOpDraft
import uniffi.vela_core_uniffi.userOpFloors
import uniffi.vela_core_uniffi.userOpHasContractCall
import uniffi.vela_core_uniffi.userOpRelayJson
import uniffi.vela_core_uniffi.CoreException
import uniffi.vela_core_uniffi.userOpReviewedSafeOpHash
import uniffi.vela_core_uniffi.userOpSafeOpHash
import uniffi.vela_core_uniffi.userOpSign
import uniffi.vela_core_uniffi.userOpStampValidity
import uniffi.vela_core_uniffi.userOpWithCalls

/**
 * The submit spine — ONE implementation for a person's own transfer and a
 * dApp's transaction (spec 044 T028, research D7; the desktop's
 * `executor::user_op::submit`).
 *
 * The order is `sendUserOpInBand`'s: isDeployed → nonce → floors → the
 * placeholder fee leg → estimate → apply → the settled fee leg → the SafeOp
 * hash is the challenge → the assertion → the envelope → the relay. The
 * assembly is the core's (`user_op_*` through uniffi); the transports are
 * [RelayClient]'s; the one seam the core cannot have is [UserOpSigner].
 * Nothing here prices, validates or classifies beyond what the core exports.
 *
 * Extracted from `SendExecutor` so the sheet that shows what will happen
 * and the code that makes it happen cannot become two opinions.
 */
class UserOpSpine(
    private val relay: RelayClient,
    private val accounts: SendExecutor.AccountPort,
    private val signer: () -> UserOpSigner,
    /**
     * `eth_estimateGas({from, to, value, data})` on one chain — the gas hex, or
     * `null` when nobody answered. The inner calls' own measurement (spec 062):
     * the relay estimates `callGasLimit` against the SENDER, and an undeployed
     * Safe has no code there, so its figure is 21k plus calldata whatever the
     * call does. Measured from the Safe's address instead, a codeless account
     * estimates like any other. The rule is the core's (`userOpRaiseCallGas`).
     */
    private val measureCall: suspend (chainId: Int, from: String, to: String, valueHex: String, data: String) -> String? =
        { _, _, _, _, _ -> null },
    /** Spec 071/102: the signing page, when this build can open one. */
    private val trustedSigner: () -> TrustedSigner? = { null },
    /**
     * Spec 102 D4: the surface this spine signs for already drew the hand-off
     * card (the dApp sheet does), so the page opens on the person's Open
     * there. `false` (a send): the channel raises the card itself first.
     */
    private val handoffShown: Boolean = false,
    /** The corpus, for the sentences a signature that cannot be made is told in. */
    private val words: (key: String, vars: Map<String, String>) -> String = { key, _ -> key },
    /**
     * send-11: the passkey prompt's deadline, ms — the core's
     * (`passkey_sign_timeout_ms`), what the platform's prompt is asked for
     * and what this spine stops waiting at. A seam for the tests only.
     */
    private val signDeadlineMs: Long = passkeySignTimeoutMs().toLong(),
    /** A monotonic clock, ms: how long the prompt was up, for the core's `passkey_ceremony_end`. */
    private val clock: () -> Long = { System.nanoTime() / 1_000_000 },
    /**
     * §35: the wall clock, Unix ms — what the core stamps the operation's
     * validity window from (`userOpStampValidity`). A seam for the tests only.
     */
    private val wallClockMs: () -> Long = System::currentTimeMillis,
) {
    /**
     * Where one ceremony goes — the core's `signing_plan` (spec 102), read
     * for this spine: the credential it is pinned to, the transports and the
     * place the native ceremony routes by, and [page] — the account's venue,
     * where its transactions and messages are reviewed and signed (R4);
     * `null` signs in Vela. [signInKey]: the plan named the key (R5), which
     * answers for that one key and no other; [keyRouteJson] is that route as
     * the core wrote it, handed back to the page request. [blocked]: nothing
     * on this device can reach the keys (the core's `VenueBlock`).
     */
    data class Route(
        val credentialId: String,
        val transports: String,
        val method: KeyMethod,
        val page: String? = null,
        val signInKey: Boolean = false,
        val keyRouteJson: String? = null,
        /** The hand-off card's key row: the plan's `key_label` (D-17); `null` for a record without one. */
        val keyLabel: SigningPlan.KeyLabel? = null,
        val blocked: JSONObject? = null,
        /**
         * R7: the core routes this method nowhere (`signing_route` answered
         * nothing — `eth_sign`) while the account reviews on a page: it is
         * refused, never signed in Vela behind the person's choice.
         */
        val unsupported: Boolean = false,
    )

    /**
     * Where this account's signature of [method] goes (spec 102, R7): the
     * core's `signing_route(account, method)` over the stored record — the
     * same rule for the wallet's own send (`""`), a site's transaction, a
     * batch, `personal_sign` and typed data, as the web's `signChallenge`
     * asks it. There is no choice at signing time — the venue is the
     * account's setting, and the key is the one it was created or last
     * signed in with (founder, 2026-09-26). A record written before the
     * sign-in key was kept signs as it always did: the first key over its
     * stored transports.
     */
    internal suspend fun routeFor(account: String, first: WalletKeyRecord, method: String = ""): Route {
        val record = runCatching { accounts.accountJson(account) }.getOrNull()
        val plan = SigningPlan.of(record)
        val keyed = plan?.credentialId?.let { credential ->
            Route(
                credentialId = credential,
                transports = plan.transports,
                method = plan.method ?: KeyMethod.Platform,
                signInKey = true,
                keyRouteJson = plan.keyRouteJson,
            )
        }
        val route = keyed ?: accounts.routingOf(account).let { (transports, method) ->
            Route(first.credentialId, transports, method)
        }
        // A record the core cannot plan for signs as before: no venue to ask.
        if (plan == null) return route
        // The plan's own block first (the web's `plan.blocked`), then R7.
        if (plan.blocked != null) return route.copy(blocked = plan.blocked, keyLabel = plan.keyLabel)
        return when (val venue = SigningRoute.of(record, method)) {
            is SigningRoute.Page -> route.copy(page = venue.url, keyLabel = plan.keyLabel)
            is SigningRoute.Blocked -> route.copy(blocked = venue.block, keyLabel = plan.keyLabel)
            SigningRoute.InVela -> route.copy(keyLabel = plan.keyLabel)
            // A method no venue signs: in Vela only for an account whose
            // venue IS Vela; refused for one that chose a page.
            null -> route.copy(keyLabel = plan.keyLabel, unsupported = plan.page != null)
        }
    }

    /**
     * Spec 102 D4: the hand-off card a surface draws for [account]'s request
     * of [method] — the page the core routes it to (R7) and what the person
     * confirms with — or `null` when it signs in Vela (or cannot sign here
     * at all, or not this method).
     */
    suspend fun handoffFor(account: String, method: String = ""): Handoff? {
        val first = runCatching { accounts.keysOf(account) }.getOrNull()?.firstOrNull() ?: return null
        val route = runCatching { routeFor(account, first, method) }.getOrNull() ?: return null
        if (route.unsupported) return null
        val page = route.page?.takeIf { route.blocked == null } ?: return null
        return Handoff(page, route.keyLabel)
    }

    /**
     * The hand-off card's two facts: where the person signs, and with which
     * key — the core's `KeyLabel`, translated where it is drawn; `null` when
     * the plan names none.
     */
    data class Handoff(val page: String, val key: SigningPlan.KeyLabel?)

    /**
     * The one signature of an attempt: the person's passkey over the account's
     * route — or, when the account's venue is a page (R4), that page ([request]
     * is only called then, with the keys the page may use and the key route).
     */
    private suspend fun ceremony(
        challenge: ByteArray,
        chainId: Int,
        account: String,
        pinned: WalletKeyRecord,
        keys: List<WalletKeyRecord>,
        /** The request's JSON-RPC method (`""`: the wallet's own send) — what the core routes by (R7). */
        method: String,
        request: (TrustedSignerLabels, List<WalletKeyRecord>, String?) -> String,
    ): Assertion = try {
        val picked = routeFor(account, pinned, method)
        // R1: a custom-domain account whose page this device does not know —
        // nothing here can reach its keys, and the core says why.
        picked.blocked?.let { block ->
            throw Refused(
                VenueBlock.of(block)?.let(Failure::VenueBlocked)
                    ?: Failure.Signer(FailureKind.Other, VenueWords.block(block, words)),
            )
        }
        // R7: the account reviews on a page, and no venue signs this method:
        // the page could not take it — nothing was signed.
        if (picked.unsupported) {
            VelaLog.event("userop.sign", "no venue signs this method; the page could not take it", "method" to method)
            throw Refused(Failure.PageFailed(app.getvela.wallet.feature.signing.core.PageFailure.Refused))
        }
        val page = picked.page
        if (page != null) {
            val channel = trustedSigner() ?: other(words(SIGNER_DOWN, emptyMap()))
            // The sign-in key only: the page is told to allow it alone, and the
            // answer is judged against it alone.
            val allowed = if (picked.signInKey) {
                keys.filter { it.credentialId.equals(picked.credentialId, ignoreCase = true) }.ifEmpty { keys }
            } else {
                keys
            }
            val requestJson = runCatching {
                request(channel.describe(chainId, account), allowed, picked.keyRouteJson)
            }.getOrElse { error ->
                if (error is Refused) throw error
                other(error.message ?: words(SIGNER_REFUSED, emptyMap()))
            }
            channel.sign(requestJson, challenge, allowed, page, picked.keyLabel, askFirst = !handoffShown)
        } else {
            // send-11: the prompt runs to the core's deadline, and how it
            // ended — closed by the person, or out of time — is the core's
            // reading of the platform's answer and the clock.
            val raised = clock()
            try {
                withTimeoutOrNull(signDeadlineMs) {
                    signer().sign(challenge, picked.credentialId, picked.transports, picked.method)
                } ?: throw PasskeyFailure(FailureKind.Cancelled, "the passkey prompt ran out of time")
            } catch (failure: PasskeyFailure) {
                throw Refused(ceremonyFailure(failure, clock() - raised))
            }
        }
    } catch (failure: PageSignFailure) {
        // The page is the venue: the person's "no" there is their answer
        // (`PageDeclined`); anything else is the page's failure, by the
        // core's name for it (round 7) — never "couldn't submit".
        throw Refused(failure.page?.let(Failure::PageFailed) ?: Failure.PageDeclined)
    } catch (failure: PasskeyFailure) {
        if (failure.kind == FailureKind.Cancelled) throw Refused(Failure.PasskeyCancelled)
        // Spec 099 R8: how it failed travels on, as the app's passkey
        // classifier read it, so the sheet and the page's record name the
        // signer rather than "couldn't submit".
        throw Refused(Failure.Signer(failure.kind, failure.message ?: "the passkey ceremony failed"))
    }

    /**
     * A multisig member's own passkey over [challenge], in Vela (native
     * multisig: an approval of a SafeTx, or the owner proof a creation asks).
     * The key is the account's plan's, as for every signature; WHERE it signs
     * — in Vela, on its page, or not from here — is the caller's to have
     * asked the core first (`multisigApprovalRoute`, `signingRoute`). A plan
     * that says nothing here can reach the keys is refused as it is for a send.
     */
    suspend fun assertInVela(account: String, challenge: ByteArray): Assertion {
        val keys = accounts.keysOf(account)
        val pinned = keys.firstOrNull() ?: other("No passkey credential for this account")
        val picked = routeFor(account, pinned, "")
        picked.blocked?.let { block ->
            throw Refused(
                VenueBlock.of(block)?.let(Failure::VenueBlocked)
                    ?: Failure.Signer(FailureKind.Other, VenueWords.block(block, words)),
            )
        }
        val raised = clock()
        return try {
            withTimeoutOrNull(signDeadlineMs) {
                signer().sign(challenge, picked.credentialId, picked.transports, picked.method)
            } ?: throw PasskeyFailure(FailureKind.Cancelled, "the passkey prompt ran out of time")
        } catch (failure: PasskeyFailure) {
            throw Refused(ceremonyFailure(failure, clock() - raised))
        }
    }

    /**
     * A multisig member's owner proof (§16): [challenge] — the genesis
     * service's sponsor challenge — signed where the core routes
     * `vela_multisigCreate` for this account: in Vela, or on its trusted page,
     * handed [paramsJson] (the core's `PasskeyPurpose::Record.params`: the
     * genesis, the member, the networks), from which the page reads the
     * address, the members and the rule itself. A page's answer is accepted
     * only over [challenge] (`trustedSignerSignatureAccept`, whose bytes the
     * core derives from the genesis). Nothing is signed behind a page the
     * person chose.
     */
    suspend fun signOwnerProof(account: String, chainId: Int, challenge: ByteArray, paramsJson: String): Assertion {
        val keys = accounts.keysOf(account)
        val pinned = keys.firstOrNull() ?: other("No passkey credential for this account")
        return ceremony(challenge, chainId, account, pinned, keys, OWNER_PROOF_METHOD) { labels, allowed, keyRoute ->
            trustedSignerRequest(
                TrustedSignerInput(
                    method = OWNER_PROOF_METHOD, paramsJson = paramsJson, origin = "",
                    originSeenByBrowser = false,
                    chainId = chainId.toUInt(), chainName = labels.chainName, nativeSymbol = labels.nativeSymbol,
                    account = account, accountName = labels.accountName,
                    credentialIdsHex = allowed.map { it.credentialId }, keyRouteJson = keyRoute, calls = emptyList(),
                ),
                null,
            )
        }
    }

    /** The displayed fee, signed verbatim. */
    data class Quoted(val amount: String, val recipient: String, val tier: FeeTier? = null)

    sealed class Failure {
        data object PasskeyCancelled : Failure()

        /**
         * send-11: the passkey prompt ran out of time — the core's reading of
         * a cancel at its deadline (`passkey_ceremony_end`). Handled as
         * [PasskeyCancelled]; only the words differ ("timed out").
         */
        data object PasskeyTimedOut : Failure()
        data object RelayerUnavailable : Failure()
        data object BundlerUnderfunded : Failure()
        data class Other(val message: String?) : Failure()

        /**
         * Spec 099 R8: the passkey ceremony failed, as the classifier create
         * and login use read the platform's error — no passkey can be used
         * here, one sign-in would never offer, or another failure. Never
         * [FailureKind.Cancelled], which is [PasskeyCancelled]. Nothing was
         * signed.
         */
        data class Signer(val kind: FailureKind, val message: String) : Failure()

        /**
         * Spec 082 RJ3: the relay refused the op — a rejection that is not
         * "relayer unavailable" and not an underfunded bundler. Nothing was
         * sent, and the same op would be refused again: a page is told the
         * core's fixed "refused" sentence, and nobody is told to retry.
         */
        data class Rejected(val message: String?) : Failure()

        /**
         * Spec 082 RJ1: the record was not written ahead in time — no
         * `ClearToPost` within `userOpWriteAheadWaitMs()` — so the op was not
         * POSTed. Nothing left the device.
         */
        data object NotCleared : Failure()

        /**
         * Spec 102: the account's venue cannot be used on this device (the
         * plan's `blocked`) — nothing was signed or sent. Told to the core as
         * `venue_blocked`, which says why in the person's language; never
         * retried.
         */
        data class VenueBlocked(val block: VenueBlock) : Failure()

        /**
         * Spec 102 (2026-10-30): the person declined on the account's trusted
         * signing page, or closed it unsigned. The page is the venue, so that
         * is their answer: a site hears 4001 (`SignSubmitOutcome.PageDeclined`);
         * a send keeps its confirm open (`passkey_cancelled`).
         */
        data object PageDeclined : Failure()

        /**
         * Round 7: the page did not sign, and not by the person's choice —
         * [failure] says which (never answered, an answer not this request's,
         * its own rules, its check). Nothing was signed.
         */
        data class PageFailed(val failure: app.getvela.wallet.feature.signing.core.PageFailure) : Failure()

        /**
         * Another operation of the account still holds the nonce on this chain
         * (`RelayRejection::NonceHeld` — the relay's `nonce_in_flight`, or an
         * older relay's `[existingHash:…]` marker). Nothing of this one went
         * out. Send says "waiting for your last transaction" with Try again
         * (`SendSubmitFailure.PreviousPending`); a page is told
         * `userOpPreviousPendingDetail()`.
         */
        data object PreviousPending : Failure()

        /**
         * send-15: the nonce this operation signed was spent already — another
         * transaction of the account went first (`RelayRejection::NonceUsed`,
         * the relay's bare AA25 as the core reads it). Nothing of this one was
         * sent. Send says it calmly, "went first", with Review again
         * (`SendSubmitFailure.NonceUsed`); a page is told the core's fixed
         * detail (`userOpNonceUsedDetail()`) as a refusal.
         */
        data object NonceUsed : Failure()

        /**
         * §35: the operation's validity window closed before the EntryPoint
         * saw it (`RelayRejection::Expired`, the relay's `AA22 expired or not
         * due`). Nothing was sent, and it can never land. Send says it calmly
         * (`SendSubmitFailure.Expired`) with Try again, which signs anew with
         * a window of its own; a page is told the core's fixed detail
         * (`userOpExpiredDetail()`).
         */
        data object Expired : Failure()

        /**
         * Spec 082 RA1: the relay was never reached and nothing left the device
         * — the one failure after the passkey where "not sent, try again" is
         * true. A page is told the core's fixed sentence, never the pool's.
         */
        data object NotSent : Failure()
    }

    /**
     * What a submit handed the relay (spec 082): the hash to follow, whether
     * the reply was lost ([maybeSent] — the hash is then the locally computed
     * one), and the head read before the first POST ([submitBlock], `null` =
     * unknown). Both flags go with the record and into the tracker.
     */
    data class Submitted(
        val userOpHash: String,
        val maybeSent: Boolean = false,
        val submitBlock: Long? = null,
        /** Round 3 (send-5): the nonce the op signed, a hex quantity — what the tracker reads a lost reply against. */
        val nonce: String? = null,
    )

    /**
     * [chainRead]: the refusal is a read of the chain that went unanswered —
     * whether the account is deployed, its nonce (r5reg-3) — the network's
     * failure, never the operation's.
     */
    class Refused(val failure: Failure, val chainRead: Boolean = false) : Exception()

    private companion object {
        /** The corpus lines a page that cannot be reached, or will not sign, is told in. */
        const val SIGNER_DOWN = "componentsUi.signing.signerDown"
        const val SIGNER_REFUSED = "componentsUi.signing.trustedSignerRefused"

        /** §16: a multisig member's owner proof (vela-core `MULTISIG_CREATE_METHOD`). */
        const val OWNER_PROOF_METHOD = "vela_multisigCreate"

        /** How long the POST waits for a head read still out; past it the head is unknown. */
        const val HEAD_WAIT_MS = 1_500L
    }

    private fun other(message: String): Nothing = throw Refused(Failure.Other(message))

    /** A read of the chain nobody answered (r5reg-3): the network's failure. */
    private fun unanswered(message: String): Nothing = throw Refused(Failure.Other(message), chainRead = true)

    /**
     * Every real contract call measured from the Safe's own address; the draft's
     * `callGasLimit` raised to the core's floor when that is higher. A call nobody
     * could measure leaves the relay's figure — and its existing guard — alone.
     */
    private suspend fun raisedToMeasuredFloor(
        draft: UserOpDraft,
        chainId: Int,
        account: String,
        calls: List<UserOpCall>,
    ): UserOpDraft {
        val toMeasure = userOpCallsToMeasure(calls)
        if (toMeasure.isEmpty()) return draft
        val measured = ArrayList<String>(toMeasure.size)
        for (index in toMeasure) {
            val call = calls[index.toInt()]
            val valueHex = if (call.value.startsWith("0x")) call.value else "0x" + java.math.BigInteger(call.value.ifBlank { "0" }).toString(16)
            val hex = measureCall(chainId, account, call.to, valueHex, call.data) ?: return draft
            measured += java.math.BigInteger(hex.removePrefix("0x"), 16).toString()
        }
        val raised = userOpRaiseCallGas(draft, measured, calls.size.toUInt())
        if (raised.callGasLimit != draft.callGasLimit) {
            VelaLog.event("userop.submit", "callGasLimit raised to the inner calls' own estimate", "relay" to draft.callGasLimit, "inner" to raised.callGasLimit)
        }
        return raised
    }

    /**
     * A page's message (spec 044): the Safe message hash under the Safe's
     * own domain is the passkey's challenge; the assertion is encoded as the
     * EIP-1271 envelope — `isValidSignature` verifies it on chain. One
     * ceremony, nothing submitted. Returns the signature hex.
     */
    suspend fun signMessage(
        chainId: Int,
        account: String,
        originalHash: ByteArray,
        signingStarted: () -> Unit = {},
        /** The page's own request — what the Trusted Signer shows and re-derives the digest from. */
        intent: TrustedSignerIntent? = null,
        /** Spec 082 RA9: the ceremony returned a signature (`CeremonyDone`). */
        ceremonyDone: () -> Unit = {},
    ): String {
        val keys = accounts.keysOf(account)
        if (keys.isEmpty()) other("No passkey credential for the active account")
        // A submit, and the first quote after it, landed or not, measure the
        // chain again (issue 212).
        relay.invalidateFeeSignals(chainId)
        val pinned = keys.first()
        val challenge = runCatching { safeMessageHash(originalHash, chainId.toULong(), account) }
            .getOrElse { other(it.message ?: "The message could not be hashed") }
        signingStarted()
        // R7: a message routes by its own method (`personal_sign`, typed data).
        val method = intent?.method ?: "personal_sign"
        val assertion = ceremony(challenge, chainId, account, pinned, keys, method) { labels, allowed, keyRoute ->
            // A message is always a site's request: the page shows that request.
            val asked = intent ?: other(words(SIGNER_REFUSED, emptyMap()))
            trustedSignerRequest(
                TrustedSignerInput(
                    method = asked.method, paramsJson = asked.paramsJson, origin = asked.origin,
                    originSeenByBrowser = asked.seenByBrowser,
                    chainId = chainId.toUInt(), chainName = labels.chainName, nativeSymbol = labels.nativeSymbol,
                    account = account, accountName = labels.accountName,
                    credentialIdsHex = allowed.map { it.credentialId }, keyRouteJson = keyRoute, calls = emptyList(),
                ),
                null,
            )
        }
        ceremonyDone()
        val signature = runCatching {
            eip1271Signature(
                WebAuthnAssertion(
                    authenticatorData = SendExecutor.unhex(assertion.authenticatorDataHex),
                    clientDataJson = SendExecutor.unhex(assertion.clientDataJsonHex),
                    signatureDer = SendExecutor.unhex(assertion.signatureDerHex),
                ),
                assertion.credentialIdHex,
                keys,
            )
        }.getOrElse { other(it.message ?: "Failed to create signature") }
        return "0x" + signature.joinToString("") { b -> "%02x".format(b.toInt() and 0xff) }
    }

    /**
     * Assembles, signs once and submits; returns what the relay was handed —
     * accepted (or the op already pending for this nonce), or may-have-been-
     * sent under the local hash. Throws [Refused] only when it was NOT sent.
     *
     * [beforePost] (spec 082 RJ1) runs after the signature, the local hash and
     * the head read, and before the first POST: the write-ahead — the caller
     * reports the op signed, waits for the core's `ClearToPost` and checks the
     * asker. It throws [Refused] (nothing is POSTed) when the record is not on
     * disk in time, or the asker is gone.
     */
    suspend fun submit(
        chainId: Int,
        account: String,
        calls: List<UserOpCall>,
        gasFeeToken: String?,
        quotedFee: Quoted?,
        /**
         * A send: the review the person confirmed (`SubmitUserOp.review`, as
         * JSON). The passkey's hash is computed only for the operation that is
         * exactly it. `null` for a site's request, reviewed on its sheet.
         */
        review: String? = null,
        signingStarted: () -> Unit = {},
        /** A site's request (spec 071); `null` for the wallet's own send. */
        intent: TrustedSignerIntent? = null,
        /** Spec 082 RA9: the ceremony returned a signature (`CeremonyDone`). */
        ceremonyDone: () -> Unit = {},
        beforePost: suspend (localHash: String, submitBlock: Long?) -> Unit = { _, _ -> },
        /**
         * core-perf-1: the gas the confirmed fee was priced on
         * (`SubmitUserOp.prepared`), fresh and for exactly these calls. Signed
         * as measured — no second relay simulation, no second measurement of
         * the inner calls — when it measured the account as it stands now.
         */
        prepared: PreparedGas? = null,
    ): Submitted = coroutineScope {
        // The head before the first POST (ruling 8): read beside the assembly
        // and the ceremony, taken only if it came back before the POST.
        val head = async { runCatching { relay.headBlock(chainId) }.getOrNull() }
        try {
            submitSigned(chainId, account, calls, gasFeeToken, quotedFee, review, signingStarted, intent, ceremonyDone, beforePost, prepared) {
                withTimeoutOrNull(HEAD_WAIT_MS) { head.await() }
            }
        } finally {
            head.cancel()
        }
    }

    private suspend fun submitSigned(
        chainId: Int,
        account: String,
        calls: List<UserOpCall>,
        gasFeeToken: String?,
        quotedFee: Quoted?,
        review: String?,
        signingStarted: () -> Unit,
        intent: TrustedSignerIntent?,
        ceremonyDone: () -> Unit,
        beforePost: suspend (localHash: String, submitBlock: Long?) -> Unit,
        prepared: PreparedGas?,
        headBeforePost: suspend () -> Long?,
    ): Submitted {
        val keys = accounts.keysOf(account)
        if (keys.isEmpty()) other("No passkey credential for the active account")
        // A submit, and the first quote after it, landed or not, measure the
        // chain again (issue 212) — the held gas readings and the held
        // simulation with them: the nonce this op consumes is not the next's.
        relay.invalidateFeeSignals(chainId)
        val pinned = keys.first()
        val tempo = isChainWithoutNativeCoin(chainId.toUInt())
        val deployed = relay.isDeployed(chainId, account)
            ?: unanswered("The network could not be reached. Please try again.")
        val nonce = if (deployed) {
            relay.nonce(chainId, account) ?: unanswered("The account's nonce could not be read. Please try again.")
        } else {
            "0x0"
        }
        val floors = userOpFloors(chainId.toUInt(), deployed, (calls.size + 1).toUInt())
        val quoted = quotedFee
            ?.takeIf { quotedFeeUsable(it.amount, it.recipient) }
            ?: other("The fee quote has expired. Please review the updated fee and try again.")
        val feeToken = if (tempo) gasFeeToken ?: SendExecutor.TEMPO_DEFAULT_FEE_TOKEN else gasFeeToken
        val settled: UserOpFeeMode
        val placeholder: UserOpFeeMode
        if (tempo) {
            val collector = relay.accountInfo(chainId, account)?.feeRecipient()
                ?: other("The Tempo gas relayer is unavailable right now. Please try again.")
            if (!quoted.recipient.equals(collector, ignoreCase = true)) {
                other("The gas quote has expired. Please review the updated fee and try again.")
            }
            settled = UserOpFeeMode.Tempo(feeToken = feeToken!!, collector = collector, reimbursement = quoted.amount)
            placeholder = UserOpFeeMode.Tempo(feeToken = feeToken, collector = collector, reimbursement = "1")
        } else {
            settled = UserOpFeeMode.InBand(gasFeeToken = feeToken, amount = quoted.amount, recipient = quoted.recipient)
            placeholder = UserOpFeeMode.InBand(gasFeeToken = feeToken, amount = "1", recipient = account)
        }
        var draft = runCatching {
            userOpDraft(
                sender = account,
                nonce = nonce,
                deployed = deployed,
                keyHexes = keys.map { it.publicKeyHex },
                calls = calls,
                fee = placeholder,
                floors = floors,
            )
        }.getOrElse { other(it.message ?: "The operation could not be assembled.") }
        val hasContractCall = userOpHasContractCall(calls)
        // core-perf-1: the quote's own measurement, when it measured THIS
        // account as it stands now (deployment read fresh above; the nonce
        // too, never the quote's). Its limits already carry the inner calls'
        // floor, and the core's one rule applies them as it applies the
        // relay's. In-band only: Tempo's quote carries none.
        val measured = prepared
            ?.takeIf { !tempo && it.deployed == deployed }
            ?.let { gas ->
                runCatching {
                    userOpApplyEstimate(draft, gas.verification_gas_limit, gas.call_gas_limit, gas.pre_verification_gas, gas.settlement_gas, floors)
                }.onFailure { VelaLog.event("userop.submit", "the quote's gas could not be applied; measuring", "why" to (it.message ?: "")) }
                    .getOrNull()
            }
        if (prepared != null) {
            VelaLog.event("userop.submit", if (measured != null) "signing the quote's measured gas" else "the quote's gas is not for this account now; measuring", "deployed" to deployed, "quoted" to prepared.deployed)
        }
        if (measured != null) {
            draft = measured
        } else when (val estimate = relay.estimateUserOpGas(chainId, userOpRelayJson(draft, feeToken.takeIf { tempo }))) {
            is RelayClient.EstimateAnswer.Estimated -> {
                // The core's one rule for the limits signed: the relay's as
                // returned (an undeployed Safe's floors; the 2M verification
                // floor only for a relay without `settlementGas`), then the
                // inner calls' own measured floor below.
                draft = userOpApplyEstimate(
                    draft,
                    estimate.verificationGasLimit,
                    estimate.callGasLimit,
                    estimate.preVerificationGas,
                    estimate.settlementGas,
                    floors,
                )
                draft = raisedToMeasuredFloor(draft, chainId, account, calls)
            }
            is RelayClient.EstimateAnswer.Refused -> {
                VelaLog.event("userop.submit", "estimate failed, defaults", "contract" to hasContractCall)
                if (hasContractCall) {
                    other("Could not estimate gas for this transaction. The network may be busy — please try again.")
                }
            }
            RelayClient.EstimateAnswer.Unreachable -> {
                VelaLog.event("userop.submit", "estimate unanswered, defaults", "contract" to hasContractCall)
                // r17-12: the relay did not answer at all — nothing was signed
                // or sent, and it is the relay that could not be reached (said
                // so by kind), never "Something went wrong".
                if (hasContractCall) throw Refused(Failure.RelayerUnavailable)
            }
        }
        draft = userOpWithCalls(draft, calls, settled)
        // §35: the FINAL operation carries its validity window — validAfter
        // 0, validUntil now + 30 min, the core's rule over this device's
        // clock — stamped on every attempt just before its hash: the hash,
        // the signature and the page's request all carry it, and what signs
        // refuses an operation that never expires.
        draft = runCatching { userOpStampValidity(draft, wallClockMs().toDouble()) }
            .getOrElse { other(it.message ?: "The operation could not be given its expiry.") }
        val challenge = if (review == null) {
            userOpSafeOpHash(draft, chainId.toUInt())
        } else {
            try {
                userOpReviewedSafeOpHash(draft, chainId.toUInt(), review)
            } catch (mismatch: CoreException) {
                VelaLog.event("sign", "not_the_review ${mismatch.message}")
                other("The operation is not the one you reviewed. Nothing was signed.")
            }
        }
        signingStarted()
        val assembled = draft
        // R7: by the site's own method (`eth_sendTransaction`, `wallet_sendCalls`),
        // or `""` — the wallet's own send.
        val assertion = ceremony(challenge, chainId, account, pinned, keys, intent?.method.orEmpty()) { labels, allowed, keyRoute ->
            trustedSignerRequest(
                TrustedSignerInput(
                    method = intent?.method.orEmpty(), paramsJson = intent?.paramsJson ?: "[]", origin = intent?.origin.orEmpty(),
                    originSeenByBrowser = intent?.seenByBrowser ?: false,
                    chainId = chainId.toUInt(), chainName = labels.chainName, nativeSymbol = labels.nativeSymbol,
                    account = account, accountName = labels.accountName,
                    credentialIdsHex = allowed.map { it.credentialId }, keyRouteJson = keyRoute, calls = calls,
                ),
                assembled,
            )
        }
        ceremonyDone()
        val signed = runCatching {
            userOpSign(
                draft,
                WebAuthnAssertion(
                    authenticatorData = SendExecutor.unhex(assertion.authenticatorDataHex),
                    clientDataJson = SendExecutor.unhex(assertion.clientDataJsonHex),
                    signatureDer = SendExecutor.unhex(assertion.signatureDerHex),
                ),
                assertion.credentialIdHex,
                keys,
            )
        }.getOrElse { other(it.message ?: "Failed to create signature") }
        // The hash a lost reply is followed under (RA6), known before any POST.
        val localHash = runCatching { userOpHash(signed, chainId.toUInt()) }
            .getOrElse { other(it.message ?: "The operation could not be hashed") }
        val submitBlock = headBeforePost()
        // The write-ahead (RJ1): the record is on disk before any byte leaves.
        beforePost(localHash, submitBlock)
        // The speed the displayed fee was priced at, named beside it (spec 069).
        // The verdict is the core's (`userOpSubmitStep`): only "not sent" is a
        // failure; a lost reply is followed under the local hash, never retried
        // by the person (owner ruling 1). The nonce is read from the chain on
        // every attempt, so nothing here advances it.
        return when (val answer = relay.sendUserOp(chainId, userOpRelayJson(signed, feeToken.takeIf { tempo }), quoted.tier, localHash)) {
            is RelayClient.SubmitAnswer.Accepted -> Submitted(answer.userOpHash, maybeSent = false, submitBlock = submitBlock, nonce = nonce)
            is RelayClient.SubmitAnswer.MaybeSent -> Submitted(answer.userOpHash, maybeSent = true, submitBlock = submitBlock, nonce = nonce)
            is RelayClient.SubmitAnswer.NotSent -> throw Refused(
                when (val rejection = answer.rejection) {
                    null -> Failure.NotSent
                    RelayRejection.RelayerUnavailable -> Failure.RelayerUnavailable
                    RelayRejection.BundlerUnderfunded -> Failure.BundlerUnderfunded
                    // The relay refused it (RJ3): never "try again".
                    is RelayRejection.Other -> Failure.Rejected(rejection.message.ifBlank { null })
                    // 083: another operation of the account holds the nonce —
                    // its hash is never this request's; nothing of this one
                    // went out.
                    is RelayRejection.NonceHeld -> Failure.PreviousPending
                    // send-15: the core's reading of a bare AA25 — another
                    // transaction of the account spent the nonce first.
                    RelayRejection.NonceUsed -> Failure.NonceUsed
                    // §35: the window closed before the EntryPoint saw it.
                    RelayRejection.Expired -> Failure.Expired
                },
            )
        }
    }
}

/**
 * send-11: how a passkey prompt that signed nothing ended — the core's rule
 * (`passkey_ceremony_end`) over the platform's classification ([failure]'s
 * kind) and how long the prompt was up ([elapsedMs]). Credential Manager says
 * a prompt that ran out of time exactly as one the person closed
 * (`GetCredentialCancellationException`); only the clock tells them apart.
 * Closed and out of time are the same refusal to every rule — the confirm
 * stays for another try — and only the words differ; every other end is the
 * passkey's failure, by its kind.
 */
internal fun ceremonyFailure(failure: PasskeyFailure, elapsedMs: Long): UserOpSpine.Failure =
    when (passkeyCeremonyEnd(failure.kind.wire, elapsedMs.toDouble())) {
        PasskeyCeremonyEnd.TIMED_OUT -> UserOpSpine.Failure.PasskeyTimedOut
        PasskeyCeremonyEnd.CANCELLED -> UserOpSpine.Failure.PasskeyCancelled
        PasskeyCeremonyEnd.NOT_SUPPORTED, PasskeyCeremonyEnd.NOT_DISCOVERABLE, PasskeyCeremonyEnd.OTHER ->
            // Spec 099 R8: how it failed travels on, as the app's passkey
            // classifier read it, so the sheet and the page's record name
            // the signer rather than "couldn't submit".
            if (failure.kind == FailureKind.Cancelled) {
                UserOpSpine.Failure.PasskeyCancelled
            } else {
                UserOpSpine.Failure.Signer(failure.kind, failure.message ?: "the passkey ceremony failed")
            }
    }
