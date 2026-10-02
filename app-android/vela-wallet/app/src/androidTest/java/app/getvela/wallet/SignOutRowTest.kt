package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.settings.SettingsPage
import app.getvela.wallet.feature.settings.SettingsScreen
import app.getvela.wallet.feature.settings.SettingsScreenState
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue #322: Sign Out read as a caption — centred grey text under About,
 * above a bordered card that did look like a control. It is now a settings
 * row like its neighbours: the same component, so the same height, glyph and
 * chevron, and a tap still asks the core.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SignOutRowTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class SignOutRowTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun signOutIsASettingsRowLikeItsNeighbours() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val strings = I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
        val model = SettingsFixtures.buildState(SettingsScreenState.ST1, strings)
        var signedOut = 0
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    SettingsScreen(
                        model = model,
                        page = SettingsPage.Home,
                        overlay = SettingsOverlay.None,
                        advancedOpen = false,
                        onSignOut = { signedOut++ },
                    )
                }
            }
        }
        val aboutTitle = model.sections.flatMap { it.rows }.first { it.id == "about" }.title
        val about = compose.onNodeWithText(aboutTitle).performScrollTo().fetchSemanticsNode().boundsInRoot
        val signOut = compose.onNodeWithText(model.signOutLabel)
            .performScrollTo()
            .assertHasClickAction()
        assertEquals(
            "Sign Out is not drawn as the row its neighbours are",
            about.height,
            signOut.fetchSemanticsNode().boundsInRoot.height,
            1f,
        )
        signOut.performClick()
        compose.waitForIdle()
        assertEquals(1, signedOut)
    }
}
