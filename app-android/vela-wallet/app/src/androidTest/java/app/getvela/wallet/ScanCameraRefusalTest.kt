package app.getvela.wallet

import android.content.Intent
import android.provider.Settings
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.platform.AppSettings
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.scan.LiveScanSurface
import app.getvela.wallet.feature.scan.ScanCallbacks
import app.getvela.wallet.feature.scan.ScanCamera
import java.util.concurrent.atomic.AtomicInteger
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * NATIVE-LAUNCH-CHECKLIST C: the scanner's "Grant permission" after a
 * permanent refusal opens this app's page in Settings (as iOS's does) instead
 * of asking a system that will show nothing; after a first refusal it asks.
 *
 * The permission answers are the test's own — no camera is opened and no
 * system dialog or Settings screen is reached.
 */
@RunWith(AndroidJUnit4::class)
class ScanCameraRefusalTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
    }

    private var before = false

    @Before
    fun aLiveSession() {
        before = ScanCamera.fixtureSession
        ScanCamera.fixtureSession = false
    }

    @After
    fun restore() {
        ScanCamera.fixtureSession = before
    }

    private fun scanner(showRationale: Boolean, asked: AtomicInteger, opened: AtomicInteger) {
        val callbacks = ScanCallbacks(
            onDecoded = {},
            onClose = {},
            // Refused, every time.
            requestPermission = { asked.incrementAndGet(); false },
            pickImage = { null },
            permissionText = "permission",
            grantLabel = "grant",
            settingsLabel = "settings",
            noQrFound = "none",
            cameraUnavailable = "unavailable",
            decodeFailed = "failed",
            showRationale = { showRationale },
            openSettings = { opened.incrementAndGet() },
        )
        val model = FlowFixtures.scan(strings)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { LiveScanSurface(model = model, callbacks = callbacks) }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("permission").assertExists()
        assertEquals("asked once on entry", 1, asked.get())
    }

    @Test
    fun aPermanentRefusalOpensSettings() {
        val asked = AtomicInteger(0)
        val opened = AtomicInteger(0)
        scanner(showRationale = false, asked = asked, opened = opened)
        // r14-10: the button says what it does — never "Grant Permission" over Settings.
        compose.onNodeWithText("grant").assertDoesNotExist()
        compose.onNodeWithText("settings").performClick()
        compose.waitForIdle()
        assertEquals("Settings opened", 1, opened.get())
        assertEquals("and the dead dialog was not asked for", 1, asked.get())
    }

    @Test
    fun aFirstRefusalAsksAgain() {
        val asked = AtomicInteger(0)
        val opened = AtomicInteger(0)
        scanner(showRationale = true, asked = asked, opened = opened)
        compose.onNodeWithText("settings").assertDoesNotExist()
        compose.onNodeWithText("grant").performClick()
        compose.waitForIdle()
        assertEquals(2, asked.get())
        assertEquals(0, opened.get())
    }

    /** r14-10: the label is the corpus's translated "Open Settings", as iOS's scanner would say it. */
    @Test
    fun theSettingsLabelIsTheTranslatedOpenSettings() {
        assertEquals("Open Settings", strings.t(app.getvela.wallet.core.i18n.I18nKeys.Flows.SCAN_OPEN_SETTINGS))
    }

    @Test
    fun theSettingsIntentIsThisAppsDetailsPage() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val intent = AppSettings.detailsIntent(context.packageName)
        assertEquals(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, intent.action)
        assertEquals("package:${context.packageName}", intent.data.toString())
        assertTrue(intent.flags and Intent.FLAG_ACTIVITY_NEW_TASK != 0)
        // A Settings app on the device takes it.
        assertTrue(context.packageManager.queryIntentActivities(intent, 0).isNotEmpty())
    }
}
