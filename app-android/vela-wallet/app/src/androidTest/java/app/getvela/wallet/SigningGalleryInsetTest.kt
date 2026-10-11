package app.getvela.wallet

import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.signing.CONFIRM_TAG
import app.getvela.wallet.feature.signing.FOOTER_TAG
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.gallery.SigningGalleryScreen
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Device walk 2026-10-11, item 15: on the signing gallery — where sheets get
 * reviewed — the pinned confirm was cut by a 3-button navigation bar (only
 * its top 80 px showed on CS67, CS68 and CS36) and the line under it lay
 * behind the bar. The gallery had the status bar's inset and not the
 * navigation bar's.
 *
 * The real gallery screen in an edge-to-edge window, as `MainActivity` hosts
 * it: on each board the walk named, and on CS69 (which has a line under its
 * confirm), the confirm and the whole footer end above the navigation bar.
 * Whatever navigation mode the emulator is in is the one checked; run it in
 * both.
 *
 * ```bash
 * adb -s <emulator> shell cmd overlay enable-exclusive --category com.android.internal.systemui.navbar.threebutton
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SigningGalleryInsetTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class SigningGalleryInsetTest {

    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply { initialize("zh") }
    }

    private fun boardStandsAboveTheNavigationBar(state: SigningScreenState) {
        // The window as the app's own activity has it: drawn behind the bars.
        compose.runOnUiThread { compose.activity.enableEdgeToEdge() }
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                SigningGalleryScreen(systemDarkTheme = false, initialState = state.name)
            }
        }
        compose.waitForIdle()
        val decor = compose.activity.window.decorView
        val bar = ViewCompat.getRootWindowInsets(decor)!!.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
        val floor = decor.height - bar
        assertTrue("this window has no navigation bar to stand above (inset $bar)", bar > 0)
        val confirm = compose.onNodeWithTag(CONFIRM_TAG).fetchSemanticsNode()
        val footer = compose.onNodeWithTag(FOOTER_TAG, useUnmergedTree = true).fetchSemanticsNode()
        Log.i(
            "pr4-fit",
            "gallery ${state.name}: window ${decor.height} navBar $bar floor $floor | confirm ${confirm.boundsInRoot.top.toInt()}..${confirm.boundsInRoot.bottom.toInt()} " +
                "(${confirm.size.height} px tall) footer ends ${footer.boundsInRoot.bottom.toInt()}",
        )
        assertTrue(
            "${state.name}: the confirm ends at ${confirm.boundsInRoot.bottom}, under the navigation bar (floor $floor)",
            confirm.boundsInRoot.bottom <= floor + 0.5f,
        )
        // Whole: not clipped by the screen's edge either.
        assertTrue("${state.name}: the confirm is cut", confirm.boundsInRoot.height >= confirm.size.height - 0.5f)
        assertTrue(
            "${state.name}: the footer — the confirm's own line included — ends at ${footer.boundsInRoot.bottom}, under the bar (floor $floor)",
            footer.boundsInRoot.bottom <= floor + 0.5f,
        )
        compose.onNodeWithTag(CONFIRM_TAG).assertIsDisplayed()
        // The line under a held confirm is on screen, above the bar.
        SigningFixtures.build(state, strings).confirmBlockLine?.let { line ->
            val said = compose.onNodeWithText(line).fetchSemanticsNode().boundsInRoot
            assertTrue("${state.name}: 「$line」 ends at ${said.bottom}, behind the bar (floor $floor)", said.bottom <= floor + 0.5f)
        }
    }

    @Test fun cs67() = boardStandsAboveTheNavigationBar(SigningScreenState.CS67)
    @Test fun cs68() = boardStandsAboveTheNavigationBar(SigningScreenState.CS68)
    @Test fun cs36() = boardStandsAboveTheNavigationBar(SigningScreenState.CS36)
    @Test fun cs69WithItsLineUnderTheConfirm() = boardStandsAboveTheNavigationBar(SigningScreenState.CS69)
}
