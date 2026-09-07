package app.getvela.wallet.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.getvela.wallet.core.designsystem.components.VelaDangerButton
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaPrimaryButton
import app.getvela.wallet.core.designsystem.components.VelaSecondaryButton
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaMonoFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.identicon.IdenticonImage
import app.getvela.wallet.feature.settings.components.SettingsDivider
import app.getvela.wallet.feature.settings.components.SettingsSectionLabel
import app.getvela.wallet.feature.settings.components.VelaAccountRow
import app.getvela.wallet.feature.settings.components.VelaCallout
import app.getvela.wallet.feature.settings.components.VelaChainMark
import app.getvela.wallet.feature.settings.components.VelaCheckList
import app.getvela.wallet.feature.settings.components.VelaDangerCard
import app.getvela.wallet.feature.settings.components.VelaKeyValueRow
import app.getvela.wallet.feature.settings.components.VelaNetworkRow
import app.getvela.wallet.feature.settings.components.VelaSegmentedControl
import app.getvela.wallet.feature.settings.components.VelaSelectRow
import app.getvela.wallet.feature.settings.components.VelaSettingsRow
import app.getvela.wallet.feature.settings.components.VelaStatusPill
import app.getvela.wallet.feature.settings.components.VelaStorageBar
import app.getvela.wallet.feature.settings.components.VelaStorageGroup
import app.getvela.wallet.feature.settings.components.VelaTextScaleSlider
import app.getvela.wallet.feature.settings.components.VelaUrlField
import app.getvela.wallet.feature.settings.components.settingsIcon
import app.getvela.wallet.feature.wallet.components.VelaTab
import app.getvela.wallet.feature.wallet.components.VelaTabBar

/**
 * The settings surface (spec 023, ST1–ST16 + SR1–SR5).
 *
 * One screen, not sixteen. The mocks are a page (`Home` plus seven pushed
 * sub-pages) crossed with an overlay (nine sheets), and everything inside both
 * is assembled from `components/`. Which page and which overlay a state shows
 * is DATA — the fixture layer says so — so the gallery pins a state by handing
 * over a model, and the real app moves between them by tapping.
 *
 * Navigation is local state seeded from the model. Business state is not wired:
 * the callbacks are how the nav host hooks the two behaviours that already
 * exist (signing out, and leaving for another tab).
 */

/** What the host can respond to. Everything else is presentation. */
data class SettingsActions(
    val onSelectTab: (VelaTab) -> Unit = {},
    val onSignOut: () -> Unit = {},
    val onOpenContacts: () -> Unit = {},
    /**
     * A row picked in one of the select sheets — currency today, the language
     * and format sheets when their machines arrive.
     *
     * The overlay says WHICH sheet, so one callback serves five of them and a
     * new sheet does not widen this class. Returning is the host's business:
     * the sheet closes here, and what the pick means is the core's.
     */
    val onSheetSelect: (SettingsOverlay, String) -> Unit = { _, _ -> },
    /**
     * A field changed, keystroke by keystroke, identified by the id its model
     * carries.
     *
     * Per keystroke rather than on blur because the CORE holds the draft: it
     * echoes the value back in its view, so the box a person types into is
     * showing the core's state rather than a second copy that can disagree
     * with it. The web shell does the same.
     */
    val onFieldEdited: (fieldId: String, value: String) -> Unit = { _, _ -> },
    /** The field lost focus — the core's commit point, where it validates and saves. */
    val onFieldCommitted: (fieldId: String) -> Unit = {},
    /** The bin on a custom network row. */
    val onRemoveNetwork: (id: String) -> Unit = {},
    /**
     * A network row was opened.
     *
     * This is what asks the core to check that endpoint — the row was drawn as
     * tappable from the start and nothing was listening, so the probe arms
     * landed with no caller. (Spec 041 phase 2.)
     */
    val onOpenNetwork: (id: String) -> Unit = {},
    /** 恢复默认 on the service-endpoints page. */
    val onResetEndpoints: () -> Unit = {},
)

