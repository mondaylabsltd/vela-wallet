package app.getvela.wallet

import app.getvela.wallet.core.net.NetHealth
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** Spec 047 FR-006: offline is three calls that never reached a server; one answered call is online again. */
class NetHealthTest {
    @Before fun fresh() = NetHealth.reset()
    @After fun clean() = NetHealth.reset()

    @Test
    fun `two misses are a blip, the third is offline, one answer is back`() {
        NetHealth.unreached(); NetHealth.unreached()
        assertTrue(NetHealth.online.value)
        NetHealth.unreached()
        assertFalse(NetHealth.online.value)
        NetHealth.reached()
        assertTrue(NetHealth.online.value)
    }

    @Test
    fun `an answer in between resets the count`() {
        NetHealth.unreached(); NetHealth.unreached(); NetHealth.reached(); NetHealth.unreached(); NetHealth.unreached()
        assertTrue(NetHealth.online.value)
    }
}
