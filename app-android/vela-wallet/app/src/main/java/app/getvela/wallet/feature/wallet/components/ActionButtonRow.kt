package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import app.getvela.wallet.core.designsystem.components.VelaCard
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.wallet.ActionsModel
import androidx.compose.ui.platform.testTag

/** One equal-width action card: glyph above label. */
data class ActionButtonItem(
    val icon: ImageVector,
    val label: String,
    val onClick: () -> Unit = {},
    /** A test tag for the card (Phase 4's journeys tap the home's Send by it). */
    val tag: String? = null,
)

/**
 * 收款 / 转账 / 扫码 dock (spec vocabulary #6): three equal raised cards,
 * icon above label. Taps have no destination yet (spec assumption).
 */
@Composable
fun ActionButtonRow(
    actions: ActionsModel,
    modifier: Modifier = Modifier,
    onReceive: () -> Unit = {},
    onSend: () -> Unit = {},
    onScan: () -> Unit = {},
) {
    ActionButtonRow(
        items = listOf(
            ActionButtonItem(VelaIcons.ArrowDownLeft, actions.receive, onReceive, tag = "home-receive"),
            ActionButtonItem(VelaIcons.ArrowUpRight, actions.send, onSend, tag = "home-send"),
            ActionButtonItem(VelaIcons.ScanLine, actions.scan, onScan, tag = "home-scan"),
        ),
        modifier = modifier,
    )
}

/**
 * Item-driven form of the same dock — spec 018 reuses it for the contact
 * detail's 转账 / 收款 / 二维码 cards (spec vocabulary #11 "[reuse] the 015
 * component with new items"), so there is still one card implementation.
 */
@Composable
fun ActionButtonRow(
    items: List<ActionButtonItem>,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        items.forEach { item ->
            ActionCard(
                icon = item.icon,
                label = item.label,
                onClick = item.onClick,
                modifier = Modifier.weight(1f).let { m -> item.tag?.let { m.testTag(it) } ?: m },
            )
        }
    }
}

@Composable
private fun ActionCard(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    VelaCard(modifier = modifier) {
        Column(
            modifier = Modifier
                .clickable(onClick = onClick)
                .fillMaxWidth()
                .padding(vertical = VelaSpacing.xl),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = colors.fgBase,
                modifier = Modifier.size(VelaIconSize.lg),
            )
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            Text(
                text = label,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.medium,
                fontSize = VelaTextSize.base,
                maxLines = 1,
            )
        }
    }
}
