package app.getvela.wallet

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.unit.Velocity
import app.getvela.wallet.core.designsystem.components.SheetOverscrollGuard
import java.io.File
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Device-found on the founder's phone (spec 078 round 3): a fast fling in a
 * bottom sheet — the content run up to its end, the leftover carried into the
 * sheet, the sheet at its top — set it jittering up and down without end.
 *
 * The sheet's size followed its own offset (Material3 pads the content by the
 * status-bar inset minus the offset), so every settle moved the anchor it was
 * settling to. The fix lives in ONE host, `VelaModalSheet`; these tests keep it
 * the only way a sheet is drawn and keep its two rules in place.
 */
class SheetHostTest {

    private val sources: File by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet")
    }

    private val host = "core/designsystem/components/VelaModalSheet.kt"

    @Test
    fun `every bottom sheet goes through the one host`() {
        val direct = sources.walkTopDown()
            .filter { it.isFile && it.extension == "kt" }
            .filter { it.relativeTo(sources).path != host }
            .filter { file ->
                file.readLines().any { line ->
                    val code = line.substringBefore("//").trimStart()
                    !code.startsWith("*") && Regex("(?<![`\\w])ModalBottomSheet\\(").containsMatchIn(code)
                }
            }
            .map { it.relativeTo(sources).path }
            .toList()
        assertEquals("draw sheets with VelaModalSheet, not Material3's ModalBottomSheet directly", emptyList<String>(), direct)
    }

    @Test
    fun `the host never lets the sheet's size follow its offset`() {
        val text = File(sources, host).readText()
        // The top inset is the offset-dependent one; only the bottom is taken.
        assertTrue(text.contains("contentWindowInsets = { WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom) }"))
        assertTrue("the overscroll guard sits on the sheet's content", text.contains(".nestedScroll(SheetOverscrollGuard)"))
    }

    @Test
    fun `upward overscroll is kept, downward passes to the sheet`() {
        val up = Offset(0f, -120f)
        val down = Offset(0f, 120f)
        assertEquals(up, SheetOverscrollGuard.onPostScroll(Offset.Zero, up, NestedScrollSource.UserInput))
        assertEquals(Offset.Zero, SheetOverscrollGuard.onPostScroll(Offset.Zero, down, NestedScrollSource.UserInput))
        runBlocking {
            assertEquals(Velocity(0f, -4000f), SheetOverscrollGuard.onPostFling(Velocity.Zero, Velocity(0f, -4000f)))
            assertEquals(Velocity.Zero, SheetOverscrollGuard.onPostFling(Velocity.Zero, Velocity(0f, 4000f)))
        }
    }
}
