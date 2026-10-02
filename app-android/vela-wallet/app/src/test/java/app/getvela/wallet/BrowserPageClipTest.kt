package app.getvela.wallet

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 087 F21, device-found on the Xiaomi: a site that was slow to answer
 * (app.uniswap.org on a China network, ~20 s to fail) showed a fully white
 * screen — the address bar, its close button and its load line were gone
 * until the failure panel came up. A person could not see what was loading or
 * close it.
 *
 * The cause is in the hosting, not the page: Compose's `ViewFactoryHolder`
 * calls `setClipChildren(false)`, and the holder is drawn straight into the
 * screen's canvas, so the WebView's own render node is not clipped to its
 * bounds. Before the first document commits the WebView has no frame and
 * paints `canvas.drawColor(background)` — which fills the WHOLE clip: the
 * address bar drawn before it went under white; the toolbar drawn after it
 * stayed. (Emulator proof: a magenta WebView background turned the whole
 * screen magenta; with `clipToBounds()` only the page's slot.)
 *
 * These tests read the source: the one host of the page keeps the clip, and
 * stays the one host.
 */
class BrowserPageClipTest {

    private val sources: File by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet")
    }

    private val host = "feature/explore/components/BrowserPage.kt"

    /** The file's code, with line comments, KDoc and block-comment lines dropped. */
    private fun code(file: File): List<String> = file.readLines()
        .map { it.substringBefore("//").trim() }
        .filter { it.isNotEmpty() && !it.startsWith("*") && !it.startsWith("/*") }

    @Test
    fun `the page is clipped to its own slot`() {
        val lines = code(File(sources, host))
        val view = lines.indexOfFirst { it.startsWith("AndroidView(") }
        assertTrue("BrowserPage hosts the engine through AndroidView", view >= 0)
        val modifier = lines.drop(view + 1).firstOrNull { it.startsWith("modifier =") }
        assertTrue(
            "the WebView's AndroidView must be clipped — a frameless WebView paints over the address bar (087 F21): $modifier",
            modifier != null && modifier.contains(".clipToBounds()"),
        )
    }

    @Test
    fun `the engine's WebView has one host`() {
        val others = sources.walkTopDown()
            .filter { it.isFile && it.extension == "kt" }
            .filter { it.relativeTo(sources).path != host }
            .filter { file ->
                val lines = code(file)
                lines.any { it.contains("AndroidView(") } && lines.any { it.contains("webView") }
            }
            .map { it.relativeTo(sources).path }
            .toList()
        assertEquals("host the page through BrowserPage, which keeps it clipped", emptyList<String>(), others)
    }
}
