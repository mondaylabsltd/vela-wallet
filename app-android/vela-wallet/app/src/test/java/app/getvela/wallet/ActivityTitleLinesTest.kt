package app.getvela.wallet

import app.getvela.wallet.feature.wallet.TitlePlace
import app.getvela.wallet.feature.wallet.components.TitleLine
import app.getvela.wallet.feature.wallet.components.TitleLines
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Device walk 2026-10-11, item 1: 「在 arcorbs.game3.app 合 / 约交互」 — a dApp
 * row's title broke inside its action. The rule the row now follows
 * ([TitleLines]), over widths alone: a title wraps only between its parts,
 * the action is never split, and the place is the one part cut.
 *
 * Widths here are counted, not measured: a Latin letter 10, a CJK glyph 20
 * (what a real face gives, near enough). `ActivityRowTitleTest` draws the
 * same titles with the app's fonts on a device.
 */
class ActivityTitleLinesTest {

    private fun width(text: String): Int = text.sumOf { if (it.code > 0x2E7F) 20 else 10 as Int }

    private val zh = TitlePlace(lead = "在", place = "arcorbs.game3.app", trail = "合约交互", gapBefore = true, gapAfter = true, intent = "合约交互")
    private val en = TitlePlace(lead = "Contract interaction on", place = "arcorbs.game3.app", trail = "", gapBefore = true, gapAfter = false, intent = "Contract interaction")
    private val ja = TitlePlace(lead = "", place = "arcorbs.game3.app", trail = "でコントラクトを操作", gapBefore = false, gapAfter = false, intent = "コントラクトを操作")
    private val de = TitlePlace(lead = "Vertragsinteraktion bei", place = "arcorbs.game3.app", trail = "", gapBefore = true, gapAfter = false, intent = "Vertragsinteraktion")

    private fun lines(parts: TitlePlace, wide: Int) = TitleLines.of(parts, wide, ::width)

    @Test
    fun `a title that fits is one line, whole`() {
        assertEquals(listOf("在 arcorbs.game3.app 合约交互"), lines(zh, 400).map { it.text })
        assertEquals(listOf("Contract interaction on arcorbs.game3.app"), lines(en, 500).map { it.text })
    }

    @Test
    fun `the phone's own title breaks after the place, and the action is whole on its line`() {
        // 「在 arcorbs.game3.app 合约交互」 is 290 wide; the row had about 260.
        val broken = lines(zh, 260)
        assertEquals(listOf("在 arcorbs.game3.app", "合约交互"), broken.map { it.text })
        assertEquals(TitleLine("在 ", "arcorbs.game3.app", ""), broken[0])
        assertEquals(TitleLine("合约交互", null, ""), broken[1])
    }

    @Test
    fun `what joins the action to the place stays on the place's line`() {
        assertEquals(listOf(TitleLine("Contract interaction", null, ""), TitleLine("on ", "arcorbs.game3.app", "")), lines(en, 300))
        assertEquals(listOf("Vertragsinteraktion", "bei arcorbs.game3.app"), lines(de, 300).map { it.text })
        // A postposition: it ends the place's line.
        assertEquals(listOf(TitleLine("", "arcorbs.game3.app", "で"), TitleLine("コントラクトを操作", null, "")), lines(ja, 300))
    }

    /** A title whose action nobody named: it breaks at the place's edge, the place beside the shorter side. */
    @Test
    fun `without the action named, the break is at the place's edge`() {
        assertEquals(listOf("Contract interaction on", "arcorbs.game3.app"), lines(en.copy(intent = ""), 300).map { it.text })
        assertEquals(listOf("在 arcorbs.game3.app", "合约交互"), lines(zh.copy(intent = ""), 260).map { it.text })
        assertEquals(listOf("arcorbs.game3.app", "でコントラクトを操作"), lines(ja.copy(intent = ""), 300).map { it.text })
        val both = TitlePlace(lead = "Interaction with", place = "arcorbs.game3.app", trail = "done", gapBefore = true, gapAfter = true)
        assertEquals(listOf("Interaction with", "arcorbs.game3.app done"), lines(both, 300).map { it.text })
    }

    /**
     * At every width from a sliver to the whole row, in each language: the
     * lines put together are the title, the action stands whole among ONE
     * line's words, the place is on one line, and a line wraps by itself only
     * when it is the action's and is wider than the row.
     */
    @Test
    fun `at no width is the action split or anything but the place cut`() {
        val longHost = "a-very-long-subdomain.of.some-protocol.example.finance"
        for (parts in listOf(zh, en, ja, de).flatMap { listOf(it, it.copy(place = longHost)) }) {
            for (wide in 60..900 step 7) {
                val drawn = lines(parts, wide)
                assertTrue("${parts.place} at $wide: $drawn", drawn.size in 1..2)
                // Nothing lost and nothing added but the gap a line break stands in for.
                assertEquals(
                    (parts.lead + parts.place + parts.trail).filterNot { it.isWhitespace() },
                    drawn.joinToString("") { it.text }.filterNot { it.isWhitespace() },
                )
                assertEquals("the action at $wide: $drawn", 1, drawn.count { parts.intent in it.before || parts.intent in it.after })
                assertEquals("the place at $wide: $drawn", 1, drawn.count { it.place == parts.place })
                if (drawn.size == 2) {
                    val (action, place) = drawn.partition { it.place == null }.let { it.first.single() to it.second.single() }
                    assertEquals("the action's line is the action, and nothing else: $drawn", parts.intent, action.text)
                    assertFalse("the place's line never wraps: it is the place that is cut", place.wraps)
                    assertEquals("the action wraps only when it alone is wider than the row ($wide): $action", width(action.text) > wide, action.wraps)
                }
            }
        }
    }
}
