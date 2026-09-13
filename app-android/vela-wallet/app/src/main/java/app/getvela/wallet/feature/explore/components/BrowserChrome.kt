package app.getvela.wallet.feature.explore.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.explore.BrowserModel
import app.getvela.wallet.feature.explore.DemoPageModel
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar

/**
 * The browsing top bar (mock E4): close, the domain in a pill with its padlock,
 * and the site menu. The pill shows the DOMAIN, never the full URL — the part
 * of an address that decides who you are talking to must not be pushed off the
 * end by a long path.
 */
@Composable
fun AddressBar(
    host: String,
    secure: Boolean,
    secureLabel: String,
    closeLabel: String,
    menuLabel: String,
    onClose: () -> Unit,
    onMenu: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
    ) {
        Box(
            modifier = Modifier
                .size(VelaSizing.hitTarget)
                .clickable(onClick = onClose),
            contentAlignment = Alignment.Center,
        ) { Icon(VelaIcons.Close, closeLabel, tint = colors.fgBase) }

        Row(
            modifier = Modifier
                .weight(1f)
                .height(ExploreMetrics.addressPill)
                .background(colors.bgRaised, CircleShape),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.Center,
        ) {
            if (secure) {
                Icon(
                    VelaIcons.Lock,
                    secureLabel,
                    tint = colors.fgMuted,
                    modifier = Modifier.size(VelaIconSize.xs),
                )
                Spacer(Modifier.size(VelaSpacing.md))
            }
            Text(
                text = host,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontSize = VelaTextSize.lg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }

        Box(
            modifier = Modifier
                .size(VelaSizing.hitTarget)
                .clickable(onClick = onMenu),
            contentAlignment = Alignment.Center,
        ) { Icon(VelaIcons.Ellipsis, menuLabel, tint = colors.fgBase) }
    }
}

/**
 * The browsing bottom bar (mock E4), which REPLACES the four-tab bar while a
 * page is open — two navigation bars on a 392dp screen is where the page would
 * have gone. The account chip's green dot IS the connection state.
 */
@Composable
fun BrowserToolbar(
    browser: BrowserModel,
    backLabel: String,
    forwardLabel: String,
    accountLabel: String,
    connectedLabel: String,
    bookmarkLabel: String,
    tabsLabel: String,
    onAccount: () -> Unit,
    onTabs: () -> Unit,
    modifier: Modifier = Modifier,
    onBack: () -> Unit = {},
    onForward: () -> Unit = {},
    onBookmark: () -> Unit = {},
) {
    val colors = VelaTheme.colors
    Row(
        modifier = modifier
            .fillMaxWidth()
            .height(ExploreMetrics.browserBar)
            .padding(horizontal = VelaSpacing.xl),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        ToolbarIcon(VelaIcons.ArrowLeft, backLabel, browser.canBack) { onBack() }
        ToolbarIcon(VelaIcons.ArrowRight, forwardLabel, browser.canForward) { onForward() }

        Row(
            modifier = Modifier
                .height(VelaSpacing.xl4)
                .background(colors.bgRaised, CircleShape)
                .clickable(onClick = onAccount)
                .padding(horizontal = VelaSpacing.md),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            IdenticonAvatar(tappable = false, seed = browser.accountSeed, size = ExploreMetrics.chipAvatar, name = browser.accountName)
            if (browser.connected) {
                Box(
                    Modifier
                        .size(VelaSpacing.md)
                        .background(colors.successBase, CircleShape),
                )
            }
        }

        ToolbarIcon(VelaIcons.Star, bookmarkLabel, enabled = true) { onBookmark() }
        Box(
            modifier = Modifier
                .defaultMinSize(ExploreMetrics.tabCount, ExploreMetrics.tabCount)
                .border(VelaBorder.emphasis, colors.fgBase, RoundedCornerShape(VelaRadius.sm))
                .clickable(onClick = onTabs),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = browser.tabCount.toString(),
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.base,
            )
        }
    }
}

@Composable
private fun ToolbarIcon(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    val colors = VelaTheme.colors
    Box(
        modifier = Modifier
            .size(VelaSizing.hitTarget)
            .clickable(enabled = enabled, onClick = onClick)
            .alpha(if (enabled) 1f else VelaOpacity.disabled),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, label, tint = if (enabled) colors.fgBase else colors.fgSubtle)
    }
}

/**
 * A stand-in for whatever site is open (spec 022 §2). Deliberately NOT chrome:
 * its words and its pink button belong to the SITE, so nothing here is
 * translated and nothing here uses a Vela colour token — the palette sits in
 * ExploreFixtures.Brand.DemoPage with the other content colours. A real WebView
 * replaces this composable wholesale.
 */
@Composable
fun DemoPage(page: DemoPageModel, onAction: () -> Unit, modifier: Modifier = Modifier) {
    val palette = ExploreFixtures.Brand.DemoPage
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(palette.surface)
            .padding(VelaSpacing.xl3),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(palette.card, RoundedCornerShape(VelaRadius.xl2))
                .padding(VelaSpacing.xl2),
            verticalArrangement = Arrangement.spacedBy(VelaSpacing.lg),
        ) {
            Text(
                text = page.title,
                color = palette.ink,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.semibold,
                fontSize = VelaTextSize.lg,
            )
            page.fields.forEach { field ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(palette.field, RoundedCornerShape(VelaRadius.lg))
                        .padding(VelaSpacing.xl),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = field.value,
                        color = palette.ink,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.xl2,
                    )
                    Text(
                        text = field.symbol,
                        color = palette.inkMuted,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(VelaSizing.controlMd)
                    .background(page.ctaTint, CircleShape)
                    .clickable(onClick = onAction),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = page.cta,
                    color = palette.card,
                    fontFamily = VelaFontFamily,
                    fontWeight = VelaFontWeight.semibold,
                    fontSize = VelaTextSize.lg,
                )
            }
        }
        Box(
            Modifier
                .fillMaxWidth(0.8f)
                .height(VelaSpacing.md)
                .background(palette.card, CircleShape),
        )
        Box(
            Modifier
                .fillMaxWidth(0.6f)
                .height(VelaSpacing.md)
                .background(palette.card, CircleShape),
        )
    }
}