@Composable
fun SettingsRoute(
    model: SettingsScreenModel,
    modifier: Modifier = Modifier,
    actions: SettingsActions = SettingsActions(),
) {
    // Seeds, not bindings: a gallery state pins where this opens, and a person
    // tapping owns it from then on.
    var page by rememberSaveable(model.state) { mutableStateOf(model.page) }
    var overlay by remember(model.state) { mutableStateOf(model.overlay) }
    var advancedOpen by rememberSaveable(model.state) {
        mutableStateOf(model.state == SettingsScreenState.ST1B)
    }

    SettingsScreen(
        model = model,
        page = page,
        overlay = overlay,
        advancedOpen = advancedOpen,
        modifier = modifier,
        onRow = { id ->
            when (id) {
                "contacts" -> actions.onOpenContacts()
                "networks" -> page = SettingsPage.Networks
                "rpc-providers" -> page = SettingsPage.RpcProviders
                "add-network" -> page = SettingsPage.AddNetwork
                "endpoints" -> page = SettingsPage.Endpoints
                "storage" -> page = SettingsPage.Storage
                "about" -> page = SettingsPage.About
                "language" -> overlay = SettingsOverlay.Language
                "currency" -> overlay = SettingsOverlay.Currency
                "number-format" -> overlay = SettingsOverlay.NumberFormat
                "date-format" -> overlay = SettingsOverlay.DateFormat
                "time-format" -> overlay = SettingsOverlay.TimeFormat
                "feedback" -> overlay = SettingsOverlay.Feedback
                else -> Unit
            }
        },
        onBack = { page = SettingsPage.Home },
        onToggleAdvanced = { advancedOpen = !advancedOpen },
        onOpenOverlay = { overlay = it },
        onDismissOverlay = { overlay = SettingsOverlay.None },
        onSheetSelect = { sheet, id ->
            actions.onSheetSelect(sheet, id)
            // The sheet closes on the pick, before the core has answered. The
            // alternative — waiting for the view to come back — leaves a
            // person tapping a row that visibly does nothing while a storage
            // write completes.
            overlay = SettingsOverlay.None
        },
        onSelectTab = actions.onSelectTab,
        onSignOut = actions.onSignOut,
        onFieldEdited = actions.onFieldEdited,
        onFieldCommitted = actions.onFieldCommitted,
        onRemoveNetwork = actions.onRemoveNetwork,
        onOpenNetwork = actions.onOpenNetwork,
        onResetEndpoints = actions.onResetEndpoints,
    )
}

@Composable
@Suppress("LongParameterList", "LongMethod")
fun SettingsScreen(
    model: SettingsScreenModel,
    page: SettingsPage,
    overlay: SettingsOverlay,
    advancedOpen: Boolean,
    modifier: Modifier = Modifier,
    onRow: (String) -> Unit = {},
    onBack: () -> Unit = {},
    onToggleAdvanced: () -> Unit = {},
    onOpenOverlay: (SettingsOverlay) -> Unit = {},
    onDismissOverlay: () -> Unit = {},
    onSheetSelect: (SettingsOverlay, String) -> Unit = { _, _ -> },
    onSelectTab: (VelaTab) -> Unit = {},
    onSignOut: () -> Unit = {},
    onFieldEdited: (String, String) -> Unit = { _, _ -> },
    onFieldCommitted: (String) -> Unit = {},
    onRemoveNetwork: (String) -> Unit = {},
    onOpenNetwork: (String) -> Unit = {},
    onResetEndpoints: () -> Unit = {},
) {
    val colors = VelaTheme.colors

    // SR5 replaces the whole screen: it blocks both creating and signing in, so
    // there is nothing behind it to go back to.
    if (model.state == SettingsScreenState.SR5) {
        IndexDownScreen(model.indexDown, modifier = modifier)
        return
    }

    // SR2–SR4 are sheets over ANOTHER screen (the wallet, the send flow), so
    // the body behind them is a dimmed title rather than the settings list.
    val rescue = model.selectedTab == "wallet"

    Box(modifier = modifier.fillMaxSize().background(colors.bgBase)) {
        Column(modifier = Modifier.fillMaxSize().safeDrawingPadding()) {
            Column(
                modifier = Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = VelaSizing.screenPaddingX),
            ) {
                when {
                    rescue -> {
                        Text(
                            text = model.backdropTitle,
                            color = colors.fgSubtle.copy(alpha = VelaOpacity.dim),
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.bold,
                            fontSize = VelaTextSize.xl3,
                            modifier = Modifier.padding(top = VelaSpacing.xl4, bottom = VelaSpacing.xl),
                        )
                        if (model.rpcBanner != null) {
                            RpcBanner(model.rpcBanner)
                        }
                    }
                    page == SettingsPage.Home -> {
                        Text(
                            text = model.title,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.bold,
                            fontSize = VelaTextSize.xl3,
                            modifier = Modifier.padding(top = VelaSpacing.xl4, bottom = VelaSpacing.xl),
                        )
                        SettingsHomeBody(
                            model = model,
                            advancedOpen = advancedOpen,
                            onRow = onRow,
                            onToggleAdvanced = onToggleAdvanced,
                            onOpenOverlay = onOpenOverlay,
                        )
                    }
                    else -> {
                        val (title, subtitle) = pageHeader(model, page)
                        SettingsNavHeader(title, subtitle, model.closeLabel, onBack)
                        SettingsPageBody(
                            model = model,
                            page = page,
                            onOpenOverlay = onOpenOverlay,
                            onFieldEdited = onFieldEdited,
                            onFieldCommitted = onFieldCommitted,
                            onRemoveNetwork = onRemoveNetwork,
                            onOpenNetwork = onOpenNetwork,
                            onResetEndpoints = onResetEndpoints,
                        )
                    }
                }
                Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            }
            VelaTabBar(
                tabs = model.tabs,
                selected = if (rescue) VelaTab.Wallet else VelaTab.Settings,
                onSelect = onSelectTab,
            )
        }

        if (overlay != SettingsOverlay.None) {
            SettingsSheet(
                model = model,
                overlay = overlay,
                onDismiss = onDismissOverlay,
                onSignOut = onSignOut,
                onSheetSelect = onSheetSelect,
            )
        }
    }
}

