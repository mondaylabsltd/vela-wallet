package app.getvela.wallet

import app.getvela.wallet.feature.wallet.core.PayLink
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The deep-link shapes, tokenized only (spec 047 D8); the core validates the query. */
class PayLinkTest {
    private val founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"

    @Test
    fun `the scheme and the two hosts carry the pay query`() {
        val a = PayLink.parse("velawallet://pay?to=$founder&chain=100&amount=0.001") as PayLink.Pay
        assertEquals(founder, a.to); assertEquals("100", a.chain); assertEquals("0.001", a.amount); assertNull(a.token)
        val b = PayLink.parse("https://wallet.getvela.app/pay?to=$founder&chain=1&token=0xabc&sym=USDC&dec=6&net=Ethereum") as PayLink.Pay
        assertEquals("0xabc", b.token); assertEquals("USDC", b.sym); assertEquals("6", b.dec); assertEquals("Ethereum", b.net)
        val c = PayLink.parse("https://getvela.app/pay/?to=$founder") as PayLink.Pay
        assertEquals(founder, c.to)
        val encoded = PayLink.parse("velawallet://pay?to=$founder&net=Gnosis%20Chain") as PayLink.Pay
        assertEquals("Gnosis Chain", encoded.net)
    }

    @Test
    fun `open names a page, and everything else is nothing`() {
        assertEquals(PayLink.Open("https://app.uniswap.org/"), PayLink.parse("velawallet://open?url=https%3A%2F%2Fapp.uniswap.org%2F"))
        assertNull(PayLink.parse("velawallet://open?url=javascript:alert(1)"))
        assertNull(PayLink.parse("https://getvela.app/"))
        assertNull(PayLink.parse("https://evil.example/pay?to=$founder"))
        assertNull(PayLink.parse("velawallet://settings"))
        assertNull(PayLink.parse(null))
        assertNull(PayLink.parse("not a uri at all ^^"))
    }
}
