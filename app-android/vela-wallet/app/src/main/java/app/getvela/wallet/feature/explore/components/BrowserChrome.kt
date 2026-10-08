package app.getvela.wallet.feature.explore.components

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.feature.explore.BrowserModel
import app.getvela.wallet.feature.explore.DemoPageModel
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.wallet.components.IdenticonAvatar

/**
 * The browsing top bar — the ONE bar the browser draws on a phone (spec 099
 * navigation, board E4). The app's tab bar stays under the page, so the old
 * bottom toolbar is gone: its controls moved up here, the rarer ones (forward,
 * the star) into the site menu.
 *
 *   ‹  [ 🔒 host ]  (account)  [n]  ⋯
 *
 * - ‹ walks the page's history; with none left it returns to 探索's home, the
 *   tab kept alive. It is never greyed: there is always somewhere to go back to.
 * - The pill shows the DOMAIN, never the full URL, and when it must be cut it
 *   loses its START: the end of a host is the registrable domain, the part that
 *   decides who you are talking to (`app.uniswap.org.evil.xyz` must never read
 *   as `app.uniswap.or…`). A tap edits the full address in place (spec 070);
 *   the account, the count and ⋯ step aside while it does, and ‹, Back or
 *   leaving the field puts it away.
 * - The account's green dot IS the connection state; not connected, the
 *   identicon carries no dot and is never dimmed (dimmed reads as disabled).
 * - The boxed count opens the tab switcher.
 *
 * The three cluster targets touch, so their 44s carry the spacing; the pill
 * keeps every point the controls do not need. A hairline under the bar is the
 * page's load.
 */
