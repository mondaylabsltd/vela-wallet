package app.getvela.wallet

import android.graphics.Bitmap
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowHost
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue #321: the Receive (QR) sheet fits one phone screen at the default text
 * size, with its bottom margin showing — "View on Explorer" was cut off at the
 * bottom edge of a Xiaomi 15 and had to be scrolled to.
 *
 * Whatever screen this runs on is the screen it checks, so run it at the
 * report's geometry and at the test phone's (an emulator is fine):
 *
 * ```bash
 * adb -s <emulator> shell wm size 1200x2670; adb -s <emulator> shell wm density 520   # Xiaomi 15
 * adb -s <emulator> shell wm size 1080x2400; adb -s <emulator> shell wm density 440   # Xiaomi alioth
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.ReceiveSheetFitTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ReceiveSheetFitTest {

    @get:Rule
    val compose = createComposeRule()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val strings = I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
        .apply { initialize("en") }

    @Test
    fun aNetworkCodeFitsWithItsBottomMargin() = fits(FlowState.R2)

    @Test
    fun anAssetCodeFitsWithItsBottomMargin() = fits(FlowState.R3)

    private fun fits(state: FlowState) {
        val drawn = FlowFixtures.build(state, strings)
        val sheet = drawn.sheet as FlowSheet.ReceiveQr
        // The report's network: "…receive assets on Avalanche" takes two lines.
        val title = strings.t(I18nKeys.Flows.RECEIVE_QR_NETWORK, mapOf("network" to "Avalanche"))
        val model = drawn.copy(sheet = FlowSheet.ReceiveQr(sheet.model.copy(title = if (state == FlowState.R2) title else sheet.model.title)))
        var root: android.view.View? = null
        compose.setContent {
            root = LocalView.current
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { FlowHost(model = model) }
            }
        }
        val receive = (model.sheet as FlowSheet.ReceiveQr).model
        val explorer = compose.onNodeWithText(receive.viewOnExplorer)
        // The sheet slides in; the test clock runs its animation to the end.
        compose.waitUntil(10_000) { runCatching { explorer.fetchSemanticsNode() }.isSuccess }
        compose.waitForIdle()
        val dp = compose.density.density
        val button = explorer.fetchSemanticsNode()
        val screen = rootOf(button).boundsInRoot
        // Above the navigation bar, the sheet's own 32dp bottom margin shows in
        // full: nothing at the bottom was scrolled away or cut off.
        // The raw inset, from the window — a test activity that is not edge to
        // edge has its own copy consumed.
        val navigationBar = root?.let { ViewCompat.getRootWindowInsets(it) }
            ?.getInsets(WindowInsetsCompat.Type.navigationBars())?.bottom ?: 0
        val margin = (screen.bottom - navigationBar - button.boundsInRoot.bottom) / dp
        save(state)
        assertTrue(
            "View on Explorer ends ${margin}dp above the navigation bar — under the sheet's own 32dp " +
                "bottom margin, so the sheet had to scroll (screen ${screen.width / dp}x${screen.height / dp}dp)",
            margin >= 31.5f,
        )
        // A code is a square at any width: it was squeezed sideways on a
        // narrow phone and kept its height, a card taller than wide.
        val code: Rect = compose.onNodeWithContentDescription(receive.title)
            .fetchSemanticsNode().boundsInRoot
        assertEquals("the code's card is not square", code.width, code.height, 1f)
    }

    private fun rootOf(node: SemanticsNode): SemanticsNode {
        var at = node
        while (true) at = at.parent ?: return at
    }

    /** Evidence for the review: the screen as measured, in the app's cache. */
    private fun save(state: FlowState) {
        val shot = InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot() ?: return
        java.io.File(context.cacheDir, "receive-${state.name}.png").outputStream().use {
            shot.compress(Bitmap.CompressFormat.PNG, 100, it)
        }
    }
}
