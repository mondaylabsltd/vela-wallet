package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.signing.core.SignExecutor
import app.getvela.wallet.feature.signing.core.SignOperation
import app.getvela.wallet.feature.signing.core.SignShellResult
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * `SwitchActiveAccount` moves the person's account only to the request's own
 * signer (the web's `sign-resident` rule). The acknowledgement is what opens
 * the approval surface, so a switch to anybody else is never acknowledged —
 * and nothing moves.
 */
class SignSwitchGuardTest {

    private val alice = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val bob = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"

    private class Ports(
        val rows: List<String>,
        val intended: String?,
        val switches: Boolean = true,
    ) : SignExecutor.Ports {
        val switched = mutableListOf<String>()
        override fun respond(transportId: String, id: String, json: JSONObject) = Unit
        override fun opSubmitted(id: String, userOpHash: String) = Unit
        override fun signingStarted() = Unit
        override fun recordsPersisted() = Unit
        override fun recordPersisted(recordId: String) = Unit
        override suspend fun switchAccount(address: String): Boolean {
            switched += address
            return switches
        }
        override fun signerAt(index: Int): String? = rows.getOrNull(index)
        override fun intendedSigner(): String? = intended
        override fun nativeSymbol(chainId: Int) = "XDAI"
    }

    private fun executor(ports: Ports): SignExecutor {
        val store = FakeStore()
        val relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0)
        return SignExecutor(
            spine = UserOpSpine(relay, StoreAccountPort(AccountStore(store)), { error("no signing in this test") }),
            relay = relay,
            feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
            ports = ports,
        )
    }

    /** The answer, or `null` when the executor withheld it. */
    private fun answer(ports: Ports, index: Int): SignShellResult? = runBlocking {
        withTimeoutOrNull(300) { executor(ports).perform(SignOperation.SwitchActiveAccount(index)) }
    }

    @Test
    fun `the request's own signer is switched to and acknowledged`() {
        val ports = Ports(rows = listOf(alice, bob), intended = bob.lowercase())
        assertEquals(SignShellResult.AccountSwitched, answer(ports, 1))
        assertEquals(listOf(bob), ports.switched)
    }

    @Test
    fun `a different account is never switched to, and never acknowledged`() {
        val ports = Ports(rows = listOf(alice, bob), intended = alice)
        assertNull(answer(ports, 1))
        assertEquals(emptyList<String>(), ports.switched)
    }

    @Test
    fun `an index with no row moves nothing`() {
        val ports = Ports(rows = listOf(alice), intended = null)
        assertNull(answer(ports, 3))
        assertEquals(emptyList<String>(), ports.switched)
    }

    @Test
    fun `a switch the session did not make is not acknowledged`() {
        val ports = Ports(rows = listOf(alice, bob), intended = bob, switches = false)
        assertNull(answer(ports, 1))
    }
}
