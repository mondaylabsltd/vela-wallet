package app.getvela.wallet.feature.contacts.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.contacts.AddressBlockModel

/**
 * 地址 block (spec vocabulary #12, mock C2): label, the full address in mono
 * split across the fixture's pre-computed lines, and a trailing copy button
 * kept vertically centred against the block.
 *
 * Lines arrive pre-split from the fixtures — nothing here measures or wraps.
 */
@Composable
fun AddressBlock(
    model: AddressBlockModel,
    modifier: Modifier = Modifier,
    onCopy: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Text(
            text = model.label,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(modifier = Modifier.weight(1f)) {
                model.lines.forEach { line ->
                    Text(
                        text = line,
                        color = colors.fgBase,
                        fontFamily = VelaMonoFontFamily,
                        fontWeight = VelaFontWeight.regular,
                        fontSize = VelaTextSize.base,
                        lineHeight = VelaLeading.normal * VelaTextSize.base,
                        softWrap = false,
                        maxLines = 1,
                    )
                }
            }
            Spacer(modifier = Modifier.width(VelaSpacing.lg))
            ContactsIconButton(
                icon = VelaIcons.Copy,
                contentDescription = model.copyLabel,
                onClick = onCopy,
                tint = colors.fgMuted,
            )
        }
    }
}