private fun pageHeader(model: SettingsScreenModel, page: SettingsPage): Pair<String, String?> =
    when (page) {
        SettingsPage.Networks -> model.networksTitle to model.networksSubtitle
        SettingsPage.NetworkDetail -> model.networkDetail.title to model.networkDetail.subtitle
        SettingsPage.AddNetwork -> model.addNetwork.title to model.addNetwork.subtitle
        SettingsPage.RpcProviders -> model.rpcProviders.title to model.rpcProviders.subtitle
        SettingsPage.Endpoints -> model.endpoints.title to null
        SettingsPage.Storage -> model.storage.title to model.storage.subtitle
        SettingsPage.About -> model.about.title to null
        SettingsPage.Home -> model.title to null
    }

/** Back arrow + title + optional second line (ST9/ST9b/ST10/ST11/ST12/…). */
@Composable
private fun SettingsNavHeader(
    title: String,
    subtitle: String?,
    backLabel: String,
    onBack: () -> Unit,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = VelaSpacing.xl, bottom = VelaSpacing.lg),
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        Icon(
            imageVector = VelaIcons.ChevronLeft,
            contentDescription = backLabel,
            tint = colors.fgBase,
            modifier = Modifier
                .size(VelaSizing.hitTarget)
                .clickable(onClick = onBack)
                .padding(VelaSpacing.lg),
        )
        Column(verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs)) {
            Text(
                text = title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl2,
            )
            if (subtitle != null) {
                Text(
                    text = subtitle,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }
    }
}

@Composable
private fun SettingsHomeBody(
    model: SettingsScreenModel,
    advancedOpen: Boolean,
    onRow: (String) -> Unit,
    onToggleAdvanced: () -> Unit,
    onOpenOverlay: (SettingsOverlay) -> Unit,
) {
    val colors = VelaTheme.colors
    VelaAccountRow(model.account) { onOpenOverlay(SettingsOverlay.Accounts) }

    model.sections.forEach { section ->
        if (section.label != null) {
            SettingsSectionLabel(
                label = section.label,
                collapsible = section.collapsible,
                collapsed = section.collapsible && !advancedOpen,
                onToggle = onToggleAdvanced,
            )
        }
        val hidden = section.collapsible && !advancedOpen
        if (!hidden) {
            section.rows.forEachIndexed { index, row ->
                VelaSettingsRow(
                    row = row,
                    divider = index < section.rows.lastIndex,
                    onClick = onRow,
                )
            }
        }
        // The three appearance controls are not rows: they are the control
        // itself, shown inline under 语言 (ST1).
        if (section.appearanceControls) {
            VelaTextScaleSlider(model.textScale.steps, model.textScale.index)
            VelaSegmentedControl(
                label = model.theme.label,
                segments = model.theme.segments.map { seg ->
                    Triple(seg.id, seg.label, seg.icon?.let(::settingsIcon))
                },
                selectedId = model.theme.selected,
            )
            Spacer(modifier = Modifier.height(VelaSpacing.lg))
            VelaSegmentedControl(
                label = model.avatar.label,
                segments = model.avatar.segments.map { seg ->
                    Triple(seg.id, seg.label, seg.icon?.let(::settingsIcon))
                },
                selectedId = model.avatar.selected,
            )
        }
    }

    Text(
        text = model.signOutLabel,
        color = colors.fgMuted,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.lg,
        textAlign = TextAlign.Center,
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onOpenOverlay(SettingsOverlay.SignOut) }
            .padding(top = VelaSpacing.xl4, bottom = VelaSpacing.xl3),
    )
    VelaDangerCard(model.eraseTitle, model.eraseSubtitle) {
        onOpenOverlay(SettingsOverlay.EraseDevice)
    }
}

/**
 * A [VelaUrlField] wired to the core.
 *
 * Focus is what commits. The core validates and saves on blur — not on every
 * keystroke — so a field that reported only its edits would let a person type a
 * perfectly good RPC URL that never reached storage, with the box still showing
 * it. Both halves or neither.
 */
