package app.getvela.wallet

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.UserOpCall
import uniffi.vela_core_uniffi.UserOpFeeMode
import uniffi.vela_core_uniffi.WalletKeyRecord
import uniffi.vela_core_uniffi.WebAuthnAssertion
import uniffi.vela_core_uniffi.computeSafeAddress
import uniffi.vela_core_uniffi.parsePublicKey
import uniffi.vela_core_uniffi.userOpDraft
import uniffi.vela_core_uniffi.userOpFloors
import uniffi.vela_core_uniffi.userOpSafeOpHash
import uniffi.vela_core_uniffi.userOpSign
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureMultiAddress

/**
 * The parallel space's keyset, through the debug-only library (spec 043 US0).
 *
 * Two facts every later device pass rests on: the fixture account the phone
 * signs in as derives the same Safe the core derives for its key (so it IS the
 * wallet the web's `/parallel` shows), and an assertion the keyset mints is
 * one the core's envelope accepts as this wallet's signature — no prompt, no
 * second WebAuthn assembly in Kotlin.
 */
class ParallelSpaceTest {

    private fun hex(bytes: ByteArray) = bytes.joinToString("") { "%02x".format(it) }
    private fun unhex(text: String) = text.chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    @Test
    fun theFixtureAccountIsTheSafeTheCoreDerivesForItsKey() {
        val accounts = fixtureAccounts()
        assertTrue(accounts.size >= 2)
        val first = accounts.first()
        val key = parsePublicKey(first.publicKeyHex)
        assertEquals(computeSafeAddress(key.x, key.y).address.lowercase(), first.address.lowercase())
        assertTrue(fixtureMultiAddress().startsWith("0x"))
    }

    @Test
    fun aFixtureAssertionIsAcceptedAsThisWalletsSignature() {
        val accounts = fixtureAccounts()
        val keys = accounts.map { WalletKeyRecord(credentialId = it.credentialIdHex, publicKeyHex = it.publicKeyHex) }
        val sender = fixtureMultiAddress()
        val floors = userOpFloors(chainId = 100u, deployed = true, subCalls = 2u)
        val draft = userOpDraft(
            sender = sender,
            nonce = "0x1",
            deployed = true,
            keyHexes = keys.map { it.publicKeyHex },
            calls = listOf(UserOpCall(to = "0x031d7D57c99CAF891e1C250554691Fd12D84772b", value = "1", data = "0x")),
            fee = UserOpFeeMode.InBand(gasFeeToken = null, amount = "1", recipient = sender),
            floors = floors,
        )
        val challenge = userOpSafeOpHash(draft, 100u)
        assertEquals(32, challenge.size)

        val signed = fixtureAssert(challenge, listOf(accounts[1].credentialIdHex), null)
        assertEquals(accounts[1].credentialIdHex, signed.credentialIdHex)
        // The challenge rides in the client data, base64url — the core checks it on the way in.
        val clientData = String(unhex(signed.clientDataJsonHex))
        assertTrue(clientData.contains("\"type\":\"webauthn.get\""))

        val finished = userOpSign(
            draft,
            WebAuthnAssertion(
                authenticatorData = unhex(signed.authenticatorDataHex),
                clientDataJson = unhex(signed.clientDataJsonHex),
                signatureDer = unhex(signed.signatureDerHex),
            ),
            credentialId = signed.credentialIdHex,
            keys = keys,
        )
        // 12-byte validity window, then the contract signature; the dummy is gone.
        assertTrue(finished.signature.size > 12 + 65)
        assertEquals("000000000000000000000000", hex(finished.signature.copyOfRange(0, 12)))
    }
}
