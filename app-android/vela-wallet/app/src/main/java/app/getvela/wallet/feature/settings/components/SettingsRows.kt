package app.getvela.wallet.feature.settings.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.feature.settings.AccountRowModel
import app.getvela.wallet.feature.settings.CheckItemModel
import app.getvela.wallet.feature.settings.KeyValueRowModel
import app.getvela.wallet.feature.settings.NetworkRowModel
import app.getvela.wallet.feature.settings.RowTone
import app.getvela.wallet.feature.settings.RowTrailing
import app.getvela.wallet.feature.settings.SelectRowModel
import app.getvela.wallet.feature.settings.SettingsRowModel
import app.getvela.wallet.feature.settings.StorageGroupModel
import app.getvela.wallet.feature.settings.StorageSegmentModel

/**
 * The settings list's rows (spec 023).
 *
 * Every entry on ST1/ST1b is [VelaSettingsRow]; every choice in the five
 * pickers is [VelaSelectRow]; every network is [VelaNetworkRow]. There is no
 * second row component anywhere in this feature.
 */

/**
 * One settings row: an optional leading glyph, a title, an optional second
 * line, an optional right-aligned value, and a trailing chevron or external
 * mark. `Danger` is the red 退出登录 / 清理数据 tone.
 */
@Composable
fun VelaSettingsRow(
    row: SettingsRowModel,
    modifier: Modifier = Modifier,
    divider: Boolean = true,
    onClick: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    val tint = when (row.tone) {
        RowTone.Default -> colors.fgBase
        RowTone.Accent -> colors.accentBase
        RowTone.Danger -> colors.errorBase
    }
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                // No action, no ripple (dead-controls #8): a control that
                // cannot act is not dressed as one. The Ethereum-backup row
                // pressed in under the finger in three of its four states and
                // the handler returned in silence.
                .then(if (row.actionable) Modifier.clickable { onClick(row.id) } else Modifier)
                .heightIn(min = VelaSizing.controlLg)
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            if (row.icon != null) {
                Icon(
                    imageVector = settingsIcon(row.icon),
                    contentDescription = null,
                    tint = if (row.tone == RowTone.Default) colors.fgMuted else tint,
                    modifier = Modifier.size(VelaIconSize.lg),
                )
            }
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
            ) {
                Text(
                    text = row.title,
                    color = tint,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (row.subtitle != null) {
                    Text(
                        text = row.subtitle,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            if (row.value != null) {
                Text(
                    text = row.value,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    maxLines = 1,
                )
            }
            when (row.trailing) {
                RowTrailing.Chevron -> Icon(
                    imageVector = VelaIcons.ChevronRight,
                    contentDescription = null,
                    tint = colors.fgSubtle,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
                RowTrailing.External -> Icon(
                    imageVector = VelaIcons.ExternalLink,
                    contentDescription = null,
                    tint = colors.fgSubtle,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
                // "Ask again" rather than "go somewhere": the row's own check
                // runs afresh and the subtitle goes back to 检查中.
                RowTrailing.Retry -> Icon(
                    imageVector = VelaIcons.RefreshCw,
                    contentDescription = null,
                    tint = colors.fgSubtle,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
                RowTrailing.None -> Unit
            }
        }
        if (divider) SettingsDivider()
    }
}

/**
 * ST1's identity block: identicon, name, truncated address, and a trailing TEXT
 * action rather than a bare chevron — "切换账户 ›" says what the tap does,
 * which a chevron alone does not.
 */
@Composable
fun VelaAccountRow(
    account: AccountRowModel,
    modifier: Modifier = Modifier,
    onClick: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onClick)
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            IdenticonImage(
                seed = account.addressFull,
                size = VelaSpacing.xl4 + VelaSpacing.md,
                contentDescription = account.name,
            )
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
            ) {
                Text(
                    text = account.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = account.addressDisplay,
                    color = colors.fgSubtle,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
            Text(
                text = account.action,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
            Icon(
                imageVector = VelaIcons.ChevronRight,
                contentDescription = null,
                tint = colors.fgSubtle,
                modifier = Modifier.size(VelaIconSize.sm),
            )
        }
        SettingsDivider()
    }
}

/**
 * One choice in a picker. Five sheets are made of nothing else: language,
 * currency, number, date and time. The differences are all data — a leading
 * currency glyph, a trailing note, the mono face every format sample wants.
 */
@Composable
fun VelaSelectRow(
    row: SelectRowModel,
    modifier: Modifier = Modifier,
    onClick: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onClick(row.id) }
                .heightIn(min = VelaSizing.controlLg)
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            if (row.glyph != null) {
                Box(
                    modifier = Modifier
                        .size(VelaSpacing.xl4)
                        .clip(RoundedCornerShape(VelaRadius.full))
                        .background(colors.bgRaised),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = row.glyph,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
            // The chosen row is stated twice — accent text and a check —
            // because the check alone disappears at the note's type size.
            Column(modifier = if (row.detail != null) Modifier.weight(1f) else Modifier) {
                Text(
                    text = row.label,
                    color = if (row.selected) colors.accentBase else colors.fgBase,
                    fontFamily = if (row.mono) VelaMonoFontFamily else VelaFontFamily,
                    fontWeight = if (row.selected) VelaFontWeight.semibold else VelaFontWeight.regular,
                    fontSize = VelaTextSize.lg,
                )
                if (row.detail != null) {
                    Text(
                        text = row.detail,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
            if (row.caption != null) {
                Text(
                    text = row.caption,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
            Spacer(modifier = Modifier.weight(1f))
            if (row.note != null) {
                Text(
                    text = row.note,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    maxLines = 1,
                )
            }
            if (row.selected) {
                Icon(
                    imageVector = VelaIcons.Check,
                    contentDescription = null,
                    tint = colors.accentBase,
                    modifier = Modifier.size(VelaIconSize.md),
                )
            }
        }
        SettingsDivider()
    }
}

/**
 * One network row (ST9 / ST10's results): mark, name, chain-id line, optional
 * latency pill, optional 自定义 tag, and a bin for the ones that can go.
 */
@Composable
fun VelaNetworkRow(
    row: NetworkRowModel,
    modifier: Modifier = Modifier,
    deleteLabel: String? = null,
    onClick: (String) -> Unit = {},
    onDelete: (String) -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onClick(row.id) }
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            VelaChainMark(row.mark)
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                ) {
                    Text(
                        text = row.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.lg,
                    )
                    if (row.tag != null) {
                        Text(
                            text = row.tag,
                            color = colors.warningBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.medium,
                            fontSize = VelaTextSize.xs,
                            modifier = Modifier
                                .clip(RoundedCornerShape(VelaRadius.sm))
                                .background(colors.warningSoft)
                                .padding(horizontal = VelaSpacing.md, vertical = VelaSpacing.xs),
                        )
                    }
                }
                Text(
                    text = row.meta,
                    color = colors.fgSubtle,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.sm,
                )
            }
            if (row.badge != null) VelaStatusPill(row.badge)
            if (row.removable && deleteLabel != null) {
                Icon(
                    imageVector = VelaIcons.Trash2,
                    contentDescription = deleteLabel,
                    tint = colors.fgSubtle,
                    modifier = Modifier
                        .size(VelaIconSize.md)
                        .clickable { onDelete(row.id) },
                )
            }
            Icon(
                imageVector = VelaIcons.ChevronRight,
                contentDescription = null,
                tint = colors.fgSubtle,
                modifier = Modifier.size(VelaIconSize.sm),
            )
        }
        SettingsDivider()
    }
}

/**
 * ST10b/ST10c's compatibility checklist. Both verdicts show all four rows — a
 * shortened list would hide WHICH requirement failed, and that is the only
 * useful part of an "incompatible" answer.
 */
@Composable
fun VelaCheckList(title: String, items: List<CheckItemModel>, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.bgSunken)
            .border(VelaBorder.hairline, colors.borderBase, RoundedCornerShape(VelaRadius.lg))
            .padding(VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Text(
            text = title,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
        items.forEach { item ->
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
            ) {
                Icon(
                    imageVector = if (item.ok) VelaIcons.Check else VelaIcons.Close,
                    contentDescription = null,
                    tint = if (item.ok) colors.successBase else colors.errorBase,
                    modifier = Modifier.size(VelaIconSize.md),
                )
                Text(
                    text = item.label,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }
    }
}

/** ST13's stacked bar plus legend. Shares, not pixels — true at any width. */
@Composable
fun VelaStorageBar(segments: List<StorageSegmentModel>, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(VelaSpacing.md)
                .clip(RoundedCornerShape(VelaRadius.sm))
                .background(colors.bgSunken),
        ) {
            // An empty share draws nothing: Compose refuses a zero weight, and
            // an emptied group (the last dApp disconnected, say) took the whole
            // app down with it (device-found, spec 070).
            segments.filter { it.fraction > 0f && it.fraction.isFinite() }.forEach { segment ->
                Box(
                    modifier = Modifier
                        .weight(segment.fraction)
                        .fillMaxWidth()
                        .height(VelaSpacing.md)
                        .background(Color(segment.colorArgb)),
                )
            }
        }
        Row(
            modifier = Modifier.padding(top = VelaSpacing.md),
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
        ) {
            segments.forEach { segment ->
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                ) {
                    Box(
                        modifier = Modifier
                            .size(VelaSpacing.md)
                            .clip(RoundedCornerShape(VelaRadius.full))
                            .background(Color(segment.colorArgb)),
                    )
                    Text(
                        text = segment.label,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                    )
                }
            }
        }
    }
}

