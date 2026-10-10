package app.getvela.wallet.feature.scan

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.flows.components.QR_MODULES
import app.getvela.wallet.feature.flows.components.qrCell
import app.getvela.wallet.navigation.DEVELOPER_ROUTES
import java.util.concurrent.atomic.AtomicInteger
import kotlin.math.min

/**
 * Whether a session may open a camera at all.
 *
 * **A gallery, a board, a preview or a developer route never does.** They
 * exist to be looked at and swept by a script, often on a desk with a person
 * in front of it, and a scanner state that binds a real camera there
 * photographs whoever is sitting at the machine — a desktop sweep did exactly
 * that (the integration's note 7). In those sessions the scanner draws
 * [ScanFixtureFrame] — the brackets over the design fixtures' sample code —
 * and the camera API is not reached: no `ProcessCameraProvider`, no bind.
 *
 * The gate sits at the ONE place a camera is started ([CameraScanner]), so no
 * caller can forget it, and it reads the signal those sessions already carry:
 * the launch extras a debug build honours (`vela.gallery`, a
 * `vela.startDestination` naming a developer route — a release build honours
 * neither, so it is never a fixture session), and Compose's inspection mode
 * for a `@Preview`.
 */
object ScanCamera {
    /**
     * This process was launched onto a gallery, a board or a developer
     * route. Set once, in `MainActivity.onCreate`, from the extras the build
     * honoured; a plain launch sets it back.
     */
    @Volatile
    var fixtureSession: Boolean = false

    /** Every time a camera start was actually reached — a test's evidence that in a fixture session it never is. */
    val starts = AtomicInteger(0)

    /** The launch rule, pure: the onboarding gallery, or a start route only the developer extra can reach. */
    fun isFixtureSession(startDestination: String, gallery: Boolean): Boolean =
        gallery || startDestination in DEVELOPER_ROUTES

    /** The scanner draws the fixture frame instead of starting a camera. */
    fun fixtureOnly(inspection: Boolean): Boolean = fixtureSession || inspection
}

/** The fixture frame's test tag. */
const val SCAN_FIXTURE_TAG: String = "scan-fixture-frame"

/**
 * What the scanner "sees" where no camera may be opened: the design
 * fixtures' sample code (the deterministic demo pattern the receive boards
 * draw — it encodes nothing) on a still surface. Drawn, never captured.
 */
@Composable
fun ScanFixtureFrame(modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Box(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgSunken)
            .testTag(SCAN_FIXTURE_TAG)
            .clearAndSetSemantics { },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize(fraction = SAMPLE_FRACTION)
                .aspectRatio(1f)
                // White in both appearances, as a printed code is.
                .background(VelaOnAccent, RoundedCornerShape(VelaRadius.md))
                .padding(VelaSpacing.lg),
        ) {
            // The receive card's own ink: a code is drawn in one ink everywhere.
            val ink = colors.fixed.shadowInk
            Canvas(modifier = Modifier.fillMaxSize()) {
                val module = min(size.width, size.height) / QR_MODULES
                for (r in 0 until QR_MODULES) {
                    for (c in 0 until QR_MODULES) {
                        if (qrCell(r, c)) drawRect(color = ink, topLeft = Offset(c * module, r * module), size = Size(module, module))
                    }
                }
            }
        }
    }
}

private const val SAMPLE_FRACTION = 0.62f
