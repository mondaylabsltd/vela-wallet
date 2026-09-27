package app.getvela.wallet.feature.contacts.components

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.times
import androidx.compose.ui.text.style.TextAlign
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.contacts.ActionMenuModel
import app.getvela.wallet.feature.contacts.ContactsIcon
import app.getvela.wallet.feature.contacts.DeleteConfirmModel
import app.getvela.wallet.feature.contacts.MenuItemModel

/**
 * Mobile action sheet (spec vocabulary #7, mocks C5/C6): the house
 * `ModalBottomSheet(containerColor = bg.raised)` with its drag handle, then
 * icon+label rows, an optional hairline before the destructive row (red icon
 * and label), and a separate outlined 取消 button.
 *
 * One composable hosts every mobile menu — C5 (add/import/export), C6 (group
 * actions) and, via [DeleteConfirmSheet], the destructive confirmation. Item
 * taps are sinks: they report the item and dismiss (spec Assumptions).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ActionMenuSheet(
    model: ActionMenuModel,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    onItem: (MenuItemModel) -> Unit = {},
) {
    VelaModalSheet(
        onDismissRequest = onDismiss,
        containerColor = VelaTheme.colors.bgRaised,
        modifier = modifier,
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
            model.items.forEach { item ->
                if (item.dividerBefore) {
                    Hairline()
                }
                MenuRow(item = item, onClick = { onItem(item) })
            }
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            SheetCancelButton(label = model.cancel, onClick = onDismiss)
        }
    }
}

/**
 * Destructive confirmation variant (c2s): title, body naming the contact, a
 * red confirm row and the same 取消 button. Confirming is visual only — the
 * gallery returns to the fixture state (spec edge case).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DeleteConfirmSheet(
    model: DeleteConfirmModel,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    onConfirm: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    VelaModalSheet(
        onDismissRequest = onDismiss,
        containerColor = colors.bgRaised,
        modifier = modifier,
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
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.md))
            Text(
                text = model.body,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.regular,
                fontSize = VelaTextSize.base,
                lineHeight = VelaLeading.normal * VelaTextSize.base,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            Hairline()
            MenuRow(
                item = MenuItemModel(
                    id = "contacts.delete",
                    icon = ContactsIcon.Delete,
                    label = model.confirm,
                    destructive = true,
                ),
                onClick = onConfirm,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.xl))
            SheetCancelButton(label = model.cancel, onClick = onDismiss)
        }
    }
}

@Composable
private fun MenuRow(item: MenuItemModel, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    val tint = if (item.destructive) colors.errorBase else colors.fgBase
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .heightIn(min = VelaSizing.hitTarget)
            .padding(vertical = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = glyphFor(item.icon),
            contentDescription = null,
            tint = tint,
            modifier = Modifier.size(VelaIconSize.lg),
        )
        Spacer(modifier = Modifier.width(VelaSpacing.xl))
        Text(
            text = item.label,
            color = tint,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
            maxLines = 1,
        )
    }
}

/** The sheet's own 取消 affordance — outlined, full width, never accent. */
@Composable
private fun SheetCancelButton(label: String, onClick: () -> Unit) {
    val colors = VelaTheme.colors
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = VelaSizing.controlLg)
            .border(
                width = VelaBorder.hairline,
                color = colors.borderStrong,
                shape = RoundedCornerShape(VelaRadius.lg),
            )
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
            textAlign = TextAlign.Center,
        )
    }
}

/**
 * The sheet's row list without the modal host — the component board renders
 * the C5/C6/delete anatomies inline (FR-004a) without stacking real sheets.
 */
@Composable
fun ActionMenuRows(
    model: ActionMenuModel,
    modifier: Modifier = Modifier,
    onItem: (MenuItemModel) -> Unit = {},
) {
    Column(modifier = modifier.fillMaxWidth()) {
        model.items.forEach { item ->
            if (item.dividerBefore) {
                Hairline()
            }
            MenuRow(item = item, onClick = { onItem(item) })
        }
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
        SheetCancelButton(label = model.cancel, onClick = {})
    }
}
