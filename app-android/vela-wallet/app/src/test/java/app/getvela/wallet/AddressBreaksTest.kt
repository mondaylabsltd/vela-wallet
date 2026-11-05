package app.getvela.wallet

import app.getvela.wallet.core.designsystem.components.AddressBreaks
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Where an address may break (r13: a member's last five digits, `e130b`, alone
 * on a line in the create sheet) — the desktop's rule (`ui/address.rs`), pinned
 * to the same vectors.
 */
class AddressBreaksTest {

    @Test
    fun `a full address is split once, evenly, and a short one never`() {
        val full = "0x777E989C821a7F1b031696f044a2fF114261875a"
        val (head, tail) = AddressBreaks.halves(full)!!
        assertEquals(full, head + tail)
        assertEquals(22 to 20, head.length to tail.length)
        assertNull(AddressBreaks.halves("0x777E98…61875a"))
        assertNull(AddressBreaks.halves("Interleave"))
        assertTrue(AddressBreaks.isShortForm("0x777E98…61875a"))
        assertFalse(AddressBreaks.isShortForm("Interleave · Vela account"))
        assertTrue(AddressBreaks.isAddressLike(full))
        assertTrue(AddressBreaks.isAddressLike("0xD40086…de130b"))
        assertFalse(AddressBreaks.isAddressLike("vela://multisig?x=0x777E989C821a7F1b031696f044a2fF114261875a"))
    }

    @Test
    fun `the r13 member splits where nobody is left alone`() {
        val (head, tail) = AddressBreaks.halves("0xD400866e00B055B20752a826CD5C89b811de130b")!!
        assertEquals("0xD400866e00B055B20752", head)
        assertEquals("a826CD5C89b811de130b", tail)
    }

    @Test
    fun `calldata wraps as text, a hash splits like an address`() {
        assertNull(AddressBreaks.halves("0x" + "ab".repeat(68)))
        val hash = "0x" + "ab".repeat(32)
        val (h, t) = AddressBreaks.halves(hash)!!
        assertEquals(34 to 32, h.length to t.length)
        // Not hex: drawn whole.
        assertNull(AddressBreaks.halves("0x" + "zz".repeat(20)))
    }
}
