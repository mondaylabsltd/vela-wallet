package app.getvela.wallet.feature.wallet.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.foundation.selection.selectable
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.feature.wallet.TabsModel

/** Main navigation destinations (only Wallet has content in spec 015). */
enum class VelaTab { Wallet, Contacts, Explore, Settings }

/**
 * Mobile tab bar (spec vocabulary #13): solid icon + accent tint when
 * selected, outline icon + fg.subtle otherwise (FR-007). Icons only since
 * spec 078 round 2; each tab keeps its label as its accessible name.
 */
@Composable
fun VelaTabBar(
    tabs: TabsModel,
    modifier: Modifier = Modifier,
    selected: VelaTab = VelaTab.Wallet,
    onSelect: (VelaTab) -> Unit = {},
) {
    val colors = VelaTheme.colors
    val haptic = rememberVelaHaptic()
    // A tab is a button under the finger (the founder's rule: press =
    // deformation + haptic; tabs too since 2026-09-26). Only a tab that
    // CHANGES the destination buzzes — re-tapping the one in force is not a
    // switch.
    val tap: (VelaTab) -> Unit = { tab ->
        if (tab != selected) haptic(VelaHaptic.Press)
        onSelect(tab)
    }
    Column(modifier = modifier.fillMaxWidth()) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(VelaBorder.hairline)
                .background(colors.borderBase),
        )
        Row(modifier = Modifier.fillMaxWidth()) {
            TabItem(
                label = tabs.wallet,
                outline = VelaIcons.NavWalletOutline,
                solid = VelaIcons.NavWalletSolid,
                selected = selected == VelaTab.Wallet,
                onClick = { tap(VelaTab.Wallet) },
                modifier = Modifier.weight(1f),
            )
            TabItem(
                label = tabs.contacts,
                outline = VelaIcons.NavContactsOutline,
                solid = VelaIcons.NavContactsSolid,
                selected = selected == VelaTab.Contacts,
                onClick = { tap(VelaTab.Contacts) },
                modifier = Modifier.weight(1f),
            )
            TabItem(
                label = tabs.explore,
                outline = VelaIcons.NavExploreOutline,
                solid = VelaIcons.NavExploreSolid,
                selected = selected == VelaTab.Explore,
                onClick = { tap(VelaTab.Explore) },
                modifier = Modifier.weight(1f),
            )
            TabItem(
                label = tabs.settings,
                outline = VelaIcons.NavSettingsOutline,
                solid = VelaIcons.NavSettingsSolid,
                selected = selected == VelaTab.Settings,
                onClick = { tap(VelaTab.Settings) },
                modifier = Modifier.weight(1f),
            )
        }
    }
}

@Composable
private fun TabItem(
    label: String,
    outline: ImageVector,
    solid: ImageVector,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    val tint = if (selected) colors.accentBase else colors.fgSubtle
    // Icons only (spec 078 round 2): the words truncated in es/pt/de/it. The
    // word is still the tab's NAME — a screen reader says "设置, 已选择" /
    // "Settings, selected" — through the whole tab's semantics, never the
    // glyph's (a tab named twice is noise).
    Box(
        modifier = modifier
            .height(VelaSizing.tabBar)
            .selectable(
                selected = selected,
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
                role = Role.Tab,
                onClick = onClick,
            )
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = if (selected) solid else outline,
            contentDescription = null,
            tint = tint,
            modifier = Modifier.size(VelaSizing.tabIcon),
        )
    }
}
