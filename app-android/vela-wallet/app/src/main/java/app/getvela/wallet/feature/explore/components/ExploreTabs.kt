package app.getvela.wallet.feature.explore.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.components.VelaLogo
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.explore.TabModel
import app.getvela.wallet.feature.explore.TabsScreenCopy

private const val TAB_CARD_ASPECT = 3f / 4f

/**
 * The tab switcher (mock E5): a two-column grid of cards, a "+" that opens the
 * start page, and the one destructive affordance — 关闭全部标签页 — kept quiet
 * at the bottom rather than beside every card.
 */
@Composable
fun ExploreTabsScreen(
    tabs: List<TabModel>,
    copy: TabsScreenCopy,
    onDone: () -> Unit,
    onOpen: (String) -> Unit,
    onClose: (String) -> Unit,
    onNew: () -> Unit,
    onCloseAll: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase)
            .padding(horizontal = VelaSizing.screenPaddingX),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = VelaSpacing.xl2),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = copy.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl3,
            )
            Text(
                text = copy.done,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.xl,
                modifier = Modifier.clickable(onClick = onDone),
            )
        }

        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.xl),
            modifier = Modifier.weight(1f),
        ) {
            items(tabs, key = { it.id }) { tab ->
                TabCard(tab, copy.close, onOpen, onClose)
            }
            item {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .aspectRatio(TAB_CARD_ASPECT)
                        .background(colors.bgSunken, RoundedCornerShape(VelaRadius.xl))
                        .clickable(onClick = onNew),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(VelaIcons.Plus, null, tint = colors.fgMuted)
                    Text(
                        text = copy.newTab,
                        color = colors.fgMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
        }

        Text(
            text = copy.closeAll,
            color = colors.fgSubtle,
            fontFamily = VelaFontFamily,
            fontSize = VelaTextSize.base,
            modifier = Modifier
                .align(Alignment.CenterHorizontally)
                .clickable(onClick = onCloseAll)
                .padding(VelaSpacing.xl2),
        )
    }
}

/** One card: a stand-in preview, the site's mark and title, and its ✕. */
@Composable
private fun TabCard(
    tab: TabModel,
    closeLabel: String,
    onOpen: (String) -> Unit,
    onClose: (String) -> Unit,
) {
    val colors = VelaTheme.colors
    Column(
        modifier = Modifier
            .clip(RoundedCornerShape(VelaRadius.xl))
            .background(colors.bgSunken)
            .border(
                VelaBorder.emphasis,
                if (tab.selected) colors.accentBase else Color.Transparent,
                RoundedCornerShape(VelaRadius.xl),
            ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(TAB_CARD_ASPECT)
                .clickable { onOpen(tab.id) }
                .padding(VelaSpacing.xl),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.md, Alignment.CenterVertically),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            if (tab.startPage) {
                VelaLogo(darkTheme = VelaTheme.isDark, modifier = Modifier.size(VelaSpacing.xl5))
                Box(
                    Modifier
                        .fillMaxWidth(0.7f)
                        .height(VelaSpacing.lg)
                        .background(colors.bgRaised, CircleShape),
                )
            } else {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(VelaSpacing.xl3)
                        .background(colors.bgRaised, RoundedCornerShape(VelaRadius.md)),
                )
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(VelaSpacing.xl3)
                        .background(colors.bgRaised, RoundedCornerShape(VelaRadius.md)),
                )
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(VelaSpacing.xl2)
                        .background(tab.site?.tint ?: colors.bgRaised, CircleShape),
                )
            }
        }
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(colors.bgRaised)
                .padding(VelaSpacing.lg),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            tab.site?.let { SiteAvatar(it, size = VelaSpacing.xl2) }
            Text(
                text = tab.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.base,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Icon(
                VelaIcons.Close, closeLabel, tint = colors.fgMuted,
                modifier = Modifier.clickable { onClose(tab.id) },
            )
        }
    }
}
