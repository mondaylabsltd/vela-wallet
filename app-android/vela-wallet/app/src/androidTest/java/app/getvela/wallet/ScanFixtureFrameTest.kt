package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowHost
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.scan.LiveScanSurface
import app.getvela.wallet.feature.scan.SCAN_FIXTURE_TAG
import app.getvela.wallet.feature.scan.ScanCallbacks
import app.getvela.wallet.feature.scan.ScanCamera
import java.util.concurrent.atomic.AtomicInteger
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * PRIVACY — the integration's note 7: in a gallery, board or developer-route
 * session the scanner draws the fixture frame and reaches no camera start.
 *
 * The LIVE scanner is composed here — the very composable a session draws,
 * with a camera permission it would be granted — in a fixture session: the
 * frame is the fixture's sample code, the camera API was never reached
 * ([ScanCamera.starts] is counted at the one place it is), and nothing asked
 * for the permission. The gallery's own scanner board draws the same frame.
 *
 * No camera is opened by this test, in either case — that is what it checks.
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.ScanFixtureFrameTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ScanFixtureFrameTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
    }

    private var before = false

    @Before
    fun aFixtureSession() {
        before = ScanCamera.fixtureSession
        ScanCamera.fixtureSession = true
        ScanCamera.starts.set(0)
    }

    @After
    fun restore() {
        ScanCamera.fixtureSession = before
    }

    @Test
    fun theLiveScannerDrawsTheFixtureFrameAndStartsNoCamera() {
        val asked = AtomicInteger(0)
        val callbacks = ScanCallbacks(
            onDecoded = {},
            onClose = {},
            // A session would be granted it; a fixture session never asks.
            requestPermission = { asked.incrementAndGet(); true },
            pickImage = { null },
            permissionText = "permission",
            grantLabel = "grant",
            noQrFound = "none",
            cameraUnavailable = "unavailable",
            decodeFailed = "failed",
        )
        val model = FlowFixtures.scan(strings)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { LiveScanSurface(model = model, callbacks = callbacks) }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText(model.hint).assertExists()
        assertEquals("the brackets hold the fixture's sample code", 1, compose.onAllNodesWithTag(SCAN_FIXTURE_TAG, useUnmergedTree = true).fetchSemanticsNodes().size)
        assertEquals("no camera start was reached", 0, ScanCamera.starts.get())
        assertEquals("and nothing asked for the camera", 0, asked.get())
        // Neither the permission line nor "camera unavailable" is said over a frame that is not a camera's.
        compose.onNodeWithText("permission").assertDoesNotExist()
        compose.onNodeWithText("unavailable").assertDoesNotExist()
    }

    @Test
    fun theGallerysScannerBoardDrawsTheFixtureFrame() {
        val board = FlowFixtures.build(FlowState.S1, strings)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { FlowHost(model = board) }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText((board.base as FlowBase.Scan).model.hint).assertExists()
        assertEquals(1, compose.onAllNodesWithTag(SCAN_FIXTURE_TAG, useUnmergedTree = true).fetchSemanticsNodes().size)
        assertEquals(0, ScanCamera.starts.get())
    }
}
