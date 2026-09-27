package app.getvela.wallet

import androidx.compose.ui.graphics.vector.PathNode
import androidx.compose.ui.graphics.vector.addPathNodes
import java.io.File
import kotlin.math.abs
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Device-found on the founder's Xiaomi (spec 078 round 3, 「android 上的设置里的
 * discord 头像不对呀」): Settings → Community drew a broken Discord mark.
 *
 * simple-icons writes arc flags compactly — `a.0741.0741 0 00-.0785.0371`, the
 * large-arc and sweep flags run together as "00". SVG reads each flag as ONE
 * character; Compose's PathParser (Android's too) reads "00" as one number, so
 * every arc after it took the wrong arguments and the glyph came out garbled.
 * X has no arcs and Telegram's are written out, so only Discord broke — and no
 * browser, preview or test that renders SVG could see it.
 *
 * So every brand path in VelaIcons must mean the same to Compose as to SVG:
 * - no arc flag is followed directly by a digit or a point (the ambiguity);
 * - Compose's own parse lands on exactly the points the SVG grammar reads,
 *   segment for segment, and every one inside the 24-unit box.
 */
class BrandIconPathTest {

    private val icons: File by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/core/designsystem/components/VelaIcons.kt")
    }

    /** Every `brandIcon("Name", "path")` in VelaIcons. */
    private val brandPaths: Map<String, String> by lazy {
        Regex("""brandIcon\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\)""")
            .findAll(icons.readText())
            .associate { it.groupValues[1] to it.groupValues[2] }
    }

    @Test
    fun `every brand mark reads the same to Compose as to SVG, inside its box`() {
        assertTrue("no brand paths found — the pattern is wrong", brandPaths.keys.containsAll(listOf("VelaBrandX", "VelaBrandTelegram", "VelaBrandDiscord")))
        val problems = brandPaths.mapValues { (_, d) -> problems(d) }.filterValues { it.isNotEmpty() }
        assertEquals("brand paths Android would draw differently from SVG", emptyMap<String, List<String>>(), problems)
    }

    @Test
    fun `the check catches the path that drew a broken Discord mark`() {
        val found = problems(OLD_DISCORD)
        assertTrue("the compact flags are named", found.any { it.startsWith("compact arc flag") })
        assertTrue("and Compose's parse is shown to differ from SVG's", found.any { it.startsWith("Compose reads") })
    }

    /** What would make Android draw [d] differently from SVG; empty when nothing would. */
    private fun problems(d: String): List<String> {
        val out = mutableListOf<String>()
        compactFlags(d).forEach { at -> out += "compact arc flag at $at: …${d.substring(maxOf(0, at - 12), minOf(d.length, at + 8))}…" }
        val svg = svgEndPoints(d)
        val compose = runCatching { composeEndPoints(addPathNodes(d)) }.getOrElse { return out + "Compose could not parse it: $it" }
        val same = svg.size == compose.size && svg.zip(compose).all { (a, b) -> abs(a.first - b.first) < 1e-3f && abs(a.second - b.second) < 1e-3f }
        if (!same) {
            val first = svg.indices.firstOrNull { it >= compose.size || abs(svg[it].first - compose[it].first) >= 1e-3f || abs(svg[it].second - compose[it].second) >= 1e-3f }
            out += "Compose reads ${compose.size} segments, SVG ${svg.size}; first difference at segment $first"
        }
        compose.filterNot { (x, y) -> x in -BOX_SLACK..24f + BOX_SLACK && y in -BOX_SLACK..24f + BOX_SLACK }
            .firstOrNull()?.let { out += "a point Compose draws leaves the 24-unit box: $it" }
        return out
    }

    // --- SVG, as the grammar reads it (flags are single characters) ---------

    private val argCount = mapOf('m' to 2, 'l' to 2, 'h' to 1, 'v' to 1, 'c' to 6, 's' to 4, 'q' to 4, 't' to 2, 'a' to 7, 'z' to 0)
    private val number = Regex("""[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?""")

    /** Command letter + its arguments, SVG-style: in an arc, arguments 4 and 5 are one-character flags. */
    private fun svgCommands(d: String): List<Pair<Char, List<Float>>> {
        val out = mutableListOf<Pair<Char, List<Float>>>()
        var i = 0
        var command = ' '
        fun skip() { while (i < d.length && (d[i].isWhitespace() || d[i] == ',')) i++ }
        while (true) {
            skip()
            if (i >= d.length) break
            if (d[i].isLetter()) {
                command = d[i++]
                if (command.lowercaseChar() == 'z') { out += command to emptyList(); continue }
            }
            val n = argCount.getValue(command.lowercaseChar())
            val args = mutableListOf<Float>()
            for (k in 0 until n) {
                skip()
                if (command.lowercaseChar() == 'a' && (k == 3 || k == 4)) {
                    args += (d[i++] - '0').toFloat()
                } else {
                    val m = number.matchAt(d, i) ?: error("no number at $i in $d")
                    args += m.value.toFloat()
                    i = m.range.last + 1
                }
            }
            out += command to args
            // Coordinates after a moveto are linetos.
            if (command == 'm') command = 'l' else if (command == 'M') command = 'L'
        }
        return out
    }

    /** Where every arc flag is followed at once by a digit or a point — "00-.07" reads as one number on Android. */
    private fun compactFlags(d: String): List<Int> {
        val out = mutableListOf<Int>()
        var i = 0
        var command = ' '
        fun skip() { while (i < d.length && (d[i].isWhitespace() || d[i] == ',')) i++ }
        while (true) {
            skip()
            if (i >= d.length) break
            if (d[i].isLetter()) {
                command = d[i++]
                if (command.lowercaseChar() == 'z') continue
            }
            val n = argCount.getValue(command.lowercaseChar())
            for (k in 0 until n) {
                skip()
                if (command.lowercaseChar() == 'a' && (k == 3 || k == 4)) {
                    i++
                    if (i < d.length && (d[i].isDigit() || d[i] == '.')) out += i - 1
                } else {
                    val m = number.matchAt(d, i) ?: error("no number at $i in $d")
                    i = m.range.last + 1
                }
            }
            if (command == 'm') command = 'l' else if (command == 'M') command = 'L'
        }
        return out
    }

    private fun svgEndPoints(d: String): List<Pair<Float, Float>> {
        var x = 0f
        var y = 0f
        var startX = 0f
        var startY = 0f
        val out = mutableListOf<Pair<Float, Float>>()
        for ((c, a) in svgCommands(d)) {
            val rel = c.isLowerCase()
            when (c.lowercaseChar()) {
                'm' -> { x = a[0] + if (rel) x else 0f; y = a[1] + if (rel) y else 0f; startX = x; startY = y }
                'l', 't' -> { x = a[0] + if (rel) x else 0f; y = a[1] + if (rel) y else 0f }
                'h' -> x = a[0] + if (rel) x else 0f
                'v' -> y = a[0] + if (rel) y else 0f
                'c' -> { x = a[4] + if (rel) x else 0f; y = a[5] + if (rel) y else 0f }
                's', 'q' -> { x = a[2] + if (rel) x else 0f; y = a[3] + if (rel) y else 0f }
                'a' -> { x = a[5] + if (rel) x else 0f; y = a[6] + if (rel) y else 0f }
                'z' -> { x = startX; y = startY }
            }
            out += x to y
        }
        return out
    }

    // --- Compose, as PathParser actually read it ----------------------------

    private fun composeEndPoints(nodes: List<PathNode>): List<Pair<Float, Float>> {
        var x = 0f
        var y = 0f
        var startX = 0f
        var startY = 0f
        val out = mutableListOf<Pair<Float, Float>>()
        for (node in nodes) {
            when (node) {
                is PathNode.MoveTo -> { x = node.x; y = node.y; startX = x; startY = y }
                is PathNode.RelativeMoveTo -> { x += node.dx; y += node.dy; startX = x; startY = y }
                is PathNode.LineTo -> { x = node.x; y = node.y }
                is PathNode.RelativeLineTo -> { x += node.dx; y += node.dy }
                is PathNode.HorizontalTo -> x = node.x
                is PathNode.RelativeHorizontalTo -> x += node.dx
                is PathNode.VerticalTo -> y = node.y
                is PathNode.RelativeVerticalTo -> y += node.dy
                is PathNode.CurveTo -> { x = node.x3; y = node.y3 }
                is PathNode.RelativeCurveTo -> { x += node.dx3; y += node.dy3 }
                is PathNode.ReflectiveCurveTo -> { x = node.x2; y = node.y2 }
                is PathNode.RelativeReflectiveCurveTo -> { x += node.dx2; y += node.dy2 }
                is PathNode.QuadTo -> { x = node.x2; y = node.y2 }
                is PathNode.RelativeQuadTo -> { x += node.dx2; y += node.dy2 }
                is PathNode.ReflectiveQuadTo -> { x = node.x; y = node.y }
                is PathNode.RelativeReflectiveQuadTo -> { x += node.dx; y += node.dy }
                is PathNode.ArcTo -> { x = node.arcStartX; y = node.arcStartY }
                is PathNode.RelativeArcTo -> { x += node.arcStartDx; y += node.arcStartDy }
                is PathNode.Close -> { x = startX; y = startY }
            }
            out += x to y
        }
        return out
    }

    private companion object {
        /** A glyph may touch its box's edge; rounding may put a point a hair past it. */
        const val BOX_SLACK = 0.01f

        /** The simple-icons Discord path as it shipped, with compact arc flags ("0 00-.0785"). */
        const val OLD_DISCORD =
            "M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189Z"
    }
}
