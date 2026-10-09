package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.SessionController
import app.getvela.wallet.feature.onboarding.core.SessionExecutor
import app.getvela.wallet.feature.onboarding.core.assertionRequest
import app.getvela.wallet.feature.onboarding.core.presentedKeyPath
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.TrustedSigner
import app.getvela.wallet.feature.send.core.TrustedSignerIntent
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.send.core.UserOpSpine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.WalletKeyRecord
import app.getvela.wallet.feature.signing.trustedsigner.SigningPlan
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureMultiAddress
import java.util.Base64
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Founder, 2026-09-26: an account signs with the key it was created or signed
 * in with, over the route that reached it — 「这个账户只能用当前登录的钥匙签名」.
 *
 * The parallel space's keyset is a real multi-key Safe, so every signature
 * here is a genuine assertion the core packs; what is under test is WHICH key
 * the ceremony was pinned to and HOW it was reached — and that the record
 * naming it survives every write Android makes.
 */
class SignInKeySigningTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val keyset = fixtureAccounts()
    private val safe = fixtureMultiAddress()
    private val digest = ByteArray(32) { 7 }

    @After
    fun stop() = scope.cancel()

    /** The record as the create / login machines write it. */
    private fun record(signedInWith: JSONObject?, address: String = safe, firstTransports: String = "internal"): JSONObject {
        val keys = JSONArray()
        keyset.forEachIndexed { index, key ->
            keys.put(
                JSONObject().put("credential_id", key.credentialIdHex).put("public_key_hex", key.publicKeyHex)
                    .put("name", key.name).put("transports", if (index == 0) firstTransports else "internal"),
            )
        }
        return JSONObject().put("id", keyset.first().credentialIdHex).put("name", "Parallel space").put("address", address)
            .put("public_key_hex", keyset.first().publicKeyHex).put("created_at_iso", "2026-09-26T00:00:00Z").put("keys", keys)
            .apply { if (signedInWith != null) put("signed_in_with", signedInWith) }
    }

    private fun signedIn(index: Int, method: String) =
        JSONObject().put("credential_id", keyset[index].credentialIdHex).put("method", method)

    private fun storeWith(record: JSONObject) = FakeStore(mapOf("vela.accounts" to JSONArray().put(record).toString()))

    private data class Ceremony(val credentialId: String?, val transports: String, val method: KeyMethod, val signedBy: String)

    private val ceremonies = CopyOnWriteArrayList<Ceremony>()

    private val passkey = object : UserOpSigner {
        override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
            val signed = fixtureAssert(challenge, listOfNotNull(credentialIdHex), 0u)
            ceremonies += Ceremony(credentialIdHex, transports, method, signed.credentialIdHex)
            return Assertion(signed.credentialIdHex, signed.signatureDerHex, signed.authenticatorDataHex, signed.clientDataJsonHex, null, "platform")
        }
    }

    private fun spine(store: FakeStore, page: TrustedSigner? = null) = UserOpSpine(
        relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
        accounts = StoreAccountPort(AccountStore(store)),
        signer = { passkey },
        trustedSigner = { page },
    )

    @Test
    fun `an account signed in with its second key over a security key signs with that key, there`() = runBlocking<Unit> {
        val store = storeWith(record(signedIn(1, "security_key")))
        val signature = spine(store).signMessage(100, safe, digest)
        val second = keyset[1].credentialIdHex
        assertEquals(listOf(Ceremony(second, "usb,nfc,ble", KeyMethod.SecurityKey, second)), ceremonies)
        assertTrue("an EIP-1271 envelope: $signature", signature.startsWith("0x") && signature.length > 66)
    }

    /**
     * "This device" was chosen at sign-in and a phone answered (a cross-platform
     * attachment). The route names both, the ceremony carries that list as it
     * is, and it goes to Credential Manager with every hint — not to the
     * app-owned USB path, which would ask a phone passkey to be plugged in.
     */
    @Test
    fun `a platform sign-in answered from elsewhere reaches the key wherever the sign-in found it`() = runBlocking<Unit> {
        val first = keyset[0].credentialIdHex
        val store = storeWith(record(signedIn(0, "platform").put("transports", "usb,nfc,ble,hybrid")))
        spine(store).signMessage(100, safe, digest)
        val ceremony = ceremonies.single()
        assertEquals(Ceremony(first, "internal,usb,nfc,ble,hybrid", KeyMethod.Platform, first), ceremony)

        assertFalse("Credential Manager, not the app-owned USB path", presentedKeyPath(ceremony.transports, ceremony.method))
        val allow = assertionRequest(digest, "getvela.app", ceremony.credentialId, ceremony.transports)
            .getJSONArray("allowCredentials").getJSONObject(0)
        val hints = allow.getJSONArray("transports").let { list -> (0 until list.length()).map(list::getString) }
        assertEquals(listOf("internal", "usb", "nfc", "ble", "hybrid"), hints)
    }

    /**
     * A record from before the sign-in key goes down the path it always did,
     * whatever its first key reported: the old rule is written out here as
     * the reference (any `usb`/`nfc`/`ble` token, or a `usb`/`nfc` method).
     */
    @Test
    fun `a record from before the sign-in key takes the path it always took`() = runBlocking<Unit> {
        fun oldPath(transports: String): Boolean {
            val tokens = transports.split(',').map { it.trim() }
            val removable = tokens.any { it == "usb" || it == "nfc" || it == "ble" }
            return removable || transports.contains("usb") || transports.contains("nfc")
        }
        listOf("", "internal", "hybrid,internal", "usb,nfc", "ble,nfc,usb", "ble", "internal,ble", "hybrid,internal,ble", "cable,internal", "internal,usb").forEach { transports ->
            val port = StoreAccountPort(AccountStore(storeWith(record(null, firstTransports = transports))))
            val (routed, method) = port.routingOf(safe)
            assertEquals(transports, routed)
            assertEquals("path for \"$transports\"", oldPath(transports), presentedKeyPath(routed, method))
        }
    }

    /** Records written before 2026-09-26 sign exactly as they always did: the first key over its stored route. */
    @Test
    fun `a record from before the sign-in key signs exactly as it always did`() = runBlocking<Unit> {
        val first = keyset[0].credentialIdHex
        spine(storeWith(record(null))).signMessage(100, safe, digest)
        spine(storeWith(record(null, firstTransports = "usb,nfc"))).signMessage(100, safe, digest)
        assertEquals(
            listOf(
                Ceremony(first, "internal", KeyMethod.Platform, first),
                Ceremony(first, "usb,nfc", KeyMethod.SecurityKey, first),
            ),
            ceremonies,
        )
        val legacy = spine(storeWith(record(null))).routeFor(safe, WalletKeyRecord(first, keyset[0].publicKeyHex))
        assertEquals(false, legacy.signInKey)
    }

    /** A sign-in key this wallet does not hold is no route at all: the record signs as an old one. */
    @Test
    fun `a sign-in key the wallet does not hold falls back to the old route`() = runBlocking<Unit> {
        val stranger = JSONObject().put("credential_id", "ab".repeat(16)).put("method", "security_key")
        spine(storeWith(record(stranger))).signMessage(100, safe, digest)
        val first = keyset[0].credentialIdHex
        assertEquals(listOf(Ceremony(first, "internal", KeyMethod.Platform, first)), ceremonies)
    }

    /** What a fake page was asked: the request, the keys it may use, the page, the card's key and whether it asked first. */
    private data class PageAsk(val request: JSONObject, val keys: List<String>, val page: String, val key: String, val askFirst: Boolean)

    private fun pageRecording(asked: MutableList<PageAsk>) = object : TrustedSigner {
        override suspend fun sign(requestJson: String, digest: ByteArray, keys: List<WalletKeyRecord>, page: String, key: String, askFirst: Boolean): Assertion {
            asked += PageAsk(JSONObject(requestJson), keys.map { it.credentialId }, page, key, askFirst)
            val signed = fixtureAssert(digest, keys.map { it.credentialId }, 0u)
            return Assertion(signed.credentialIdHex, signed.signatureDerHex, signed.authenticatorDataHex, signed.clientDataJsonHex, null, "")
        }
    }

    /**
     * Spec 102 migration: an account a ≤ 0.9.7 build signed in through a
     * self-hosted page (`signed_in_with.method = trusted_signer`, a custom
     * origin) is now a wallet on that domain, locked to that page — it signs
     * there, allowing that key alone, and the page is told which key and where
     * it lives (R5).
     */
    @Test
    fun `an account from a self-hosted page signs on that page, allowing that key alone, with its route`() = runBlocking<Unit> {
        val second = keyset[1].credentialIdHex
        val store = storeWith(record(signedIn(1, "trusted_signer").put("signer_origin", "https://my.signer.test")))
        val asked = CopyOnWriteArrayList<PageAsk>()
        spine(store, pageRecording(asked)).signMessage(
            100, safe, digest,
            intent = TrustedSignerIntent("personal_sign", JSONArray().put("0x68656c6c6f").put(safe).toString(), "https://app.test"),
        )
        assertTrue("no platform ceremony: $ceremonies", ceremonies.isEmpty())
        val ask = asked.single()
        assertEquals("https://my.signer.test/", ask.page)
        assertEquals(listOf(second), ask.keys)
        // A send's spine raises the hand-off card itself: no sheet drew one.
        assertTrue(ask.askFirst)
        val context = ask.request.getJSONObject("context")
        val allow = context.getJSONArray("allowCredentials")
        assertEquals(1, allow.length())
        val expected = Base64.getUrlEncoder().withoutPadding().encodeToString(second.chunked(2).map { it.toInt(16).toByte() }.toByteArray())
        assertEquals(expected, allow.getString(0))
        val route = context.getJSONObject("keyRoute")
        assertEquals(expected, route.getString("credentialId"))
    }

    /**
     * Spec 102 R4: a `getvela.app` account whose venue is the official page
     * signs there — never natively — and the dApp sheet's spine (which drew
     * the hand-off card itself) does not ask for a second card.
     */
    @Test
    fun `an account whose venue is the official page signs there, and a sheet that drew the card is not asked again`() = runBlocking<Unit> {
        val first = keyset[0].credentialIdHex
        val record = record(signedIn(0, "platform").put("transports", "internal"))
            .put("signing_domain", "getvela.app")
            .put("signing_venue", JSONObject().put("type", "page").put("url", "https://sign.getvela.app/"))
        val asked = CopyOnWriteArrayList<PageAsk>()
        val sheet = UserOpSpine(
            relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            accounts = StoreAccountPort(AccountStore(storeWith(record))),
            signer = { passkey },
            trustedSigner = { pageRecording(asked) },
            handoffShown = true,
        )
        sheet.signMessage(
            100, safe, digest,
            intent = TrustedSignerIntent("personal_sign", JSONArray().put("0x68656c6c6f").put(safe).toString(), "https://app.test"),
        )
        assertTrue("no native ceremony on a page venue: $ceremonies", ceremonies.isEmpty())
        val ask = asked.single()
        assertEquals("https://sign.getvela.app/", ask.page)
        assertEquals(listOf(first), ask.keys)
        assertFalse(ask.askFirst)
        // The card's key is the plan's `key_label` (D-17): the key's own name
        // when it is not the wallet's, else its place's title — never read
        // off the record by the shell.
        val expected = keyset[0].name.trim().takeIf { it.isNotEmpty() && !it.equals("Parallel space", ignoreCase = true) }
            ?: "onboarding.create.methodPlatformTitle"
        assertEquals(expected, ask.key)
    }

    /** R1: a custom-domain account whose page this device does not know signs nothing, and says why. */
    @Test
    fun `an account no venue here can reach signs nothing`() = runBlocking<Unit> {
        val record = record(signedIn(0, "platform"))
            .put("signing_domain", "example.com")
            .put("signing_venue", JSONObject().put("type", "in_vela"))
        val refused = runCatching { spine(storeWith(record), pageRecording(CopyOnWriteArrayList())).signMessage(100, safe, digest) }
            .exceptionOrNull() as UserOpSpine.Refused
        // Told to the core as `venue_blocked` with the block itself — the
        // sheet and the send say why in the person's language (P2b-A10).
        val failure = refused.failure as UserOpSpine.Failure.VenueBlocked
        assertEquals(app.getvela.wallet.feature.signing.trustedsigner.VenueBlock.AppCannotReach("example.com"), failure.block)
        assertEquals("settings.venue.blockedApp", failure.block.key())
        assertTrue("nothing was signed: $ceremonies", ceremonies.isEmpty())
    }

    /**
     * The record naming the key survives Android's writes: a save, a reload,
     * and the re-save of a HELD record after a sign-in with another key (the
     * `save_account` operation both executors answer the same way) — replaced
     * in place, every founding key kept.
     */
    @Test
    fun `the sign-in key survives a save, a reload and the re-save of a held record`() = runBlocking<Unit> {
        val accounts = AccountStore(FakeStore())
        accounts.saveAccount(record(signedIn(0, "platform")))
        assertEquals("platform", accounts.loadAccounts().getJSONObject(0).getJSONObject("signed_in_with").getString("method"))

        SessionExecutor(accounts).perform(
            JSONObject().put("type", "save_account").put("account", record(signedIn(1, "security_key"))),
        )
        val stored = accounts.loadAccounts()
        assertEquals("one wallet, one row", 1, stored.length())
        val held = stored.getJSONObject(0)
        assertEquals(keyset.size, held.getJSONArray("keys").length())
        assertEquals(keyset[1].credentialIdHex, held.getJSONObject("signed_in_with").getString("credential_id"))

        // …and what the spine hands the core is that record, whole: the plan's key route.
        val plan = SigningPlan.of(StoreAccountPort(accounts).accountJson(safe))!!
        assertEquals(keyset[1].credentialIdHex, plan.credentialId)
        assertEquals(KeyMethod.SecurityKey, plan.method)
        assertEquals("usb,nfc,ble", plan.transports)
    }

    /**
     * The real session machine rewrites a record whose stored address is not
     * what its keys compute to (invariant ②). That write-back goes through the
     * same executor and store, and must not strip the sign-in key — the shape
     * of the bug that once stripped `keys`.
     */
    @Test
    fun `the session's own write-back keeps the sign-in key`() = runBlocking<Unit> {
        val wrong = "0x0000000000000000000000000000000000000001"
        val store = FakeStore(
            mapOf(
                "vela.accounts" to JSONArray().put(record(signedIn(1, "hybrid"), address = wrong)).toString(),
                "vela.activeAccountIndex" to "0",
            ),
        )
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        withTimeout(10_000) { session.view.first { !it.loading } }
        val written = withTimeout(10_000) {
            var held = JSONArray(store.values.getValue("vela.accounts")).getJSONObject(0)
            while (!held.optString("address").equals(safe, ignoreCase = true)) {
                delay(20)
                held = JSONArray(store.values.getValue("vela.accounts")).getJSONObject(0)
            }
            held
        }
        assertEquals(keyset.size, written.getJSONArray("keys").length())
        // Spec 102: the key is `sign_in_key`, and older builds still find a
        // `signed_in_with` copy naming its place (D-1).
        val key = written.getJSONObject("sign_in_key")
        assertEquals(keyset[1].credentialIdHex, key.getString("credential_id"))
        assertEquals("hybrid", key.getString("method"))
        assertEquals("hybrid", written.getJSONObject("signed_in_with").getString("method"))
        assertEquals("getvela.app", written.getString("signing_domain"))
    }

    /**
     * Spec 102 (P2-09): "Where you review and sign" goes to the real session
     * machine by address. A venue that reaches the account's keys is saved on
     * the record (the session view carries it); one that cannot is refused by
     * the core and nothing is written.
     */
    @Test
    fun `choosing where you review and sign saves the venue, and an unreachable one changes nothing`() = runBlocking<Unit> {
        val store = FakeStore(
            mapOf(
                "vela.accounts" to JSONArray().put(record(signedIn(0, "platform"))).toString(),
                "vela.activeAccountIndex" to "0",
            ),
        )
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        withTimeout(10_000) { session.view.first { !it.loading && it.hasWallet } }
        assertEquals("""{"type":"in_vela"}""", JSONObject(session.view.value.activeRow!!.signingVenueJson).toString())

        session.chooseSigningVenue(safe, """{"type":"page","url":"https://sign.getvela.app/"}""")
        val chosen = withTimeout(10_000) {
            session.view.first { JSONObject(it.activeRow!!.signingVenueJson).optString("type") == "page" }
        }
        assertEquals("https://sign.getvela.app/", JSONObject(chosen.activeRow!!.signingVenueJson).getString("url"))
        val saved = withTimeout(10_000) {
            var held = JSONArray(store.values.getValue("vela.accounts")).getJSONObject(0)
            while (held.optJSONObject("signing_venue")?.optString("type") != "page") {
                delay(20)
                held = JSONArray(store.values.getValue("vela.accounts")).getJSONObject(0)
            }
            held
        }
        assertEquals("getvela.app", saved.getString("signing_domain"))
        // …and the spine now signs there.
        assertEquals("https://sign.getvela.app/", SigningPlan.of(saved.toString())!!.page)

        // A page on another domain cannot reach this account's keys: refused, nothing written.
        session.chooseSigningVenue(safe, """{"type":"page","url":"https://sign.example.com/"}""")
        delay(300)
        assertEquals("https://sign.getvela.app/", JSONObject(session.view.value.activeRow!!.signingVenueJson).getString("url"))
        assertEquals(
            "https://sign.getvela.app/",
            JSONArray(store.values.getValue("vela.accounts")).getJSONObject(0).getJSONObject("signing_venue").getString("url"),
        )
    }
}
