package app.getvela.wallet.feature.wallet.gallery

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaLetterSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.components.ActionButtonRow
import app.getvela.wallet.feature.wallet.components.ActivityRow
import app.getvela.wallet.feature.wallet.components.AssetRow
import app.getvela.wallet.feature.wallet.components.BalanceDisplay
import app.getvela.wallet.feature.wallet.components.BalanceRefreshControl
import app.getvela.wallet.feature.wallet.components.ChainFilterList
import app.getvela.wallet.feature.wallet.components.EmptyState
import app.getvela.wallet.feature.wallet.components.NetworkFilterPill
import app.getvela.wallet.feature.wallet.components.SectionHeader
import app.getvela.wallet.feature.wallet.components.SkeletonActivityRow
import app.getvela.wallet.feature.wallet.components.SkeletonAssetRow
import app.getvela.wallet.feature.wallet.components.TokenIcon
import app.getvela.wallet.feature.wallet.components.VelaTabBar
import app.getvela.wallet.feature.wallet.components.WalletHeaderRow

/**
 * Component board (FR-004a): each vocabulary item with its variants, driven by
 * the same fixture builders as the full screens. Board captions are component
 * names — technical identifiers, not translated copy.
 */
@Composable
internal fun ComponentBoard() {
    val strings = LocalVelaStrings.current
    val h1 = remember(strings) { WalletFixtures.buildMobileState(WalletScreenState.H1, strings) }
    val h2 = remember(strings) { WalletFixtures.buildMobileState(WalletScreenState.H2, strings) }
    val h3 = remember(strings) { WalletFixtures.buildMobileState(WalletScreenState.H3, strings) }
    val h4 = remember(strings) { WalletFixtures.buildMobileState(WalletScreenState.H4, strings) }
    val h5 = remember(strings) { WalletFixtures.buildMobileState(WalletScreenState.H5, strings) }
    val h6 = remember(strings) { WalletFixtures.buildMobileState(WalletScreenState.H6, strings) }
    val h7 = remember(strings) { WalletFixtures.buildMobileState(WalletScreenState.H7, strings) }
    val h8 = remember(strings) { WalletFixtures.buildMobileState(WalletScreenState.H8, strings) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = VelaSizing.screenPaddingX, vertical = VelaSpacing.xl),
    ) {
        // The two no longer share a row on the wallet home — the header owns
        // it (founder call, 2026-08-26) — but the pill is still a component of
        // the vocabulary, so the board still shows both.
        BoardLabel("WalletHeader · NetworkFilterPill")
        WalletHeaderRow(header = h1.header)
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        NetworkFilterPill(model = h1.pill)
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        NetworkFilterPill(model = h7.pill)

        BoardLabel("BalanceDisplay · normal / zero-live / loading / hidden")
        BalanceDisplay(model = h1.balance)
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        BalanceDisplay(model = h2.balance)
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        BalanceDisplay(model = h3.balance)
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        BalanceDisplay(model = h5.balance)

        BoardLabel("BalanceStatusLine · warning / refreshing")
        BalanceDisplay(model = h4.balance)
        Spacer(modifier = Modifier.height(VelaSpacing.lg))
        BalanceDisplay(model = h6.balance)

        // Issue 462: the refresh control, idle and turning — the same box in
        // both, so the board shows nothing moves.
        BoardLabel("BalanceRefreshControl · idle / updating / never read")
        BalanceRefreshControl(model = WalletFixtures.refresh(strings), spinning = false, onRefresh = null)
        BalanceRefreshControl(model = WalletFixtures.refresh(strings, refreshing = true), spinning = true, onRefresh = null)
        BalanceRefreshControl(model = WalletFixtures.refresh(strings, read = false), spinning = false, onRefresh = null)

        BoardLabel("ActionButtonRow")
        ActionButtonRow(actions = h1.actions)

        BoardLabel("SectionHeader")
        SectionHeader(title = h1.activitySection.title, action = h1.activitySection.action)

        BoardLabel("ActivityRow · sent / received / dapp / masked")
        h1.activityGroups.flatMap { it.rows }.forEach { ActivityRow(model = it) }
        h5.activityGroups.firstOrNull()?.rows?.firstOrNull()?.let { ActivityRow(model = it) }

        BoardLabel("AssetRow · value / no-price / masked + TokenIcon")
        h1.assetRows.take(2).forEach { AssetRow(model = it) }
        h4.assetRows.lastOrNull()?.let { AssetRow(model = it) }
        h5.assetRows.firstOrNull()?.let { AssetRow(model = it) }
        Spacer(modifier = Modifier.height(VelaSpacing.md))
        TokenIcon(ticker = "BNB", badgeColor = WalletFixtures.ChainColors.bnb)

        BoardLabel("EmptyState")
        h2.activitySection.empty?.let { EmptyState(icon = VelaIcons.Inbox, model = it) }
        h2.assetsSection.empty?.let { EmptyState(icon = VelaIcons.Wallet, model = it) }

        BoardLabel("SkeletonRow")
        SkeletonActivityRow()
        SkeletonAssetRow()

        BoardLabel("ChainFilterList")
        h8.sheet?.let { ChainFilterList(rows = it.rows) }

        BoardLabel("TabBar")
        VelaTabBar(tabs = h1.tabs)
        Spacer(modifier = Modifier.height(VelaSpacing.xl4))
    }
}

@Composable
private fun BoardLabel(text: String) {
    Column {
        Spacer(modifier = Modifier.height(VelaSpacing.xl3))
        Text(
            text = text,
            color = VelaTheme.colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontWeight = VelaFontWeight.semibold,
            fontSize = VelaTextSize.sm,
            letterSpacing = VelaLetterSpacing.sectionLabel,
        )
        Spacer(modifier = Modifier.height(VelaSpacing.md))
    }
}
