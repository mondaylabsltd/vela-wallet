package app.getvela.wallet

import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performSemanticsAction
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.feature.settings.components.VelaTextScaleSlider
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * 087 F09: TalkBack read Settings' text-size slider as "text-scale-slider" and
 * its dots as "text-scale-0"…"text-scale-5" — test ids worn as accessible
 * names. It is one named, adjustable slider; the ids are test tags.
 *
 * ```
 * ./gradlew :app:connectedDebugAndroidTest \
 *   -Pandroid.testInstrumentationRunnerArguments.class=app.getvela.wallet.TextScaleSliderSemanticsTest
 * ```
 */
@RunWith(AndroidJUnit4::class)
class TextScaleSliderSemanticsTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun theSliderIsNamedStatesItsStepAndMovesByAction() {
        var picked = -1
        compose.setContent {
            VelaTheme(darkTheme = false) {
                VelaTextScaleSlider(steps = 6, index = 2, label = "文字大小", onChange = { picked = it })
            }
        }
        val slider = compose.onNodeWithTag("text-scale-slider")
        slider.assertContentDescriptionEquals("文字大小")
        slider.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "3/6"))
        slider.assert(
            SemanticsMatcher.expectValue(
                SemanticsProperties.ProgressBarRangeInfo,
                ProgressBarRangeInfo(current = 2f, range = 0f..5f, steps = 4),
            ),
        )
        // No node speaks a test id.
        compose.onAllNodesWithContentDescription("text-scale", substring = true).assertCountEquals(0)
        compose.onNodeWithTag("text-scale-0").assertExists()

        slider.performSemanticsAction(SemanticsActions.SetProgress) { it(3f) }
        compose.waitForIdle()
        assertEquals(3, picked)
    }
}
