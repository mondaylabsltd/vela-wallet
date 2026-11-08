package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.TrustedSigner
import app.getvela.wallet.feature.send.core.TrustedSignerIntent
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.wallet.core.RpcResult
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.WalletKeyRecord
import uniffi.vela_core_uniffi.WebAuthnAssertion
import uniffi.vela_core_uniffi.eip1271Signature
import uniffi.vela_core_uniffi.messageSignature
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureMultiAddress

/**
 * §37 (review 1-11): every MESSAGE signature this wallet returns — through the
 * passkey and through the signing page — is the core's `message_signature`
 * of the EIP-1271 envelope, given the account's code on the REQUEST's chain,
 * read right then through the wallet's own RPC: bare when the account is
 * deployed there, in its ERC-6492 envelope when it is not (or the code could
 * not be read). The rule is the core's; pinned here is that this shell reads
 * the code and hands it over.
 */
class MessageSignature6492Test {
    private val keyset = fixtureAccounts()
    private val safe = fixtureMultiAddress()
    private val digest = ByteArray(32) { 9 }

    /** ERC-6492's magic suffix, `0x6492` × 16. */
    private val magic = "6492".repeat(16)

    private fun record(viaPage: Boolean): JSONObject {
        val keys = JSONArray()
        keyset.forEach { key -> keys.put(JSONObject().put("credential_id", key.credentialIdHex).put("public_key_hex", key.publicKeyHex).put("name", key.name).put("transports", "internal")) }
        return JSONObject().put("id", keyset.first().credentialIdHex).put("name", "Parallel space").put("address", safe)
            .put("public_key_hex", keyset.first().publicKeyHex).put("created_at_iso", "2026-11-08T00:00:00Z").put("keys", keys)
            .apply {
                if (viaPage) {
                    put("signing_domain", "getvela.app")
                    put("signing_venue", JSONObject().put("type", "page").put("url", "https://sign.getvela.app/"))
                }
            }
    }

    private val signed = mutableListOf<Assertion>()

    private val passkey = object : UserOpSigner {
        override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
            val a = fixtureAssert(challenge, listOfNotNull(credentialIdHex), 0u)
            return Assertion(a.credentialIdHex, a.signatureDerHex, a.authenticatorDataHex, a.clientDataJsonHex, null, "platform").also { signed += it }
        }
    }

    private val page = object : TrustedSigner {
        override suspend fun sign(
            requestJson: String,
            digest: ByteArray,
            keys: List<WalletKeyRecord>,
            page: String,
            key: app.getvela.wallet.feature.signing.trustedsigner.SigningPlan.KeyLabel?,
            askFirst: Boolean,
        ): Assertion {
            val a = fixtureAssert(digest, keys.map { it.credentialId }, 0u)
            return Assertion(a.credentialIdHex, a.signatureDerHex, a.authenticatorDataHex, a.clientDataJsonHex, null, "").also { signed += it }
        }
    }

    /** The spine over a chain whose `eth_getCode` at the account answers [code] (`null`: nobody answers). */
    private fun spine(code: String?, viaPage: Boolean, port: FakeRelayPort = FakeRelayPort()): UserOpSpine {
        if (code != null) port.always("eth_getCode") { FakeRelayPort.body(code) }
        return UserOpSpine(
            relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            accounts = StoreAccountPort(AccountStore(FakeStore(mapOf("vela.accounts" to JSONArray().put(record(viaPage)).toString())))),
            signer = { passkey },
            trustedSigner = { page },
            handoffShown = true,
        )
    }

    private fun hex(bytes: ByteArray) = "0x" + bytes.joinToString("") { "%02x".format(it.toInt() and 0xff) }

    /** The bare EIP-1271 envelope of the last assertion, as the core builds it. */
    private fun envelope(): ByteArray {
        val a = signed.last()
        val keys = keyset.map { WalletKeyRecord(credentialId = it.credentialIdHex, publicKeyHex = it.publicKeyHex) }
        fun unhex(h: String) = h.removePrefix("0x").chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        return eip1271Signature(
            WebAuthnAssertion(authenticatorData = unhex(a.authenticatorDataHex), clientDataJson = unhex(a.clientDataJsonHex), signatureDer = unhex(a.signatureDerHex)),
            a.credentialIdHex, keys,
        )
    }

    private val intent = TrustedSignerIntent("personal_sign", JSONArray().put("0x68656c6c6f").put(fixtureMultiAddress()).toString(), "https://app.test")

    @Test
    fun `deployed on the request's chain - the bare EIP-1271 signature`() = runBlocking<Unit> {
        listOf(false, true).forEach { viaPage ->
            val port = FakeRelayPort()
            val signature = spine("0x6080604052", viaPage, port).signMessage(100, safe, digest, intent = intent)
            assertEquals("viaPage=$viaPage", hex(envelope()), signature)
            assertFalse(signature.endsWith(magic))
            assertTrue("the code was read: ${port.calls}", port.calls.any { it.endsWith(":eth_getCode") })
        }
    }

    @Test
    fun `not deployed there, or the code unread - the ERC-6492 envelope the core builds`() = runBlocking<Unit> {
        listOf(false, true).forEach { viaPage ->
            listOf("0x", null).forEach { code ->
                val signature = spine(code, viaPage).signMessage(100, safe, digest, intent = intent)
                assertTrue("viaPage=$viaPage code=$code: $signature", signature.endsWith(magic))
                val keys = keyset.map { WalletKeyRecord(credentialId = it.credentialIdHex, publicKeyHex = it.publicKeyHex) }
                assertEquals(hex(messageSignature(envelope(), keys, safe, code)), signature)
            }
        }
    }

    @Test
    fun `an error from the chain is no code - wrapped, never bare`() = runBlocking<Unit> {
        val port = FakeRelayPort()
        port.always("eth_getCode") { RpcResult.Body(JSONObject().put("error", JSONObject().put("code", -32000).put("message", "boom"))) }
        val signature = spine(null, viaPage = false, port = port).signMessage(100, safe, digest, intent = intent)
        assertTrue(signature.endsWith(magic))
    }
}
