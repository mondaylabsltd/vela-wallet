package app.getvela.wallet.feature.wallet

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.flows.WalletFlowEntry
import app.getvela.wallet.feature.wallet.components.ActionButtonRow
import app.getvela.wallet.feature.wallet.components.ActivityRow
import app.getvela.wallet.feature.wallet.components.AssetRow
import app.getvela.wallet.feature.wallet.components.ChainSelectSheet
import app.getvela.wallet.feature.wallet.components.DayLabel
import app.getvela.wallet.feature.wallet.components.EmptyState
import app.getvela.wallet.feature.wallet.components.IdenticonViewerSheet
import app.getvela.wallet.feature.wallet.components.SectionHeader
import app.getvela.wallet.feature.wallet.components.SkeletonActivityRow
import app.getvela.wallet.feature.wallet.components.SkeletonAssetRow
import app.getvela.wallet.feature.wallet.components.VelaTab
import app.getvela.wallet.feature.wallet.components.VelaTabBar
import app.getvela.wallet.feature.wallet.components.WalletHeaderRow
import app.getvela.wallet.feature.wallet.components.BalanceDisplay

/**
 * Mobile wallet home (spec 015 FR-002): assembles the component vocabulary for
 * any of the ten H-states from a fixture model alone — no business state, no
 * fetching. H7x applies its 1.35× text scale through LocalDensity (FR-011).
 */
@Composable
fun WalletScreen(
    model: WalletHomeModel,
    modifier: Modifier = Modifier,
    /**
     * The Settings tab (spec 019).
     *
     * The tab has existed since spec 015 with an `onSelect` hook nothing used.
     * It is the way back OUT of a signed-in wallet now — because wiring a route
     * guard without wiring its exit produces an app you cannot leave, which is
     * what the founder hit on iOS within a minute of the first successful
     * create, and what Phase 5 had already hit on desktop.
     *
     * ⚠ Sign-out is currently the ONLY thing behind it. A real settings screen
     * is a later feature; an unreachable wallet is not something to wait for it.
     */
    onSelectTab: (VelaTab) -> Unit = {},
    onSheetDismiss: () -> Unit = {},
    /**
     * Spec 021: the dock, the two section actions and the rows are the entries
     * into Receive / Send / Scan / Activity / Assets. Absent in the gallery,
     * where this screen is a picture.
     */
    onFlow: (WalletFlowEntry, String?) -> Unit = { _, _ -> },
    /** Spec 047: the header's name line opens the account switcher. */
    onSwitcher: () -> Unit = {},
) {
    if (model.textScale != 1f) {
        val density = LocalDensity.current
        CompositionLocalProvider(
            LocalDensity provides Density(density.density, density.fontScale * model.textScale),
        ) {
            WalletHomeContent(model, modifier, onSelectTab, onFlow, onSwitcher)
        }
    } else {
        WalletHomeContent(model, modifier, onSelectTab, onFlow, onSwitcher)
    }

    model.sheet?.let { sheet ->
        ChainSelectSheet(model = sheet, onDismiss = onSheetDismiss)
    }
}

@Composable
private fun WalletHomeContent(
    model: WalletHomeModel,
    modifier: Modifier = Modifier,
    /**
     * The Settings tab (spec 019).
     *
     * The tab has existed since spec 015 with an `onSelect` hook nothing used.
     * It is the way back OUT of a signed-in wallet now — because wiring a route
     * guard without wiring its exit produces an app you cannot leave, which is
     * what the founder hit on iOS within a minute of the first successful
     * create, and what Phase 5 had already hit on desktop.
     *
     * ⚠ Sign-out is currently the ONLY thing behind it. A real settings screen
     * is a later feature; an unreachable wallet is not something to wait for it.
     */
    onSelectTab: (VelaTab) -> Unit = {},
    onFlow: (WalletFlowEntry, String?) -> Unit = { _, _ -> },
    onSwitcher: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    val strings = LocalVelaStrings.current
    // The identicon viewer, opened from the artwork itself (founder call,
    // 2026-08-26). Sheet state is this screen's, not the model's: it is a way
    // of LOOKING at the wallet, not a state of it.
    var viewingIdenticon by remember { mutableStateOf(false) }

    if (viewingIdenticon) {
        IdenticonViewerSheet(
            address = model.header.identiconSeed,
            onDismiss = { viewingIdenticon = false },
        )
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase),
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .statusBarsPadding(),
        ) {
            Column(
                modifier = Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = VelaSizing.screenPaddingX),
            ) {
                Spacer(modifier = Modifier.height(VelaSpacing.xl))
                WalletHeaderRow(
                    header = model.header,
                    onIdenticon = { viewingIdenticon = true },
                onSwitcher = onSwitcher,
                    identiconLabel = strings.t(I18nKeys.Wallet.IDENTICON_A11Y_OPEN),
                )
                Spacer(modifier = Modifier.height(VelaSpacing.xl3))
                BalanceDisplay(model = model.balance)
                Spacer(modifier = Modifier.height(VelaSpacing.xl3))
                ActionButtonRow(
                    actions = model.actions,
                    onReceive = { onFlow(WalletFlowEntry.Receive, null) },
                    onSend = { onFlow(WalletFlowEntry.Send, null) },
                    onScan = { onFlow(WalletFlowEntry.Scan, null) },
                )
                Spacer(modifier = Modifier.height(VelaSpacing.xl4))

                SectionHeader(
                    title = model.activitySection.title,
                    action = model.activitySection.action,
                    onAction = { onFlow(WalletFlowEntry.Activity, null) },
                )
                when (model.activitySection.mode) {
                    SectionMode.Rows -> model.activityGroups.forEach { group ->
                        DayLabel(label = group.label)
                        group.rows.forEach { row ->
                            Box(modifier = Modifier.clickable { onFlow(WalletFlowEntry.TxDetail, row.id) }) {
                                ActivityRow(model = row)
                            }
                        }
                    }
                    SectionMode.Empty -> model.activitySection.empty?.let {
                        EmptyState(icon = VelaIcons.Inbox, model = it)
                    }
                    SectionMode.Loading -> {
                        Spacer(modifier = Modifier.height(VelaSpacing.md))
                        repeat(2) { SkeletonActivityRow() }
                    }
                }
                Spacer(modifier = Modifier.height(VelaSpacing.xl4))

                SectionHeader(
                    title = model.assetsSection.title,
                    action = model.assetsSection.action,
                    onAction = { onFlow(WalletFlowEntry.Assets, null) },
                )
                when (model.assetsSection.mode) {
                    SectionMode.Rows -> {
                        Spacer(modifier = Modifier.height(VelaSpacing.sm))
                        model.assetRows.forEach { row ->
                            AssetRow(
                                model = row,
                                onClick = { onFlow(WalletFlowEntry.TokenDetail, row.id) },
                            )
                        }
                    }
                    SectionMode.Empty -> model.assetsSection.empty?.let {
                        EmptyState(icon = VelaIcons.Wallet, model = it)
                    }
                    SectionMode.Loading -> {
                        Spacer(modifier = Modifier.height(VelaSpacing.md))
                        repeat(3) { SkeletonAssetRow() }
                    }
                }
                Spacer(modifier = Modifier.height(VelaSpacing.xl3))
            }
            VelaTabBar(
                tabs = model.tabs,
                modifier = Modifier
                    .fillMaxWidth()
                    .navigationBarsPadding(),
                onSelect = onSelectTab,
            )
        }
    }
}
