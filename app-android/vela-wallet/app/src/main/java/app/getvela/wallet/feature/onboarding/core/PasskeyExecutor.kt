package app.getvela.wallet.feature.onboarding.core

import android.app.Activity
import android.content.Context
import android.os.SystemClock
import androidx.credentials.CreatePublicKeyCredentialRequest
import androidx.credentials.CredentialManager
import androidx.credentials.GetCredentialRequest
import androidx.credentials.GetCredentialResponse
import androidx.credentials.GetPublicKeyCredentialOption
import com.google.android.gms.fido.fido2.api.common.AuthenticatorAssertionResponse
import com.google.android.gms.fido.fido2.api.common.AuthenticatorAttestationResponse
import app.getvela.wallet.core.diagnostics.VelaLog
import androidx.credentials.PublicKeyCredential
import androidx.credentials.exceptions.CreateCredentialCancellationException
import androidx.credentials.exceptions.CreateCredentialException
import androidx.credentials.exceptions.CreateCredentialNoCreateOptionException
import androidx.credentials.exceptions.CreateCredentialUnsupportedException
import androidx.credentials.exceptions.GetCredentialCancellationException
import androidx.credentials.exceptions.GetCredentialException
import androidx.credentials.exceptions.GetCredentialProviderConfigurationException
import androidx.credentials.exceptions.GetCredentialUnsupportedException
import androidx.credentials.exceptions.NoCredentialException
import androidx.credentials.exceptions.publickeycredential.CreatePublicKeyCredentialDomException
import androidx.credentials.exceptions.publickeycredential.GetPublicKeyCredentialDomException
import java.security.SecureRandom
import java.util.UUID
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.fromBase64url
import uniffi.vela_core_uniffi.fromHex
import uniffi.vela_core_uniffi.toBase64url
import uniffi.vela_core_uniffi.toHex

/**
 * Every WebAuthn ceremony this app performs, and nothing else.
 *
 * Android reaches WebAuthn through Credential Manager, whose request and
 * response are **the same WebAuthn JSON the web path builds** — so the document
 * below is not an Android translation of the browser's options, it is the same
 * document. That is the point: a difference here produces a credential the other
 * three clients cannot use, and a wallet IS its key set.
 *
 * Nothing here decides what a failure means. It classifies the platform's
 * exception into the vocabulary the core branches on ([FailureKind]) and stops.
 */
