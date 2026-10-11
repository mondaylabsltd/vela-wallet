package app.getvela.wallet

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.browser.core.DbrConsentView
import app.getvela.wallet.feature.browser.core.OriginLock
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreenState
import app.getvela.wallet.feature.explore.SiteLock
import app.getvela.wallet.feature.explore.components.AddressBar
import app.getvela.wallet.feature.explore.components.AddressBarLabels
import app.getvela.wallet.feature.explore.components.ConnectionPanel
import app.getvela.wallet.feature.explore.components.SITE_LOCK_INSECURE_TAG
import app.getvela.wallet.feature.explore.components.SITE_LOCK_LOCAL_TAG
import app.getvela.wallet.feature.explore.components.SITE_LOCK_SECURE_TAG
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Device walk 2026-10-11, item 11, as it is DRAWN: the address bar and the
 * connect sheet put a padlock beside `http://127.0.0.1:8137`.
 *
 * The real address bar and the real consent panel, for each kind of page:
 * exactly one mark, and the right one — the padlock for https alone; for a
 * local page the neutral mark, said as "Local page — not encrypted", with
 * the words "Local page" beside it on the sheet; the open lock, said as
 * insecure, for public http.
 *
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SiteLockMarkTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class SiteLockMarkTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply { initialize("zh") }
    }
    private val saysLocal = strings.t("connect.browser.a11yLocal")
    private val saysInsecure = strings.t("connect.browser.a11yInsecure")

    /** How many of each mark are on screen: padlock, local, open lock. */
    private fun marks() = listOf(SITE_LOCK_SECURE_TAG, SITE_LOCK_LOCAL_TAG, SITE_LOCK_INSECURE_TAG)
        .map { tag -> compose.onAllNodes(hasTestTag(tag), useUnmergedTree = true).fetchSemanticsNodes().size }

    @Test
    fun theAddressBarDrawsAPadlockForHttpsAlone() {
        val fixture = ExploreFixtures.buildState(ExploreScreenState.E2, strings).browser
        var browser by mutableStateOf(fixture.copy(host = "127.0.0.1:8137", url = "http://127.0.0.1:8137/", lock = SiteLock.Local))
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    AddressBar(
                        browser = browser,
                        labels = AddressBarLabels(back = "back", account = "account", connected = "connected", tabs = "tabs", menu = "menu", field = "field", insecure = saysInsecure, local = saysLocal),
                        onBack = {}, onAccount = {}, onTabs = {}, onMenu = {},
                    )
                }
            }
        }
        compose.waitForIdle()
        assertEquals("a local page: its own mark, no padlock, no warning", listOf(0, 1, 0), marks())
        compose.onNode(hasTestTag(SITE_LOCK_LOCAL_TAG) and hasContentDescription(saysLocal), useUnmergedTree = true).assertExists()

        browser = fixture.copy(host = "arcorbs.game3.app", url = "https://arcorbs.game3.app/", lock = SiteLock.Secure)
        compose.waitForIdle()
        assertEquals("https: the padlock", listOf(1, 0, 0), marks())

        browser = fixture.copy(host = "dapp.example", url = "http://dapp.example/", lock = SiteLock.Insecure)
        compose.waitForIdle()
        assertEquals("public http: the open lock", listOf(0, 0, 1), marks())
        compose.onNode(hasTestTag(SITE_LOCK_INSECURE_TAG) and hasContentDescription(saysInsecure), useUnmergedTree = true).assertExists()

        // No mark at all for a page that is not there (a failure panel, a pending load).
        browser = fixture.copy(host = "arcorbs.game3.app", lock = SiteLock.Secure, lockShown = false)
        compose.waitForIdle()
        assertEquals(listOf(0, 0, 0), marks())
    }

    @Test
    fun theConnectSheetSaysLocalPageAndDrawsNoPadlock() {
        val fallback = ExploreFixtures.buildState(ExploreScreenState.E2, strings).connection
        var consent by mutableStateOf(DbrConsentView(tab = "t1", origin = "http://127.0.0.1:8137", lock = OriginLock.Local))
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    Column {
                        ConnectionPanel(
                            connection = ExploreLive.consent(fallback, consent, strings, ExploreLive.Identity()),
                            closeLabel = "close", onClose = {}, onDisconnect = {},
                        )
                    }
                }
            }
        }
        compose.waitForIdle()
        assertEquals(listOf(0, 1, 0), marks())
        compose.onNodeWithText(strings.t("explore.localSite")).assertExists()

        consent = DbrConsentView(tab = "t1", origin = "https://arcorbs.game3.app", lock = OriginLock.Secure)
        compose.waitForIdle()
        assertEquals(listOf(1, 0, 0), marks())
        compose.onNodeWithText(strings.t("explore.localSite")).assertDoesNotExist()
    }
}
