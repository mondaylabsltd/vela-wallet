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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaDangerButton
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
import app.getvela.wallet.feature.settings.components.ChosenMark
import app.getvela.wallet.feature.settings.components.RowGlyph
import app.getvela.wallet.feature.settings.components.SettingsDivider
import app.getvela.wallet.feature.settings.components.SigningPageItem
import app.getvela.wallet.feature.settings.components.SigningPageTextAction
import app.getvela.wallet.feature.settings.components.TrustAnswer
import app.getvela.wallet.feature.settings.components.VelaUrlField

/**
 * Spec 102, P2-08: Settings → Signing pages — a LIST, not spec 071's free-text
 * field. A field any message could talk a person into overwriting ("just
 * paste this address") was a door; the list keeps every page the person
 * chose, the official one first and never removable, and which page an
 * ACCOUNT signs on stays that account's "Where you review and sign".
 *
 * Every row says the two things a person needs before trusting a page: which
 * keys it can reach ("Keys on …", R1) and what was checked about it (its
 * integrity line). Removing a page changes no account (D-12).
 */
@Composable
internal fun SigningPagesPageBody(
    model: SigningPagesModel,
    onAdd: (String) -> Unit,
    onRename: (String) -> Unit,
    onRemove: (String) -> Unit,
    onTrust: (url: String, version: String) -> Unit,
) {
    val colors = VelaTheme.colors
    model.rows.forEach { row ->
        val editable = !row.official && model.loaded
        SigningPageItem(
            model = row,
            actions = if (!editable) {
                null
            } else {
                {
                    SigningPageTextAction(model.rename) { onRename(row.url) }
                    SigningPageTextAction(model.remove) { onRemove(row.url) }
                }
            },
        )
        row.trustVersion?.let { version -> TrustAnswer(model.trust) { onTrust(row.url, version) } }
        SettingsDivider()
    }
    // "Add a page": the address and the field's own Save. The core decides
    // whether it may be saved; a refused one says why under the field and
    // nothing is stored, an accepted one joins the list and the field clears.
    var draft by remember(model.draft) { mutableStateOf(model.draft) }
    var adding by remember { mutableStateOf(false) }
    // Saved, the keyboard goes too: the new row and its check are what the
    // person looks at next, and the keys covered them.
    val focus = androidx.compose.ui.platform.LocalFocusManager.current
    LaunchedEffect(model.rows.size) {
        if (adding) {
            draft = ""
            adding = false
            focus.clearFocus()
        }
    }
    LaunchedEffect(model.addError) { if (model.addError != null) adding = false }
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaUrlField(
        label = model.add,
        value = draft,
        placeholder = model.addressPlaceholder,
        hint = model.addError?.takeIf { !adding },
        tone = if (model.addError != null && !adding) SettingsTone.Error else null,
        action = model.save.takeIf { model.loaded && draft.isNotBlank() },
        onAction = {
            adding = true
            onAdd(draft)
        },
        onValueChange = { draft = it },
        modifier = Modifier.alpha(if (model.loaded) 1f else VelaOpacity.disabled),
    )
}

/** A saved page's new name; empty names it by its host again. */
@Composable
internal fun RenameSigningPageSheetBody(
    model: SigningPagesModel,
    url: String,
    onRename: (String) -> Unit,
    onDone: () -> Unit,
) {
    val row = model.rows.firstOrNull { it.url == url } ?: run {
        LaunchedEffect(Unit) { onDone() }
        return
    }
    val host = row.address.substringBefore('/')
    var name by remember(url) { mutableStateOf(row.label) }
    SheetTitle(model.rename, row.address)
    VelaUrlField(
        label = model.nameLabel,
        value = name,
        placeholder = host,
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
}

/**
 * "Remove this page?" — asked before it happens, as iOS asks: the page's own
 * name over the two answers, and no body (the keys the two shells share are
 * `settings.signing.pageRemove` and `common.cancel`). Removing a page changes
 * no account, but it does drop the version this device trusted for it.
 */
@Composable
internal fun RemoveSigningPageSheetBody(
    model: SigningPagesModel,
    url: String,
    onConfirm: () -> Unit,
    onCancel: () -> Unit,
) {
    val row = model.rows.firstOrNull { it.url == url } ?: run {
        LaunchedEffect(Unit) { onCancel() }
        return
    }
    // A row named by its domain does not say its address a second time.
    SheetTitle(row.title, row.address.takeIf { row.showAddress })
    VelaDangerButton(model.remove, onClick = onConfirm, modifier = Modifier.fillMaxWidth())
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaSecondaryButton(model.cancel, onClick = onCancel, modifier = Modifier.fillMaxWidth())
}

/**
 * Spec 102, P2-09: "Where you review and sign" — this account's venue on this
 * device. The two are not two kinds of key: the same keys sign in either
 * place (D2); what differs is who shows the person what they sign. So the
 * list is drawn as exactly that choice — Vela's sheet, then the pages under
 * one heading that says what a trusted page IS, each with the line that backs
 * the word "trusted". A choice that cannot reach the account's keys stays on
 * the list, dimmed, with the core's reason — a row that vanished could not
 * say why. The account's signing domain closes the list: it is the fact
 * every reason refers to.
 */
@Composable
internal fun VenuePageBody(
    model: VenueModel,
    onPick: (venueJson: String) -> Unit,
    onManage: () -> Unit,
    onTrust: (url: String, version: String) -> Unit = { _, _ -> },
) {
    val colors = VelaTheme.colors
    model.choices.filter { it.page == null }.forEach { choice -> InVelaChoice(choice, onPick) }
    SettingsDivider()
    Column(modifier = Modifier.padding(top = VelaSpacing.xl), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
        Text(
            text = model.pageSection,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.base,
        )
        Text(
            text = model.pageSectionBody,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            lineHeight = VelaLeading.normal * VelaTextSize.sm,
        )
    }
    model.choices.mapNotNull { choice -> choice.page?.let { choice to it } }.forEach { (choice, page) ->
        SigningPageItem(
            model = page,
            selected = choice.selected,
            reason = choice.reason,
            onClick = { if (!choice.selected) onPick(choice.venueJson) },
        )
        // A self-hosted page's build new to Vela: the question's answer, as
        // on Signing pages — stored on that page, then it is checked again.
        page.trustVersion?.let { version -> TrustAnswer(model.trust) { onTrust(page.url, version) } }
        SettingsDivider()
    }
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = VelaSpacing.lg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = model.domainLine,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.medium,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.weight(1f),
        )
        Text(
            text = model.manage,
            color = colors.accentBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.sm,
            modifier = Modifier
                .clickable(role = Role.Button, onClick = onManage)
                .padding(vertical = VelaSpacing.md),
        )
    }
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
        RowGlyph(VelaIcons.Wallet, enabled)
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            Text(
                text = choice.title,
                color = if (choice.selected) colors.accentBase else colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
                modifier = Modifier.alpha(if (enabled) 1f else VelaOpacity.disabled),
            )
            Text(
                text = choice.reason ?: choice.body,
                color = if (enabled) colors.fgSubtle else colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                lineHeight = VelaLeading.normal * VelaTextSize.sm,
            )
        }
        if (choice.selected) ChosenMark()
    }
}
