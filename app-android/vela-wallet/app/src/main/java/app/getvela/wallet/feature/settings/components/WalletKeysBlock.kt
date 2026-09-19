package app.getvela.wallet.feature.settings.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.feature.settings.KeyPillTone
import kotlinx.coroutines.delay
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
                KeyRow(row, model.copyLabel, model.copiedLabel)
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

        model.backup?.let {
            VelaSettingsRow(row = it, divider = false, onClick = onRow)
            // PUBLIC keys: "back up keys" read as handing over the keys themselves.
            Text(
                text = model.backupExplain,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.padding(start = VelaIconSize.lg + VelaSpacing.lg, bottom = VelaSpacing.md),
            )
        }
    }
}

@Composable
private fun KeyRow(row: WalletKeyRowModel, copyLabel: String, copiedLabel: String) {
    val colors = VelaTheme.colors
    val expandable = row.details.isNotEmpty()
    var open by rememberSaveable(row.fingerprint) { mutableStateOf(false) }
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                // The row is the summary; opening it shows what the registry holds
                // about this key. Nothing to open when only the device answered.
                .then(if (expandable) Modifier.clickable { open = !open } else Modifier)
                .heightIn(min = VelaSizing.controlLg)
                .padding(vertical = VelaSpacing.lg)
                // One sentence for a screen reader, not five fragments.
                .semantics(mergeDescendants = true) {
                    contentDescription = (listOf(row.name, row.holder) + row.pills.map { it.text }).joinToString(", ")
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
                // Under the name, not beside it: a phone has no room for a name
                // AND two pills on one line, and the name is what loses.
                if (row.pills.isNotEmpty()) {
                    Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm), modifier = Modifier.padding(top = VelaSpacing.xs)) {
                        row.pills.forEach { pill ->
                            val tint = when (pill.tone) {
                                KeyPillTone.Verified -> colors.infoBase
                                KeyPillTone.Synced -> colors.successBase
                                KeyPillTone.Local -> colors.fgMuted
                            }
                            Text(
                                text = pill.text,
                                color = tint,
                                fontFamily = VelaFontFamily,
                                fontSize = VelaTextSize.sm,
                                maxLines = 1,
                                modifier = Modifier
                                    .border(VelaBorder.hairline, tint, CircleShape)
                                    .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
                            )
                        }
                    }
                }
            }
            if (expandable) {
                Icon(
                    imageVector = VelaIcons.ChevronDown,
                    contentDescription = null,
                    tint = colors.fgSubtle,
                    modifier = Modifier.size(VelaIconSize.sm).rotate(if (open) 180f else 0f),
                )
            }
        }
        if (expandable && open) KeyDetails(row, copyLabel, copiedLabel)
    }
}

/** What a key row opens onto — the registry explorer's facts, the two a person pastes elsewhere copyable. */
@Composable
private fun KeyDetails(row: WalletKeyRowModel, copyLabel: String, copiedLabel: String) {
    val colors = VelaTheme.colors
    val clipboard = LocalClipboardManager.current
    var copied by remember { mutableStateOf("") }
    LaunchedEffect(copied) {
        if (copied.isNotEmpty()) {
            delay(1200)
            copied = ""
        }
    }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(bottom = VelaSpacing.lg)
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.bgSunken)
            .border(VelaBorder.hairline, colors.borderBase, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        row.details.forEach { detail ->
            Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                Text(text = detail.label.uppercase(), color = colors.fgMuted, fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm)
                Text(
                    text = detail.value,
                    color = colors.fgBase,
                    fontFamily = if (detail.mono) VelaMonoFontFamily else VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
                if (detail.copy) {
                    Text(
                        text = if (copied == detail.label) copiedLabel else copyLabel,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        modifier = Modifier
                            .clip(RoundedCornerShape(VelaRadius.md))
                            .border(VelaBorder.hairline, colors.borderStrong, RoundedCornerShape(VelaRadius.md))
                            .clickable {
                                clipboard.setText(AnnotatedString(detail.value))
                                copied = detail.label
                            }
                            .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
                    )
                }
            }
        }
    }
}