class PasskeyExecutor(
    private val context: Context,
    /**
     * The FIDO2 path, for the ceremonies Credential Manager cannot run.
     *
     * A USB security key is not a provider, so whether its sheet can reach one
     * is the OEM's decision — and on a Galaxy S22 it cannot: the create sheet
     * lists two password managers and nothing else. `null` (previews, the
     * gallery) means Credential Manager for everything, which is what those
     * surfaces want. See [SecurityKeyCeremony].
     */
    private val securityKey: SecurityKeyCeremony? = null,
    /**
     * The app-owned CTAP2-over-USB path (`vela_core::ctap`, the same protocol
     * the desktop runs), for security keys reached with NO Google service.
     *
     * Preferred over [securityKey] when a USB FIDO key is plugged in, because
     * it is the only route that works on a phone without (full) GMS —
     * GrapheneOS, CalyxOS, much of the China market — where Credential Manager
     * has no passkey provider at all. Where GMS exists, the GMS FIDO2 path
     * stays as the fallback. `null` on surfaces with no activity to run a
     * ceremony through (previews, the gallery).
     */
    private val usbSecurityKey: UsbSecurityKeyCeremony? = null,
    /**
     * The caBLE "sign in with your phone" path (spec 019), for [KeyMethod.Hybrid]
     * — the scan method. Shows a QR the OTHER phone scans, then runs the ceremony
     * over the BLE/tunnel channel that phone opens. `null` on surfaces with no
     * activity (previews, the gallery).
     */
    private val hybrid: HybridCeremony? = null,
    /**
     * Show (or clear, with `null`) the caBLE QR. The ViewModel renders it; the
     * ceremony calls this the moment the payload is ready and again to clear it,
     * exactly as the desktop shows its QR card.
     */
    private val showQr: (String?) -> Unit = {},
    /**
     * What to tell the person when the system's passkey sheet never appears
     * (see [awaitingSelector]). The shell owns the words; this class has no
     * catalog, so the sentence is handed in.
     */
    private val selectorUnresponsiveMessage: () -> String = { SELECTOR_UNRESPONSIVE_FALLBACK },
    /**
     * The relying party. A passkey is bound to it: change it and every existing
     * wallet becomes unreachable from this app.
     */
    val relyingPartyId: String = RELYING_PARTY,
) {
    private val manager: CredentialManager? =
        runCatching { CredentialManager.create(context) }.getOrNull()

    /** When this app last minted a credential, and which one (see `assert`). */
    private var mintedAt: Long = 0
    private var mintedCredentialIdHex: String? = null

    /**
     * Whether a passkey ceremony can be attempted at all.
     *
     * The question is whether the SERVICE is reachable, not whether a credential
     * exists or a provider is currently enrolled. Answering `false` for "no
     * passkeys saved yet" would make the core raise "this device cannot create a
     * wallet" on a device that can — and leave no way back, because a person
     * cannot enrol a credential from inside a flow that refuses to start.
     */
    fun supported(): Boolean = manager != null

    /** `navigator.credentials.create()`, in Credential Manager's clothes. */
    suspend fun register(
        name: String,
        excludeCredentialIds: List<String>,
        method: KeyMethod,
    ): Registration {
        if (method == KeyMethod.Hybrid && hybrid != null) {
            // The scan method mints the key on the OTHER phone over caBLE.
            return runHybrid(forGet = false) { session ->
                hybrid.register(session, name, excludeCredentialIds)
            }
        }
        if (method == KeyMethod.SecurityKey) {
            // The app-owned USB path IS the security-key route, on every device.
            // It talks CTAP2 straight to the key — using the KEY's own PIN or
            // fingerprint, never the phone's biometrics (an external key has
            // nothing to do with the phone's screen lock), with no GMS, no
            // domain association and no OEM sheet. It prompts to plug a key in
            // when none is present. The GMS FIDO2 path was the source of every
            // failure on a GMS-degraded phone — "FIDO2_API is not available",
            // "no provider dependencies", "this device does not support
            // biometrics" — so it is no longer used for this method where the
            // app-owned path exists (device-found on a OnePlus 5T, 2026-08-26).
            if (usbSecurityKey != null) {
                VelaLog.event("passkey.register", "asking (app-owned usb ctap)", "excluded" to excludeCredentialIds.size)
                return usbSecurityKey.register(name, excludeCredentialIds)
            }
            if (securityKey != null) {
                return registerOnSecurityKey(name, excludeCredentialIds)
            }
        }

        val credentialManager = manager ?: throw PasskeyFailure(
            FailureKind.NotSupported,
            "Credential Manager is unavailable on this device",
        )

        val request = JSONObject().apply {
            put("rp", JSONObject().put("id", relyingPartyId).put("name", RELYING_PARTY_NAME))
            put(
                "user",
                JSONObject()
                    .put("id", toBase64url(encodeUserHandle(name).toByteArray(Charsets.UTF_8)))
                    .put("name", name)
                    .put("displayName", name),
            )
            put("challenge", toBase64url(random(CHALLENGE_BYTES)))
            // ES256 (P-256) ONLY, deliberately without an RS256 fallback. The
            // on-chain verifier is the RIP-7212 P-256 precompile and
            // two-signature recovery is ECDSA math, so an RSA credential can
            // never become a working wallet: it would pass creation and then die
            // during key extraction — after minting an orphan passkey in the
            // person's provider. Restricting the list makes an RSA-only
            // authenticator fail up front instead.
            put(
                "pubKeyCredParams",
                JSONArray().put(JSONObject().put("type", PUBLIC_KEY).put("alg", ES256)),
            )
            if (excludeCredentialIds.isNotEmpty()) {
                // A multi-key wallet registers each founding key separately, and
                // the provider must refuse to silently REPLACE an earlier one —
                // the Safe address depends on every key in the set.
                put(
                    "excludeCredentials",
                    JSONArray().apply {
                        excludeCredentialIds.forEach { id ->
                            put(JSONObject().put("type", PUBLIC_KEY).put("id", base64urlOfHex(id)))
                        }
                    },
                )
            }
            put(
                "authenticatorSelection",
                JSONObject()
                    .put("residentKey", "required")
                    // WebAuthn L2 5.4.4: set iff residentKey is 'required'. A
                    // client that honours only the L1 boolean would otherwise
                    // silently mint a NON-discoverable credential (issue #1).
                    .put("requireResidentKey", true)
                    .put("userVerification", "required")
                    .apply {
                        // Unlike the browser, this client OWNS the picker, so the
                        // person's choice has to be honoured here or it means
                        // nothing. `Hybrid` never arrives: the key screen offers
                        // it as present-and-unavailable rather than issuing a
                        // ceremony no transport can run.
                        attachmentFor(method)?.let { put("authenticatorAttachment", it) }
                    },
            )
            put("attestation", "direct")
            put("extensions", JSONObject().put("credProps", true))
        }

        VelaLog.event(
            "passkey.register",
            "asking",
            "method" to method,
            "attachment" to (attachmentFor(method) ?: "any"),
            "excluded" to excludeCredentialIds.size,
        )
        val response = try {
            credentialManager.createCredential(
                context = context,
                request = CreatePublicKeyCredentialRequest(request.toString()),
            )
        } catch (error: CreateCredentialException) {
            val failure = classifyCreate(error)
            VelaLog.failure(
                "passkey.register",
                "refused",
                error,
                "type" to error.type,
                "kind" to failure.kind,
                "domError" to (error as? CreatePublicKeyCredentialDomException)?.domError?.type,
            )
            throw failure
        }

        val json = JSONObject(
            response.data.getString(BUNDLE_REGISTRATION_RESPONSE)
                ?: throw PasskeyFailure(FailureKind.Other, "No credential returned"),
        )

        // Sign-in and cross-device recovery both need a discoverable credential:
        // a non-discoverable one signs fine when pinned by id but never appears
        // in the picker and never syncs, so the wallet would die with this
        // device. Fail HERE, before anything is saved or funded. An absent `rk`
        // means the provider cannot say — give it the benefit of the doubt.
        val credProps = json.optJSONObject("clientExtensionResults")?.optJSONObject("credProps")
        if (credProps != null && credProps.has("rk") && !credProps.optBoolean("rk", true)) {
            throw PasskeyFailure(
                FailureKind.NotDiscoverable,
                "Authenticator created a non-discoverable credential",
            )
        }

        val inner = json.getJSONObject("response")
        val credentialIdHex = hexOfBase64url(json.getString("rawId"))
        // When this credential was minted, and which one it was. The very next
        // thing the create flow does is ASK FOR IT BACK (the membership proof),
        // and on Android the provider is not always ready to answer yet — see
        // `assert`.
        mintedAt = SystemClock.elapsedRealtime()
        mintedCredentialIdHex = credentialIdHex
        VelaLog.event(
            "passkey.register",
            "minted",
            "cred" to VelaLog.shortId(credentialIdHex),
            "attachment" to json.optString("authenticatorAttachment"),
            "transports" to inner.optJSONArray("transports").strings().joinToString(","),
            "rk" to (credProps?.opt("rk") ?: "unreported"),
        )
        return Registration(
            credentialIdHex = credentialIdHex,
            attestationObjectHex = hexOfBase64url(inner.getString("attestationObject")),
            clientDataJsonHex = hexOfBase64url(inner.getString("clientDataJSON")),
            authenticatorAttachment = json.optString("authenticatorAttachment"),
            transports = inner.optJSONArray("transports").strings().joinToString(","),
        )
    }

    /**
     * Registration straight onto the key the person is holding.
     *
     * Credential Manager is skipped ENTIRELY here — not narrowed, skipped. Its
     * sheet is a provider picker, and on a Galaxy S22 the create sheet lists two
     * password managers and no way to reach a security key at all
     * ("CreateOptions size=2"), so somebody who chose "USB security key" in our
     * own picker was handed a dialog that could not do it and every way out of
     * it reported a cancellation (device-found 2026-08-26).
     */
    private suspend fun registerOnSecurityKey(
        name: String,
        excludeCredentialIds: List<String>,
    ): Registration {
        val ceremony = securityKey ?: throw PasskeyFailure(
            FailureKind.NotSupported,
            "No security-key ceremony on this surface",
        )
        VelaLog.event(
            "passkey.register",
            "asking (fido2 security key)",
            "excluded" to excludeCredentialIds.size,
        )
        val credential = try {
            ceremony.register(
                rpId = relyingPartyId,
                rpName = RELYING_PARTY_NAME,
                challenge = random(CHALLENGE_BYTES),
                userId = encodeUserHandle(name).toByteArray(Charsets.UTF_8),
                userName = name,
                excludeCredentialIds = excludeCredentialIds.map {
                    uniffi.vela_core_uniffi.fromHex(it)
                },
            )
        } catch (unavailable: Fido2Unavailable) {
            // The GMS FIDO2 module is not usable here and this surface has no
            // app-owned USB path to fall back to (the real activity always
            // does, so this is a defensive translation, not a live route).
            throw PasskeyFailure(FailureKind.NotSupported, "Plug in a USB security key and try again.")
        }
        val response = credential.response as AuthenticatorAttestationResponse
        val credentialIdHex = toHex(credential.rawId ?: ByteArray(0), false)
        mintedAt = SystemClock.elapsedRealtime()
        mintedCredentialIdHex = credentialIdHex
        VelaLog.event(
            "passkey.register",
            "minted (fido2)",
            "cred" to VelaLog.shortId(credentialIdHex),
        )
        return Registration(
            credentialIdHex = credentialIdHex,
            attestationObjectHex = toHex(response.attestationObject, false),
            clientDataJsonHex = toHex(response.clientDataJSON, false),
            // What it is, by construction: this path only ever talks to a
            // removable key.
            authenticatorAttachment = "cross-platform",
            transports = "usb,nfc",
        )
    }

    /**
     * An assertion. [credentialIdHex] pins it to one credential; `null` is the
     * "who are you?" ceremony sign-in starts with.
     *
     * ## The freshly-minted-credential race (Android only)
     *
     * The create flow signs a membership proof with the passkey it minted
     * seconds earlier. Credential Manager answers a get by asking every
     * installed provider what it has for this rpId — and a provider answers
     * from its own INDEX, which it updates asynchronously after a create. On a
     * phone with several providers (this one has Google Password Manager,
     * Samsung Pass, a YubiKey provider and our own security-keys app) the
     * first get after a create can arrive before the one that stored the
     * credential has indexed it. Nobody returns an entry, and the system draws
     * its "no passkeys available" sheet over a wallet whose passkey was made
     * moments ago. Cancelling and pressing the row's confirm — which sends the
     * IDENTICAL request — then works, because the seconds spent tapping were
     * what the provider needed (founder-found on device 2026-08-25; iOS does
     * not do this).
     *
     * Two things stop that sheet from ever being drawn:
     *
     * * A pinned get for a credential we JUST minted waits until the mint is
     *   at least [SETTLE_AFTER_CREATE_MS] old. The app is showing "creating
     *   your wallet" at that moment, so the wait costs nothing visible.
     * * Pinned attempts ask with `preferImmediatelyAvailableCredentials`, which
     *   makes Credential Manager throw instead of drawing the fallback UI when
     *   no provider has an entry. That turns "an unexplained sheet" into
     *   "retry in a moment", which is what the situation actually is. Only the
     *   LAST attempt drops the flag, so a genuinely-missing credential still
     *   reaches the platform's own UI, where "use another device" lives.
     *
     * An UNPINNED assertion does none of this: there, "no credential" is the
     * honest answer to "who are you?" on a device with no wallet, and the
     * platform's UI is exactly what should be shown.
     */
    suspend fun assert(
        challenge: ByteArray,
        credentialIdHex: String?,
        /**
         * WHERE the credential lives, as its authenticator reported at
         * registration (`hybrid,internal`, `usb,nfc`, …), or empty when unknown.
         *
         * **This is load-bearing, not a hint.** An `allowCredentials` entry with
         * no transports leaves Credential Manager to guess where to look, and it
         * guesses REMOVABLE SECURITY KEY: a passkey living in Apple Passwords on
         * another phone drew "Connect your security key", which is a dead end
         * the person cannot answer — they have no key to plug in, and the route
         * that would work (scan the QR with the phone that holds it) is never
         * offered (device-found 2026-08-26). With `hybrid` present the platform
         * offers that route; with `usb` it offers the sheet a security key
         * owner actually wants.
         */
        transports: String = "",
        /**
         * The method the person chose on the sign-in screen, for the "who are
         * you?" ceremony that carries no transports. `SecurityKey` forces the
         * app-owned path so a wallet living on a hardware key is reachable even
         * when a platform passkey is also present and the system would silently
         * use it.
         */
        method: KeyMethod = KeyMethod.Platform,
    ): Assertion {
        // The scan method reaches the credential on the OTHER phone over caBLE —
        // a sign-in (no credential id) offers whatever it holds, a proof
        // (recovery's second signature) pins the same credential the first used.
        if (method == KeyMethod.Hybrid && hybrid != null) {
            return runHybrid(forGet = true) { session ->
                hybrid.assert(session, challenge, credentialIdHex)
            }
        }
        // A credential that lives on a removable key — or a sign-in explicitly
        // asked to run on a security key — takes the app-owned USB path, on
        // every device: the key's own PIN/fingerprint, no GMS, no domain
        // association, no OEM sheet. It prompts to plug the key in when it is
        // not present.
        if ((removable(transports) || method == KeyMethod.SecurityKey) && usbSecurityKey != null) {
            VelaLog.event("passkey.assert", "asking (app-owned usb ctap)", "cred" to VelaLog.shortId(credentialIdHex))
            return usbSecurityKey.assert(challenge, credentialIdHex)
        }

        if (credentialIdHex != null && removable(transports) && securityKey != null) {
            return assertThroughGms(challenge, credentialIdHex, attachment = "cross-platform")
        }

        val credentialManager = manager ?: throw PasskeyFailure(
            FailureKind.NotSupported,
            "Credential Manager is unavailable on this device",
        )

        val request = JSONObject().apply {
            put("challenge", toBase64url(challenge))
            put("rpId", relyingPartyId)
            put("userVerification", "required")
            if (credentialIdHex != null) {
                val descriptor = JSONObject()
                    .put("type", PUBLIC_KEY)
                    .put("id", base64urlOfHex(credentialIdHex))
                val hints = transports.split(',')
                    .map { it.trim() }
                    .filter { it.isNotEmpty() }
                if (hints.isNotEmpty()) {
                    descriptor.put("transports", JSONArray(hints))
                }
                put("allowCredentials", JSONArray().put(descriptor))
            }
        }

        val options = listOf(GetPublicKeyCredentialOption(request.toString()))
        VelaLog.event(
            "passkey.assert",
            if (credentialIdHex == null) "asking (any credential)" else "asking (pinned)",
            "cred" to VelaLog.shortId(credentialIdHex),
            "transports" to transports.ifEmpty { "unknown" },
            "removable" to removable(transports),
        )
        val response = if (credentialIdHex == null) {
            try {
                awaitingSelector {
                    credentialManager.getCredential(
                        context = context,
                        request = GetCredentialRequest(options),
                    )
                }
            } catch (dead: SelectorUnresponsive) {
                // The system's sheet cannot draw, but the passkeys are still
                // there. GMS can list its own without that sheet, so ask it
                // directly before telling the person anything went wrong.
                if (securityKey == null) throw dead
                VelaLog.event("passkey.assert", "system selector dead → gms fido2")
                return try {
                    assertThroughGms(challenge, null, attachment = "platform")
                } catch (unavailable: PasskeyFailure) {
                    if (unavailable.kind == FailureKind.NotSupported) throw dead else throw unavailable
                }
            } catch (error: GetCredentialException) {
                // The system route does not exist here — no passkey provider at
                // all on a GMS-free phone ("no provider dependencies found"), or
                // the relying party's domain association could not be verified
                // (the developer's assetlinks server is down). Neither is a
                // reason a person cannot reach THEIR OWN key: the app-owned USB
                // path consults no provider and no domain association. It is the
                // escape hatch, so offer it — it prompts to plug a key in, and
                // runs the same "who are you?" ceremony on it.
                // (device-found on a OnePlus 5T, 2026-08-26.)
                if (usbSecurityKey != null && isSystemRouteUnavailable(error)) {
                    VelaLog.event("passkey.assert", "system route unavailable → app-owned usb", "type" to error.type)
                    return usbSecurityKey.assert(challenge, null)
                }
                val failure = classifyGet(error)
                VelaLog.failure(
                    "passkey.assert",
                    "refused",
                    error,
                    "type" to error.type,
                    "kind" to failure.kind,
                )
                throw failure
            }
        } else {
            settleAfterMint(credentialIdHex)
            getPinned(credentialManager, options, removable(transports))
        }

        val credential = response.credential as? PublicKeyCredential
            ?: throw PasskeyFailure(FailureKind.Other, "No credential returned")
        val json = JSONObject(credential.authenticationResponseJson)
        val inner = json.getJSONObject("response")

        VelaLog.event(
            "passkey.assert",
            "signed",
            "cred" to VelaLog.shortId(hexOfBase64url(json.getString("rawId"))),
            "attachment" to json.optString("authenticatorAttachment"),
            "userHandle" to (inner.nullableString("userHandle") != null),
        )
        return Assertion(
            credentialIdHex = hexOfBase64url(json.getString("rawId")),
            // `signatureDerHex`, not `signatureHex`: the provider hands back a
            // DER signature and the core normalises it (including low-S) itself.
            // Naming it otherwise would invite a shell to "helpfully" convert.
            signatureDerHex = hexOfBase64url(inner.getString("signature")),
            authenticatorDataHex = hexOfBase64url(inner.getString("authenticatorData")),
            clientDataJsonHex = hexOfBase64url(inner.getString("clientDataJSON")),
            // Absent, not empty: no user handle is a different fact from an empty
            // one, and the core's name resolution branches on it.
            userIdHex = inner.nullableString("userHandle")?.let { hexOfBase64url(it) },
            authenticatorAttachment = json.optString("authenticatorAttachment"),
        )
    }

    /**
     * Signing with the key the person is holding, straight through FIDO2.
     *
     * The same reason as registration: a credential that lives on a removable
     * key is held by no PROVIDER, so Credential Manager's sheet has nothing to
     * offer for it — it lists the password managers that do not have it, and
     * whatever the person does there comes back as a cancellation.
     */
    private suspend fun assertThroughGms(
        challenge: ByteArray,
        credentialIdHex: String?,
        /** What this route reaches: a removable key when pinned to one, GMS's own vault otherwise. */
        attachment: String,
    ): Assertion {
        val ceremony = securityKey ?: throw PasskeyFailure(
            FailureKind.NotSupported,
            "No security-key ceremony on this surface",
        )
        VelaLog.event(
            "passkey.assert",
            if (credentialIdHex == null) "asking (gms fido2, any credential)" else "asking (fido2 security key)",
            "cred" to VelaLog.shortId(credentialIdHex),
        )
        val credential = try {
            ceremony.assert(
                rpId = relyingPartyId,
                challenge = challenge,
                credentialId = credentialIdHex?.let { uniffi.vela_core_uniffi.fromHex(it) },
            )
        } catch (unavailable: Fido2Unavailable) {
            throw PasskeyFailure(FailureKind.NotSupported, "Plug in a USB security key and try again.")
        }
        val response = credential.response as AuthenticatorAssertionResponse
        VelaLog.event(
            "passkey.assert",
            "signed (fido2)",
            "cred" to VelaLog.shortId(toHex(credential.rawId ?: ByteArray(0), false)),
        )
        return Assertion(
            credentialIdHex = toHex(credential.rawId ?: ByteArray(0), false),
            // `signatureDerHex`, not `signatureHex`: the authenticator hands
            // back a DER signature and the core normalises it (including low-S).
            signatureDerHex = toHex(response.signature, false),
            authenticatorDataHex = toHex(response.authenticatorData, false),
            clientDataJsonHex = toHex(response.clientDataJSON, false),
            // Absent, not empty: no user handle is a different fact from an
            // empty one, and the core's name resolution branches on it.
            userIdHex = response.userHandle?.takeIf { it.isNotEmpty() }?.let { toHex(it, false) },
            authenticatorAttachment = attachment,
        )
    }

    /**
     * Do not ask a provider for a credential it may still be writing down.
     * Only ever waits after THIS app minted THAT credential, so a sign-in or a
     * later signature is never delayed.
     */
    private suspend fun settleAfterMint(credentialIdHex: String) {
        if (!credentialIdHex.equals(mintedCredentialIdHex, ignoreCase = true)) return
        val since = SystemClock.elapsedRealtime() - mintedAt
        if (since in 0 until SETTLE_AFTER_CREATE_MS) {
            delay(SETTLE_AFTER_CREATE_MS - since)
        }
    }

    /**
     * The pinned get, retried while providers catch up. Attempts before the
     * last one ask for an immediately-available credential only, so a miss
     * throws instead of drawing the platform's "no passkeys" sheet.
     */
    private suspend fun getPinned(
        credentialManager: CredentialManager,
        options: List<GetPublicKeyCredentialOption>,
        removable: Boolean,
    ): GetCredentialResponse {
        var attempt = 0
        while (true) {
            // A REMOVABLE key gets one attempt, and it draws real UI.
            //
            // This loop exists for a race between providers that store a
            // credential and providers that INDEX it — a phone's own passkey
            // vault needs a moment after a create, and asking for an
            // immediately-available credential turns that moment into a retry
            // instead of an unexplained sheet. None of that describes a
            // security key: it is never "immediately available" because it has
            // to be tapped or plugged, so every early attempt is a guaranteed
            // miss, and the backoff between them is dead time in front of
            // somebody holding a key against their phone (2026-08-26).
            val last = removable || attempt == RETRY_BACKOFF_MS.size
            try {
                return awaitingSelector {
                    credentialManager.getCredential(
                        context = context,
                        request = GetCredentialRequest(
                            credentialOptions = options,
                            preferImmediatelyAvailableCredentials = !last,
                        ),
                    )
                }
            } catch (error: NoCredentialException) {
                // A REMOVABLE key is never "immediately available" — it has to
                // be tapped or plugged — so these misses are expected for one
                // and say nothing about whether the credential exists. The
                // attempt number and the flag are logged because that
                // distinction is the whole reason this loop is here.
                VelaLog.failure(
                    "passkey.assert",
                    if (last) "no credential (final attempt)" else "no credential yet",
                    error,
                    "attempt" to attempt,
                    "preferImmediate" to !last,
                )
                if (last) throw classifyGet(error)
                delay(RETRY_BACKOFF_MS[attempt])
                attempt += 1
            } catch (error: GetCredentialException) {
                val failure = classifyGet(error)
                VelaLog.failure(
                    "passkey.assert",
                    "refused",
                    error,
                    "attempt" to attempt,
                    "preferImmediate" to !last,
                    "type" to error.type,
                    "kind" to failure.kind,
                    "domError" to (error as? GetPublicKeyCredentialDomException)?.domError?.type,
                )
                throw failure
            }
        }
    }

    /**
     * Run a Credential Manager get, and notice when its sheet never arrives.
     *
     * Credential Manager hands every candidate passkey — icon bitmaps included —
     * to the system's selector activity in ONE intent. With enough passkeys
     * saved for this rpId that intent outgrows a binder transaction
     * (`TransactionTooLargeException: data parcel size 556892 bytes`, system
     * log, HyperOS 3 / Android 16, 2026-09-19). The selector process starts, is
     * never handed its arguments, and never draws a window; the framework
     * reports neither success nor failure, so the request stays pending FOREVER
     * and the person watches a spinner that cannot end.
     *
     * The platform gives no callback for "my own UI failed to start", so the
     * signal is indirect but exact: a selector that draws TAKES WINDOW FOCUS
     * from this activity (measured: well under a second), and one that died
     * leaves it here (`mCurrentFocus` stays on the caller). Holding focus for
     * [SELECTOR_WATCHDOG_MS] UNBROKEN with a get still pending therefore means
     * no sheet is up. While a sheet is up the person may take as long as they
     * like: focus is elsewhere the whole time, and closing the sheet ends the
     * request, so a pending get never sits behind a focused caller for long.
     *
     * Fail-safe by construction: no activity, or no focus to begin with (one of
     * our own dialogs holds it), and this is a plain call — never a false alarm.
     */
    private suspend fun <T> awaitingSelector(request: suspend () -> T): T {
        val activity = context as? Activity
        if (activity == null) return request()
        return coroutineScope {
            val ceremony = async { request() }
            var gaveUp = false
            val watchdog = launch {
                var held = 0L
                while (held < SELECTOR_WATCHDOG_MS) {
                    delay(SELECTOR_POLL_MS)
                    // Reset, not stand down: a dead selector ALSO takes focus
                    // away — for the ~5 s the window manager waits on a window
                    // that never comes — and then hands it back (device-found).
                    held = if (activity.hasWindowFocus()) held + SELECTOR_POLL_MS else 0L
                }
                gaveUp = true
                // Cancelling reaches the framework through the request's
                // CancellationSignal, which also retires the dead selector.
                ceremony.cancel()
            }
            try {
                ceremony.await()
            } catch (cancelled: CancellationException) {
                if (!gaveUp) throw cancelled
                VelaLog.event(
                    "passkey.assert",
                    "system selector never appeared",
                    "waitedMs" to SELECTOR_WATCHDOG_MS,
                )
                throw SelectorUnresponsive(selectorUnresponsiveMessage())
            } finally {
                watchdog.cancel()
            }
        }
    }

    /**
     * Does this credential live on something the person has to present — a USB
     * stick, an NFC card — rather than in a vault this phone can consult?
     *
     * Unknown transports answer `false`: the settle-race retry is harmless for
     * a credential that turns out to be removable (it costs one extra attempt),
     * while skipping it for one that turns out to be local would bring back the
     * sheet it exists to prevent.
     */
    private fun removable(transports: String): Boolean =
        transports.split(',').map { it.trim() }.any { it == "usb" || it == "nfc" || it == "ble" }

    fun random(bytes: Int): ByteArray = ByteArray(bytes).also(secureRandom::nextBytes)

    /**
     * Run one caBLE ceremony: mint fresh secrets, put the QR on screen, run
     * [body], and clear the QR however it ends. A malformed payload (a static
     * seed that is not a valid scalar — rare, from the CSPRNG) is retried with
     * new secrets a few times before giving up.
     */
    private suspend inline fun <T> runHybrid(
        forGet: Boolean,
        body: (HybridCeremony.Session) -> T,
    ): T {
        val ceremony = hybrid ?: throw PasskeyFailure(
            FailureKind.NotSupported,
            "Sign in with your phone is unavailable here.",
        )
        var session = ceremony.newSession()
        var payload = ceremony.qrPayload(session, forGet)
        var tries = 0
        while (payload == null && tries < 4) {
            session = ceremony.newSession()
            payload = ceremony.qrPayload(session, forGet)
            tries++
        }
        val qr = payload ?: throw PasskeyFailure(
            FailureKind.Other,
            "Could not start sign in with your phone.",
        )
        showQr(qr)
        return try {
            body(session)
        } finally {
            showQr(null)
        }
    }

    /**
     * `name` + NUL + `uuid` — the handle shape every Vela client mints and
     * `Assertion::user_name` in the core parses back. A handle without the NUL
     * separator, or without a uuid tail, is read as a credential this app did
     * not create, and its name is discarded rather than shown.
     */
    private fun encodeUserHandle(name: String): String = name + NUL + UUID.randomUUID()

    /**
     * The `authenticatorAttachment` constraint to put on a registration — which
     * on this platform is NONE, whatever the person picked.
     *
     * Naming `platform` would exclude a provider that is neither platform nor
     * roaming, which on Android is the common case: the credential usually
     * lives in a password manager rather than in the device itself.
     *
     * Naming `cross-platform` is worse. It is standard WebAuthn, and Credential
     * Manager refuses it outright:
     *
     *     Create credential errorMsg=[28460] This provider does not support
     *     creating cross-platform public key credentials.
     *     Provider status changed: CANCELED, and source: REMOTE_PROVIDER
     *
     * The providers that could serve a security key decline, the selector falls
     * back to whatever platform provider is left, and the person — holding the
     * key they just chose in OUR picker — is told setup was cancelled
     * (device-found on a Galaxy S22, 2026-08-26). Registering the SAME key with
     * no constraint at all works: the selector offers "security key" among the
     * ways in, and GMS runs the CTAP2 ceremony.
     *
     * So the method stays the person's stated intent — it labels the row and
     * chooses the fallback artwork — and the system sheet remains the arbiter of
     * which object actually answers, which is what the response reports back.
     */
    private fun attachmentFor(method: KeyMethod): String? = when (method) {
        // `TrustedSigner` never arrives here: the executor hands a `trusted_signer`
        // ceremony to the Trusted Signer instead of to Credential Manager, and the
        // page constrains its own ceremony (spec 075).
        KeyMethod.Platform, KeyMethod.SecurityKey, KeyMethod.Hybrid, KeyMethod.TrustedSigner -> null
    }

    private companion object {
        const val RELYING_PARTY = "getvela.app"
        const val RELYING_PARTY_NAME = "Vela Wallet"
        const val PUBLIC_KEY = "public-key"
        const val ES256 = -7
        const val CHALLENGE_BYTES = 32

        /**
         * How old a mint must be before this app asks for it back. Spent
         * behind the "creating your wallet" screen, so it costs no visible
         * time; a provider that is slower than this is covered by the retries.
         */
        const val SETTLE_AFTER_CREATE_MS = 1_200L

        /**
         * Waits between pinned attempts. Four tries over ~4.2 s in total, then
         * one final attempt that lets the platform speak for itself.
         */
        val RETRY_BACKOFF_MS = longArrayOf(600L, 1_200L, 2_400L)

        /**
         * How long this activity may keep window focus with a get pending
         * before the system's sheet is declared dead. A working selector takes
         * focus in well under a second, slow phones included; providers are
         * given a few seconds by the framework before the sheet is drawn, and
         * this sits comfortably past that.
         */
        const val SELECTOR_WATCHDOG_MS = 10_000L
        const val SELECTOR_POLL_MS = 250L
        const val SELECTOR_UNRESPONSIVE_FALLBACK =
            "The system passkey picker is not responding."
        const val NUL = '\u0000'

        /**
         * Where Credential Manager puts the registration response JSON. There is
         * no typed accessor for it the way there is for an assertion, so the key
         * is spelled out once, here.
         */
        const val BUNDLE_REGISTRATION_RESPONSE =
            "androidx.credentials.BUNDLE_KEY_REGISTRATION_RESPONSE_JSON"

        val secureRandom = SecureRandom()
    }
}

