package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.components.HOLD_HINT_TAG
import app.getvela.wallet.core.designsystem.components.VelaHoldToConfirmButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * R22 (review 1-3): the one hold-to-confirm button the signing sheet's
 * confirm, the hand-off card's Open and every multisig sign button use.
 * Pointer and touch hold; releasing early does nothing; an accessibility
 * activation (TalkBack's double tap, Switch Access, any accessibility click)
 * arms it and says "Press again to confirm", a second within the core's
 * window confirms, and the window running out disarms it (WCAG 2.5.1/2.5.7).
 */
@RunWith(AndroidJUnit4::class)
class HoldToConfirmDeviceTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
    }

    private val hold = uniffi.vela_core_uniffi.holdToConfirmMs().toInt()
    private val window = uniffi.vela_core_uniffi.confirmAgainWindowMs().toLong()
    private val holdHint get() = strings.t("componentsUi.signing.holdToConfirm")
    private val again get() = strings.t("componentsUi.signing.confirmAgain")

    private var confirms = 0
    private var enabled by mutableStateOf(true)

    /** §39 (A1): what the button said of the confirm beginning (`true`) and being let go (`false`), in order. */
    private val engaged = mutableListOf<Boolean>()

    /** r22-6: a redraw of the sheet that changes nothing of the door (a re-check landing the same verdict). */
    private var redraws by mutableStateOf(0)

    private fun show(holdMs: Int = hold) {
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    redraws // read: a redraw recomposes the button with the same door
                    VelaHoldToConfirmButton(
                        text = strings.t("componentsUi.signing.certain.signAnyway"),
                        onConfirm = { confirms++ },
                        holdMs = holdMs,
                        holdHint = holdHint,
                        confirmAgain = again,
                        danger = true,
                        enabled = enabled,
                        modifier = Modifier.testTag(BUTTON),
                        onEngage = { engaged += it },
                    )
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun aHoldThatLastsConfirmsOnce() {
        show()
        compose.onNodeWithText(holdHint).assertExistsCompat()
        compose.mainClock.autoAdvance = false
        compose.onNodeWithTag(BUTTON).performTouchInput { down(center) }
        compose.mainClock.advanceTimeBy(hold + 200L)
        compose.onNodeWithTag(BUTTON).performTouchInput { up() }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        assertEquals("held to the end: confirmed once", 1, confirms)
    }

    @Test
    fun releasingEarlyDoesNothing() {
        show()
        compose.mainClock.autoAdvance = false
        compose.onNodeWithTag(BUTTON).performTouchInput { down(center) }
        compose.mainClock.advanceTimeBy(hold / 3L)
        compose.onNodeWithTag(BUTTON).performTouchInput { up() }
        compose.mainClock.advanceTimeBy(hold.toLong())
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        assertEquals("released early: nothing happened", 0, confirms)
        // …and a tap is the earliest release of all.
        compose.onNodeWithTag(BUTTON).performClick()
        compose.waitForIdle()
        assertEquals(0, confirms)
    }

    @Test
    fun theAccessiblePathArmsAnnouncesAndConfirmsOnTheSecondActivation() {
        show()
        val button = compose.onNodeWithTag(BUTTON)
        button.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, holdHint))
        button.performSemanticsAction(SemanticsActions.OnClick)
        compose.waitForIdle()
        assertEquals("the first activation only arms", 0, confirms)
        // Said, politely, as it changes: the hint line is a live region.
        compose.onNodeWithTag(HOLD_HINT_TAG).assert(SemanticsMatcher.expectValue(SemanticsProperties.Text, listOf(androidx.compose.ui.text.AnnotatedString(again))))
        compose.onNodeWithTag(HOLD_HINT_TAG).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.LiveRegion))
        button.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, again))
        button.performSemanticsAction(SemanticsActions.OnClick)
        compose.waitForIdle()
        assertEquals("the second confirms", 1, confirms)
    }

    @Test
    fun theWindowRunningOutDisarms() {
        show()
        val button = compose.onNodeWithTag(BUTTON)
        compose.mainClock.autoAdvance = false
        button.performSemanticsAction(SemanticsActions.OnClick)
        compose.mainClock.advanceTimeBy(window + 500L)
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        button.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, holdHint))
        button.performSemanticsAction(SemanticsActions.OnClick)
        compose.waitForIdle()
        assertEquals("after the window the next activation arms again — never confirms", 0, confirms)
    }

    @Test
    fun aShutButtonTakesNeitherPath() {
        enabled = false
        show()
        compose.onNodeWithTag(BUTTON).assertIsNotEnabled()
        compose.onNodeWithTag(BUTTON).performClick()
        compose.waitForIdle()
        assertEquals(0, confirms)
    }

    @Test
    fun noHoldIsAPlainPress() {
        show(holdMs = 0)
        compose.onNodeWithTag(BUTTON).performClick()
        compose.waitForIdle()
        assertEquals("hold_ms 0: one tap, exactly as before", 1, confirms)
    }

    /**
     * §39 (A1): the signing sheet holds its fee row still from the moment a
     * confirm BEGINS — a hold's start — and signs that figure; a hold let go
     * early lets the row follow the quote again. A hold that completes
     * confirms, and is not "let go".
     */
    @Test
    fun aHoldSaysWhenItBeganAndWhenItWasLetGo() {
        show()
        compose.mainClock.autoAdvance = false
        compose.onNodeWithTag(BUTTON).performTouchInput { down(center) }
        compose.mainClock.advanceTimeBy(hold / 3L)
        assertEquals("began at the press", listOf(true), engaged.toList())
        compose.onNodeWithTag(BUTTON).performTouchInput { up() }
        compose.mainClock.advanceTimeBy(hold.toLong())
        assertEquals("let go early", listOf(true, false), engaged.toList())
        engaged.clear()
        compose.onNodeWithTag(BUTTON).performTouchInput { down(center) }
        compose.mainClock.advanceTimeBy(hold + 200L)
        compose.onNodeWithTag(BUTTON).performTouchInput { up() }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        assertEquals("held to the end: began, never let go", listOf(true), engaged.toList())
        assertEquals(1, confirms)
    }

    /** §39 (A1): the assistive path's FIRST press begins the confirm; the window running out lets it go. */
    @Test
    fun theFirstAssistivePressBeginsAndTheWindowLetsGo() {
        show()
        val button = compose.onNodeWithTag(BUTTON)
        compose.mainClock.autoAdvance = false
        button.performSemanticsAction(SemanticsActions.OnClick)
        compose.mainClock.advanceTimeBy(100L)
        assertEquals(listOf(true), engaged.toList())
        compose.mainClock.advanceTimeBy(window + 500L)
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        assertEquals(listOf(true, false), engaged.toList())
        assertEquals(0, confirms)
    }

    /**
     * r22-6: a re-check that lands the same verdict redraws the sheet with
     * the same door — an arm made on it survives, and the second press
     * confirms.
     */
    @Test
    fun anArmSurvivesARedrawWithTheSameDoor() {
        show()
        val button = compose.onNodeWithTag(BUTTON)
        button.performSemanticsAction(SemanticsActions.OnClick)
        compose.waitForIdle()
        redraws++
        compose.waitForIdle()
        redraws++
        compose.waitForIdle()
        button.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, again))
        assertEquals("still armed: never let go", listOf(true), engaged.toList())
        button.performSemanticsAction(SemanticsActions.OnClick)
        compose.waitForIdle()
        assertEquals("the second press confirms", 1, confirms)
    }

    private fun androidx.compose.ui.test.SemanticsNodeInteraction.assertExistsCompat() = assertExists()

    private companion object {
        const val BUTTON = "hold.button"
    }
}
