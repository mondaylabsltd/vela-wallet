package app.getvela.wallet.feature.onboarding.core

import android.app.Activity
import android.content.Intent
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import com.google.android.gms.fido.Fido
import com.google.android.gms.fido.fido2.api.common.AttestationConveyancePreference
import com.google.android.gms.fido.fido2.api.common.Attachment
import com.google.android.gms.fido.fido2.api.common.AuthenticatorAssertionResponse
import com.google.android.gms.fido.fido2.api.common.AuthenticatorAttestationResponse
import com.google.android.gms.fido.fido2.api.common.AuthenticatorErrorResponse
import com.google.android.gms.fido.fido2.api.common.AuthenticatorSelectionCriteria
import com.google.android.gms.fido.fido2.api.common.EC2Algorithm
import com.google.android.gms.fido.fido2.api.common.PublicKeyCredential
import com.google.android.gms.fido.fido2.api.common.PublicKeyCredentialCreationOptions
import com.google.android.gms.fido.fido2.api.common.PublicKeyCredentialDescriptor
import com.google.android.gms.fido.fido2.api.common.PublicKeyCredentialParameters
import com.google.android.gms.fido.fido2.api.common.PublicKeyCredentialRequestOptions
import com.google.android.gms.fido.fido2.api.common.PublicKeyCredentialRpEntity
import com.google.android.gms.fido.fido2.api.common.PublicKeyCredentialType
import com.google.android.gms.fido.fido2.api.common.PublicKeyCredentialUserEntity
import com.google.android.gms.fido.fido2.api.common.ResidentKeyRequirement
import com.google.android.gms.fido.fido2.api.common.UserVerificationRequirement
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.tasks.await

/**
 * The security-key ceremony, run through GMS's FIDO2 API rather than through
 * Credential Manager.
 *
 * **Why a second path exists at all.** Credential Manager is a PROVIDER picker,
 * and a USB security key is not a provider. Whether the key is reachable from
 * its sheet is therefore the OEM's decision, and on a Galaxy S22 the answer is
 * no:
 *
 *     CredentialSelector: CreateOptions size=2, Default provider=Google Password Manager
 *
 * Two options, both password managers. Somebody who chose "USB security key" in
 * OUR picker is handed a sheet that cannot do it, and every route out of that
 * sheet reports itself as a cancellation — which is what "setup was cancelled"
 * had been meaning all afternoon (device-found 2026-08-26). Touching the key to
 * hurry it along makes it worse: a YubiKey types its OTP as KEYSTROKES, and the
 * Enter at the end dismisses the sheet.
 *
 * This API is the one behind the system's own "insert or tap your security key"
 * screen (`Fido2FullScreenActivity`), which both test phones reach and complete
 * successfully once something actually launches it. It is deprecated in favour
 * of Credential Manager — for platform passkeys, where Credential Manager is
 * the right tool. For hardware keys it remains the only tool that works.
 *
 * The platform path is untouched: everything that is not explicitly a security
 * key still goes through Credential Manager.
 */
class SecurityKeyCeremony(private val activity: ComponentActivity) {

    /** One launcher for the whole activity, registered before it starts. */
    private var pending: ((Intent?) -> Unit)? = null

