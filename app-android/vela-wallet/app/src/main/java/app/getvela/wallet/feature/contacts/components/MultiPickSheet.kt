package app.getvela.wallet.feature.contacts.components

import androidx.compose.foundation.border
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.contacts.MultiPickModel
import app.getvela.wallet.feature.contacts.MultiPickRowModel
import app.getvela.wallet.feature.contacts.SearchFieldModel
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar

/**
 * C10 — a tick list and one button that commits the whole list: 添加成员
 * and 移入分组 (iOS's `MultiPickSheet`, the web's `PickList`).
 *
 * Both questions are answered as a SET, because that is how the core takes
 * them — `SetGroupMembers` and `SetContactGroups` replace the membership
 * outright — so a tick only changes the sheet, and 保存 commits. Closing
 * the sheet any other way changes nothing.
 *
 * Issue #437: the list is a lazy, scrolling column under a capped sheet. The
 * menu it replaces was a wrap-height column, so a book of 59 showed the 14
 * that fit the screen and nothing — not even its own Cancel — below them.
 * The cap matters as much as the scroll: a column with no maximum never
 * overflows, so there is nothing for it to scroll.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MultiPickSheet(
    model: MultiPickModel,
    onToggle: (String) -> Unit,
    onQueryChange: (String) -> Unit,
    onSave: () -> Unit,
    onDismiss: () -> Unit,
) {
    val colors = VelaTheme.colors
    val maxSheetHeight = (LocalConfiguration.current.screenHeightDp * 0.88f).dp
    VelaModalSheet(
        onDismissRequest = onDismiss,
        // Fully up at once: half-way, the save button would sit off screen.
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(max = maxSheetHeight)
                .padding(horizontal = VelaSizing.screenPaddingX)
                .padding(bottom = VelaSpacing.xl3),
        ) {
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            if (model.searchable) {
                Spacer(modifier = Modifier.height(VelaSpacing.lg))
                ContactsSearchField(
                    model = SearchFieldModel(placeholder = model.searchPlaceholder, query = model.query),
                    onClear = { onQueryChange("") },
                    onQueryChange = onQueryChange,
                )
            }
            if (model.rows.isEmpty()) {
                model.empty?.let { line ->
                    Text(
                        text = line,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                        textAlign = TextAlign.Center,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = VelaSpacing.xl3),
                    )
                }
            } else {
                // Takes what the title and the buttons leave, and no more: a
                // short list keeps the sheet short.
                LazyColumn(
                    modifier = Modifier
                        .weight(1f, fill = false)
                        .padding(top = VelaSpacing.md),
                ) {
                    items(model.rows, key = { it.id }) { row ->
                        PickRow(row = row, onToggle = { onToggle(row.id) })
                    }
                }
            }
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            VelaPrimaryButton(text = model.save, onClick = onSave, modifier = Modifier.fillMaxWidth())
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            VelaSecondaryButton(text = model.cancel, onClick = onDismiss, modifier = Modifier.fillMaxWidth())
        }
    }
}

@Composable
private fun PickRow(row: MultiPickRowModel, onToggle: () -> Unit) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            // A checkbox to accessibility too: a tick that only exists as a
            // drawing is invisible to somebody who cannot see it.
            .toggleable(value = row.picked, role = Role.Checkbox, onValueChange = { onToggle() })
            .heightIn(min = VelaSizing.hitTarget)
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val seed = row.identiconSeed
        if (seed != null) {
            IdenticonAvatar(seed = seed, size = ContactsMetrics.memberAvatar, tappable = false)
        } else {
            Box(
                modifier = Modifier
                    .size(ContactsMetrics.memberAvatar)
                    .border(VelaBorder.hairline, colors.borderBase, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = VelaIcons.UsersRound,
                    contentDescription = null,
                    tint = colors.fgSubtle,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
            }
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = row.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = row.subtitle,
                color = colors.fgMuted,
                // An address reads in the mono face, as on the list page.
                fontFamily = if (seed != null) VelaMonoFontFamily else VelaFontFamily,
                fontSize = VelaTextSize.base,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Spacer(modifier = Modifier.width(VelaSpacing.lg))
        Icon(
            imageVector = if (row.picked) VelaIcons.Check else VelaIcons.Plus,
            contentDescription = null,
            tint = if (row.picked) colors.accentBase else colors.fgSubtle,
            modifier = Modifier.size(VelaIconSize.base),
        )
    }
}
