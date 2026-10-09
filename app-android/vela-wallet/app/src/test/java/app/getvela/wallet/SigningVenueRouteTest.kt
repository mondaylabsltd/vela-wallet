package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSpine
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.WalletKeyRecord

/**
 * Spec 102 R4: where an account's signatures go is its signing plan — the
 * venue the core settles from the stored record — never a fourth key method.
 *
 * Two facts, and the seam between them:
 *
 * - the account record round-trips `signer_origin` — older builds still route
 *   by it (D-5: old builds keep working), so a rewrite that dropped it would
 *   send a ≤ 0.9.7 build to a sheet that cannot reach a custom domain's key;
 * - `UserOpSpine.routeFor` reads the core's `signing_plan`: a page venue goes
 *   to that page with the key route, Vela's sheet signs natively, and a
 *   record written before the sign-in key was kept signs as it always did.
 */
class SigningVenueRouteTest {

    private val address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val credential = "aabbcc"
    private val publicKey = "04" + "11".repeat(64)
    private val origin = "https://sign.example.test"

    private fun record(signerOrigin: String?, venue: String? = null, signInKey: Boolean = false): JSONObject = JSONObject()
        .put("id", credential)
        .put("name", "Savings")
        .put("created_at_iso", "2026-09-30T10:00:00.000Z")
        .put("address", address)
        .put("public_key_hex", publicKey)
        .put(
            "keys",
            JSONArray().put(
                JSONObject()
                    .put("credential_id", credential)
                    .put("public_key_hex", publicKey)
                    .put("name", "Key 1")
                    .put("transports", "internal")
                    .apply { if (signerOrigin != null) put("signer_origin", signerOrigin) },
            ),
        )
        .apply {
            if (signInKey) put("sign_in_key", JSONObject().put("credential_id", credential).put("method", "platform").put("transports", "internal"))
            if (venue != null) put("signing_domain", "getvela.app").put("signing_venue", JSONObject(venue))
        }

    private fun portWith(record: JSONObject): StoreAccountPort {
        val accounts = AccountStore(FakeStore())
        runBlocking { accounts.saveAccount(record) }
        return StoreAccountPort(accounts)
    }

    private fun spine(port: StoreAccountPort) = UserOpSpine(
        relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
        accounts = port,
        signer = { error("no ceremony in this test") },
    )

    private fun routeOf(record: JSONObject) = runBlocking {
        spine(portWith(record)).routeFor(address, WalletKeyRecord(credential, publicKey))
    }

    @Test
    fun `an account record keeps signer_origin through a save and a reload`() {
        val accounts = AccountStore(FakeStore())
        runBlocking {
            accounts.saveAccount(record(origin))
            // The same id again: an upsert REPLACES, and a replacement that
            // reshaped the record would lose the field silently.
            accounts.saveAccount(record(origin))
            val reloaded = accounts.loadAccounts()
            assertEquals(1, reloaded.length())
            val key = reloaded.getJSONObject(0).getJSONArray("keys").getJSONObject(0)
            assertEquals(origin, key.getString("signer_origin"))
        }
    }

    @Test
    fun `a record without a sign-in key whose key lives behind a custom page signs on that page`() {
        val route = routeOf(record(origin))
        assertEquals("$origin/", route.page)
        assertNull(route.blocked)
        assertEquals(credential, route.credentialId)
        // No key route: the old record's place, from its stored transports.
        assertEquals(KeyMethod.Platform, route.method)
        assertNull(route.keyRouteJson)
    }

    @Test
    fun `an ordinary key signs in Vela, as it always did`() {
        val route = routeOf(record(null))
        assertNull(route.page)
        assertEquals(KeyMethod.Platform, route.method)
    }

    @Test
    fun `a page venue goes to that page with the key route, and the hand-off card names the key`() {
        val withPage = record(null, venue = """{"type":"page","url":"https://sign.getvela.app/"}""", signInKey = true)
        val route = routeOf(withPage)
        assertEquals("https://sign.getvela.app/", route.page)
        assertTrue(route.signInKey)
        assertEquals(credential, route.credentialId)
        assertTrue("R5: the key route rides to the page", route.keyRouteJson!!.contains("\"credential_id\":\"aabbcc\""))

        val handoff = runBlocking { spine(portWith(withPage)).handoffFor(address) }!!
        assertEquals("https://sign.getvela.app/", handoff.page)
        // The key row, untranslated until drawn: "Confirm with | Key 1".
        assertEquals("Key 1", handoff.key!!.name)
        assertEquals("componentsUi.signing.confirmWithLabel", handoff.key!!.labelKey)

        // The same account in Vela: no card.
        val inVela = record(null, venue = """{"type":"in_vela"}""", signInKey = true)
        assertNull(routeOf(inVela).page)
        assertNull(runBlocking { spine(portWith(inVela)).handoffFor(address) })
    }
}