    private val launcher: ActivityResultLauncher<IntentSenderRequest> =
        activity.registerForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) {
            val answer = pending
            pending = null
            answer?.invoke(if (it.resultCode == Activity.RESULT_OK) it.data else null)
        }

    /** Mint a discoverable ES256 credential on the key the person presents. */
    suspend fun register(
        rpId: String,
        rpName: String,
        challenge: ByteArray,
        userId: ByteArray,
        userName: String,
        excludeCredentialIds: List<ByteArray>,
    ): PublicKeyCredential {
        val options = PublicKeyCredentialCreationOptions.Builder()
            .setRp(PublicKeyCredentialRpEntity(rpId, rpName, null))
            .setUser(PublicKeyCredentialUserEntity(userId, userName, null, userName))
            .setChallenge(challenge)
            // ES256 only, deliberately without an RS256 fallback: the on-chain
            // verifier is the RIP-7212 P-256 precompile, so an RSA credential
            // could never become a working wallet — it would pass creation and
            // die at key extraction, after minting an orphan on the key.
            .setParameters(
                listOf(
                    PublicKeyCredentialParameters(
                        PublicKeyCredentialType.PUBLIC_KEY.toString(),
                        EC2Algorithm.ES256.algoValue,
                    ),
                ),
            )
            .setAuthenticatorSelection(
                AuthenticatorSelectionCriteria.Builder()
                    // The key is the point of this path.
                    .setAttachment(Attachment.CROSS_PLATFORM)
                    // Discoverable, or the credential never appears in a picker
                    // and the wallet dies with this key (issue #1).
                    .setResidentKeyRequirement(ResidentKeyRequirement.RESIDENT_KEY_REQUIRED)
                    .build(),
            )
            .setAttestationConveyancePreference(AttestationConveyancePreference.DIRECT)
            .setExcludeList(descriptors(excludeCredentialIds))
            .build()

        return awaitCeremony {
            Fido.getFido2ApiClient(activity).getRegisterPendingIntent(options).await()
        }
    }

    /**
     * Sign a challenge with one known credential — or, with no [credentialId],
     * ask GMS "who are you?" and let ITS picker list what it holds.
     *
     * The unpinned form is the way round a dead system selector (see
     * `PasskeyExecutor.awaitingSelector`): GMS loads its passkeys inside its
     * own process, so nothing has to fit through an intent.
     */
    suspend fun assert(
        rpId: String,
        challenge: ByteArray,
        credentialId: ByteArray?,
    ): PublicKeyCredential {
        val options = PublicKeyCredentialRequestOptions.Builder()
            .setRpId(rpId)
            .setChallenge(challenge)
            .apply { if (credentialId != null) setAllowList(descriptors(listOf(credentialId))) }
            .build()

        return awaitCeremony {
            Fido.getFido2ApiClient(activity).getSignPendingIntent(options).await()
        }
    }

    /**
     * Transports are deliberately absent from the descriptors: this path IS the
     * security-key path, so there is nothing left for a transport hint to
     * route, and GMS drops a descriptor whose transports it cannot place.
     */
    private fun descriptors(ids: List<ByteArray>): List<PublicKeyCredentialDescriptor> =
        ids.map {
            PublicKeyCredentialDescriptor(PublicKeyCredentialType.PUBLIC_KEY.toString(), it, null)
        }

    private suspend fun awaitCeremony(
        start: suspend () -> android.app.PendingIntent,
    ): PublicKeyCredential {
        // `getRegisterPendingIntent`/`getSignPendingIntent` throw before any UI
        // is shown when the GMS FIDO2 module is not provisioned on this device
        // — "FIDO2_API is not available … SERVICE_INVALID", seen on a degraded
        // GMS build (OnePlus 5T, Android 10, 2026-08-26). That is not a
        // ceremony failure; it means this ROUTE does not exist here, and the
        // caller must use the app-owned USB path instead. It is signalled with
        // a distinct type so the executor can tell it apart from a cancellation.
        val intent = try {
            start()
        } catch (error: Exception) {
            throw Fido2Unavailable(error.message ?: "The FIDO2 service is unavailable")
        }
        val answer: Intent? = suspendCancellableCoroutine { continuation ->
            pending = { data -> continuation.resume(data) }
            continuation.invokeOnCancellation { pending = null }
            launcher.launch(IntentSenderRequest.Builder(intent.intentSender).build())
        }
        val data = answer
            ?: throw PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")

        val bytes = data.getByteArrayExtra(Fido.FIDO2_KEY_CREDENTIAL_EXTRA)
            ?: throw PasskeyFailure(FailureKind.Other, "No credential returned")
        val credential = PublicKeyCredential.deserializeFromBytes(bytes)

        when (val response = credential.response) {
            is AuthenticatorErrorResponse -> throw failureFor(response)
            is AuthenticatorAttestationResponse, is AuthenticatorAssertionResponse -> Unit
            else -> throw PasskeyFailure(FailureKind.Other, "Unexpected authenticator response")
        }
        return credential
    }

    /**
     * The authenticator's own words. `NOT_ALLOWED` is what a key reports for a
     * timeout or a refused touch, which is a cancellation to the person;
     * `INVALID_STATE` with an exclude list means this key already holds one of
     * this wallet's founding keys.
     */
    private fun failureFor(response: AuthenticatorErrorResponse): PasskeyFailure {
        val message = response.errorMessage?.takeIf { it.isNotBlank() }
            ?: response.errorCode.name
        return when (response.errorCode.name) {
            "NOT_ALLOWED_ERR", "TIMEOUT_ERR", "ABORT_ERR" ->
                PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")
            "INVALID_STATE_ERR" -> PasskeyFailure(
                FailureKind.Other,
                "This authenticator already holds one of this wallet's keys",
            )
            "NOT_SUPPORTED_ERR" -> PasskeyFailure(FailureKind.NotSupported, message)
            else -> PasskeyFailure(FailureKind.Other, message)
        }
    }
}

/**
 * The GMS FIDO2 route does not exist on this device — its module is missing or
 * degraded ("FIDO2_API is not available", `SERVICE_INVALID`). Raised BEFORE any
 * UI is shown, so the executor can silently fall through to the app-owned USB
 * path, which is the only route on a phone without a working GMS FIDO2 service.
 */
class Fido2Unavailable(message: String) : Exception(message)
