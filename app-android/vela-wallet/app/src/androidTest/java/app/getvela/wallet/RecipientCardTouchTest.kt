package app.getvela.wallet

import androidx.compose.foundation.layout.width
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.click
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.feature.flows.RecipientCardModel
import app.getvela.wallet.feature.flows.components.RecipientCard
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue #331: on a split send, a tap meant for a recipient's amount must never
 * remove the recipient. The ✕ sat 8dp from a one-line amount field, and its
 * 48dp touch target swallowed every tap just above, below or right of the "0".
 *
 * On the device's real input pipeline (touch-target expansion is the system's,
 * so a JVM test cannot see it):
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.RecipientCardTouchTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class RecipientCardTouchTest {

    @get:Rule
    val compose = createComposeRule()

    private val row = RecipientCardModel(
        ordinal = "Recipient 1",
        name = "0x9F3c…21aE",
        identiconSeed = "0x9f3c000000000000000000000000000000021ae",
        amount = "",
        removeLabel = "Remove recipient 1",
        id = "row-1",
        address = "0x9f3c000000000000000000000000000000021ae",
        amountValue = "",
        addressPlaceholder = "0x…",
    )

    private var removed = 0

    private fun show() {
        compose.setContent {
            VelaTheme(darkTheme = false) {
                // A phone's width less the form's 24dp gutters.
                RecipientCard(
                    recipient = row,
                    modifier = Modifier.width(345.dp),
                    onRemove = { removed++ },
                    onAmountChange = {},
                    onAddressChange = {},
                    onPick = {},
                )
            }
        }
    }

    private val amount get() = compose.onNodeWithContentDescription("recipient-amount-Recipient 1")
    private val remove get() = compose.onNodeWithContentDescription("Remove recipient 1")

    @Test
    fun aTapAnywhereNearTheAmountNeverRemovesTheRecipient() {
        show()
        val field = amount.fetchSemanticsNode().boundsInRoot
        val card = compose.onRoot().fetchSemanticsNode().boundsInRoot
        val dp = compose.density.density
        // The amount's right edge, and a thumb's overshoot just past it, at
        // every height of the card: what a tap aimed at the figure lands on.
        val xs = listOf(field.right - 2 * dp, field.right - 1, field.right + 2 * dp)
        val ys = (0..6).map { card.top + 6 * dp + it * (card.height - 12 * dp) / 6 }
        for (x in xs) {
            for (y in ys) {
                compose.onRoot().performTouchInput { click(Offset(x, y)) }
            }
        }
        compose.waitForIdle()
        assertEquals("a tap meant for the amount removed the recipient", 0, removed)
    }

    @Test
    fun theAmountTakesFocusAcrossItsWholeHeight() {
        show()
        val field = amount.fetchSemanticsNode().boundsInRoot
        val dp = compose.density.density
        assertTrue(
            "the amount field is shorter than a 44dp control (${field.height / dp}dp)",
            field.height >= 44 * dp - 1,
        )
        amount.assertIsNotFocused()
        // Near its bottom right corner, where the ✕ used to answer.
        compose.onRoot().performTouchInput { click(Offset(field.right - 4 * dp, field.bottom - 4 * dp)) }
        amount.assertIsFocused()
        assertEquals(0, removed)
    }

    @Test
    fun theCrossStillRemovesAndItsTargetDoesNotReachTheField() {
        show()
        val field = amount.fetchSemanticsNode().boundsInRoot
        val cross = remove.fetchSemanticsNode().boundsInRoot
        val dp = compose.density.density
        // Compose grows a smaller target to 48dp around its centre; that grown
        // area must end before the amount begins, with a gap where neither
        // answers.
        val grownLeft = cross.center.x - maxOf(cross.width, 48 * dp) / 2
        assertTrue(
            "the ✕'s touch target reaches the amount field (gap ${(grownLeft - field.right) / dp}dp)",
            grownLeft - field.right >= 4 * dp,
        )
        remove.performTouchInput { click(center) }
        compose.waitForIdle()
        assertEquals(1, removed)
    }
}
