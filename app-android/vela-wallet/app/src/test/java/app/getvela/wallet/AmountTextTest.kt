package app.getvela.wallet

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.amountFieldOf
import app.getvela.wallet.core.format.cleanAmountEdit
import app.getvela.wallet.core.format.cleanAmountFieldEdit
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test

/**
 * Spec 073: every Compose amount field cleans its edit through the core's
 * rule, with the field's own text as `previous` and no paste flag (a native
 * field cannot say). A decimal-comma pad's "4,5" went out raw: 4 in fiat mode,
 * an allowance of 45 on a custom cap.
 */
class AmountTextTest {
    private val saved = Formats.current

    @After fun restore() {
        Formats.current = saved
    }

    @Test fun `a decimal-comma preset reads the comma as the decimal mark`() {
        Formats.current = Formats(NumberFormatKey.DotComma)
        assertEquals("4.", cleanAmountEdit("4,", "4"))
        assertEquals("4.5", cleanAmountEdit("4.5", "4."))
    }

    @Test fun `one typed comma is the decimal mark when the pad and the preset disagree`() {
        // The phone's region gives the pad "," while the app reads comma-dot.
        Formats.current = Formats(NumberFormatKey.CommaDot)
        assertEquals("4.", cleanAmountEdit("4,", "4"))
        // Typed into a figure that has its point: the comma is dropped.
        assertEquals("12.5", cleanAmountEdit("1,2.5", "12.5"))
    }

    @Test fun `more than one character at once reads as a paste`() {
        Formats.current = Formats(NumberFormatKey.CommaDot)
        assertEquals("1234.56", cleanAmountEdit("1.234,56", ""))
        // A tiny balance as many screens print it: refused, never 1.57.
        assertNull(cleanAmountEdit("1.5e-7", ""))
        assertNull(cleanAmountEdit("0x10", "2"))
        // A comma that is neither this preset's decimal mark nor grouping:
        // 150 would be a hundred times somebody's 1,50.
        assertNull(cleanAmountEdit("1,50", ""))
        assertEquals("1500", cleanAmountEdit("1,500", ""))
    }

    @Test fun `a clean figure is left as typed`() {
        Formats.current = Formats(NumberFormatKey.DotComma)
        for (text in listOf("", "0", "0.", "0.0", "0.08", "4", "4.", "0.50", "53.4836")) {
            assertEquals(text, cleanAmountEdit(text, text.dropLast(1)))
        }
    }

    /**
     * Issue #421 (a Xiaomi 15, Send POL): "08" stayed in the field, read as 8
     * — ten times what somebody who missed the point meant — and Continue
     * stayed lit. A zero leading a digit is not kept; only a mark follows it.
     */
    @Test fun `a zero leading a digit is not kept`() {
        for (preset in listOf(NumberFormatKey.CommaDot, NumberFormatKey.DotComma)) {
            Formats.current = Formats(preset)
            assertEquals("8", cleanAmountEdit("08", "0"))
            assertEquals("0", cleanAmountEdit("00", "0"))
            assertEquals("0.08", cleanAmountEdit("0.08", "0.0"))
            assertEquals("0.0", cleanAmountEdit("0.0", "0."))
            assertEquals("0.", cleanAmountEdit(".", ""))
            assertEquals("8.5", cleanAmountEdit("008.5", ""))
        }
        // A decimal-comma pad's "0,8" is 0.8 — never 8.
        Formats.current = Formats(NumberFormatKey.DotComma)
        assertEquals("0.", cleanAmountEdit("0,", "0"))
        assertEquals("0.8", cleanAmountEdit("0,8", "0."))
    }

    /**
     * What every Compose amount field does with a key (the send figure, a
     * split row's share, the custom allowance — `cleanAmountFieldEdit` is the
     * whole of their `onValueChange` rule): the IME's edit at the caret, then
     * the core's rule, caret included. Each key lands where the last one left
     * the caret, exactly as on the phone.
     */
    private fun type(keys: String, start: TextFieldValue = amountFieldOf("")): TextFieldValue {
        var field = start
        for (key in keys) {
            val at = field.selection.end
            val raw = field.text.substring(0, at) + key + field.text.substring(at)
            field = cleanAmountFieldEdit(TextFieldValue(raw, TextRange(at + 1)), field) ?: field
        }
        return field
    }

    @Test fun `the field shows what the core holds, key by key`() {
        Formats.current = Formats(NumberFormatKey.CommaDot)
        assertEquals(TextFieldValue("8", TextRange(1)), type("08"))
        assertEquals(TextFieldValue("0", TextRange(1)), type("00"))
        assertEquals(TextFieldValue("0.08", TextRange(4)), type("0.08"))
        assertEquals(TextFieldValue("0.0", TextRange(3)), type("0.0"))
        assertEquals(TextFieldValue("12", TextRange(2)), type("012"))
    }

    /**
     * "." becomes "0." — a character the person did not type. Left where the
     * key put it, the caret sat between the "0" and the ".", and the next "5"
     * made "05." → "5.": five, where 0.5 was being typed.
     */
    @Test fun `a mark on an empty field reads with its zero, and the caret goes after it`() {
        Formats.current = Formats(NumberFormatKey.CommaDot)
        assertEquals(TextFieldValue("0.", TextRange(2)), type("."))
        assertEquals(TextFieldValue("0.5", TextRange(3)), type(".5"))
        Formats.current = Formats(NumberFormatKey.DotComma)
        assertEquals(TextFieldValue("0.", TextRange(2)), type(","))
        assertEquals(TextFieldValue("0.8", TextRange(3)), type(",8"))
        assertEquals(TextFieldValue("0.8", TextRange(3)), type("0,8"))
    }

    @Test fun `a zero typed in front of a figure is refused where it was typed`() {
        Formats.current = Formats(NumberFormatKey.CommaDot)
        // "8" on screen, the caret before it, "0" typed: still 8, caret unmoved.
        assertEquals(TextFieldValue("8", TextRange(0)), type("0", TextFieldValue("8", TextRange(0))))
    }

    @Test fun `a caret move is not an edit, and a refused paste keeps the field`() {
        Formats.current = Formats(NumberFormatKey.CommaDot)
        val field = TextFieldValue("4.5", TextRange(3))
        val moved = TextFieldValue("4.5", TextRange(1))
        assertSame(moved, cleanAmountFieldEdit(moved, field))
        assertNull(cleanAmountFieldEdit(TextFieldValue("1.5e-7", TextRange(6)), amountFieldOf("")))
        // A clean edit is the IME's own value, untouched.
        val clean = TextFieldValue("4.56", TextRange(4))
        assertSame(clean, cleanAmountFieldEdit(clean, field))
    }
}
