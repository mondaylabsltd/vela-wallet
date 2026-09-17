package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.ActivityKind
import app.getvela.wallet.feature.wallet.ActivityRowModel

/**
 * Day-group label above activity rows (今天 / 昨天; spec vocabulary #8).
 */
@Composable
fun DayLabel(label: String, modifier: Modifier = Modifier) {
    Text(
        text = label,
        color = VelaTheme.colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.medium,
        fontSize = VelaTextSize.sm,
        modifier = modifier.padding(top = VelaSpacing.lg, bottom = VelaSpacing.sm),
    )
}

/**
 * Activity row (spec vocabulary #8): direction circle with a chain-dot badge,
 * title + counterparty subtitle, trailing signed amount (+success / −fg.base)
 * with a small unit. Masked variant renders the fixture's dot glyphs while the
 * unit stays visible (mock H5).
 */
@Composable
fun ActivityRow(model: ActivityRowModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(contentAlignment = Alignment.BottomEnd) {
            Box(
                modifier = Modifier
                    .size(WalletMetrics.avatarSize)
                    .background(colors.bgSunken, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = when (model.kind) {
                        ActivityKind.Sent -> VelaIcons.ArrowUpRight
                        ActivityKind.Received -> VelaIcons.ArrowDownLeft
                        ActivityKind.Dapp -> VelaIcons.Link2
                    },
                    contentDescription = null,
                    tint = when (model.kind) {
                        ActivityKind.Received -> colors.successBase
                        else -> colors.fgMuted
                    },
                    modifier = Modifier.size(VelaIconSize.md),
                )
            }
            ChainBadge(color = model.badgeColor, logoUrl = model.badgeLogoUrl)
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = model.subtitle,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.base,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        // Fixed share for the amount so extreme values wrap the unit below
        // instead of clipping or overlapping the title (H7 edge case).
        Box(modifier = Modifier.weight(1f), contentAlignment = Alignment.CenterEnd) {
            AmountText(model)
        }
    }
}

@Composable
private fun AmountText(model: ActivityRowModel) {
    val colors = VelaTheme.colors
    val amountColor = if (model.positive) colors.successBase else colors.fgBase
    Text(
        text = buildAnnotatedString {
            withStyle(
                SpanStyle(
                    color = amountColor,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                ),
            ) {
                append(model.amount)
            }
            append(" ")
            withStyle(
                SpanStyle(
                    color = colors.fgSubtle,
                    fontWeight = VelaFontWeight.medium,
                    fontSize = VelaTextSize.sm,
                ),
            ) {
                append(model.unit)
            }
        },
        fontFamily = VelaFontFamily,
        textAlign = TextAlign.End,
    )
}
