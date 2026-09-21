package app.getvela.wallet.feature.signing

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.explore.components.LetterAvatar
import app.getvela.wallet.feature.signing.components.SigningPositive

/**
 * The fee row, and its expanded fee-token selector (mock CS33) — the last thing
 * between the request and the slide.
 */
@Composable
fun SigningFee(
    fee: FeeModel,
    modifier: Modifier = Modifier,
    /** The row's tap: retry a failed quote, or open / close the coin list. */
    onFee: () -> Unit = {},
    /** A coin from the list, by id (`SigningLive.NATIVE_FEE_ID` for the chain's own). */
    onPick: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    when (fee) {
        is FeeModel.Hidden -> Unit
        is FeeModel.OffChain -> SigningPositive(fee.note, modifier, quiet = true)
        is FeeModel.OnChain -> Column(modifier = modifier.fillMaxWidth()) {
            SigningFeeBody(fee, onFee, onPick)
            // Issue #262: the reason the slide below is shut, said where the fix is.
            fee.warning?.let {
                Text(
                    text = it,
                    color = colors.errorBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.medium,
                    fontSize = VelaTextSize.sm,
                    modifier = Modifier.padding(top = VelaSpacing.sm, start = VelaSpacing.xl, end = VelaSpacing.xl),
                )
            }
        }
    }
}

@Composable
private fun SigningFeeBody(fee: FeeModel.OnChain, onFee: () -> Unit, onPick: (String) -> Unit) {
    val colors = VelaTheme.colors
    run {
        if (fee.selectorTitle == null) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(VelaRadius.lg))
                    .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
                    .clickable(enabled = fee.tappable, onClick = onFee)
                    .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.lg),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = fee.label,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                ) {
                    Text(
                        text = fee.value,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                    Icon(
                        VelaIcons.ChevronRight, null, tint = colors.fgMuted,
                        modifier = Modifier.size(VelaIconSize.sm),
                    )
                }
            }
        } else {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
                    .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.md),
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(onClick = onFee)
                        .padding(vertical = VelaSpacing.md),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = fee.selectorTitle,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                    Icon(
                        VelaIcons.ChevronDown, null, tint = colors.fgMuted,
                        modifier = Modifier.size(VelaIconSize.sm),
                    )
                }
                fee.options.forEach { option ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(VelaRadius.lg))
                            .background(
                                if (option.selected) colors.bgRaised else Color.Transparent,
                                RoundedCornerShape(VelaRadius.lg),
                            )
                            // A coin that cannot pay is shown for context, never picked.
                            .clickable(enabled = !option.disabled) { onPick(option.id) }
                            .alpha(if (option.disabled) 0.45f else 1f)
                            .padding(VelaSpacing.md),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
                    ) {
                        LetterAvatar(
                            option.mark.letter, option.mark.tint, size = VelaSpacing.xl4,
                        )
                        Column(
                            Modifier.weight(1f),
                            verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
                        ) {
                            Text(
                                text = option.name,
                                color = colors.fgBase,
                                fontFamily = VelaFontFamily,
                                fontWeight = VelaFontWeight.semibold,
                                fontSize = VelaTextSize.xl,
                            )
                            Text(
                                text = option.balance,
                                color = colors.fgMuted,
                                fontFamily = VelaFontFamily,
                                fontSize = VelaTextSize.base,
                            )
                        }
                        Text(
                            text = option.fee,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                        )
                        if (option.selected) {
                            Icon(
                                VelaIcons.Check, null, tint = colors.accentBase,
                                modifier = Modifier.size(VelaIconSize.sm),
                            )
                        }
                    }
                }
            }
        }
    }
}