@Composable
private fun EditableUrlField(
    field: UrlFieldModel,
    onEdited: (String, String) -> Unit,
    onCommitted: (String) -> Unit,
    action: String? = null,
) {
    var focused by remember(field.id) { mutableStateOf(false) }
    VelaUrlField(
        label = field.label,
        value = field.value,
        placeholder = field.placeholder,
        hint = field.hint,
        badge = field.badge,
        tone = field.tone,
        action = action,
        onValueChange = { value -> onEdited(field.id, value) },
        modifier = Modifier.onFocusChanged { state ->
            if (focused && !state.isFocused) onCommitted(field.id)
            focused = state.isFocused
        },
    )
}

@Composable
@Suppress("LongMethod")
private fun SettingsPageBody(
    model: SettingsScreenModel,
    page: SettingsPage,
    onOpenOverlay: (SettingsOverlay) -> Unit,
    onFieldEdited: (String, String) -> Unit = { _, _ -> },
    onFieldCommitted: (String) -> Unit = {},
    onRemoveNetwork: (String) -> Unit = {},
    onOpenNetwork: (String) -> Unit = {},
    onResetEndpoints: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    when (page) {
        SettingsPage.Networks -> {
            model.networks.forEach { row ->
                VelaNetworkRow(
                    row = row,
                    deleteLabel = model.addNetworkLabel,
                    onClick = onOpenNetwork,
                    onDelete = onRemoveNetwork,
                )
            }
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = VelaSpacing.xl3),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = VelaIcons.Plus,
                    contentDescription = null,
                    tint = colors.infoBase,
                    modifier = Modifier.size(VelaIconSize.md),
                )
                Spacer(modifier = Modifier.size(VelaSpacing.md))
                // A link, not a CTA: adding a network is navigation, and accent
                // is reserved for actions that move value.
                Text(
                    text = model.addNetworkLabel,
                    color = colors.infoBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                )
            }
        }

        SettingsPage.NetworkDetail -> {
            val detail = model.networkDetail
            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl3),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
            ) {
                VelaChainMark(detail.mark)
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = detail.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.bold,
                        fontSize = VelaTextSize.xl,
                    )
                    Text(
                        text = detail.note,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                }
                VelaStatusPill(detail.badge)
            }
            VelaUrlField(
                label = detail.rpc.label,
                value = detail.rpc.value,
                hint = detail.rpc.hint,
                badge = detail.rpc.badge,
                tone = detail.rpc.tone,
            )
            if (detail.callout != null) {
                Spacer(modifier = Modifier.height(VelaSpacing.xl))
                VelaCallout(detail.callout)
            }
            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            VelaUrlField(label = detail.explorer.label, value = detail.explorer.value)
        }

        SettingsPage.AddNetwork -> {
            val add = model.addNetwork
            if (add.candidate == null) {
                VelaUrlField(label = "", value = "", placeholder = add.searchPlaceholder)
                Spacer(modifier = Modifier.height(VelaSpacing.xl))
                add.results.forEach { VelaNetworkRow(it) }
            } else {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
                ) {
                    VelaChainMark(add.candidate.mark)
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = add.candidate.name,
                            color = colors.fgBase,
                            fontFamily = VelaFontFamily,
                            fontWeight = VelaFontWeight.bold,
                            fontSize = VelaTextSize.xl,
                        )
                        Text(
                            text = add.candidate.meta,
                            color = colors.fgSubtle,
                            fontFamily = VelaFontFamily,
                            fontSize = VelaTextSize.base,
                        )
                    }
                    if (add.candidate.badge != null) VelaStatusPill(add.candidate.badge)
                }
                if (add.checksTitle != null) {
                    VelaCheckList(add.checksTitle, add.checks)
                    Spacer(modifier = Modifier.height(VelaSpacing.xl))
                }
                if (add.customRpc != null) {
                    VelaUrlField(
                        label = add.customRpc.label,
                        value = add.customRpc.value,
                        placeholder = add.customRpc.placeholder,
                    )
                    Spacer(modifier = Modifier.height(VelaSpacing.xl))
                }
                if (add.callout != null) {
                    VelaCallout(add.callout)
                    Spacer(modifier = Modifier.height(VelaSpacing.xl))
                }
                // An outline CTA plus a re-check link when it cannot be added:
                // an action you cannot take should not be dressed as the action
                // you came for.
                if (add.primary != null) {
                    VelaPrimaryButton(add.primary, onClick = {}, modifier = Modifier.fillMaxWidth())
                }
                if (add.secondary != null) {
                    VelaSecondaryButton(add.secondary, onClick = {}, modifier = Modifier.fillMaxWidth())
                }
                if (add.recheck != null) {
                    Text(
                        text = add.recheck,
                        color = colors.infoBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.base,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(top = VelaSpacing.xl),
                    )
                }
            }
        }

        SettingsPage.RpcProviders -> {
            Text(
                text = model.rpcProviders.description,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.padding(bottom = VelaSpacing.xl3),
            )
            model.rpcProviders.providers.forEach { provider ->
                Row(
                    modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.lg),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = provider.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.bold,
                        fontSize = VelaTextSize.xl,
                        modifier = Modifier.weight(1f),
                    )
                    VelaStatusPill(provider.badge)
                }
                EditableUrlField(
                    field = provider.field,
                    action = provider.action,
                    onEdited = onFieldEdited,
                    onCommitted = onFieldCommitted,
                )
                if (provider.support != null) {
                    Text(
                        text = provider.support,
                        color = colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        modifier = Modifier.padding(top = VelaSpacing.md),
                    )
                }
                if (provider.link != null) {
                    Text(
                        text = provider.link,
                        color = colors.infoBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                        modifier = Modifier.padding(top = VelaSpacing.md),
                    )
                }
                Spacer(modifier = Modifier.height(VelaSpacing.xl4))
            }
        }

        SettingsPage.Endpoints -> {
            Text(
                text = model.endpoints.description,
                color = colors.fgMuted,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.padding(bottom = VelaSpacing.xl3),
            )
            model.endpoints.fields.forEach { field ->
                EditableUrlField(
                    field = field,
                    onEdited = onFieldEdited,
                    onCommitted = onFieldCommitted,
                )
                Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            }
            // Drawn as a label in spec 023 and never given a click — found on
            // a device by tapping it and watching the store not change. The
            // core has had `reset_endpoints_to_defaults` all along.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onResetEndpoints)
                    .padding(top = VelaSpacing.xl),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = VelaIcons.RefreshCw,
                    contentDescription = null,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.sm),
                )
                Spacer(modifier = Modifier.size(VelaSpacing.md))
                Text(
                    text = model.endpoints.reset,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
        }

        SettingsPage.Storage -> {
            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl),
                verticalAlignment = Alignment.Bottom,
                horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
            ) {
                Text(
                    text = model.storage.amount,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.bold,
                    fontSize = VelaTextSize.xl4,
                )
                Text(
                    text = model.storage.unit,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                )
                Text(
                    text = model.storage.summary,
                    color = colors.fgSubtle,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
            VelaStorageBar(model.storage.segments)
            model.storage.groups.forEach { group ->
                VelaStorageGroup(
                    group = group,
                    onGroupAction = { onOpenOverlay(SettingsOverlay.ClearCaches) },
                )
            }
        }

        SettingsPage.About -> {
            Column(
                modifier = Modifier.fillMaxWidth().padding(vertical = VelaSpacing.xl3),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
            ) {
                Text(
                    text = model.about.tagline,
                    color = colors.fgMuted,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.lg,
                )
                Text(
                    text = model.about.version,
                    color = colors.fgSubtle,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
            Text(
                text = model.about.sectionTechnical,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                modifier = Modifier.padding(bottom = VelaSpacing.md),
            )
            model.about.rows.forEach { VelaKeyValueRow(it) }
            Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            model.about.links.forEach { VelaKeyValueRow(it) }
            Text(
                text = model.about.footer,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth().padding(top = VelaSpacing.xl3),
            )
        }

        SettingsPage.Home -> Unit
    }
}

/**
 * SR1's amber banner: the count of unreachable networks, then one chip per
 * network with its own 修复. Per-chain rather than one global button, because
 * the fix IS per chain — a shared button would have to ask which one first.
 */
@Composable
private fun RpcBanner(banner: RpcBannerModel) {
    val colors = VelaTheme.colors
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = VelaSpacing.lg)
            .background(colors.warningSoft)
            .padding(VelaSpacing.xl),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            Icon(
                imageVector = VelaIcons.TriangleAlert,
                contentDescription = null,
                tint = colors.warningBase,
                modifier = Modifier.size(VelaIconSize.md),
            )
            Text(
                text = banner.text,
                color = colors.warningBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.base,
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            banner.chips.forEach { chip ->
                Row(
                    modifier = Modifier
                        .background(colors.bgBase)
                        .padding(VelaSpacing.md),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                ) {
                    VelaChainMark(chip.mark, size = VelaIconSize.xl)
                    Text(
                        text = chip.name,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                    // The only accent on this banner: the thing that fixes it.
                    Text(
                        text = chip.action,
                        color = colors.accentBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
        }
    }
}

/**
 * SR5 — the passkey index is unreachable. The endpoint is editable right here,
 * because "the service is down" and "you pointed it at the wrong host" look
 * identical from the inside, and only one is something the person can fix.
 */
@Composable
private fun IndexDownScreen(model: IndexDownModel, modifier: Modifier = Modifier) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase)
            .safeDrawingPadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = VelaSizing.screenPaddingX, vertical = VelaSpacing.xl5),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
    ) {
        Text(
            text = model.title,
            color = colors.fgBase,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.bold,
            fontSize = VelaTextSize.xl3,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            text = model.subtitle,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
        VelaCallout(model.callout)
        VelaUrlField(
            label = model.field.label,
            value = model.field.value,
            badge = model.field.badge,
        )
        VelaPrimaryButton(model.primary, onClick = {}, modifier = Modifier.fillMaxWidth())
        VelaSecondaryButton(model.secondary, onClick = {}, modifier = Modifier.fillMaxWidth())
        Text(
            text = model.footer,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** Every overlay the phone draws as a bottom sheet. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
@Suppress("LongMethod")
private fun SettingsSheet(
    model: SettingsScreenModel,
    overlay: SettingsOverlay,
    onDismiss: () -> Unit,
    onSignOut: () -> Unit,
    onSheetSelect: (SettingsOverlay, String) -> Unit = { _, _ -> },
) {
    val colors = VelaTheme.colors
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = colors.bgBase,
    ) {
        // The ✕ lives in the host, not in each body: every sheet opens with a
        // SheetTitle, so one overlay anchored top-end lands on the title line
        // for all of them — and none of them can forget it. The drag handle
        // alone is not an affordance a first-time reader recognises.
        // The cap is what makes the scroll below mean anything: a wrap-height
        // column has no overflow to scroll, so verticalScroll alone silently
        // did nothing and the sheet still ended at Português.
        val maxSheetHeight = (LocalConfiguration.current.screenHeightDp * 0.88f).dp
        Box(modifier = Modifier.fillMaxWidth().heightIn(max = maxSheetHeight)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                // Without this the language sheet simply ends at Português:
                // fifteen locales are taller than the sheet, and the three
                // below the fold — plus the contribute footer — were
                // unreachable. Every sheet here can outgrow the screen once a
                // translation runs long, so the scroll belongs to the host.
                .verticalScroll(rememberScrollState())
                .padding(horizontal = VelaSizing.screenPaddingX)
                .padding(bottom = VelaSpacing.xl3),
        ) {
            when (overlay) {
                SettingsOverlay.Accounts -> AccountsSheetBody(model.accountsSheet)
                SettingsOverlay.SignOut -> ConfirmSheetBody(
                    model.signOutSheet,
                    onConfirm = onSignOut,
                    onCancel = onDismiss,
                )
                SettingsOverlay.Language -> SelectSheetBody(model.languageSheet) {
                    onSheetSelect(SettingsOverlay.Language, it)
                }
                SettingsOverlay.Currency -> SelectSheetBody(model.currencySheet) {
                    onSheetSelect(SettingsOverlay.Currency, it)
                }
                SettingsOverlay.NumberFormat -> SelectSheetBody(model.numberSheet) {
                    onSheetSelect(SettingsOverlay.NumberFormat, it)
                }
                SettingsOverlay.DateFormat -> SelectSheetBody(model.dateSheet) {
                    onSheetSelect(SettingsOverlay.DateFormat, it)
                }
                SettingsOverlay.TimeFormat -> SelectSheetBody(model.timeSheet) {
                    onSheetSelect(SettingsOverlay.TimeFormat, it)
                }
                SettingsOverlay.ClearCaches -> ConfirmSheetBody(
                    model.clearCachesSheet,
                    onConfirm = onDismiss,
                    onCancel = onDismiss,
                )
                SettingsOverlay.EraseDevice -> ConfirmSheetBody(
                    model.eraseSheet,
                    onConfirm = onDismiss,
                    onCancel = onDismiss,
                )
                SettingsOverlay.Feedback -> FeedbackSheetBody(model.feedback)
                SettingsOverlay.RpcFix -> RpcFixSheetBody(model.rpcFix, onDismiss)
                SettingsOverlay.BalanceDetail -> BalanceDetailSheetBody(model.balanceDetail)
                SettingsOverlay.Relayer -> RelayerSheetBody(model.relayer, onDismiss)
                SettingsOverlay.None -> Unit
            }
        }

            IconButton(
                onClick = onDismiss,
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(end = VelaSizing.screenPaddingX),
            ) {
                Box(
                    modifier = Modifier
                        .size(VelaSpacing.xl4)
                        .clip(CircleShape)
                        .background(colors.bgRaised),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = VelaIcons.Close,
                        contentDescription = model.closeLabel,
                        tint = colors.fgMuted,
                        modifier = Modifier.size(VelaIconSize.md),
                    )
                }
            }
        }
    }
}

