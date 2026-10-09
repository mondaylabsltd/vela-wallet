package app.getvela.wallet.feature.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.settings.components.Radio
import app.getvela.wallet.feature.settings.components.SettingsDivider
import app.getvela.wallet.feature.settings.components.SigningPageAction
import app.getvela.wallet.feature.settings.components.SigningPageItem
import app.getvela.wallet.feature.settings.components.VelaUrlField
import androidx.compose.ui.draw.alpha

/**
 * Spec 102, P2-08: Settings → Signing pages. The official page first, then
 * the pages this person added — each with whose keys it reaches and the
 * integrity line the phone's own check gives it. There is no free-text "the
 * page every signature opens" field any more: which page an account signs on
 * is that account's "Where you review and sign".
 */
@Composable
internal fun SigningPagesPageBody(
    model: SigningPagesModel,
    onAdd: () -> Unit,
    onEdit: (String) -> Unit,
    onTrust: (url: String, version: String) -> Unit,
) {
    val colors = VelaTheme.colors
    model.rows.forEachIndexed { index, row ->
        SigningPageItem(
            model = row,
            onClick = if (row.official || !model.loaded) null else ({ onEdit(row.url) }),
            trailing = if (row.official || !model.loaded) {
                null
            } else {
                { SigningPageAction(VelaIcons.Pencil, row.title) { onEdit(row.url) } }
            },
        )
        row.trustVersion?.let { version -> TrustAnswer(model.trust) { onTrust(row.url, version) } }
        if (index < model.rows.lastIndex) SettingsDivider()
    }
    SettingsDivider()
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = model.loaded, role = Role.Button, onClick = onAdd)
            .heightIn(min = VelaSizing.controlLg)
            .padding(vertical = VelaSpacing.lg)
            .alpha(if (model.loaded) 1f else VelaOpacity.disabled),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Icon(imageVector = VelaIcons.Plus, contentDescription = null, tint = colors.accentBase, modifier = Modifier.size(VelaIconSize.md))
        Text(
            text = model.add,
            color = colors.accentBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.lg,
        )
    }
}

/** A custom page's own build asks to be trusted on this device: the line is the question, this the answer. */
@Composable
private fun TrustAnswer(label: String, onClick: () -> Unit) {
    Text(
        text = label,
        color = VelaTheme.colors.accentBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.base,
        modifier = Modifier
            .padding(bottom = VelaSpacing.md)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(vertical = VelaSpacing.sm),
    )
}

/**
 * "Add a page": the address as typed and an optional name. The core decides
 * whether it may be saved — a refused one leaves nothing stored and the sheet
 * says why under the field; an accepted one closes the sheet.
 */
