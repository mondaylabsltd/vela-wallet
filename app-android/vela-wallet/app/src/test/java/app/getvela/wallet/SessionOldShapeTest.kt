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