@Composable
private fun SheetTitle(title: String, subtitle: String? = null) {
    val colors = VelaTheme.colors
    Text(
        text = title,
        color = colors.fgBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.bold,
        fontSize = VelaTextSize.xl2,
    )
    if (subtitle != null) {
        Text(
            text = subtitle,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(top = VelaSpacing.sm, bottom = VelaSpacing.lg),
        )
    } else {
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
    }
}

@Composable
private fun SelectSheetBody(sheet: SelectSheetModel, onSelect: (String) -> Unit = {}) {
    val colors = VelaTheme.colors
    SheetTitle(sheet.title, sheet.subtitle)
    if (sheet.searchPlaceholder != null) {
        VelaUrlField(label = "", value = "", placeholder = sheet.searchPlaceholder)
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
    }
    sheet.rows.forEach { VelaSelectRow(it, onClick = onSelect) }
    if (sheet.footerNote != null) {
        Text(
            text = sheet.footerNote,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.padding(top = VelaSpacing.xl),
        )
    }
    if (sheet.footerLink != null) {
        Text(
            text = sheet.footerLink,
            color = colors.infoBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(top = VelaSpacing.md),
        )
    }
}

@Composable
private fun ConfirmSheetBody(
    sheet: ConfirmSheetModel,
    onConfirm: () -> Unit,
    onCancel: () -> Unit,
) {
    val colors = VelaTheme.colors
    SheetTitle(sheet.title)
    Text(
        text = sheet.body,
        color = colors.fgBase,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.lg,
        modifier = Modifier.padding(bottom = VelaSpacing.xl),
    )
    if (sheet.note != null) {
        Text(
            text = sheet.note,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(bottom = VelaSpacing.xl),
        )
    }
    if (sheet.callout != null) {
        VelaCallout(sheet.callout)
        Spacer(modifier = Modifier.height(VelaSpacing.xl))
    }
    // The tone picks the CTA's colour, so "清除缓存" is accent and "全部清除" is
    // red without either screen owning a button of its own.
    if (sheet.danger) {
        VelaDangerButton(sheet.confirm, onClick = onConfirm, modifier = Modifier.fillMaxWidth())
    } else {
        VelaPrimaryButton(sheet.confirm, onClick = onConfirm, modifier = Modifier.fillMaxWidth())
    }
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaSecondaryButton(sheet.cancel, onClick = onCancel, modifier = Modifier.fillMaxWidth())
}