@Composable
internal fun AddSigningPageSheetBody(
    model: SigningPagesModel,
    onSave: (url: String, name: String) -> Unit,
    onDone: () -> Unit,
) {
    val colors = VelaTheme.colors
    var url by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var saving by remember { mutableStateOf(false) }
    val before = remember { model.rows.size }
    // Saved: the list grew. Refused: the core's error came back. Either way
    // the attempt is over; only the first closes the sheet.
    LaunchedEffect(model.rows.size, model.addError) {
        if (saving) {
            if (model.rows.size > before) onDone() else if (model.addError != null) saving = false
        }
    }
    SheetTitle(model.add, model.subtitle)
    VelaUrlField(
        label = "",
        value = url,
        placeholder = model.addressPlaceholder,
        tone = if (model.addError != null && !saving) SettingsTone.Error else SettingsTone.Neutral,
        onValueChange = { url = it },
    )
    if (model.addError != null && !saving) {
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        Text(
            model.addError,
            color = colors.errorBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            lineHeight = VelaLeading.normal * VelaTextSize.sm,
        )
    }
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaUrlField(
        label = model.nameLabel,
        value = name,
        keyboard = KeyboardType.Text,
        onValueChange = { name = it },
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaPrimaryButton(
        model.save,
        onClick = {
            saving = true
            onSave(url, name)
        },
        enabled = url.isNotBlank(),
        modifier = Modifier.fillMaxWidth(),
    )
}

/**
 * One saved page: what it is (address, domain, integrity line), its name, and
 * the way to forget it. Removing a page changes no account (D-12): an account
 * that signs there still lists it as where it signs.
 */
@Composable
internal fun EditSigningPageSheetBody(
    model: SigningPagesModel,
    url: String,
    onRename: (String) -> Unit,
    onRemove: () -> Unit,
    onDone: () -> Unit,
) {
    val row = model.rows.firstOrNull { it.url == url } ?: run {
        LaunchedEffect(Unit) { onDone() }
        return
    }
    var name by remember(url) { mutableStateOf(if (row.official) "" else row.title.takeIf { it != row.address.substringBefore('/') }.orEmpty()) }
    SheetTitle(row.title)
    SigningPageItem(model = row)
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaUrlField(
        label = model.nameLabel,
        value = name,
        keyboard = KeyboardType.Text,
        onValueChange = { name = it },
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaPrimaryButton(
        model.save,
        onClick = {
            onRename(name)
            onDone()
        },
        modifier = Modifier.fillMaxWidth(),
    )
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaSecondaryButton(model.remove, onClick = { onRemove(); onDone() }, modifier = Modifier.fillMaxWidth())
}

/**
 * Spec 102, P2-09: "Where you review and sign" — this account's venue on this
 * device. Vela's own sheet, then the pages (official first), each a choice;
 * a venue that cannot reach the account's keys is shown dimmed with the
 * core's reason, never hidden — a custom-domain account sees that it is
 * locked to its page, and why.
 */
@Composable
internal fun VenueSheetBody(
    model: VenueModel,
    onPick: (venueJson: String) -> Unit,
    onManage: () -> Unit,
) {
    val colors = VelaTheme.colors
    SheetTitle(model.title, model.subtitle)
    Text(
        text = model.domainLine,
        color = colors.fgMuted,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.medium,
        fontSize = VelaTextSize.sm,
        modifier = Modifier.padding(bottom = VelaSpacing.md),
    )
    model.choices.filter { it.page == null }.forEach { choice -> InVelaChoice(choice, onPick) }
    SettingsDivider()
    Text(
        text = model.pageSection,
        color = colors.fgBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.base,
        modifier = Modifier.padding(top = VelaSpacing.xl),
    )
    Text(
        text = model.pageSectionBody,
        color = colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.sm,
        lineHeight = VelaLeading.normal * VelaTextSize.sm,
    )
    model.choices.mapNotNull { choice -> choice.page?.let { choice to it } }.forEach { (choice, page) ->
        SigningPageItem(
            model = page,
            selected = choice.selected,
            reason = choice.reason,
            onClick = { if (!choice.selected) onPick(choice.venueJson) },
        )
    }
    Text(
        text = model.manage,
        color = colors.accentBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.base,
        modifier = Modifier
            .padding(top = VelaSpacing.md)
            .clickable(role = Role.Button, onClick = onManage)
            .padding(vertical = VelaSpacing.md),
    )
}

@Composable
private fun InVelaChoice(choice: VenueChoiceModel, onPick: (String) -> Unit) {
    val colors = VelaTheme.colors
    val enabled = choice.reason == null
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = enabled && !choice.selected, role = Role.RadioButton) { onPick(choice.venueJson) }
            .heightIn(min = VelaSizing.controlLg)
            .padding(vertical = VelaSpacing.lg)
            .semantics { selected = choice.selected },
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Radio(choice.selected, enabled)
        Column(
            modifier = Modifier.weight(1f).alpha(if (enabled) 1f else VelaOpacity.disabled),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
        ) {
            Text(
                text = choice.title,
                color = if (choice.selected) colors.accentBase else colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
            Text(
                text = choice.reason ?: choice.body,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                lineHeight = VelaLeading.normal * VelaTextSize.sm,
            )
        }
    }
}
