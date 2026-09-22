package app.getvela.wallet

import app.getvela.wallet.feature.signing.clearsigner.ClearSignerAnswer
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerAsk
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerLoopback
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.ClearSignerCeremonyOutcome
import uniffi.vela_core_uniffi.clearSignerCeremonyRequest
import uniffi.vela_core_uniffi.fromHex
import uniffi.vela_core_uniffi.toBase64url
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureRegistration
import java.io.InputStream
import java.net.Socket
import java.security.SecureRandom
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.concurrent.thread

/**
 * Spec 075: a whole create flow through the Clear Signer, on one page visit.
 *
 * `register_passkey` mints a key on the page and `sign_member_proof` confirms
 * it — two requests, one listener, one socket, then `bye` (contract §1.5). The
 * page is the parallel space's keyset (debug-only), which mints the same
 * genuine WebAuthn bytes a real authenticator would, so the core's
 * verification is doing real work here rather than accepting a stub.
 *
 * The page's origin has to be the one the core was told to expect, so the
 * "signer page" is `https://getvela.app/` — the origin the fixture keyset
 * claims, and the one this wallet's passkeys belong to.
 */
class ClearSignerCeremonyTest {

    private val page = "https://getvela.app/"

    @Test
    fun `create then member proof run on one page visit, and the flow ends with bye`() {
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
            .put("method", "clear_signer")
            .toString()
        val memberOp = JSONObject()
            .put("type", "sign_member_proof")
            .put("credential_id", account.credentialIdHex)
            .put("public_key_hex", account.publicKeyHex)
            .put("attestation_hex", "")
            .put("group_public_key_hex", "04" + "11".repeat(64))
            .put("method", "clear_signer")
            .toString()

        val seen = CopyOnWriteArrayList<String>()
        val wire = ClearSignerLoopback(
            base = page,
            openPage = { url ->
                val fragment = url.substringAfter('#').split('&')
                    .associate { it.substringBefore('=') to it.substringAfter('=') }
                thread {
                    Page(fragment.getValue("p").toInt()).use { browser ->
                        browser.upgrade(origin = "https://getvela.app")
                        browser.send("""{"v":1,"t":"hello","token":"${fragment.getValue("t")}"}""")

                        // 1 — the create card.
                        val create = JSONObject(browser.receive())
                        seen += create.getJSONObject("intent").getString("method")
                        browser.send(
                            JSONObject()
                                .put("v", 1).put("t", "result").put("id", create.getString("id"))
                                .put(
                                    "registration",
                                    JSONObject()
                                        .put("credentialId", toBase64url(fromHex(registration.credentialIdHex)))
                                        .put("attestationObject", registration.attestationObjectHex)
                                        .put("clientDataJSON", registration.clientDataJsonHex)
                                        .put("authenticatorAttachment", "platform")
                                        .put("transports", "internal"),
                                )
                                .put("origin", "https://getvela.app")
                                .toString(),
                        )

                        // 2 — "confirm this key joins the wallet", on the SAME
                        // socket: no second page visit, no second tab.
                        val member = JSONObject(browser.receive())
                        seen += member.getJSONObject("intent").getString("method")
                        val signed = fixtureAssert(memberChallenge, listOf(account.credentialIdHex), null)
                        browser.send(
                            JSONObject()
                                .put("v", 1).put("t", "result").put("id", member.getString("id"))
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
                                .put("origin", "https://getvela.app")
                                .toString(),
                        )

                        seen += JSONObject(browser.receive()).getString("t")
                        browser.drain()
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
                    ClearSignerAsk.Ceremony(request(createOp), createOp, null),
                )
                val second = wire.ask(
                    ClearSignerAsk.Ceremony(request(memberOp), memberOp, memberChallenge),
                )
                first to second
            }
        }
        wire.end()

        val minted = (created as ClearSignerAnswer.Ceremonial).outcome
        assertTrue("$minted", minted is ClearSignerCeremonyOutcome.Registered)
        val key = JSONObject((minted as ClearSignerCeremonyOutcome.Registered).registrationJson)
        assertEquals(account.credentialIdHex.lowercase(), key.getString("credential_id").lowercase())
        // Spec 075's whole point: the key remembers the page it lives behind.
        assertEquals("https://getvela.app", key.getString("signer_origin"))

        val proof = (confirmed as ClearSignerAnswer.Ceremonial).outcome
        assertTrue("$proof", proof is ClearSignerCeremonyOutcome.Asserted)
        val assertion = JSONObject((proof as ClearSignerCeremonyOutcome.Asserted).assertionJson)
        assertEquals(account.credentialIdHex.lowercase(), assertion.getString("credential_id").lowercase())
        assertEquals("https://getvela.app", assertion.getString("signer_origin"))

        runBlocking { withTimeout(5_000L) { while (seen.size < 3) kotlinx.coroutines.delay(20) } }
        assertEquals(listOf("vela_createPasskey", "vela_memberProof", "bye"), seen.toList())
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
            .put("method", "clear_signer")
            .toString()
        val wallets = ByteArray(32) { (it + 1).toByte() }
        val pages = ByteArray(32) { (it + 9).toByte() }

        val wire = ClearSignerLoopback(
            base = page,
            openPage = { url ->
                val fragment = url.substringAfter('#').split('&')
                    .associate { it.substringBefore('=') to it.substringAfter('=') }
                thread {
                    Page(fragment.getValue("p").toInt()).use { browser ->
                        browser.upgrade(origin = "https://getvela.app")
                        browser.send("""{"v":1,"t":"hello","token":"${fragment.getValue("t")}"}""")
                        val member = JSONObject(browser.receive())
                        // A page that fetched — or was handed — some OTHER
                        // challenge signs something the wallet never asked for.
                        val signed = fixtureAssert(pages, listOf(account.credentialIdHex), null)
                        browser.send(
                            JSONObject()
                                .put("v", 1).put("t", "result").put("id", member.getString("id"))
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
                                .put("origin", "https://getvela.app")
                                .toString(),
                        )
                        browser.drain()
                    }
                }
                true
            },
            timeoutMs = 20_000L,
            random = SecureRandom(),
        )
        val answer = runBlocking {
            withTimeout(30_000L) { wire.ask(ClearSignerAsk.Ceremony(request(memberOp), memberOp, wallets)) }
        }
        wire.end()
        val outcome = (answer as ClearSignerAnswer.Ceremonial).outcome
        assertTrue("$outcome", outcome is ClearSignerCeremonyOutcome.Refused)
    }

    private fun request(operationJson: String): String = clearSignerCeremonyRequest(
        operationJson,
        "abc123",
        "Parallel One",
        "https://index.example",
    ) ?: error("the core builds a request for every ceremony operation")

    /** The browser's side of the socket, as `ClearSignerChannelTest` has it. */
    private class Page(port: Int) : AutoCloseable {
        private val socket = Socket("127.0.0.1", port).apply { soTimeout = 15_000 }
        private val input: InputStream = socket.getInputStream()

        fun upgrade(origin: String): String {
            socket.getOutputStream().write(
                ("GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n" +
                    "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: $origin\r\n\r\n")
                    .toByteArray(),
            )
            val head = StringBuilder()
            while (!head.endsWith("\r\n\r\n")) {
                val byte = input.read()
                if (byte < 0) break
                head.append(byte.toChar())
            }
            return head.toString()
        }

        fun send(text: String) {
            val payload = text.toByteArray()
            val mask = byteArrayOf(0x37, 0xfa.toByte(), 0x21, 0x3d)
            val frame = java.io.ByteArrayOutputStream()
            frame.write(0x81)
            when {
                payload.size < 126 -> frame.write(0x80 or payload.size)
                else -> {
                    frame.write(0x80 or 126)
                    frame.write(payload.size shr 8)
                    frame.write(payload.size and 0xff)
                }
            }
            frame.write(mask)
            payload.forEachIndexed { i, b -> frame.write(b.toInt() xor mask[i % 4].toInt()) }
            socket.getOutputStream().write(frame.toByteArray())
        }

        fun receive(): String {
            while (true) {
                val first = input.read()
                var length = input.read() and 0x7f
                if (length == 126) length = (input.read() shl 8) or input.read()
                val payload = input.readNBytes(length)
                if (first and 0x0f == 0x1) return String(payload)
            }
        }

        fun drain() {
            runCatching { while (input.read() >= 0) Unit }
        }

        override fun close() = socket.close()
    }
}