@Composable
private fun AccountsSheetBody(sheet: AccountsSheetModel) {
    val colors = VelaTheme.colors
    SheetTitle(sheet.title)
    Text(
        text = sheet.summary,
        color = colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        modifier = Modifier.padding(bottom = VelaSpacing.lg),
    )
    // Not VelaSelectRow: a select row is a label and a note, so reusing it
    // silently dropped the identicon and the address, and three accounts became
    // three names with no way to tell which key each one is.
    sheet.rows.forEachIndexed { index, row ->
        if (index > 0) SettingsDivider()
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable {}
                .padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            IdenticonImage(
                seed = row.addressFull,
                size = VelaSpacing.xl4,
                contentDescription = row.name,
            )
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(VelaSpacing.xs),
            ) {
                Text(
                    text = row.name,
                    color = if (row.selected) colors.accentBase else colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = row.addressDisplay,
                    color = colors.fgSubtle,
                    fontFamily = VelaMonoFontFamily,
                    fontSize = VelaTextSize.sm,
                )
            }
            Text(
                text = row.amount,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
            )
            if (row.selected) {
                Spacer(modifier = Modifier.width(VelaSpacing.sm))
                Icon(
                    imageVector = VelaIcons.Check,
                    contentDescription = null,
                    tint = colors.accentBase,
                    modifier = Modifier.size(VelaIconSize.md),
                )
            }
        }
    }
    Spacer(modifier = Modifier.height(VelaSpacing.xl3))
    VelaPrimaryButton(sheet.primary, onClick = {}, modifier = Modifier.fillMaxWidth())
    Spacer(modifier = Modifier.height(VelaSpacing.lg))
    VelaSecondaryButton(sheet.secondary, onClick = {}, modifier = Modifier.fillMaxWidth())
}

