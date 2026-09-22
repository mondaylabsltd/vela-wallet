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
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
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
 *
 * Spec 070: a tap on the pill edits the full address in place (Go opens it in
 * this tab); an insecure page shows a warning, never nothing; a hairline under
 * the bar is the page's load.
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
    url: String = "",
    insecureLabel: String = "",
    loading: Boolean = false,
    progress: Int = 100,
    onSubmitUrl: ((String) -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    var editing by remember { mutableStateOf(false) }
    var draft by remember { mutableStateOf(TextFieldValue("")) }
    val focus = remember { FocusRequester() }
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.lg, vertical = VelaSpacing.md),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
        ) {
            Box(
                modifier = Modifier
                    .size(VelaSizing.hitTarget)
                    .clickable { if (editing) editing = false else onClose() },
                contentAlignment = Alignment.Center,
            ) { Icon(VelaIcons.Close, closeLabel, tint = colors.fgBase) }

            Row(
                modifier = Modifier
                    .weight(1f)
                    .height(ExploreMetrics.addressPill)
                    .background(colors.bgRaised, CircleShape)
                    .clickable(enabled = onSubmitUrl != null && !editing) {
                        draft = TextFieldValue(url, selection = TextRange(0, url.length))
                        editing = true
                    }
                    .padding(horizontal = VelaSpacing.lg),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.Center,
            ) {
                if (editing) {
                    BasicTextField(
                        value = draft,
                        onValueChange = { draft = it },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go, keyboardType = KeyboardType.Uri),
                        keyboardActions = KeyboardActions(onGo = {
                            editing = false
                            if (draft.text.isNotBlank()) onSubmitUrl?.invoke(draft.text)
                        }),
                        cursorBrush = SolidColor(colors.accentBase),
                        textStyle = TextStyle(color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.lg),
                        modifier = Modifier
                            .fillMaxWidth()
                            .focusRequester(focus),
                    )
                    LaunchedEffect(Unit) { focus.requestFocus() }
                } else {
                    if (secure) {
                        Icon(VelaIcons.Lock, secureLabel, tint = colors.fgMuted, modifier = Modifier.size(VelaIconSize.xs))
                        Spacer(Modifier.size(VelaSpacing.md))
                    } else if (host.isNotBlank()) {
                        Icon(VelaIcons.TriangleAlert, insecureLabel, tint = colors.warningBase, modifier = Modifier.size(VelaIconSize.xs))
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
            }

            Box(
                modifier = Modifier
                    .size(VelaSizing.hitTarget)
                    .clickable(onClick = onMenu),
                contentAlignment = Alignment.Center,
            ) { Icon(VelaIcons.Ellipsis, menuLabel, tint = colors.fgBase) }
        }
        // The load, as a hairline the width of the screen: present only while
        // something is loading, so a finished page carries no chrome for it.
        Box(Modifier.fillMaxWidth().height(VelaBorder.emphasis)) {
            if (loading) {
                Box(
                    Modifier
                        .fillMaxWidth(progress.coerceIn(5, 100) / 100f)
                        .fillMaxHeight()
                        .background(colors.accentBase),
                )
            }
        }
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
            IdenticonAvatar(tappable = false, seed = browser.accountSeed, size = ExploreMetrics.chipAvatar)
            if (browser.connected) {
                Box(
                    Modifier
                        .size(VelaSpacing.md)
                        .background(colors.successBase, CircleShape),
                )
            }
        }

        ToolbarIcon(VelaIcons.Star, bookmarkLabel, enabled = true, tint = if (browser.bookmarked) colors.accentBase else null) { onBookmark() }
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
    tint: androidx.compose.ui.graphics.Color? = null,
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
        Icon(icon, label, tint = tint ?: if (enabled) colors.fgBase else colors.fgSubtle)
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
