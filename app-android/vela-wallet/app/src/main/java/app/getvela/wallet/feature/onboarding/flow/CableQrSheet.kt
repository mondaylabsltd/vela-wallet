package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import uniffi.vela_core_uniffi.cableQrMatrix

/**
 * "Sign in with your phone": the caBLE QR the OTHER device scans. Shown while a
 * [KeyMethod.Hybrid] ceremony is finding that phone; cleared when the phone's
 * own prompt takes over, or the ceremony ends. The matrix comes from the core
 * (`cableQrMatrix`, the same encoder every platform draws with), so the shell
 * owns only pixels.
 *
 * Putting it away — Cancel, a tap outside, a swipe down, Back — is [onCancel]:
 * the ceremony ends as the core's quiet cancel (issue #459). It used to let the
 * scan run its 90 s out and then say the phone had not connected.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CableQrSheet(payload: String, chooser: KeyChooser, onCancel: () -> Unit) {
    val strings = LocalVelaStrings.current
    // The hybrid row's own words, as the chooser that opened this card drew
    // them (087 F02): a sign-in's line is the scan — it creates nothing.
    val (title, line) = methodCopy(KeyMethod.Hybrid, chooser, strings)
    val colors = VelaTheme.colors
    val matrix = remember(payload) { cableQrMatrix(payload) }

    VelaModalSheet(
        onDismissRequest = onCancel,
        containerColor = colors.bgRaised,
        // Issue #447: the sheet's one job is this code. Opened half-way, its
        // lower part ran off the screen until somebody thought to drag it up.
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.xl2)
                .padding(bottom = VelaSpacing.xl3),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            // Issue #480: the code is never wider than CODE_MAX and Cancel is
            // never off the screen. The code used to fill the sheet's width
            // with no cap — on a tablet that is a 600dp square, taller than a
            // landscape window, and Cancel scrolled away under it (#447 again).
            // A Column measures Cancel first, so what this box is offered is
            // exactly the room above it: the code takes what is left of that
            // under the words, down to CODE_MIN; only a window too short even
            // for that scrolls, and then it is the words and the code that
            // scroll, with Cancel still in place.
            BoxWithConstraints(modifier = Modifier.weight(1f, fill = false).fillMaxWidth()) {
                val room = constraints.maxHeight
                val words: @Composable () -> Unit = {
                    Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg)) {
                        Text(
                            text = title,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.bold,
                            fontSize = VelaTextSize.xl2,
                        )
                        Text(
                            text = line,
                            color = colors.fgMuted,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                        )
                    }
                }
                Column(modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                    if (matrix != null) {
                        CodeUnderWords(room = room, gap = VelaSpacing.lg, words = words) { CableCode(matrix) }
                    } else {
                        words()
                    }
                }
            }
            // The desktop's QR card has the same button (`common.cancel`).
            VelaSecondaryButton(
                text = strings.t(I18nKeys.Common.CANCEL),
                onClick = onCancel,
                modifier = Modifier.fillMaxWidth().testTag(CABLE_CANCEL_TAG),
            )
        }
    }
}

/** The code's side at most — iOS draws the same 260. */
internal val CODE_MAX = 260.dp

/** … and at least: below this a phone's camera has to come too close. */
internal val CODE_MIN = 160.dp

internal const val CABLE_CODE_TAG = "cable.code"
internal const val CABLE_CANCEL_TAG = "cable.cancel"

/**
 * The words, then the code centred under them, the code a square of
 * [CODE_MAX] — or of what [room] (the height on offer, in px) leaves under
 * the words, never under [CODE_MIN], never wider than the sheet.
 */
@Composable
private fun CodeUnderWords(room: Int, gap: Dp, words: @Composable () -> Unit, code: @Composable () -> Unit) {
    Layout(content = { Box { words() }; Box { code() } }, modifier = Modifier.fillMaxWidth()) { measurables, constraints ->
        val wide = constraints.maxWidth
        val head = measurables[0].measure(Constraints(maxWidth = wide))
        val space = gap.roundToPx()
        val left = if (room == Constraints.Infinity) Int.MAX_VALUE else room - head.height - space
        val side = minOf(CODE_MAX.roundToPx(), wide, maxOf(left, CODE_MIN.roundToPx()))
        val drawn = measurables[1].measure(Constraints.fixed(side, side))
        layout(wide, head.height + space + side) {
            head.place(0, 0)
            drawn.place((wide - side) / 2, head.height + space)
        }
    }
}

/** The core's matrix as pixels: the light ground with its quiet zone, then only the dark modules. */
@Composable
private fun CableCode(matrix: uniffi.vela_core_uniffi.QrMatrix) {
    val width = matrix.width.toInt()
    Canvas(modifier = Modifier.fillMaxSize().testTag(CABLE_CODE_TAG)) {
        // A quiet zone keeps scanners happy.
        val quiet = 2
        val units = width + quiet * 2
        val cell = size.minDimension / units
        drawRect(color = Color.White, size = Size(size.width, size.height))
        for (row in 0 until width) {
            for (col in 0 until width) {
                if (matrix.modules[row * width + col]) {
                    drawRect(
                        color = Color.Black,
                        topLeft = Offset((col + quiet) * cell, (row + quiet) * cell),
                        size = Size(cell, cell),
                    )
                }
            }
        }
    }
}
