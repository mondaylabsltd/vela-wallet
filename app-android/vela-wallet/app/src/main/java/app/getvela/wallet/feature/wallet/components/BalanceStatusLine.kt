package app.getvela.wallet.feature.wallet.components

import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.draw.alpha
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.BalanceStatusKind
import app.getvela.wallet.feature.wallet.BalanceStatusModel

/**
 * One-line tappable status under the hero amount (spec vocabulary #5):
 * warning (⚠ + warning color, mock H4) or refreshing (↻ + fg.muted, mock H6),
 * both with a trailing chevron.
 */
@Composable
fun BalanceStatusLine(
    model: BalanceStatusModel,
    modifier: Modifier = Modifier,
    onClick: () -> Unit = {},
) {
    StatusLineRow(model = model, modifier = modifier.clickable(onClick = onClick))
}

/**
 * The line's ROOM: the very row a status draws, one line of it, unseen and
 * unsaid and taking no tap. The hero keeps it from the first frame
 * ([BalanceDisplay]), so a status arriving, changing or going moves nothing
 * under it — it is the same row, so it is the same height.
 */
@Composable
fun BalanceStatusRoom(modifier: Modifier = Modifier) {
    StatusLineRow(
        model = BalanceStatusModel(BalanceStatusKind.Refreshing, " "),
        modifier = modifier.alpha(0f).clearAndSetSemantics { },
    )
}

@Composable
private fun StatusLineRow(model: BalanceStatusModel, modifier: Modifier) {
    val colors = VelaTheme.colors
    val tint = when (model.kind) {
        BalanceStatusKind.Warning -> colors.warningBase
        BalanceStatusKind.Refreshing -> colors.fgMuted
    }
    Row(
        modifier = modifier,
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
    ) {
        Icon(
            imageVector = when (model.kind) {
                BalanceStatusKind.Warning -> VelaIcons.TriangleAlert
                BalanceStatusKind.Refreshing -> VelaIcons.RefreshCw
            },
            contentDescription = null,
            tint = tint,
            modifier = Modifier.size(VelaIconSize.sm),
        )
        Text(
            text = model.text,
            color = tint,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.sm,
        )
        Icon(
            imageVector = VelaIcons.ChevronRight,
            contentDescription = null,
            tint = tint,
            modifier = Modifier.size(VelaIconSize.xs),
        )
    }
}