@Composable
fun AddressBar(
    browser: BrowserModel,
    labels: AddressBarLabels,
    onBack: () -> Unit,
    onAccount: () -> Unit,
    onTabs: () -> Unit,
    onMenu: () -> Unit,
    modifier: Modifier = Modifier,
    /** A typed address, opened in this tab. Absent, the pill is not editable. */
    onSubmitUrl: ((String) -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    var editing by remember { mutableStateOf(false) }
    var draft by remember { mutableStateOf(TextFieldValue("")) }
    val focus = remember { FocusRequester() }
    // Back while the address is being edited puts the field away and goes
    // nowhere — registered inside the bar, so it wins over the page's Back.
    BackHandler(enabled = editing) { editing = false }
    Column(modifier = modifier.fillMaxWidth().background(colors.bgBase)) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = VelaSpacing.sm, vertical = VelaSpacing.md),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VelaSpacing.sm),
        ) {
            BarButton(onClick = { if (editing) editing = false else onBack() }) {
                Icon(VelaIcons.ChevronLeft, labels.back, tint = colors.fgBase, modifier = Modifier.size(VelaIconSize.lg))
            }

            if (editing) {
                var focused by remember { mutableStateOf(false) }
                BasicTextField(
                    value = draft,
                    onValueChange = { draft = it },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go, keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
                    keyboardActions = KeyboardActions(onGo = {
                        editing = false
                        if (draft.text.isNotBlank()) onSubmitUrl?.invoke(draft.text)
                    }),
                    cursorBrush = SolidColor(colors.accentBase),
                    textStyle = TextStyle(color = colors.fgBase, fontFamily = VelaFontFamily, fontSize = VelaTextSize.lg),
                    modifier = Modifier
                        .weight(1f)
                        // The field ends a gutter in from the edge, as ‹'s glyph
                        // starts one in from the other.
                        .padding(end = VelaSpacing.lg)
                        .height(ExploreMetrics.addressPill)
                        .background(colors.bgRaised, CircleShape)
                        .padding(horizontal = VelaSpacing.lg)
                        .focusRequester(focus)
                        .onFocusChanged { state ->
                            // Leaving the field cancels, as ‹ does — only once it had the focus.
                            if (focused && !state.isFocused) editing = false
                            focused = state.isFocused
                        }
                        .semantics { contentDescription = labels.field },
                    decorationBox = { field -> Box(contentAlignment = Alignment.CenterStart) { field() } },
                )
                LaunchedEffect(Unit) { focus.requestFocus() }
            } else {
                Row(
                    modifier = Modifier
                        .weight(1f)
                        .height(ExploreMetrics.addressPill)
                        .clip(CircleShape)
                        .background(colors.bgRaised)
                        .clickable(enabled = onSubmitUrl != null) {
                            draft = TextFieldValue(browser.url, selection = TextRange(0, browser.url.length))
                            editing = true
                        }
                        .padding(horizontal = VelaSpacing.lg),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md, Alignment.CenterHorizontally),
                ) {
                    // Spec 079 (owner): a lock, and only a lock. Closed and quiet
                    // for https — which says the line is encrypted, not that the
                    // site is honest, so it is decorative to a screen reader too;
                    // open and in the warning colour for plain http.
                    if (browser.lockShown) {
                        if (browser.secure) {
                            Icon(VelaIcons.Lock, null, tint = colors.fgMuted, modifier = Modifier.size(VelaIconSize.xs))
                        } else if (browser.host.isNotBlank()) {
                            Icon(VelaIcons.LockOpen, labels.insecure, tint = colors.warningBase, modifier = Modifier.size(VelaIconSize.xs))
                        }
                    }
                    Text(
                        text = browser.host,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontSize = VelaTextSize.lg,
                        maxLines = 1,
                        softWrap = false,
                        // Cut from the START (see the doc above).
                        overflow = TextOverflow.StartEllipsis,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                }

                // The account, the count and ⋯ read as one cluster: their 44s touch.
                Row(verticalAlignment = Alignment.CenterVertically) {
                    val account = if (browser.connected) "${labels.account}, ${labels.connected}" else labels.account
                    BarButton(onClick = onAccount, description = account) {
                        Box {
                            IdenticonAvatar(tappable = false, seed = browser.accountSeed, size = ExploreMetrics.barAvatar)
                            if (browser.connected) {
                                // The connection, as one green dot on the account —
                                // ringed in the bar's own colour so it reads on any
                                // artwork, sitting on the circle's edge.
                                Box(
                                    Modifier
                                        .align(Alignment.BottomEnd)
                                        .offset(x = VelaBorder.emphasis, y = VelaBorder.emphasis)
                                        .size(VelaSpacing.md + VelaBorder.emphasis * 2)
                                        .background(colors.bgBase, CircleShape)
                                        .padding(VelaBorder.emphasis)
                                        .background(colors.successBase, CircleShape),
                                )
                            }
                        }
                    }
                    BarButton(onClick = onTabs, description = labels.tabs) {
                        Box(
                            modifier = Modifier
                                .height(ExploreMetrics.tabCount)
                                .defaultMinSize(minWidth = ExploreMetrics.tabCount)
                                .border(VelaBorder.emphasis, colors.fgBase, RoundedCornerShape(VelaRadius.sm))
                                .padding(horizontal = VelaSpacing.sm),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = tabCountText(browser.tabCount),
                                color = colors.fgBase,
                                fontFamily = VelaFontFamily,
                                fontWeight = VelaFontWeight.semibold,
                                fontSize = VelaTextSize.base,
                                style = TextStyle(fontFeatureSettings = "tnum"),
                                maxLines = 1,
                                softWrap = false,
                            )
                        }
                    }
                    BarButton(onClick = onMenu) {
                        Icon(VelaIcons.Ellipsis, labels.menu, tint = colors.fgBase, modifier = Modifier.size(VelaIconSize.lg))
                    }
                }
            }
        }
        // The load, as a hairline the width of the screen: present only while
        // something is loading, so a finished page carries no chrome for it.
        Box(Modifier.fillMaxWidth().height(VelaBorder.emphasis)) {
            if (browser.loading) {
                Box(
                    Modifier
                        .fillMaxWidth(browser.progress.coerceIn(5, 100) / 100f)
                        .fillMaxHeight()
                        .background(colors.accentBase),
                )
            }
        }
    }
}

/** The bar's words, resolved by the screen (every one a corpus key). */
data class AddressBarLabels(
    /** explore.back */
    val back: String,
    /** explore.account */
    val account: String,
    /** explore.connectedTag */
    val connected: String,
    /** explore.tabs */
    val tabs: String,
    /** explore.siteMenu */
    val menu: String,
    /** explore.addressBar — the field while the address is being edited. */
    val field: String,
    /** connect.browser.a11yInsecure — the open lock of a plain-http page. */
    val insecure: String,
)

/**
 * The count in the bar's box. The core caps the strip at 24 tabs, so two
 * digits is the most it shows; past 99 it would say "99+" rather than wrap
 * or push the pill.
 */
fun tabCountText(count: Int): String = if (count > MAX_COUNT_SHOWN) "$MAX_COUNT_SHOWN+" else count.toString()

private const val MAX_COUNT_SHOWN = 99

/** One 44 target of the bar. */
@Composable
private fun BarButton(onClick: () -> Unit, description: String? = null, content: @Composable () -> Unit) {
    Box(
        modifier = Modifier
            .size(VelaSizing.hitTarget)
            .clickable(role = Role.Button, onClick = onClick)
            .let { base -> description?.let { text -> base.semantics(mergeDescendants = true) { contentDescription = text } } ?: base },
        contentAlignment = Alignment.Center,
    ) { content() }
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