/** The core's `FailureKind` vocabulary. */
enum class FailureKind(val wire: String) {
    Cancelled("cancelled"),
    NotSupported("not_supported"),
    NotDiscoverable("not_discoverable"),
    Other("other"),
}

/**
 * A ceremony that produced no credential, already classified.
 *
 * Classification is the ONE judgement call a shell makes, and it is deliberately
 * narrow: everything unrecognised becomes `other` carrying the platform's own
 * words, which the core forwards verbatim into the bug report.
 */
open class PasskeyFailure(val kind: FailureKind, message: String) : Exception(message)

/**
 * The system's passkey sheet never appeared (see `awaitingSelector`). To the
 * core it is an `other` failure carrying the shell's sentence; to the executor
 * it is the cue to try a route that needs no system sheet.
 */
class SelectorUnresponsive(message: String) : PasskeyFailure(FailureKind.Other, message)

/** A completed registration, in the core's hex vocabulary. */
data class Registration(
    val credentialIdHex: String,
    val attestationObjectHex: String,
    val clientDataJsonHex: String,
    val authenticatorAttachment: String,
    val transports: String,
)

/** A completed assertion. */
data class Assertion(
    val credentialIdHex: String,
    val signatureDerHex: String,
    val authenticatorDataHex: String,
    val clientDataJsonHex: String,
    val userIdHex: String?,
    val authenticatorAttachment: String,
)

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

