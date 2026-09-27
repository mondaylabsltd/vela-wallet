package app.getvela.wallet

import app.getvela.wallet.feature.flows.fitHeadline
import app.getvela.wallet.feature.flows.truncate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The saved share card's text rules, the web's `fitHeadline` and `truncate`
 * (`share-image.ts`) ported: a headline takes one line at 32 when it fits,
 * shrinks to 26, then breaks into two balanced lines; a long account name is
 * cut with an ellipsis, never wrapped under the identicon.
 *
 * The measure is a stand-in (every character 0.6 em, CJK a full em), as the
 * web's test uses `estimateWidth` — the rules are what is under test, not a face.
 */
class ShareCardHeadlineTest {

    private fun measure(text: String, size: Float): Float =
        text.sumOf { if (it.code >= 0x2e80) 1.0 else 0.6 }.toFloat() * size

    @Test
    fun `a short headline is one line at the full size`() {
        assertEquals(32f to listOf("扫码向我转账"), fitHeadline("扫码向我转账", ::measure))
    }

    @Test
    fun `a headline a little too wide shrinks on one line`() {
        // 22 characters × 0.6 × 32 = 422 > 400; at 30 it is 396.
        val (size, lines) = fitHeadline("Scan to send me crypto", ::measure)
        assertEquals(listOf("Scan to send me crypto"), lines)
        assertEquals(30f, size)
    }

    @Test
    fun `a long headline breaks at a space into halves that each fit`() {
        val headline = "Отсканируйте, чтобы отправить мне крипто"
        val (size, lines) = fitHeadline(headline, ::measure)
        assertEquals(2, lines.size)
        assertEquals(headline, lines.joinToString(" "))
        lines.forEach { assertTrue("$it fits at $size", measure(it, size) <= 400f) }
        // Balanced: no break gives a narrower longer half.
        assertEquals("Отсканируйте, чтобы", lines[0])
    }

    @Test
    fun `a name that fits is whole, one that does not is cut with an ellipsis`() {
        val fits = { text: String -> measure(text, 17f) }
        assertEquals("大表哥", truncate("大表哥", 164f, fits))
        val cut = truncate("Gemeinsames Haushaltskonto der Familie", 164f, fits)
        assertTrue(cut.endsWith("…"))
        assertTrue(fits(cut) <= 164f)
        assertTrue("Gemeinsames Haushaltskonto der Familie".startsWith(cut.removeSuffix("…")))
    }
}
