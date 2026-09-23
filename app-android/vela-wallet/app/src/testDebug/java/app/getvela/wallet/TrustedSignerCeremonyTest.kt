package app.getvela.wallet

import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerAnswer
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerAsk
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerCallbacks
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerScheme
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.TrustedSignerCeremonyOutcome
import uniffi.vela_core_uniffi.trustedSignerCeremonyRequest
import uniffi.vela_core_uniffi.fromHex
import uniffi.vela_core_uniffi.toBase64url
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureRegistration
import java.net.URLDecoder
import java.net.URLEncoder
import java.security.SecureRandom
import java.util.Base64
import java.util.concurrent.CopyOnWriteArrayList
import java.util.zip.Inflater
import kotlin.concurrent.thread

/**
 * Spec 075: a whole create flow through the Trusted Signer, over the channel
 * the phones have (spec 076) — the request in the launch URL's fragment, the
 * answer in a `velawallet://sign-result?…`.
 *
 * `register_passkey` mints a key on the page and `sign_member_proof` confirms
 * it. On this transport that is TWO page visits, because a URL carries exactly
 * one request; what makes them one flow is the wallet's side, not the
 * channel's. The page is the parallel space's keyset (debug-only), which mints
 * the same genuine WebAuthn bytes a real authenticator would, so the core's
 * verification is doing real work here rather than accepting a stub.
 *
 * The page's origin has to be the one the core was told to expect, so the
 * "signer page" is `https://getvela.app/` — the origin the fixture keyset
 * claims, and the one this wallet's passkeys belong to.
 */
class TrustedSignerCeremonyTest {

    private val page = "https://getvela.app/"

    @Test
    fun `create then member proof each ride their own visit, and both are accepted`() {
        val account = fixtureAccounts().first()
        val registration = fixtureRegistration(account.index)
        // The member challenge the WALLET fetched from the registry. The page
        // fetches its own for the same inputs; the core refuses the answer
        // unless the two are the same bytes.
        val memberChallenge = ByteArray(32) { (it + 1).toByte() }

        val createOp = JSONObject()
            .put("type", "register_passkey")
            .put("name", "Parallel One")
            .put("exclude_credential_ids", org.json.JSONArray())
            .put("method", "trusted_signer")
            .toString()
        val memberOp = JSONObject()
            .put("type", "sign_member_proof")
            .put("credential_id", account.credentialIdHex)
            .put("public_key_hex", account.publicKeyHex)
            .put("attestation_hex", "")
            .put("group_public_key_hex", "04" + "11".repeat(64))
            .put("method", "trusted_signer")
            .toString()

        val seen = CopyOnWriteArrayList<String>()
        val tokens = CopyOnWriteArrayList<String>()
        val wire = TrustedSignerScheme(
            base = page,
            openPage = { url ->
                val visit = Visit(url)
                tokens += visit.token
                val method = visit.request.getJSONObject("intent").getString("method")
                seen += method
                thread {
                    if (method == "vela_createPasskey") {
                        visit.answer(
                            JSONObject()
                                .put(
                                    "registration",
                                    JSONObject()
                                        .put("credentialId", toBase64url(fromHex(registration.credentialIdHex)))
                                        .put("attestationObject", registration.attestationObjectHex)
                                        .put("clientDataJSON", registration.clientDataJsonHex)
                                        .put("authenticatorAttachment", "platform")
                                        .put("transports", "internal"),
                                )
                                .put("origin", "https://getvela.app"),
                        )
                    } else {
                        val signed = fixtureAssert(memberChallenge, listOf(account.credentialIdHex), null)
                        visit.answer(
                            JSONObject()
                                .put(
                                    "assertion",
                                    JSONObject()
                                        .put("credentialId", toBase64url(fromHex(signed.credentialIdHex)))
                                        .put("signatureDer", signed.signatureDerHex)
                                        .put("authenticatorData", signed.authenticatorDataHex)
                                        .put("clientDataJSON", signed.clientDataJsonHex)
                                        .put("userHandle", JSONObject.NULL)
                                        .put("authenticatorAttachment", "platform"),
                                )
                                .put("origin", "https://getvela.app"),
                        )
                    }
                }
                true
            },
            timeoutMs = 20_000L,
            random = SecureRandom(),
        )

        val (created, confirmed) = runBlocking {
            withTimeout(30_000L) {
                val first = wire.ask(
                    TrustedSignerAsk.Ceremony(request(createOp), createOp, null),
                )
                val second = wire.ask(
                    TrustedSignerAsk.Ceremony(request(memberOp), memberOp, memberChallenge),
                )
                first to second
            }
        }
        wire.end()

        val minted = (created as TrustedSignerAnswer.Ceremonial).outcome
        assertTrue("$minted", minted is TrustedSignerCeremonyOutcome.Registered)
        val key = JSONObject((minted as TrustedSignerCeremonyOutcome.Registered).registrationJson)
        assertEquals(account.credentialIdHex.lowercase(), key.getString("credential_id").lowercase())
        // Spec 075's whole point: the key remembers the page it lives behind.
        assertEquals("https://getvela.app", key.getString("signer_origin"))

        val proof = (confirmed as TrustedSignerAnswer.Ceremonial).outcome
        assertTrue("$proof", proof is TrustedSignerCeremonyOutcome.Asserted)
        val assertion = JSONObject((proof as TrustedSignerCeremonyOutcome.Asserted).assertionJson)
        assertEquals(account.credentialIdHex.lowercase(), assertion.getString("credential_id").lowercase())
        assertEquals("https://getvela.app", assertion.getString("signer_origin"))

        assertEquals(listOf("vela_createPasskey", "vela_memberProof"), seen.toList())
        // Two visits, two one-time tokens: neither request's answer could be
        // read as the other's.
        assertEquals(2, tokens.size)
        assertTrue("each visit has its own token", tokens[0] != tokens[1])
    }

