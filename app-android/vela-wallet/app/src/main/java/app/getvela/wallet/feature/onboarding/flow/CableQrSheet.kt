package app.getvela.wallet.feature.onboarding.flow

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
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
                // A short or landscape screen can still be shorter than the code.
                .verticalScroll(rememberScrollState())
                .padding(horizontal = VelaSpacing.xl2)
                .padding(bottom = VelaSpacing.xl3),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
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
            if (matrix != null) {
                val width = matrix.width.toInt()
                val dark = Color.Black
                val light = Color.White
                Canvas(
                    modifier = Modifier
                        .fillMaxWidth()
                        .aspectRatio(1f)
                        .padding(vertical = VelaSpacing.md),
                ) {
                    // A quiet zone keeps scanners happy; draw the light ground
                    // then only the dark modules.
                    val quiet = 2
                    val units = width + quiet * 2
                    val cell = size.minDimension / units
                    drawRect(color = light, size = Size(size.width, size.height))
                    for (row in 0 until width) {
                        for (col in 0 until width) {
                            if (matrix.modules[row * width + col]) {
                                drawRect(
                                    color = dark,
                                    topLeft = Offset((col + quiet) * cell, (row + quiet) * cell),
                                    size = Size(cell, cell),
                                )
                            }
                        }
                    }
                }
            }
            // The desktop's QR card has the same button (`common.cancel`).
            VelaSecondaryButton(
                text = strings.t(I18nKeys.Common.CANCEL),
                onClick = onCancel,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