/**
 * One storage group. The group label carries the consequence — "清除后无法找回"
 * against "清除后自动重建" — which is why the same word 清除 is red in the first
 * group and plain in the second.
 */
@Composable
fun VelaStorageGroup(
    group: StorageGroupModel,
    modifier: Modifier = Modifier,
    onClear: (String) -> Unit = {},
    onGroupAction: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth().padding(top = VelaSpacing.xl)) {
        Text(
            text = group.label,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.padding(bottom = VelaSpacing.sm),
        )
        group.items.forEach { item ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = VelaSizing.controlMd)
                    .padding(vertical = VelaSpacing.lg),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
            ) {
                Text(
                    text = item.label,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.lg,
                    // "Custom tokens and networks" does not fit beside its size
                    // and its Clear on a 392dp screen, and one line clipped it
                    // to "Custom tokens and netw…". The label is what the row
                    // IS, so it wraps; the size and the action keep their space.
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    text = item.meta,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.sm,
                    maxLines = 1,
                )
                Text(
                    text = item.action,
                    color = if (item.destructive) colors.errorBase else colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    modifier = Modifier.clickable { onClear(item.id) },
                )
            }
            SettingsDivider()
        }
        if (group.action != null) {
            Text(
                text = group.action,
                color = colors.infoBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                // Centred, so it reads as the group's own action rather than as
                // one more left-aligned row in the list it follows.
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onGroupAction)
                    .padding(top = VelaSpacing.xl, bottom = VelaSpacing.md),
            )
        }
    }
}

