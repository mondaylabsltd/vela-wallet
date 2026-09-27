package app.getvela.wallet.feature.wallet.components

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ExperimentalMaterial3Api
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
import app.getvela.wallet.feature.wallet.ChainRowModel
import app.getvela.wallet.feature.wallet.SheetModel

/**
 * Chain-select bottom sheet (spec vocabulary #14/#15, mock H8): title 选择链
 * with a trailing search icon, then ChainFilterList rows — dot / name / count,
 * accent check on the active row, 所有网络 first.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChainSelectSheet(
    model: SheetModel,
    onDismiss: () -> Unit,
    onSelect: (ChainRowModel) -> Unit = {},
) {
    val colors = VelaTheme.colors
    VelaModalSheet(
        onDismissRequest = onDismiss,
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(
                    start = VelaSizing.screenPaddingX,
                    end = VelaSizing.screenPaddingX,
                    bottom = VelaSpacing.xl4,
                ),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = model.title,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl2,
                )
                Spacer(modifier = Modifier.weight(1f))
                Icon(
                    imageVector = VelaIcons.Search,
                    contentDescription = null,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.lg),
                )
            }
            Spacer(modifier = Modifier.heightIn(min = VelaSpacing.md))
            ChainFilterList(rows = model.rows, onSelect = onSelect)
        }
    }
}

/** Reusable chain list (also the desktop sidebar 网络 section on that platform). */
@Composable
fun ChainFilterList(
    rows: List<ChainRowModel>,
    modifier: Modifier = Modifier,
    onSelect: (ChainRowModel) -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        rows.forEach { row ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = VelaSizing.hitTarget)
                    .clickable { onSelect(row) },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                ChainDot(color = row.dot, size = WalletMetrics.listDotSize)
                Spacer(modifier = Modifier.width(VelaSpacing.lg))
                Text(
                    text = row.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.medium,
                    fontSize = VelaTextSize.lg,
                )
                Spacer(modifier = Modifier.weight(1f))
                if (row.selected) {
                    Icon(
                        imageVector = VelaIcons.Check,
                        contentDescription = null,
                        tint = colors.accentBase,
                        modifier = Modifier.size(VelaIconSize.base),
                    )
                    Spacer(modifier = Modifier.width(VelaSpacing.md))
                }
                Text(
                    text = row.count.toString(),
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.regular,
                    fontSize = VelaTextSize.base,
                )
            }
        }
    }
}
