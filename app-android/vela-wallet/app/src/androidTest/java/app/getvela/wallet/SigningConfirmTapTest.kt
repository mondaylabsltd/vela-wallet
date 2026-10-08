package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.signing.CONFIRM_TAG
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenModel
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningSheetContent
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue #461: the signing sheet confirms with a TAP — the Send screen's own
 * primary button, saying the action alone, found by its stable hook
 * ([CONFIRM_TAG]) rather than by words that change with the request. Shut
 * while the core says so, with its reason under it, and a tap on it then
 * does nothing. TalkBack says it is a button.
 *
 * Emulator only (MIUI blocks instrumentation; never on the owner's phone,
 * whose wallet `connectedAndroidTest` would wipe):
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SigningConfirmTapTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class SigningConfirmTapTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("zh") }
    }

    private fun show(model: SigningScreenModel, onClose: () -> Unit = {}, onConfirm: () -> Unit) {
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    SigningSheetContent(model = model, onConfirm = onConfirm, onClose = onClose)
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun oneTapConfirmsAndTheButtonSaysTheActionAlone() {
        val model = SigningFixtures.build(SigningScreenState.CS11, strings)
        var confirms = 0
        show(model) { confirms++ }
        val confirm = compose.onNodeWithTag(CONFIRM_TAG).performScrollTo()
        confirm.assertIsEnabled()
        confirm.assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
        confirm.assert(hasText(model.confirmAction!!))
        confirm.performClick()
        compose.waitForIdle()
        assertEquals(1, confirms)
    }

    @Test
    fun aShutConfirmSaysWhyAndTakesNoTap() {
        val reason = strings.t("componentsUi.signing.confirmBlock.feeMeasuring")
        val model = SigningFixtures.build(SigningScreenState.CS11, strings)
            .copy(confirmEnabled = false, confirmBlockLine = reason)
        var confirms = 0
        show(model) { confirms++ }
        compose.onNodeWithTag(CONFIRM_TAG).performScrollTo().assertIsNotEnabled().performClick()
        compose.waitForIdle()
        assertEquals(0, confirms)
        compose.onNodeWithText(reason).assertExists()
    }

    @Test
    fun aRefusedRequestOffersNoConfirmAtAll() {
        val model = SigningFixtures.build(SigningScreenState.CS11, strings).copy(confirmAction = null)
        show(model) {}
        compose.onNodeWithTag(CONFIRM_TAG).assertDoesNotExist()
        compose.onAllNodesWithText(strings.t("componentsUi.signing.confirmSwap")).assertCountEquals(0)
    }
}
