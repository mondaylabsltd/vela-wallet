package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing

/**
 * Loading placeholders matching row geometry (spec vocabulary #12, mock H3).
 * Widths are fractions of the container; heights/radii come from tokens.
 */
@Composable
fun SkeletonBlock(modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .pulsing()
            .background(VelaTheme.colors.bgSunken, RoundedCornerShape(VelaRadius.md)),
    )
}

/** Hero balance placeholder (mock H3: one wide block under the label). */
@Composable
fun SkeletonBalanceBlock(modifier: Modifier = Modifier) {
    SkeletonBlock(
        modifier = modifier
            .fillMaxWidth(0.55f)
            .height(VelaSizing.controlSm),
    )
}

@Composable
private fun SkeletonCircle() {
    Box(
        modifier = Modifier
            .size(WalletMetrics.avatarSize)
            .pulsing()
            .background(VelaTheme.colors.bgSunken, CircleShape),
    )
}

/** Activity-row placeholder: circle + title line + trailing amount line. */
@Composable
fun SkeletonActivityRow(modifier: Modifier = Modifier) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        SkeletonCircle()
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            SkeletonBlock(
                modifier = Modifier
                    .fillMaxWidth(0.6f)
                    .height(VelaSpacing.lg),
            )
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        SkeletonBlock(
            modifier = Modifier
                .weight(0.4f)
                .height(VelaSpacing.lg),
        )
    }
}

/** Asset-row placeholder: circle + name line + two trailing lines. */
@Composable
fun SkeletonAssetRow(modifier: Modifier = Modifier) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        SkeletonCircle()
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            SkeletonBlock(
                modifier = Modifier
                    .fillMaxWidth(0.5f)
                    .height(VelaSpacing.lg),
            )
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(
            modifier = Modifier.weight(0.5f),
            horizontalAlignment = Alignment.End,
        ) {
            SkeletonBlock(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(VelaSpacing.lg),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.sm))
            SkeletonBlock(
                modifier = Modifier
                    .fillMaxWidth(0.6f)
                    .height(VelaSpacing.md),
            )
        }
    }
}
