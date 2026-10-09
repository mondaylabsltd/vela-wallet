package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.SessionController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Leaving the parallel space puts the person back on the wallet they were on
 * (device pass 2026-10-09: it always landed on the first account). Through the
 * real `session` machine, the real executor and an in-memory store: the space
 * appends its record (`addAccount`, which makes it active), and the exit drops
 * it and hands the list back (`removeFixtureAccount`).
 */
class ParallelSpaceExitTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun tearDown() = scope.cancel()

    private val ann = JSONObject(
        """{"id":"cred-1","name":"Ann","address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        "public_key_hex":"04ab","created_at_iso":"2026-08-25T10:00:00.000Z",
        "keys":[{"credential_id":"cred-1","public_key_hex":"04ab","name":"Ann"}]}""",
    )
    private val bo = JSONObject(
        """{"id":"cred-2","name":"Bo","address":"0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984",
        "public_key_hex":"04cd","created_at_iso":"2026-08-26T10:00:00.000Z",
        "keys":[{"credential_id":"cred-2","public_key_hex":"04cd","name":"Bo"}]}""",
    )
    private val fixture = JSONObject(
        """{"id":"cred-fixture","name":"Parallel space","address":"0x6B175474E89094C44Da98b954EedeAC495271d0F",
        "public_key_hex":"04ef","created_at_iso":"2026-10-09T10:00:00.000Z",
        "keys":[{"credential_id":"cred-fixture","public_key_hex":"04ef","name":"Parallel space","transports":"internal"}]}""",
    )
    private val annAddress = ann.getString("address")
    private val boAddress = bo.getString("address")
    private val fixtureAddress = fixture.getString("address")

    /** A device holding Ann and Bo, Bo in front, inside the space. */
    private suspend fun insideTheSpace(): Pair<SessionController, FakeStore> {
        val store = FakeStore(mapOf("vela.accounts" to JSONArray().put(ann).put(bo).toString(), "vela.activeAccountIndex" to "1"))
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        val before = withTimeout(10_000) { session.view.first { !it.loading && it.accounts.size == 2 } }
        assertEquals(boAddress, before.address)
        session.addAccount(fixture)
        withTimeout(10_000) { session.view.first { it.address.equals(fixtureAddress, ignoreCase = true) } }
        return session to store
    }

    @Test
    fun `leaving goes back to the wallet that was in front before the space`() = runBlocking {
        val (session, store) = insideTheSpace()

        session.removeFixtureAccount("cred-fixture", returnTo = boAddress)

        val after = withTimeout(10_000) { session.view.first { it.accounts.size == 2 } }
        assertEquals("Bo was in front before the space", boAddress, after.address)
        assertEquals(1, after.activeIndex)
        assertEquals("1", store.values["vela.activeAccountIndex"])
        assertEquals(listOf(annAddress, boAddress), after.accounts.map { it.address })
    }

    @Test
    fun `a wallet switched to inside the space stays in front`() = runBlocking {
        val (session, _) = insideTheSpace()
        session.switchAccount(0)
        withTimeout(10_000) { session.view.first { it.address == annAddress } }

        session.removeFixtureAccount("cred-fixture", returnTo = boAddress)

        val after = withTimeout(10_000) { session.view.first { it.accounts.size == 2 } }
        assertEquals("the person moved to Ann inside the space", annAddress, after.address)
        assertEquals(0, after.activeIndex)
    }

    @Test
    fun `with nothing to go back to, the first wallet is in front`() = runBlocking {
        val (session, _) = insideTheSpace()

        session.removeFixtureAccount("cred-fixture", returnTo = null)

        val after = withTimeout(10_000) { session.view.first { it.accounts.size == 2 } }
        assertEquals(annAddress, after.address)
        assertEquals(0, after.activeIndex)
    }

    @Test
    fun `the index is found by address, whatever its case, and absent reads as 0`() {
        val list = JSONArray().put(ann).put(bo)
        assertEquals(1, SessionController.indexOfAddress(list, boAddress.lowercase()))
        assertEquals(0, SessionController.indexOfAddress(list, annAddress))
        assertEquals(0, SessionController.indexOfAddress(list, fixtureAddress))
        assertEquals(0, SessionController.indexOfAddress(list, null))
        assertEquals(0, SessionController.indexOfAddress(list, ""))
    }
}
