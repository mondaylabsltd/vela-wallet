package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletHomeModel
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreen
import app.getvela.wallet.feature.wallet.WalletScreenState
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * r13-7 on a device (docs/multisig-machines.md §17): a multisig's history
 * that could not be read is said on the home's Activity with Try again, and
 * the button reads it again; one still being read never says "No activity
 * yet". Each state is also kept as a picture in the app's cache
 * (`r13-history-*.png`) for the review.
 */
@RunWith(AndroidJUnit4::class)
class MultisigHistoryHomeDeviceTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
    }

    private fun history(stand: String) = WalletLive.MultisigHistory(
        stand = stand,
        unreadText = "Couldn't read this multisig's history on Arc Testnet: transactions executed elsewhere may be missing here.",
        retryText = strings.t("common.tryAgain"),
    )

    private fun shot(name: String) {
        InstrumentationRegistry.getInstrumentation().let { run ->
            run.uiAutomation.takeScreenshot()?.let { picture ->
                java.io.File(run.targetContext.cacheDir, "r13-history-$name.png").outputStream().use {
                    picture.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
                }
            }
        }
    }

    @Test
    fun anUnreadHistoryIsSaidWithTryAgainAndAReadingOneNeverSaysNoActivityYet() {
        val empty = WalletFixtures.buildMobileState(WalletScreenState.H2, strings)
        var current by mutableStateOf<WalletHomeModel>(empty)
        var retried = 0
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { WalletScreen(model = current, onActivityRetry = { retried += 1 }) }
            }
        }
        // Before: what the home said over a history nobody had read yet.
        compose.onNodeWithText(strings.t("home.emptyNoActivity")).performScrollTo().assertExists()
        shot("before-no-activity-yet")

        current = WalletLive.withMultisigHistory(empty, history("reading"))
        compose.waitForIdle()
        compose.onNodeWithText(strings.t("home.emptyNoActivity")).assertDoesNotExist()
        shot("after-reading")

        current = WalletLive.withMultisigHistory(empty, history("unread"))
        compose.waitForIdle()
        compose.onNodeWithText(strings.t("home.emptyNoActivity")).assertDoesNotExist()
        compose.onNodeWithTag("home.activity.retry").performScrollTo().assertExists()
        shot("after-unread-try-again")
        compose.onNodeWithTag("home.activity.retry").performClick()
        compose.waitForIdle()
        assertEquals("Try again reads the history again", 1, retried)
    }
}
