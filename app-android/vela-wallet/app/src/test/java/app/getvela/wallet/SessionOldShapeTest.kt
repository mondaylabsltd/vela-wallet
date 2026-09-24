package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.SessionController
import app.getvela.wallet.feature.onboarding.core.SessionRoute
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 048, on the real `session` machine through the host dylib: the retired
 * client's account spelling restores a session, and an unreadable list settles
 * on onboarding instead of `loading` forever (the refused answer is re-answered
 * as `accounts_unavailable` by the driver).
 */
class SessionOldShapeTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun `the retired client's records still restore the session`() = runBlocking {
        val old = """[{"id":"cred-1","name":"Ann","address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            "publicKeyHex":"04ab","createdAt":"2026-08-25T10:00:00.000Z",
            "keys":[{"credentialId":"cred-1","publicKeyHex":"04ab","name":"Ann"}]}]"""
        val store = FakeStore(mapOf("vela.accounts" to old, "vela.activeAccountIndex" to "0"))
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        val settled = withTimeout(10_000) { session.view.first { !it.loading } }
        assertTrue(settled.hasWallet)
        assertEquals(1, settled.accounts.size)
        assertEquals("0x88cCA0EeDbF2C4426110bbFc998F048689266894", settled.accounts[0].address)
        assertEquals(SessionRoute.Wallet, settled.allowedRoute)
    }

    /**
     * One wallet leaves this device and the other stays — through the real
     * machine, the real executor and a real store (2026-09-23:
     * 「有时候不想退出所有，只想退出单个」).
     *
     * The interesting half is the STORE: the core names the row by address,
     * and what is written back must be the other record, whole.
     */
    @Test
    fun `one account can be removed while the other stays`() = runBlocking {
        val ann = """{"id":"cred-1","name":"Ann","address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            "publicKeyHex":"04ab","createdAt":"2026-08-25T10:00:00.000Z",
            "keys":[{"credentialId":"cred-1","publicKeyHex":"04ab","name":"Ann"}]}"""
        val bo = """{"id":"cred-2","name":"Bo","address":"0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984",
            "publicKeyHex":"04cd","createdAt":"2026-08-26T10:00:00.000Z",
            "keys":[{"credentialId":"cred-2","publicKeyHex":"04cd","name":"Bo"}]}"""
        val store = FakeStore(
            mapOf("vela.accounts" to "[$ann,$bo]", "vela.activeAccountIndex" to "0"),
        )
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        withTimeout(10_000) { session.view.first { !it.loading && it.accounts.size == 2 } }

        session.removeAccount(0)
        val after = withTimeout(10_000) { session.view.first { it.accounts.size == 1 } }
        assertTrue("the device is still signed in", after.hasWallet)
        assertEquals("0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984", after.address)
        assertEquals(SessionRoute.Wallet, after.allowedRoute)

        // And the list on disk is the other record, not an emptied one.
        val written = withTimeout(10_000) {
            var raw = store.read("vela.accounts").orEmpty()
            while (raw.contains("cred-1")) {
                kotlinx.coroutines.delay(50)
                raw = store.read("vela.accounts").orEmpty()
            }
            raw
        }
        assertTrue("the other wallet survived the write: $written", written.contains("cred-2"))
    }

    @Test
    fun `an unreadable list settles on onboarding, never loading forever`() = runBlocking {
        val store = FakeStore(mapOf("vela.accounts" to """[{"id":"x","address":"0x1","keys":[{"credential_id":7}]}]"""))
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        val settled = withTimeout(10_000) { session.view.first { !it.loading } }
        assertFalse(settled.hasWallet)
        assertEquals(SessionRoute.Onboarding, settled.allowedRoute)
    }
}
