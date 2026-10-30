package app.getvela.wallet.core.designsystem.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing

/**
 * Stepped progress bar, segmented mode (spec 014, A4–A8): [totalSteps] equal
 * segments, segments below [step] filled accent, the rest neutral. Purely
 * presentational — no animation, no timing (FR-011).
 */
@Composable
fun VelaStepProgress(
    step: Int,
    totalSteps: Int,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
    ) {
        repeat(totalSteps) { index ->
            Box(
                modifier = Modifier
                    .weight(1f)
                    .height(VelaSizing.progressBar)
                    .clip(RoundedCornerShape(VelaRadius.full))
                    .background(if (index < step) colors.fgBase else colors.borderBase),
            )
        }
    }
}

/**
 * Single-bar mode (spec 014, B1): one neutral track with an accent fill of
 * [fraction] width — the login flow's indeterminate wait presentation.
 */
@Composable
fun VelaStepProgress(
    fraction: Float,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(VelaSizing.progressBar)
            .clip(RoundedCornerShape(VelaRadius.full))
            .background(colors.borderBase),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth(fraction)
                .fillMaxHeight()
                .clip(RoundedCornerShape(VelaRadius.full))
                .background(colors.fgBase),
        )
    }
}