/**
 * About's technical-detail and link rows: label at the start, value at the end,
 * mono where the value is an identifier, external mark where it is a place.
 */
@Composable
fun VelaKeyValueRow(row: KeyValueRowModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = VelaSizing.controlMd)
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Text(
                text = row.label,
                color = if (row.external) colors.fgBase else colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = if (row.external) VelaFontWeight.semibold else VelaFontWeight.regular,
                fontSize = VelaTextSize.base,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = row.value,
                color = if (row.external) colors.fgSubtle else colors.fgBase,
                fontFamily = if (row.mono) VelaMonoFontFamily else VelaFontFamily,
                fontSize = VelaTextSize.base,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (row.external) {
                Icon(
                    imageVector = VelaIcons.ExternalLink,
                    contentDescription = null,
                    tint = colors.fgSubtle,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
            }
        }
        SettingsDivider()
    }
}

/**
 * ST1b's 清理数据 card — the one place in settings drawn as a bordered box
 * rather than a hairline row, because it is the only action on the screen that
 * cannot be undone.
 */
@Composable
fun VelaDangerCard(
    title: String,
    subtitle: String,
    modifier: Modifier = Modifier,
    onClick: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(VelaRadius.lg))
            .background(colors.errorSoft)
            .border(VelaBorder.hairline, colors.errorBase, RoundedCornerShape(VelaRadius.lg))
            .clickable(onClick = onClick)
            .padding(VelaSpacing.xl),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
        ) {
            Text(
                text = title,
                color = colors.errorBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
            Text(
                text = subtitle,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
        Icon(
            imageVector = VelaIcons.Trash2,
            contentDescription = null,
            tint = colors.errorBase,
            modifier = Modifier.size(VelaIconSize.lg),
        )
    }
}
