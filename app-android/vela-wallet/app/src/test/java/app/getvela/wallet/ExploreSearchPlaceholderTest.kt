package app.getvela.wallet

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Device pass 2026-10-09: at 360 dp the Explore home's English placeholder
 * read "Search dApps, or type a web" — one line, clipped, nothing to say it
 * went on. It is cut with an ellipsis now, as the iPhone's text field cuts its
 * placeholder.
 *
 * Read from the source (as BrowserPageClipTest does): there is no JVM Compose
 * harness to lay the field out at 360 dp.
 */
class ExploreSearchPlaceholderTest {

    private val field: List<String> by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        val lines = File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/explore/components/ExploreComponents.kt")
            .readLines()
            .map { it.substringBefore("//").trim() }
            .filter { it.isNotEmpty() && !it.startsWith("*") && !it.startsWith("/*") }
        val start = lines.indexOfFirst { it.startsWith("fun ExploreSearchField(") }
        assertTrue("ExploreSearchField is in ExploreComponents.kt", start >= 0)
        val end = lines.drop(start + 1).indexOfFirst { it.startsWith("fun ") || it.startsWith("private fun ") || it.startsWith("internal fun ") }
        lines.subList(start, if (end < 0) lines.size else start + 1 + end)
    }

    @Test
    fun `the placeholder is one line, cut with an ellipsis`() {
        val text = field.indexOf("text = placeholder,")
        assertTrue("the placeholder is drawn by a Text", text > 0 && field[text - 1] == "Text(")
        val arguments = field.drop(text).takeWhile { it != ")" }
        assertTrue("one line: $arguments", "maxLines = 1," in arguments)
        assertTrue("an ellipsis where it runs out of room: $arguments", "overflow = TextOverflow.Ellipsis," in arguments)
    }
}
