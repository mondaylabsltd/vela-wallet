package app.getvela.wallet.feature.settings.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.passkey.PasskeyProviderMark
import app.getvela.wallet.feature.settings.WalletKeyRowModel
import app.getvela.wallet.feature.settings.WalletKeysModel

/**
 * The keys that control this wallet, and their Ethereum backup (spec 062).
 *
 * A key row says what a person can act on: what it is called, WHO is holding it
 * (the vault's own mark and name when the core's catalog knows — a passkey
 * manager, a security key), a short fingerprint that tells two unnamed keys
 * apart, and whether it is synced. The backup is the block's last row and its
 * only button.
 *
 * De-containered and hairline-divided like the rest of settings: facts to read,
 * not cards to tap. Metrics are [VelaSettingsRow]'s, so names line up with the
 * page below.
 */
@Composable
fun VelaWalletKeysBlock(model: WalletKeysModel, onRow: (String) -> Unit) {
    val colors = VelaTheme.colors
    Column(modifier = Modifier.fillMaxWidth().padding(top = VelaSpacing.xl2)) {
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.xl,
            )
            if (model.count.isNotEmpty()) {
                Text(text = model.count, color = colors.fgSubtle, fontFamily = VelaFontFamily, fontSize = VelaTextSize.base)
            }
        }
        Text(
            text = model.subtitle,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(top = VelaSpacing.xs, bottom = VelaSpacing.md),
        )

        if (model.loading) {
            // The shape of one row, so the block does not jump when the answer lands.
            Row(
                modifier = Modifier.fillMaxWidth().heightIn(min = VelaSizing.controlLg).padding(vertical = VelaSpacing.lg).clearAndSetSemantics {},
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
            ) {
                Box(Modifier.size(VelaIconSize.lg).clip(CircleShape).background(colors.borderBase))
                Box(Modifier.width(VelaSpacing.xl5 * 3).height(VelaSpacing.md).clip(CircleShape).background(colors.borderBase))
            }
            SettingsDivider()
        } else {
            model.rows.forEach { row ->
                KeyRow(row)
                SettingsDivider()
            }
            model.note?.let {
                Text(
                    text = it,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    modifier = Modifier.padding(vertical = VelaSpacing.md),
                )
            }
        }

        model.backup?.let { VelaSettingsRow(row = it, divider = false, onClick = onRow) }
    }
}

@Composable
private fun KeyRow(row: WalletKeyRowModel) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = VelaSizing.controlLg)
            .padding(vertical = VelaSpacing.lg)
            // One sentence for a screen reader, not four fragments.
            .semantics(mergeDescendants = true) {
                contentDescription = listOfNotNull(row.name, row.holder, row.badge).joinToString(", ")
            },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Box(modifier = Modifier.size(VelaIconSize.lg), contentAlignment = Alignment.Center) {
            val drawn = PasskeyProviderMark(key = row.key, label = "", size = VelaIconSize.lg)
            if (!drawn) {
                Icon(imageVector = VelaIcons.Lock, contentDescription = null, tint = colors.fgMuted, modifier = Modifier.size(VelaIconSize.lg))
            }
        }
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            Text(
                text = row.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = buildAnnotatedString {
                    append(row.holder)
                    if (row.fingerprint.isNotEmpty()) {
                        append("  ·  ")
                        withStyle(SpanStyle(fontFamily = VelaMonoFontFamily)) { append(row.fingerprint) }
                    }
                },
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        row.badge?.let {
            Text(
                text = it,
                color = if (row.badgeSynced) colors.successBase else colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                maxLines = 1,
            )
        }
    }
}
