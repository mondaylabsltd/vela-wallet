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
import app.getvela.wallet.feature.flows.components.FeeSpeedControl
import app.getvela.wallet.feature.signing.components.SigningPositive
import app.getvela.wallet.feature.wallet.components.TokenIcon
import app.getvela.wallet.feature.wallet.components.WalletMetrics

/**
 * The fee row, and its expanded fee-token selector (mock CS33) — the last thing
 * between the request and the slide. Under the row, inside the same card, the
 * speed control the send form draws (spec 069).
 */
@Composable
fun SigningFee(
    fee: FeeModel,
    modifier: Modifier = Modifier,
    /** The row's tap: retry a failed quote, or open / close the coin list. */
    onFee: () -> Unit = {},
    /** A coin from the list, by id (`SigningLive.NATIVE_FEE_ID` for the chain's own). */
    onPick: (String) -> Unit = {},
    /** Spec 069: the speed control under the fee — fold/unfold, and a speed by id. */
    onToggleSpeed: () -> Unit = {},
    onPickSpeed: (String) -> Unit = {},
    /** Spec 079: the refresh control (the send form's own). */
    onRefresh: (() -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    when (fee) {
        is FeeModel.Hidden -> Unit
        is FeeModel.OffChain -> SigningPositive(fee.note, modifier, quiet = true)
        is FeeModel.OnChain -> Column(modifier = modifier.fillMaxWidth()) {
            SigningFeeBody(fee, onFee, onPick, onToggleSpeed, onPickSpeed, onRefresh)
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
private fun SigningFeeBody(
    fee: FeeModel.OnChain,
    onFee: () -> Unit,
    onPick: (String) -> Unit,
    onToggleSpeed: () -> Unit,
    onPickSpeed: (String) -> Unit,
    onRefresh: (() -> Unit)?,
) {
    val colors = VelaTheme.colors
    run {
        if (fee.selectorTitle == null) Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(VelaRadius.lg))
                .background(colors.bgSunken, RoundedCornerShape(VelaRadius.lg)),
        ) {
            // Issue #438: a gap the label and the figure always keep (the iOS
            // row's 8). `SpaceBetween` had none to give once a long figure
            // filled the row: "Network fee~0.00783 ETH · ≈CN¥141.11". The
            // figure takes what the label leaves, and wraps at its end.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(enabled = fee.tappable, onClick = onFee)
                    .padding(horizontal = VelaSpacing.xl, vertical = VelaSpacing.lg),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                Text(
                    text = fee.label,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
                Row(
                    modifier = Modifier.weight(1f),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md, Alignment.End),
                ) {
                    Text(
                        text = fee.value,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                        textAlign = androidx.compose.ui.text.style.TextAlign.End,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                    if (fee.chevron) {
                        Icon(
                            VelaIcons.ChevronRight, null, tint = colors.fgMuted,
                            modifier = Modifier.size(VelaIconSize.sm),
                        )
                    }
                    fee.refreshLabel?.let { label ->
                        app.getvela.wallet.feature.flows.components.FeeRefreshButton(label = label, refreshing = fee.refreshing, onRefresh = onRefresh)
                    }
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
                    Column(Modifier.fillMaxWidth()) {
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
                            TokenIcon(mark = option.mark)
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
                        // Issue #408: why a greyed coin cannot pay, under its row and
                        // at full strength — the row's dimming is not a reason. Set
                        // in from the row's edge past the coin's mark, under the name.
                        option.reason?.let { reason ->
                            Text(
                                text = reason,
                                color = colors.errorBase,
                                fontFamily = VelaFontFamily,
                                fontWeight = VelaFontWeight.medium,
                                fontSize = VelaTextSize.sm,
                                modifier = Modifier.padding(
                                    start = VelaSpacing.md + WalletMetrics.avatarSize + VelaSpacing.lg,
                                    end = VelaSpacing.md,
                                    bottom = VelaSpacing.sm,
                                ),
                            )
                        }
                    }
                }
            }
        }
    }
}
