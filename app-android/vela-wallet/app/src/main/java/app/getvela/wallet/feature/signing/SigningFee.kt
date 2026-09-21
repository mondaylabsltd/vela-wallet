package app.getvela.wallet.feature.signing

import androidx.compose.foundation.background
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
import app.getvela.wallet.feature.flows.components.FeeSpeedControl
import app.getvela.wallet.feature.signing.components.SigningPositive

/**
 * The fee row, and its expanded fee-token selector (mock CS33) — the last thing
 * between the request and the slide. Under the row, inside the same card, the
 * speed control the send form draws (spec 069).
 */
@Composable
fun SigningFee(
    fee: FeeModel,
    modifier: Modifier = Modifier,
    onToggleSpeed: () -> Unit = {},
    onPickSpeed: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    when (fee) {
        is FeeModel.Hidden -> Unit
        is FeeModel.OffChain -> SigningPositive(fee.note, modifier, quiet = true)
        is FeeModel.OnChain -> if (fee.selectorTitle == null) Column(
            modifier = modifier
                .fillMaxWidth()
                .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg)),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
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
            fee.speed?.let { speed ->
                // The control pads its rows by `lg`; the row above by `xl`.
                FeeSpeedControl(
                    speed = speed,
                    modifier = Modifier.padding(horizontal = VelaSpacing.sm).padding(bottom = VelaSpacing.sm),
                    onToggle = onToggleSpeed,
                    onPick = onPickSpeed,
                )
            }
        } else {
            Column(
                modifier = modifier
                    .fillMaxWidth()
                    .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg))
                    .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.md),
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
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
                            .background(
                                if (option.selected) colors.bgRaised else Color.Transparent,
                                RoundedCornerShape(VelaRadius.lg),
                            )
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
