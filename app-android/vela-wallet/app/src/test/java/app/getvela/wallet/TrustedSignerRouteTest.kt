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
import org.junit.Test
import uniffi.vela_core_uniffi.WalletKeyRecord

/**
 * Spec 075: a key that lives behind a Trusted Signer page signs through it.
 *
 * Two facts, and the seam between them:
 *
 * - the account record round-trips `signer_origin` — a rewrite that dropped it
 *   would make the key forget where it lives, and the wallet would ask the
 *   system's passkey sheet for a credential no provider on this phone holds;
 * - for a record that names no sign-in key (written before 2026-09-26),
 *   `UserOpSpine.routeFor` asks the core's `signRoute` for `auto`, so such a
 *   key is routed to its own page rather than to a sheet that has never heard
 *   of it.
 */
class TrustedSignerRouteTest {

    private val address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val credential = "aabbcc"
    private val publicKey = "04" + "11".repeat(64)
    private val origin = "https://sign.example.test"

    private fun record(signerOrigin: String?): JSONObject = JSONObject()
        .put("id", credential)
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

    private fun portWith(signerOrigin: String?): StoreAccountPort {
        val store = FakeStore()
        val accounts = AccountStore(store)
        runBlocking { accounts.saveAccount(record(signerOrigin)) }
        return StoreAccountPort(accounts)
    }

    private fun spine(port: StoreAccountPort) = UserOpSpine(
        relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
        accounts = port,
        signer = { error("no ceremony in this test") },
    )

    @Test
    fun `an account record keeps signer_origin through a save and a reload`() {
        val store = FakeStore()
        val accounts = AccountStore(store)
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
    fun `the routes the core is given carry signer_origin`() {
        val routes = runBlocking { JSONArray(portWith(origin).keyRoutesJson(address)) }
        assertEquals(1, routes.length())
        assertEquals(origin, routes.getJSONObject(0).getString("signer_origin"))
        // A key that lives nowhere in particular carries an empty one, which
        // the core reads as "not behind a page".
        val plain = runBlocking { JSONArray(portWith(null).keyRoutesJson(address)) }
        assertEquals("", plain.getJSONObject(0).getString("signer_origin"))
    }

    @Test
    fun `a record without a sign-in key follows a key that lives behind a page, to that page`() {
        val port = portWith(origin)
        val route = runBlocking {
            spine(port).routeFor(address, WalletKeyRecord(credential, publicKey))
        }
        assertEquals(KeyMethod.TrustedSigner, route.method)
        assertEquals(origin, route.signerOrigin)
        assertEquals(credential, route.credentialId)
    }

    @Test
    fun `a record without a sign-in key leaves an ordinary key on the route it always had`() {
        val port = portWith(null)
        val route = runBlocking {
            spine(port).routeFor(address, WalletKeyRecord(credential, publicKey))
        }
        assertEquals(KeyMethod.Platform, route.method)
        assertEquals("", route.signerOrigin)
    }
}
