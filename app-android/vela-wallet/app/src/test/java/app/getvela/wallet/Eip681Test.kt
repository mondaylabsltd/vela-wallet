package app.getvela.wallet

import app.getvela.wallet.feature.scan.Eip681
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The web's shapes read back, the desktop's two refusals, and nonsense (spec 046 D5). */
class Eip681Test {
    private val me = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val token = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"

    @Test
    fun `the shapes this app writes read back`() {
        val native = Eip681.parse("ethereum:$me@100?value=1000000000000000")!!
        assertEquals(100L, native.chainId)
        assertEquals(me, native.recipient)
        assertTrue(native.isNative)
        assertNull(native.tokenAddress)
        assertEquals("1000000000000000", native.amountBaseUnits)

        val erc20 = Eip681.parse("ethereum:$token@100/transfer?address=$me&uint256=2500000")!!
        assertEquals(token, erc20.tokenAddress)
        assertEquals(me, erc20.recipient)
        assertEquals("2500000", erc20.amountBaseUnits)
        assertTrue(!erc20.isNative)

        val bare = Eip681.parse("ethereum:$me")!!
        assertNull(bare.chainId)
        assertNull(bare.amountBaseUnits)
        val exp = Eip681.parse("ethereum:pay-$me@1?value=2.5e18")!!
        assertEquals("2500000000000000000", exp.amountBaseUnits)
    }

    @Test
    fun `a call that is not a payment is refused`() {
        assertNull(Eip681.parse("ethereum:$token@1/approve?address=$me&uint256=1"))
        val transfer = Eip681.parse("ethereum:$token@1/transfer?address=$me&value=1000000000000000000")!!
        assertNull("value is ether attached to the call, not the token amount", transfer.amountBaseUnits)
    }

    @Test
    fun `nonsense is not a request and a negative amount carries nothing`() {
        assertNull(Eip681.parse(""))
        assertNull(Eip681.parse(null))
        assertNull(Eip681.parse("https://getvela.app"))
        assertNull(Eip681.parse(me))
        assertNull(Eip681.parse("ethereum:"))
        assertNull(Eip681.parse("ethereum:notanaddress@1"))
        val negative = Eip681.parse("ethereum:$me@1?value=-5")!!
        assertNull(negative.amountBaseUnits)
    }

    @Test
    fun `a percent-encoded query survives and the tolerant forms are accepted`() {
        val encoded = Eip681.parse("ethereum:$token@100/transfer?address=${me.replace("0x", "0x")}&uint256=1%30")!!
        assertEquals("10", encoded.amountBaseUnits)
        assertEquals(mapOf("a" to "1", "b" to "", "c d" to "x y"), Eip681.parseQuery("a=1&b&c%20d=x%20y"))
        assertEquals("1000", Eip681.parseAmount("1e3"))
        assertEquals("1", Eip681.parseAmount("1.9"))
        assertEquals("1500", Eip681.parseAmount("1.5e3"))
        assertNull(Eip681.parseAmount("abc"))
        assertNull(Eip681.parseAmount("1e99999"))
    }
}
