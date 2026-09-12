package app.getvela.wallet.feature.explore.components

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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.explore.ConnectionModel
import app.getvela.wallet.feature.explore.ExploreSheet
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar

/** The ⋯ sheet over a page (mock E6). */
@Composable
fun SiteMenuSheetContent(
    sheet: ExploreSheet.SiteMenu,
    closeLabel: String,
    onClose: () -> Unit,
    onPick: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.padding(horizontal = VelaSizing.screenPaddingX)) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = VelaSpacing.xl),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            LetterAvatar(sheet.site.letter, sheet.site.tint)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                Text(
                    text = sheet.site.host,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.xl2,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
                ) {
                    Icon(
                        VelaIcons.Lock, null, tint = colors.successBase,
                        modifier = Modifier.size(VelaIconSize.xs),
                    )
                    Text(
                        text = sheet.statusLine,
                        color = colors.successBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
            Icon(
                VelaIcons.Close, closeLabel, tint = colors.fgMuted,
                modifier = Modifier.clickable(onClick = onClose),
            )
        }

        sheet.items.forEachIndexed { index, item ->
            if (index > 0) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(VelaBorder.hairline)
                        .background(colors.borderBase),
                )
            }
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onPick(item.id) }
                    .padding(vertical = VelaSpacing.xl),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
            ) {
                val ink = if (item.danger) colors.errorBase else colors.fgBase
                Icon(item.icon, null, tint = ink)
                Text(
                    text = item.label,
                    color = ink,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.xl,
                )
            }
        }
        Spacer(Modifier.height(VelaSpacing.xl))
    }
}

/**
 * What a connected site can and cannot do (mock E7), in that order: who it is,
 * which account it sees, which network, then the sentence that says a
 * connection is not a permission to move money.
 */
@Composable
fun ConnectionPanel(
    connection: ConnectionModel,
    closeLabel: String,
    onClose: () -> Unit,
    onDisconnect: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .padding(horizontal = VelaSizing.screenPaddingX)
            .padding(bottom = VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            LetterAvatar(connection.site.letter, connection.site.tint)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                Text(
                    text = connection.site.host,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.xl2,
                    maxLines = 1,
                )
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
                ) {
                    Icon(
                        VelaIcons.Lock, null, tint = colors.successBase,
                        modifier = Modifier.size(VelaIconSize.xs),
                    )
                    Text(
                        text = connection.statusLine,
                        color = colors.successBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
            Icon(
                VelaIcons.Close, closeLabel, tint = colors.fgMuted,
                modifier = Modifier.clickable(onClick = onClose),
            )
        }

        Divider()

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            IdenticonAvatar(tappable = false, seed = connection.accountSeed)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
                Text(
                    text = connection.accountName,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.xl,
                )
                Text(
                    text = connection.accountAddress,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
            Text(
                text = connection.switchLabel,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
            Icon(
                VelaIcons.ChevronRight, null, tint = colors.fgMuted,
                modifier = Modifier.size(VelaIconSize.sm),
            )
        }

        Divider()

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = connection.networkLabel,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                Box(
                    Modifier
                        .size(VelaSpacing.md)
                        .background(connection.networkDot, CircleShape),
                )
                Text(
                    text = connection.networkName,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.lg,
                )
            }
        }

        Text(
            text = connection.explainer,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )

        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(VelaSizing.controlLg)
                .border(
                    VelaBorder.hairline, colors.borderStrong,
                    RoundedCornerShape(VelaRadius.lg),
                )
                .clickable(onClick = onDisconnect),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = connection.disconnect,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
        }

        Text(
            text = connection.footnote,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/**
 * Group management (mock E3), mirroring the contacts vocabulary spec 018 set.
 * System groups (收藏 / 最近的 dApp) can be hidden but never deleted: their
 * trash affordance is ABSENT rather than disabled, because an affordance that
 * is only ever refused is a lie about what is possible.
 */
@Composable
fun GroupManageSheetContent(
    sheet: ExploreSheet.GroupManage,
    hidden: Set<String>,
    closeLabel: String,
    hideLabel: String,
    showLabel: String,
    deleteLabel: String,
    onClose: () -> Unit,
    onToggle: (String) -> Unit,
    onNew: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Column(modifier = modifier.padding(horizontal = VelaSizing.screenPaddingX)) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = VelaSpacing.xl),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = sheet.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Icon(
                VelaIcons.Close, closeLabel, tint = colors.fgMuted,
                modifier = Modifier.clickable(onClick = onClose),
            )
        }

        sheet.rows.forEach { row ->
            val isHidden = row.hidden || hidden.contains(row.id)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = VelaSpacing.xl),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
            ) {
                Icon(VelaIcons.GripVertical, null, tint = colors.fgSubtle)
                // Hidden reads as hidden: the row dims, so the eye is a
                // confirmation rather than the only clue.
                Row(
                    modifier = Modifier
                        .weight(1f)
                        .alpha(if (isHidden) VelaOpacity.dim else 1f),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                ) {
                    Text(
                        text = row.title,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.xl,
                    )
                    row.meta?.let {
                        Text(
                            text = it,
                            color = colors.fgSubtle,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                        )
                    }
                }
                Icon(
                    if (isHidden) VelaIcons.EyeOff else VelaIcons.Eye,
                    if (isHidden) showLabel else hideLabel,
                    tint = colors.fgMuted,
                    modifier = Modifier.clickable { onToggle(row.id) },
                )
                if (!row.system) {
                    Icon(VelaIcons.Trash2, deleteLabel, tint = colors.fgMuted)
                }
            }
            Box(
                Modifier
                    .fillMaxWidth()
                    .height(VelaBorder.hairline)
                    .background(colors.borderBase),
            )
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onNew)
                .padding(vertical = VelaSpacing.xl),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Box(
                modifier = Modifier
                    .size(VelaSpacing.xl4)
                    .background(colors.bgSunken, CircleShape),
                contentAlignment = Alignment.Center,
            ) { Icon(VelaIcons.Plus, null, tint = colors.fgSubtle) }
            Text(
                text = sheet.newGroup,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.xl,
            )
        }
        Spacer(Modifier.height(VelaSpacing.xl))
    }
}

@Composable
private fun Divider() {
    Box(
        Modifier
            .fillMaxWidth()
            .height(VelaBorder.hairline)
            .background(VelaTheme.colors.borderBase),
    )
}