@Composable
private fun FeedbackSheetBody(model: FeedbackModel) {
    val colors = VelaTheme.colors
    SheetTitle(model.title, model.subtitle)
    VelaUrlField(label = "", value = "", placeholder = model.placeholder)
    Text(
        text = model.addSteps,
        color = colors.infoBase,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        modifier = Modifier.padding(vertical = VelaSpacing.lg),
    )
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(colors.bgSunken)
            .padding(VelaSpacing.lg),
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
    ) {
        Text(
            text = model.previewToggle,
            color = colors.fgMuted,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
        )
        model.previewLines.forEach { line ->
            Text(
                text = line,
                color = colors.fgSubtle,
                fontFamily = VelaMonoFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
    }
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaCallout(CalloutModel(CalloutTone.Info, model.consent))
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaPrimaryButton(model.send, onClick = {}, modifier = Modifier.fillMaxWidth())
    Text(
        text = model.githubLink,
        color = colors.infoBase,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        textAlign = TextAlign.Center,
        modifier = Modifier.fillMaxWidth().padding(top = VelaSpacing.lg),
    )
}

@Composable
private fun RpcFixSheetBody(model: RpcFixModel, onPrimary: () -> Unit) {
    val colors = VelaTheme.colors
    SheetTitle(model.title)
    Row(
        modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        VelaChainMark(model.mark)
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = model.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl,
            )
            Text(
                text = model.meta,
                color = colors.fgSubtle,
                fontFamily = VelaMonoFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
        VelaStatusPill(model.badge)
    }
    VelaCallout(model.callout)
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaUrlField(
        label = model.field.label,
        value = model.field.value,
        badge = model.field.badge,
        tone = model.field.tone,
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaPrimaryButton(model.primary, onClick = onPrimary, modifier = Modifier.fillMaxWidth())
    if (model.providersLabel != null) {
        Text(
            text = model.providersLabel,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.sm,
            modifier = Modifier.padding(top = VelaSpacing.xl, bottom = VelaSpacing.md),
        )
        Row(horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md)) {
            model.providers.forEach { name ->
                Text(
                    text = name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                    modifier = Modifier
                        .background(colors.bgRaised)
                        .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
                )
            }
        }
    }
    if (model.report != null) {
        Text(
            text = model.report,
            color = colors.infoBase,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier.padding(top = VelaSpacing.xl),
        )
    }
}

@Composable
private fun BalanceDetailSheetBody(model: BalanceDetailModel) {
    val colors = VelaTheme.colors
    SheetTitle(model.title)
    Text(
        text = model.summary,
        color = colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        modifier = Modifier.padding(bottom = VelaSpacing.xl),
    )
    Text(
        text = model.sectionPending,
        color = colors.fgBase,
        fontFamily = VelaFontFamily,
        fontWeight = VelaFontWeight.semibold,
        fontSize = VelaTextSize.base,
    )
    Text(
        text = model.pendingNote,
        color = colors.fgSubtle,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.sm,
        modifier = Modifier.padding(vertical = VelaSpacing.md),
    )
    (model.pending + model.done).forEach { row ->
        Row(
            modifier = Modifier.fillMaxWidth().padding(vertical = VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            VelaChainMark(row.mark)
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = row.name,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.lg,
                )
                if (row.status != null) {
                    // Rate-limiting gets a grey line and no button because it
                    // resolves itself; a dead RPC gets red and 立即重试.
                    Text(
                        text = row.status,
                        color = if (row.tone == SettingsTone.Error) colors.errorBase else colors.fgSubtle,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.sm,
                    )
                }
            }
            if (row.action != null) {
                Text(
                    text = row.action,
                    color = colors.infoBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.base,
                )
            }
            if (row.amount != null) {
                Text(
                    text = row.amount,
                    color = colors.fgBase,
                    fontFamily = VelaFontFamily,
                    fontSize = VelaTextSize.lg,
                )
            }
        }
    }
}