internal fun classifyCreate(error: CreateCredentialException): PasskeyFailure = when (error) {
    is CreateCredentialCancellationException ->
        PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")

    // No provider offered to create one, or the platform has no passkey support
    // at all. Both are "this device will not do it", which is the case the core's
    // `not_supported` path exists for.
    is CreateCredentialNoCreateOptionException, is CreateCredentialUnsupportedException ->
        PasskeyFailure(FailureKind.NotSupported, describe(error))

    is CreatePublicKeyCredentialDomException -> when (domType(error.domError)) {
        // With excludeCredentials set this means the chosen authenticator
        // already holds one of this wallet's founding keys.
        DOM_INVALID_STATE -> PasskeyFailure(
            FailureKind.Other,
            "This authenticator already holds one of this wallet's keys",
        )
        DOM_NOT_SUPPORTED -> PasskeyFailure(FailureKind.NotSupported, describe(error))
        DOM_NOT_ALLOWED -> PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")
        else -> PasskeyFailure(FailureKind.Other, describe(error))
    }

    else -> PasskeyFailure(FailureKind.Other, describe(error))
}

/**
 * Does this failure mean the SYSTEM passkey route does not exist here — as
 * opposed to a cancellation or an honest "you have no passkey"?
 *
 * Two situations, both of which the app-owned CTAP path can step past because
 * it consults neither a provider nor a domain association:
 *
 *  * **No provider at all** — a phone without (working) GMS has no passkey
 *    provider, and Credential Manager throws
 *    `GetCredentialProviderConfigurationException` ("no provider dependencies
 *    found"). This is the OnePlus 5T sign-in error.
 *  * **The relying party could not be verified** — if the developer's
 *    `.well-known/assetlinks.json` is unreachable the association check fails,
 *    which surfaces here too. A person must still be able to reach their own
 *    key when the wallet's own domain server is down; that is the whole point
 *    of an app-owned path (FR-009c).
 *
 * A plain `NoCredentialException` is deliberately NOT here: it means the route
 * works and there simply is no passkey to sign in with, which is an honest
 * answer, not a broken route.
 */
