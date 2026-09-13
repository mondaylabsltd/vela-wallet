package app.getvela.wallet.feature.flows.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
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
import app.getvela.wallet.core.marks.RemoteLogo

/** One network a list can be narrowed to: its id, its name, its logo when the chain-data endpoint has one. */
data class ChainFilterRow(val chainId: Int, val name: String, val logoUrl: String?, val badgeColor: androidx.compose.ui.graphics.Color)

/**
 * Spec 048: the 全部网络 pill's sheet (the web's phone chain sheet, 028 phase
 * 10) — every network this device has, or all of them. The choice is shell
 * render state, one per list; the feed machine is told, the asset and send
 * lists narrow themselves.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChainFilterSheet(
    title: String,
    allLabel: String,
    rows: List<ChainFilterRow>,
    selected: Int?,
    onPick: (Int?) -> Unit,
    onDismiss: () -> Unit,
) {
    val colors = VelaTheme.colors
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState, containerColor = colors.bgBase) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = VelaSpacing.xl)
                .padding(bottom = VelaSpacing.xl)
                .navigationBarsPadding(),
        ) {
            Text(
                text = title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.xl,
                modifier = Modifier.padding(bottom = VelaSpacing.lg),
            )
            ChainRow(label = allLabel, selected = selected == null, onClick = { onPick(null) }) {
                Box(modifier = Modifier.size(VelaIconSize.xl).background(colors.bgSunken, CircleShape))
            }
            rows.forEach { row ->
                ChainRow(label = row.name, selected = selected == row.chainId, onClick = { onPick(row.chainId) }) {
                    RemoteLogo(urls = listOfNotNull(row.logoUrl), size = VelaIconSize.xl) {
                        Box(modifier = Modifier.size(VelaIconSize.xl).background(row.badgeColor, CircleShape))
                    }
                }
            }
        }
    }
}

@Composable
private fun ChainRow(label: String, selected: Boolean, onClick: () -> Unit, mark: @Composable () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        mark()
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Text(
            text = label,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.weight(1f),
        )
        if (selected) {
            Icon(imageVector = VelaIcons.Check, contentDescription = null, tint = colors.accentBase, modifier = Modifier.size(VelaIconSize.md))
        }
    }
}
