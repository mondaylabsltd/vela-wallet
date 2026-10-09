package app.getvela.wallet.feature.onboarding.flow

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.times
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaModalSheet
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsTone
import app.getvela.wallet.feature.settings.components.SigningPageAction
import app.getvela.wallet.feature.settings.components.SigningPageItem
import app.getvela.wallet.feature.settings.components.SigningPageItemModel
import app.getvela.wallet.feature.settings.components.VelaUrlField
import app.getvela.wallet.feature.settings.core.SigningPagesView
import uniffi.vela_core_uniffi.SignerIntegrityLine
import uniffi.vela_core_uniffi.venueWords

/**
 * Spec 102, P2-02: "Use my own signing page" — the one advanced entry beside
 * the three places a key lives. It picks a saved signing page (the official
 * one first, then the person's own, each with whose keys it reaches and its
 * integrity line), or adds one by address. A page on a custom domain then
 * mints every key and runs every ceremony (R3), and the wallet is locked to
 * it; a `getvela.app` page runs them in the app and becomes the venue (D-8).
 */
data class OwnPageModel(
    val title: String,
    val body: String,
    val pages: List<SigningPageItemModel>,
    val add: String,
    val addError: String?,
    val save: String,
    /** The list has been read: adding is offered only then. */
    val loaded: Boolean,
) {
    companion object {
        fun of(view: SigningPagesView, line: (String) -> SignerIntegrityLine, strings: VelaStrings): OwnPageModel {
            val words = venueWords("own_page")
            return OwnPageModel(
                title = words?.titleKey?.let(strings::t).orEmpty(),
                body = words?.lineName ?: words?.lineKey?.let(strings::t).orEmpty(),
                pages = view.pages.map { row ->
                    SettingsLive.pageItem(row.url, row.name, row.domain, row.official, line(row.url), strings)
                },
                add = strings.t("settings.signing.pageAdd"),
                addError = when (view.add_error) {
                    "invalid" -> strings.t("settings.signing.pageInvalid")
                    "insecure" -> strings.t("settings.signing.pageInsecure")
                    "duplicate" -> strings.t("settings.signing.pageDuplicate")
                    else -> null
                },
                save = strings.t("settings.signing.pageSave"),
                loaded = view.loaded,
            )
        }

        /** The entry's own two lines (`venueWords("own_page")`). */
        fun entry(strings: VelaStrings): Pair<String, String> {
            val words = venueWords("own_page")
            return words?.titleKey?.let(strings::t).orEmpty() to
                (words?.lineName ?: words?.lineKey?.let(strings::t).orEmpty())
        }
    }
}

/**
 * The entry row, in a chooser beside the three places: its two lines and a
 * chevron while no page is chosen; the chosen page itself (address, domain,
 * integrity line) with a ✕ back to the app once one is.
 */
@Composable
fun OwnPageEntry(
    chosen: SigningPageItemModel?,
    onOpen: () -> Unit,
    onClear: (() -> Unit)?,
    clearLabel: String,
) {
    val strings = LocalVelaStrings.current
    val colors = VelaTheme.colors
    if (chosen != null) {
        SigningPageItem(
            model = chosen,
            trailing = onClear?.let { clear -> { SigningPageAction(VelaIcons.Close, clearLabel, clear) } },
        )
        return
    }
    val (title, body) = OwnPageModel.entry(strings)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(role = Role.Button, onClick = onOpen)
            .padding(vertical = VelaSpacing.lg)
            .semantics { contentDescription = title },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
            Text(
                text = body,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                lineHeight = VelaLeading.normal * VelaTextSize.sm,
            )
        }
        Icon(
            imageVector = VelaIcons.ChevronRight,
            contentDescription = null,
            tint = colors.fgSubtle,
            modifier = Modifier.size(VelaIconSize.lg),
        )
    }
}

/** The picker's body: the pages, then a page added by address. Hosted by a sheet or inline. */
@Composable
fun OwnPageList(
    model: OwnPageModel,
    onPick: (String) -> Unit,
    onAdd: (String) -> Unit,
) {
    val colors = VelaTheme.colors
    var address by remember { mutableStateOf("") }
    var adding by remember { mutableStateOf(false) }
    val count = model.pages.size
    // A page added lands at the end of the list: the field clears for the next.
    LaunchedEffect(count) { if (adding) { address = ""; adding = false } }
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = model.body,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            lineHeight = VelaLeading.normal * VelaTextSize.base,
            modifier = Modifier.padding(bottom = VelaSpacing.md),
        )
        model.pages.forEachIndexed { index, page ->
            if (index > 0) HorizontalDivider(color = colors.borderBase, thickness = VelaBorder.hairline)
            SigningPageItem(model = page, onClick = { onPick(page.url) })
        }
        HorizontalDivider(color = colors.borderBase, thickness = VelaBorder.hairline)
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        VelaUrlField(
            label = model.add,
            value = address,
            placeholder = "https://",
            tone = if (model.addError != null && !adding) SettingsTone.Error else SettingsTone.Neutral,
            action = model.save.takeIf { model.loaded && address.isNotBlank() },
            onAction = {
                adding = true
                onAdd(address)
            },
            onValueChange = { address = it },
        )
        if (model.addError != null && !adding) {
            Text(
                text = model.addError,
                color = colors.errorBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                lineHeight = VelaLeading.normal * VelaTextSize.sm,
                modifier = Modifier.padding(top = VelaSpacing.md),
            )
        }
    }
}

/** The create flow's picker, as its own sheet over the key list. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OwnPageSheet(
    model: OwnPageModel,
    onPick: (String) -> Unit,
    onAdd: (String) -> Unit,
    onDismiss: () -> Unit,
    cancelLabel: String,
) {
    val colors = VelaTheme.colors
    VelaModalSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = colors.bgRaised,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = VelaSizing.screenPaddingX)
                .padding(bottom = VelaSpacing.xl2),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
        ) {
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
                modifier = Modifier.padding(bottom = VelaSpacing.sm),
            )
            OwnPageList(model, onPick = onPick, onAdd = onAdd)
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            VelaSecondaryButton(cancelLabel, onClick = onDismiss, modifier = Modifier.fillMaxWidth())
        }
    }
}
