package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize

/**
 * Decorative open arc — a frozen snapshot, deliberately NOT proportional to
 * the value and never animated (research D8, FR-011: no timing behaviour).
 */
private const val RING_SWEEP_DEGREES = 300f
private const val RING_START_DEGREES = -90f

/**
 * Elapsed-seconds ring (spec 014, A4c–A8c / B1c): static accent arc over a
 * neutral track with the frozen seconds value centered. Fits 1- and 2-digit
 * values without resizing. [contentDescription] is the localized
 * `onboarding.common.waitedSeconds` string, resolved by the caller.
 */
@Composable
fun VelaElapsedRing(
    seconds: Int,
    contentDescription: String,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Box(
        modifier = modifier
            .size(VelaSizing.elapsedRing)
            .clearAndSetSemantics { this.contentDescription = contentDescription },
        contentAlignment = Alignment.Center,
    ) {
        Canvas(modifier = Modifier.fillMaxSize()) {
            val strokeWidth = VelaSizing.elapsedRingStroke.toPx()
            val inset = strokeWidth / 2f
            val arcSize = Size(size.width - strokeWidth, size.height - strokeWidth)
            drawArc(
                color = colors.borderBase,
                startAngle = 0f,
                sweepAngle = 360f,
                useCenter = false,
                topLeft = Offset(inset, inset),
                size = arcSize,
                style = Stroke(width = strokeWidth),
            )
            drawArc(
                color = colors.fgBase,
                startAngle = RING_START_DEGREES,
                sweepAngle = RING_SWEEP_DEGREES,
                useCenter = false,
                topLeft = Offset(inset, inset),
                size = arcSize,
                style = Stroke(width = strokeWidth, cap = StrokeCap.Round),
            )
        }
        Text(
            text = seconds.toString(),
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.base,
        )
    }
}
