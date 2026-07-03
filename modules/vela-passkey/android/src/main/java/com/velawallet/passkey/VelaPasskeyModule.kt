package com.velawallet.passkey

import android.util.Base64
import androidx.credentials.*
import androidx.credentials.exceptions.*
import com.facebook.react.bridge.*
import kotlinx.coroutines.*
import org.json.JSONArray
import org.json.JSONObject
import java.security.SecureRandom

/**
 * Passkey native module for Android using Credential Manager.
 * Requires androidx.credentials in the project's build.gradle.
 */
class VelaPasskeyModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

    companion object {
        const val NAME = "VelaPasskey"
        const val RELYING_PARTY = "getvela.app"
    }

    override fun getName(): String = NAME

    private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())

    // invalidate() is the module teardown hook called under BOTH the old and the
    // New Architecture; onCatalystInstanceDestroy() is deprecated and is NOT called
    // on the New Arch, which would leak the coroutine scope.
    override fun invalidate() {
        scope.cancel()
        super.invalidate()
    }

    @ReactMethod
    fun isSupported(promise: Promise) {
        promise.resolve(true)
    }

    @ReactMethod
    fun register(userName: String, promise: Promise) {
        val activity = getCurrentActivity()
        if (activity == null) {
            promise.reject("PASSKEY_FAILED", "No activity available")
            return
        }

        scope.launch {
            try {
                val challenge = generateChallenge()
                val userId = encodeUserID(userName)
                val challengeB64 = base64UrlEncode(challenge)
                val userIdB64 = base64UrlEncode(userId)

                // Build the request with JSONObject/JSONArray so values (notably the
                // user-supplied wallet name) are escaped. Hand-interpolating userName into
                // a JSON string breaks on a quote/backslash/newline and fails registration.
                val json = JSONObject().apply {
                    put("rp", JSONObject().apply {
                        put("id", RELYING_PARTY)
                        put("name", "Vela Wallet")
                    })
                    put("user", JSONObject().apply {
                        put("id", userIdB64)
                        put("name", userName)
                        put("displayName", userName)
                    })
                    put("challenge", challengeB64)
                    put("pubKeyCredParams", JSONArray().apply {
                        put(JSONObject().apply {
                            put("type", "public-key")
                            put("alg", -7)
                        })
                    })
                    put("authenticatorSelection", JSONObject().apply {
                        put("authenticatorAttachment", "platform")
                        put("residentKey", "required")
                        // WebAuthn L2: set alongside residentKey=required. Third-party
                        // credential providers parse this JSON themselves and some only
                        // honor the L1 boolean — without it they silently create a
                        // NON-discoverable credential that never appears at sign-in.
                        put("requireResidentKey", true)
                        put("userVerification", "required")
                    })
                    put("attestation", "direct")
                }.toString()

                val request = CreatePublicKeyCredentialRequest(json)
                val credentialManager = CredentialManager.create(reactApplicationContext)
                val result = credentialManager.createCredential(activity!!, request)
                val response = result as CreatePublicKeyCredentialResponse

                val responseJson = JSONObject(response.registrationResponseJson)
                val responseObj = responseJson.getJSONObject("response")

                val rawId = base64UrlDecode(responseJson.getString("rawId"))
                val attestationObject = base64UrlDecode(responseObj.getString("attestationObject"))
                val clientDataJSON = base64UrlDecode(responseObj.getString("clientDataJSON"))

                val dict = Arguments.createMap().apply {
                    putString("credentialId", toHex(rawId))
                    putString("attestationObjectHex", toHex(attestationObject))
                    putString("clientDataJSONHex", toHex(clientDataJSON))
                }
                promise.resolve(dict)

            } catch (e: CreateCredentialCancellationException) {
                promise.reject("PASSKEY_CANCELLED", "User cancelled registration", e)
            } catch (e: CreateCredentialException) {
                promise.reject("PASSKEY_FAILED", "Registration failed: [${e.type}] ${e.message ?: "Unknown"}", e)
            } catch (e: Exception) {
                promise.reject("PASSKEY_FAILED", e.message ?: "Unknown error", e)
            }
        }
    }

    @ReactMethod
    fun authenticate(promise: Promise) {
        val activity = getCurrentActivity()
        if (activity == null) {
            promise.reject("PASSKEY_FAILED", "No activity available")
            return
        }

        scope.launch {
            try {
                val challenge = generateChallenge()
                val challengeB64 = base64UrlEncode(challenge)

                val json = """
                {
                    "challenge": "$challengeB64",
                    "rpId": "$RELYING_PARTY",
                    "userVerification": "required"
                }
                """.trimIndent()

                val request = GetCredentialRequest(listOf(GetPublicKeyCredentialOption(json)))
                val credentialManager = CredentialManager.create(reactApplicationContext)
                val result = credentialManager.getCredential(activity!!, request)
                resolveAssertion(result, promise)

            } catch (e: GetCredentialCancellationException) {
                promise.reject("PASSKEY_CANCELLED", "User cancelled", e)
            } catch (e: NoCredentialException) {
                promise.reject("PASSKEY_NO_CREDENTIAL", "No passkey found", e)
            } catch (e: GetCredentialException) {
                promise.reject("PASSKEY_FAILED", "Authentication failed: [${e.type}] ${e.message ?: "Unknown"}", e)
            } catch (e: Exception) {
                promise.reject("PASSKEY_FAILED", e.message ?: "Unknown error", e)
            }
        }
    }

    @ReactMethod
    fun sign(challengeHex: String, credentialId: String?, promise: Promise) {
        val activity = getCurrentActivity()
        if (activity == null) {
            promise.reject("PASSKEY_FAILED", "No activity available")
            return
        }

        scope.launch {
            try {
                val challengeBytes = fromHex(challengeHex)
                val challengeB64 = base64UrlEncode(challengeBytes)
                val allowCredentialsJson = credentialId
                    ?.takeIf { it.isNotBlank() }
                    ?.let {
                        val credentialIdB64 = base64UrlEncode(fromHex(it))
                        """,
                    "allowCredentials": [{"type": "public-key", "id": "$credentialIdB64"}]"""
                    }
                    ?: ""

                val json = """
                {
                    "challenge": "$challengeB64",
                    "rpId": "$RELYING_PARTY",
                    "userVerification": "required"$allowCredentialsJson
                }
                """.trimIndent()

                val request = GetCredentialRequest(listOf(GetPublicKeyCredentialOption(json)))
                val credentialManager = CredentialManager.create(reactApplicationContext)
                val result = credentialManager.getCredential(activity!!, request)
                resolveAssertion(result, promise)

            } catch (e: GetCredentialCancellationException) {
                promise.reject("PASSKEY_CANCELLED", "User cancelled", e)
            } catch (e: NoCredentialException) {
                promise.reject("PASSKEY_NO_CREDENTIAL", "No passkey found", e)
            } catch (e: GetCredentialException) {
                promise.reject("PASSKEY_FAILED", "Sign failed: [${e.type}] ${e.message ?: "Unknown"}", e)
            } catch (e: Exception) {
                promise.reject("PASSKEY_FAILED", e.message ?: "Unknown error", e)
            }
        }
    }

    private fun resolveAssertion(result: GetCredentialResponse, promise: Promise) {
        val credential = result.credential
        if (credential !is PublicKeyCredential) {
            promise.reject("PASSKEY_NO_CREDENTIAL", "No public key credential returned")
            return
        }

        val responseJson = JSONObject(credential.authenticationResponseJson)
        val responseObj = responseJson.getJSONObject("response")

        val rawId = base64UrlDecode(responseJson.getString("rawId"))
        val authenticatorData = base64UrlDecode(responseObj.getString("authenticatorData"))
        val signature = base64UrlDecode(responseObj.getString("signature"))
        val clientDataJSON = base64UrlDecode(responseObj.getString("clientDataJSON"))

        val dict = Arguments.createMap().apply {
            putString("credentialId", toHex(rawId))
            putString("signatureHex", toHex(signature))
            putString("authenticatorDataHex", toHex(authenticatorData))
            putString("clientDataJSONHex", toHex(clientDataJSON))
        }

        if (responseObj.has("userHandle")) {
            val userHandle = base64UrlDecode(responseObj.getString("userHandle"))
            if (userHandle.isNotEmpty()) {
                dict.putString("userIdHex", toHex(userHandle))
            }
        }

        promise.resolve(dict)
    }

    private fun encodeUserID(name: String): ByteArray {
        val combined = "$name\u0000${java.util.UUID.randomUUID()}"
        return combined.toByteArray(Charsets.UTF_8)
    }

    private fun generateChallenge(): ByteArray {
        val bytes = ByteArray(32)
        SecureRandom().nextBytes(bytes)
        return bytes
    }

    private fun base64UrlEncode(data: ByteArray): String =
        Base64.encodeToString(data, Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP)

    private fun base64UrlDecode(str: String): ByteArray =
        Base64.decode(str, Base64.URL_SAFE or Base64.NO_PADDING)

    private fun toHex(bytes: ByteArray): String =
        bytes.joinToString("") { "%02x".format(it) }

    private fun fromHex(hex: String): ByteArray {
        val clean = if (hex.startsWith("0x")) hex.substring(2) else hex
        return ByteArray(clean.length / 2) { i ->
            clean.substring(i * 2, i * 2 + 2).toInt(16).toByte()
        }
    }
}
