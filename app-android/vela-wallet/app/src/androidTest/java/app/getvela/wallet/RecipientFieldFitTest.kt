package app.getvela.wallet

import androidx.compose.foundation.layout.width
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.feature.flows.RecipientFieldModel
import app.getvela.wallet.feature.flows.components.RecipientField
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The Send form's recipient row (issue #468), on a real renderer.
 *
 * #468 gave the row a second door — the QR scan beside the person — which
 * takes 40dp from the address. A 42-character address must still be read
 * WHOLE beside both doors at the largest text size: the characters that do
 * not show are the ones a poisoned look-alike changes.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.RecipientFieldFitTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class RecipientFieldFitTest {

    @get:Rule
    val compose = createComposeRule()

    private val address = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"

    private fun field(fontScale: Float) {
        compose.setContent {
            val density = LocalDensity.current
            CompositionLocalProvider(LocalDensity provides Density(density.density, fontScale)) {
                VelaTheme(darkTheme = false) {
                    // A 392dp phone (the Xiaomi alioth) less the form's 24dp gutters.
                    RecipientField(
                        field = RecipientFieldModel(
                            label = "Recipient",
                            lines = address.take(21) to address.drop(21),
                            identiconSeed = address,
                            pickLabel = "Choose recipient",
                            scanLabel = "Scan a QR code",
                            raw = address,
                        ),
                        modifier = Modifier.width(344.dp),
                        onValueChange = {},
                    )
                }
            }
        }
    }

    /** Every line of the address lies inside the field — nothing scrolled away under a 2-line cap. */
    private fun assertWholeAddressShows() {
        val node = compose.onNode(hasSetTextAction()).fetchSemanticsNode()
        val layouts = mutableListOf<TextLayoutResult>()
        node.config[SemanticsActions.GetTextLayoutResult].action?.invoke(layouts)
        val layout = layouts.single()
        assertEquals(address, layout.layoutInput.text.text)
        assertTrue(
            "the address needs ${layout.size.height}px over ${layout.lineCount} lines; the field shows ${node.size.height}px",
            layout.size.height <= node.size.height,
        )
        assertTrue("no character is cut off at a line's end", !layout.didOverflowWidth)
        compose.onNodeWithContentDescription("Choose recipient").assertIsDisplayed()
        compose.onNodeWithContentDescription("Scan a QR code").assertIsDisplayed()
    }

    @Test
    fun aFullAddressShowsWholeBesideBothDoors() {
        field(fontScale = 1f)
        assertWholeAddressShows()
    }

    @Test
    fun aFullAddressShowsWholeBesideBothDoorsAtTheLargestText() {
        field(fontScale = 1.35f)
        assertWholeAddressShows()
    }

    @Test
    fun aFullAddressShowsWholeAtTheSystemsLargestFont() {
        field(fontScale = 2f)
        assertWholeAddressShows()
    }
}
