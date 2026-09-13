package app.getvela.wallet

import app.getvela.wallet.core.identicon.initialsLetter
import org.junit.Assert.assertEquals
import org.junit.Test

/** Spec 049: the initials style's letter rule (the web's `initialsSvg`, plus the address guard). */
class IdenticonInitialsTest {
    @Test
    fun `the first letter of a name, upper-cased`() {
        assertEquals("A", initialsLetter("alice"))
        assertEquals("觉", initialsLetter("觉得九点半"))
        assertEquals("P", initialsLetter("  Parallel One"))
        assertEquals("😀", initialsLetter("😀 team"))
    }

    @Test
    fun `no name, or an address standing in for one, is V`() {
        assertEquals("V", initialsLetter(null))
        assertEquals("V", initialsLetter(""))
        assertEquals("V", initialsLetter("   "))
        assertEquals("V", initialsLetter("0x88cC…6894"))
        assertEquals("V", initialsLetter("0x88cCA0EeDbF2C4426110bbFc998F048689266894"))
    }
}
