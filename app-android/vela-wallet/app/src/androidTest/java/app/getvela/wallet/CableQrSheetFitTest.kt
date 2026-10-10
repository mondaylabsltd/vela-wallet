package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.onboarding.flow.CABLE_CANCEL_TAG
import app.getvela.wallet.feature.onboarding.flow.CABLE_CODE_TAG
import app.getvela.wallet.feature.onboarding.flow.CODE_MAX
import app.getvela.wallet.feature.onboarding.flow.CableQrSheet
import app.getvela.wallet.feature.onboarding.flow.FlowFixtures
import app.getvela.wallet.feature.onboarding.flow.KeyChooser
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue #480 (and #447 before it): the "Phone or tablet · scan a code" sheet
 * shows its whole code and its Cancel, whatever the window. The code filled
 * the sheet's width with no cap — a 600dp square on a tablet, taller than a
 * landscape window — so Cancel scrolled away under it.
 *
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.CableQrSheetFitTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class CableQrSheetFitTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply { initialize("en") }
    }

    @Test
    fun theCodeIsASquareNoWiderThanItsCapAndCancelIsOnScreen() {
        var cancelled = 0
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    CableQrSheet(payload = FlowFixtures.CABLE_PAYLOAD, chooser = KeyChooser.Create, onCancel = { cancelled++ })
                }
            }
        }
        compose.waitForIdle()
        val dp = compose.density.density
        val code = compose.onNodeWithTag(CABLE_CODE_TAG).fetchSemanticsNode().boundsInRoot
        assertEquals("the code is a square", code.width, code.height, 1f)
        assertTrue("the code is ${code.width / dp}dp wide, over its ${CODE_MAX.value}dp cap", code.width <= CODE_MAX.value * dp + 1)
        // Whole, not scrolled half out of its box.
        compose.onNodeWithTag(CABLE_CODE_TAG).assertIsDisplayed()
        val cancel = compose.onNodeWithTag(CABLE_CANCEL_TAG)
        cancel.assertIsDisplayed()
        assertTrue("Cancel sits under the code", cancel.fetchSemanticsNode().boundsInRoot.top >= code.bottom)
        cancel.performClick()
        compose.waitForIdle()
        assertEquals(1, cancelled)
    }
}
