package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.BalanceModel
import app.getvela.wallet.feature.wallet.BalanceStateKind

/**
 * Hero balance (spec vocabulary #4): label line (总余额 · USD), amount with
 * de-emphasised decimals, and exactly one of normal / zero-live / loading /
 * hidden, plus the optional BalanceStatusLine slot.
 */
@Composable
fun BalanceDisplay(
    model: BalanceModel,
    modifier: Modifier = Modifier,
    onToggleVisibility: () -> Unit = {},
    onStatusClick: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier) {
        Text(
            text = "${model.label} · ${model.currency}",
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.sm,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.sm))
        when (model.state) {
            BalanceStateKind.Normal -> AmountRow(model, onToggleVisibility)
            BalanceStateKind.ZeroLive -> {
                AmountRow(model, onToggleVisibility)
                Spacer(modifier = Modifier.height(VelaSpacing.md))
                LiveIndicatorRow(model.liveText.orEmpty())
            }
            BalanceStateKind.Loading -> SkeletonBalanceBlock()
            BalanceStateKind.Hidden -> HiddenRow(model, onToggleVisibility)
        }
        model.status?.let { status ->
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            BalanceStatusLine(model = status, onClick = onStatusClick)
        }
    }
}

@Composable
private fun AmountRow(model: BalanceModel, onToggle: () -> Unit = {}) {
    val colors = VelaTheme.colors
    // Spec 048: the amount itself hides the figures (the web's BalanceDisplay button).
    Row(modifier = Modifier.clickable(onClick = onToggle)) {
        Text(
            text = model.integer.orEmpty(),
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl5,
            modifier = Modifier.alignByBaseline(),
        )
        model.decimals?.let { decimals ->
            Text(
                text = model.decimalMark + decimals,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl3,
                modifier = Modifier.alignByBaseline(),
            )
        }
    }
}

@Composable
private fun LiveIndicatorRow(text: String) {
    val colors = VelaTheme.colors
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        Box(
            modifier = Modifier
                .size(WalletMetrics.liveDotSize)
                .alpha(rememberPulseAlpha())
                .background(colors.successBase, CircleShape),
        )
        Text(
            text = text,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.sm,
        )
    }
}

@Composable
private fun HiddenRow(model: BalanceModel, onToggleVisibility: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier.clickable(onClick = onToggleVisibility),verticalAlignment = Alignment.CenterVertically) {
        Text(
            text = model.integer.orEmpty(),
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl4,
        )
        Spacer(modifier = Modifier.width(VelaSpacing.md))
        IconButton(onClick = onToggleVisibility) {
            Icon(
                imageVector = VelaIcons.EyeOff,
                // While hidden, the affordance reveals: announce "show balance".
                contentDescription = model.a11yShow,
                tint = colors.fgMuted,
                modifier = Modifier.size(VelaIconSize.lg),
            )
        }
    }
}
