package app.getvela.wallet

import app.getvela.wallet.core.marks.Marks
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** The web's logo rules read back (spec 047): the coin's chain, the hidden badge, checksummed then lowercase. */
class MarksTest {
    @Before fun base() { Marks.base = "https://data.example/" }
    @After fun reset() { Marks.base = "" }

    @Test
    fun `a native coin is its coin's chain, and the badge hides when it would repeat`() {
        val ethOnEthereum = Marks.tokenMark(1, "ETH", null)
        assertEquals(listOf("https://data.example/chainlogos/eip155-1.png"), ethOnEthereum.logoUrls)
        assertTrue(ethOnEthereum.badgeHidden); assertNull(ethOnEthereum.badgeLogoUrl)
        val ethOnBase = Marks.tokenMark(8453, "ETH", null)
        assertEquals(listOf("https://data.example/chainlogos/eip155-1.png"), ethOnBase.logoUrls)
        assertEquals("https://data.example/chainlogos/eip155-8453.png", ethOnBase.badgeLogoUrl)
        assertTrue(!ethOnBase.badgeHidden)
        assertTrue(Marks.tokenMark(100, "XDAI", null).badgeHidden)
        assertTrue(Marks.tokenMark(137, "POL", null).badgeHidden)
    }

    @Test
    fun `a token's logo is the asset entry, checksummed first, and its badge is its chain`() {
        val usdc = Marks.tokenMark(100, "USDC", "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83")
        assertEquals(2, usdc.logoUrls.size)
        assertTrue(usdc.logoUrls[0].startsWith("https://data.example/assets/eip155-100/0x"))
        assertTrue(usdc.logoUrls[0] != usdc.logoUrls[1] && usdc.logoUrls[1].endsWith("0xddafbb505ad214d7b80b1f830fccc89b60fb7a83/logo.png"))
        assertEquals("https://data.example/chainlogos/eip155-100.png", usdc.badgeLogoUrl)
        assertTrue(!usdc.badgeHidden)
        assertTrue(Marks.tokenLogoUrls(1, "X", "not-an-address").isEmpty())
        val named = Marks.tokenMark(1, "USDT", "0xdAC17F958D2ee523a2206206994597C13D831ec7", listOf("https://cdn/usdt.png", ""))
        assertEquals("https://cdn/usdt.png", named.logoUrls.first())
    }

    @Test
    fun `no endpoint means glyphs only`() {
        Marks.base = ""
        assertTrue(Marks.tokenMark(1, "ETH", null).logoUrls.isEmpty())
        assertTrue(Marks.chainMark(1).logoUrls.isEmpty())
        assertNull(Marks.chainLogoUrl(1))
    }
}