@Composable
private fun RelayerSheetBody(model: RelayerModel, onPrimary: () -> Unit) {
    val colors = VelaTheme.colors
    SheetTitle(model.title)
    Text(
        text = model.lead,
        color = colors.fgMuted,
        fontFamily = VelaFontFamily,
        fontSize = VelaTextSize.base,
        modifier = Modifier.padding(bottom = VelaSpacing.xl),
    )
    Row(
        modifier = Modifier.fillMaxWidth().padding(bottom = VelaSpacing.xl),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        VelaChainMark(model.mark)
        Column {
            Text(
                text = model.name,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl,
            )
            Text(
                text = model.amountHint,
                color = colors.fgSubtle,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.sm,
            )
        }
    }
    Text(
        text = model.addressDisplay,
        color = colors.fgBase,
        fontFamily = VelaMonoFontFamily,
        fontSize = VelaTextSize.base,
        textAlign = TextAlign.Center,
        modifier = Modifier
            .fillMaxWidth()
            .background(colors.bgSunken)
            .padding(VelaSpacing.lg),
    )
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaCallout(model.callout)
    Spacer(modifier = Modifier.height(VelaSpacing.xl))
    VelaPrimaryButton(model.primary, onClick = onPrimary, modifier = Modifier.fillMaxWidth())
}
