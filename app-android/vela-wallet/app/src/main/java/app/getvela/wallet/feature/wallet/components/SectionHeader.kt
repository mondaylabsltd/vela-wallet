package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
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
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize

/**
 * Section header (spec vocabulary #7): bold title + trailing text action with
 * chevron (全部 › / 添加 ›).
 *
 * Spec 018 adds [showChevron] so the same header renders a plain trailing
 * count (联系人 · 8 位, mock C1) without a second implementation (SC-006).
 */
@Composable
fun SectionHeader(
    title: String,
    action: String,
    modifier: Modifier = Modifier,
    showChevron: Boolean = true,
    onAction: () -> Unit = {},
    /**
     * Spec 099 navigation: the action is a control of its own (the resume
     * section's 标签页 ›, which opens the switcher) and takes a full 44 target.
     */
    actionHitTarget: Boolean = false,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = title,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl2,
        )
        Spacer(modifier = Modifier.weight(1f))
        Row(
            modifier = Modifier
                .let { if (actionHitTarget) it.defaultMinSize(minHeight = VelaSizing.hitTarget) else it }
                .clickable(onClick = onAction),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
        ) {
            Text(
                text = action,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.medium,
                fontSize = VelaTextSize.base,
            )
            if (showChevron) {
                Icon(
                    imageVector = VelaIcons.ChevronRight,
                    contentDescription = null,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
            }
        }
    }
}
