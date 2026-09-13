package app.getvela.wallet

import app.getvela.wallet.feature.settings.core.DeviceStorage
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DeviceStorageTest {
    private fun store() = FakeStore(
        mapOf(
            "vela.transactionHistory" to """[{"id":1},{"id":2},{"id":3}]""",
            "vela.contacts" to """[{"address":"0x1"}]""",
            "vela.contactGroups" to """[]""",
            "vela.balanceCache" to "x".repeat(600),
            "vela.fiatRates.v1" to """{"USD":1}""",
            "vela.perm.https://app.uniswap.org" to """{"grants":[{"a":1}]}""",
            "vela.accounts" to """[{"address":"0x88"}]""",
            "vela.language" to "zh",
        ),
    )

    @Test
    fun `keys file under the drawn rows, with bytes and records`() = runBlocking<Unit> {
        val report = DeviceStorage.measure(store())
        val tx = report.items.first { it.id == "transactions" }
        assertEquals(3, tx.records)
        assertEquals(listOf("vela.transactionHistory"), tx.keys)
        val contacts = report.items.first { it.id == "contacts" }
        assertEquals(1, contacts.records)
        assertEquals(2, contacts.keys.size)
        val balances = report.items.first { it.id == "balances" }
        assertEquals(600L, balances.bytes)
        assertNull(balances.records)
        assertEquals(1, report.items.first { it.id == "dapps" }.records)
        assertEquals(600L, report.bytesOf(DeviceStorage.Group.Cache) - report.items.first { it.id == "rates" }.bytes)
        assertNull(DeviceStorage.itemOfKey("vela.accounts"))
        assertNull(DeviceStorage.itemOfKey("vela.language"))
    }

    @Test
    fun `clearing removes exactly the item's keys, caches all of theirs`() = runBlocking<Unit> {
        val s = store()
        assertTrue(DeviceStorage.clear(s, "contacts"))
        assertTrue("vela.contacts" !in s.values && "vela.contactGroups" !in s.values)
        assertTrue("vela.transactionHistory" in s.values)
        assertTrue(DeviceStorage.clearCaches(s))
        assertTrue("vela.balanceCache" !in s.values && "vela.fiatRates.v1" !in s.values)
        assertTrue("vela.transactionHistory" in s.values && "vela.accounts" in s.values)
    }

    @Test
    fun `erase sweeps everything but the keep-list and says what is left`() = runBlocking<Unit> {
        val s = store()
        val left = DeviceStorage.erase(s, keep = setOf("vela.accounts"))
        assertTrue(left.isEmpty())
        assertEquals(setOf("vela.accounts"), s.values.keys)
        val stubborn = store().apply { refuseWrites = true }
        val leftovers = DeviceStorage.erase(stubborn, keep = emptySet())
        assertEquals(8, leftovers.size)
    }
}