internal fun isSystemRouteUnavailable(error: GetCredentialException): Boolean =
    error is GetCredentialProviderConfigurationException ||
        error is GetCredentialUnsupportedException

internal fun classifyGet(error: GetCredentialException): PasskeyFailure = when (error) {
    is GetCredentialCancellationException ->
        PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")

    // "No credential" is NOT `not_supported`: the device is perfectly capable,
    // there is simply nothing here to sign in with. The core reads it as a
    // failed sign-in and offers creating a wallet, which is the right next step;
    // reading it as unsupported would tell the person their phone cannot do
    // something it had just done.
    is NoCredentialException -> PasskeyFailure(FailureKind.Other, describe(error))

    is GetCredentialUnsupportedException -> PasskeyFailure(FailureKind.NotSupported, describe(error))

    is GetPublicKeyCredentialDomException -> when (domType(error.domError)) {
        DOM_NOT_SUPPORTED -> PasskeyFailure(FailureKind.NotSupported, describe(error))
        DOM_NOT_ALLOWED -> PasskeyFailure(FailureKind.Cancelled, "User cancelled the operation")
        else -> PasskeyFailure(FailureKind.Other, describe(error))
    }

    else -> PasskeyFailure(FailureKind.Other, describe(error))
}

/**
 * The DOM error's identity.
 *
 * Matched by TYPE STRING rather than by class: the `androidx.credentials` DOM
 * error classes are final and carry the spec name in `type`, so the string is
 * the stable identity — and it is the same three names the web path branches on.
 */
private fun domType(domError: Any): String = runCatching {
    domError.javaClass.getMethod("getType").invoke(domError) as? String
}.getOrNull() ?: domError.javaClass.simpleName

private const val DOM_INVALID_STATE = "androidx.credentials.TYPE_INVALID_STATE_ERROR"
private const val DOM_NOT_SUPPORTED = "androidx.credentials.TYPE_NOT_SUPPORTED_ERROR"
private const val DOM_NOT_ALLOWED = "androidx.credentials.TYPE_NOT_ALLOWED_ERROR"

private fun describe(error: Throwable): String =
    error.message ?: error::class.simpleName ?: "Unknown passkey error"

// ---------------------------------------------------------------------------
// base64url and hex
// ---------------------------------------------------------------------------
//
// WebAuthn JSON is base64url everywhere; the core is hex everywhere. Both
// conversions go through vela-core, so a padding or alphabet difference cannot
// appear on one client only.

internal fun hexOfBase64url(value: String): String = toHex(fromBase64url(value), false)

internal fun base64urlOfHex(value: String): String = toBase64url(fromHex(value))

private fun JSONArray?.strings(): List<String> =
    if (this == null) emptyList() else (0 until length()).map { optString(it) }