    @Test
    fun `a member proof over a challenge the wallet did not fetch is refused`() {
        val account = fixtureAccounts().first()
        val memberOp = JSONObject()
            .put("type", "sign_member_proof")
            .put("credential_id", account.credentialIdHex)
            .put("public_key_hex", account.publicKeyHex)
            .put("attestation_hex", "")
            .put("group_public_key_hex", "04" + "11".repeat(64))
            .put("method", "trusted_signer")
            .toString()
        val wallets = ByteArray(32) { (it + 1).toByte() }
        val pages = ByteArray(32) { (it + 9).toByte() }

        val wire = TrustedSignerScheme(
            base = page,
            openPage = { url ->
                val visit = Visit(url)
                thread {
                    // A page that fetched — or was handed — some OTHER
                    // challenge signs something the wallet never asked for.
                    val signed = fixtureAssert(pages, listOf(account.credentialIdHex), null)
                    visit.answer(
                        JSONObject()
                            .put(
                                "assertion",
                                JSONObject()
                                    .put("credentialId", toBase64url(fromHex(signed.credentialIdHex)))
                                    .put("signatureDer", signed.signatureDerHex)
                                    .put("authenticatorData", signed.authenticatorDataHex)
                                    .put("clientDataJSON", signed.clientDataJsonHex)
                                    .put("userHandle", JSONObject.NULL)
                                    .put("authenticatorAttachment", "platform"),
                            )
                            .put("origin", "https://getvela.app"),
                    )
                }
                true
            },
            timeoutMs = 20_000L,
            random = SecureRandom(),
        )
        val answer = runBlocking {
            withTimeout(30_000L) { wire.ask(TrustedSignerAsk.Ceremony(request(memberOp), memberOp, wallets)) }
        }
        wire.end()
        val outcome = (answer as TrustedSignerAnswer.Ceremonial).outcome
        assertTrue("$outcome", outcome is TrustedSignerCeremonyOutcome.Refused)
    }

    private fun request(operationJson: String): String = trustedSignerCeremonyRequest(
        operationJson,
        "abc123",
        "Parallel One",
        "https://index.example",
    ) ?: error("the core builds a request for every ceremony operation")

    /**
     * The browser's side of one visit, as `TrustedSignerChannelTest` has it:
     * the request out of the fragment, the answer back through the scheme.
     *
     * A ceremony's answer goes in WHOLE — `{registration|assertion, origin}` —
     * because that is what the URL channel carries for one (`intake.js`:
     * `body.result !== undefined ? body.result : body`). Only a signature's
     * answer is the `result` object.
     */
    private class Visit(url: String) {
        private val fragment = url.substringAfter('#').split('&')
            .associate { it.substringBefore('=') to it.substringAfter('=') }

        val token: String = URLDecoder.decode(fragment.getValue("t"), "UTF-8")

        private val callbackUrl: String =
            String(Base64.getUrlDecoder().decode(fragment.getValue("cb").trimEnd('=')))

        val request: JSONObject = JSONObject(
            inflateRaw(Base64.getUrlDecoder().decode(fragment.getValue("i").trimEnd('='))),
        )

        fun answer(body: JSONObject) {
            TrustedSignerCallbacks.deliver(
                callbackUrl + "?t=" + URLEncoder.encode(token, "UTF-8") + "&result=" +
                    Base64.getUrlEncoder().withoutPadding().encodeToString(body.toString().toByteArray()),
            )
        }

        private companion object {
            /** `DecompressionStream('deflate-raw')`, which is what the page uses. */
            fun inflateRaw(bytes: ByteArray): String {
                val inflater = Inflater(true)
                inflater.setInput(bytes)
                val out = java.io.ByteArrayOutputStream()
                val buffer = ByteArray(8 * 1024)
                while (!inflater.finished()) {
                    val read = inflater.inflate(buffer)
                    if (read == 0) break
                    out.write(buffer, 0, read)
                }
                inflater.end()
                return out.toString("UTF-8")
            }
        }
    }
}
