package app.getvela.wallet.feature.explore

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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
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
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.explore.components.AddressBar
import app.getvela.wallet.feature.explore.components.BrowserToolbar
import app.getvela.wallet.feature.explore.components.ConnectionPanel
import app.getvela.wallet.feature.explore.components.DemoPage
import app.getvela.wallet.feature.explore.components.ExploreEmpty
import app.getvela.wallet.feature.explore.components.ExploreMetrics
import app.getvela.wallet.feature.explore.components.ExploreSearchField
import app.getvela.wallet.feature.explore.components.ExploreTabsScreen
import app.getvela.wallet.feature.explore.components.GroupManageSheetContent
import app.getvela.wallet.feature.explore.components.SiteMenuSheetContent
import app.getvela.wallet.feature.explore.components.SiteRow
import app.getvela.wallet.feature.explore.components.SiteTile
import app.getvela.wallet.feature.signing.SigningScreenModel
import app.getvela.wallet.feature.signing.SigningSheet
import app.getvela.wallet.feature.wallet.components.SectionHeader
import app.getvela.wallet.feature.wallet.components.VelaTab
import app.getvela.wallet.feature.wallet.components.VelaTabBar

/**
 * The Explore tab (spec 022 FR-002): one surface with three views — the start
 * page, a page being browsed, and the tab switcher — assembled from the
 * component vocabulary. Screens compose components, never re-implement them.
 *
 * Every E-state renders from fixtures alone; what a person DOES here is local
 * state layered over the model, so swapping the model (a locale change, the
 * preview gallery's state picker) still lands.
 */
/** Spec 044: the taps that reach the browser controller when the tab is live. A site's id is its URL. */
class ExploreCallbacks(
    val onOpenSite: (String) -> Unit,
    val onTabOpen: (String) -> Unit,
    val onTabClose: (String) -> Unit,
    val onTabNew: () -> Unit,
    val onTabsCloseAll: () -> Unit,
    val onGroupToggle: (String, Boolean) -> Unit,
    val onGroupNew: () -> Unit,
    val onSiteMenuPick: (String) -> Unit,
    val onBookmark: () -> Unit,
    val onRecentClear: () -> Unit,
    /** The connection sheet's disconnect: the permissions machine's revoke (spec 044). */
    val onDisconnect: () -> Unit = {},
    /** The consent card's answer (spec 044). */
    val onConsent: (approved: Boolean) -> Unit = {},
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ExploreScreen(
    model: ExploreScreenModel,
    modifier: Modifier = Modifier,
    signing: SigningScreenModel? = null,
    onSelectTab: (VelaTab) -> Unit = {},
    /** Spec 044: the live page, drawn where the demo page is when present. */
    page: (@Composable () -> Unit)? = null,
    /** Spec 044: which view to open on when a live page exists. */
    initialView: ExploreView? = null,
    /** Spec 044: the address typed on the start page becomes a real navigation. */
    onOpenUrl: ((String) -> Unit)? = null,
    onClosePage: () -> Unit = {},
    onPageBack: () -> Unit = {},
    onPageForward: () -> Unit = {},
    live: ExploreCallbacks? = null,
    /** Spec 044: the core is asking whether this origin may connect; drawn as the connection sheet's not-yet-connected form. */
    consent: ConnectionModel? = null,
) {
    val colors = VelaTheme.colors
    val strings = LocalVelaStrings.current

    var viewOverride by rememberSaveable(model.state, initialView) { mutableStateOf(initialView) }
    var sheet by remember(model.state) { mutableStateOf(model.sheet) }
    var signingUp by remember(model.state) { mutableStateOf(false) }
    /// Groups hidden HERE rather than in the fixture: hiding is something a
    /// person does, and the sheet has to show it happening.
    var hidden by rememberSaveable(model.state) { mutableStateOf(emptySet<String>()) }

    // Live (spec 044): the browsing view exists only while a page does. Without
    // this the switcher's Done, with only the start tab left, drew the demo
    // page — a fixture on a live route (device-found).
    val view = (viewOverride ?: model.view).let { if (live != null && it == ExploreView.Browsing && page == null) ExploreView.Start else it }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.bgBase),
    ) {
        when (view) {
            ExploreView.Tabs -> ExploreTabsScreen(
                modifier = Modifier
                    .weight(1f)
                    .statusBarsPadding()
                    .navigationBarsPadding(),
                tabs = model.tabs,
                copy = model.tabsScreen,
                onDone = { viewOverride = if (live == null || page != null) ExploreView.Browsing else ExploreView.Start },
                onOpen = { id -> live?.onTabOpen(id); viewOverride = ExploreView.Browsing },
                onClose = { id -> live?.onTabClose(id) ?: run { viewOverride = ExploreView.Start } },
                onNew = { live?.onTabNew(); viewOverride = ExploreView.Start },
                onCloseAll = { live?.onTabsCloseAll(); viewOverride = ExploreView.Start },
            )

            ExploreView.Browsing -> {
                AddressBar(
                    modifier = Modifier.statusBarsPadding(),
                    host = model.browser.host,
                    secure = model.browser.secure,
                    secureLabel = strings.t("explore.secureSite"),
                    closeLabel = strings.t("explore.closePage"),
                    menuLabel = strings.t("explore.siteMenu"),
                    onClose = { onClosePage(); viewOverride = ExploreView.Start },
                    onMenu = { sheet = model.siteMenuSheet },
                )
                Box(Modifier.weight(1f)) {
                    if (page != null) page() else DemoPage(model.browser.page, onAction = { if (signing != null) signingUp = true })
                }
                BrowserToolbar(
                    modifier = Modifier.navigationBarsPadding(),
                    browser = model.browser,
                    backLabel = strings.t("explore.back"),
                    forwardLabel = strings.t("explore.forward"),
                    accountLabel = strings.t("explore.account"),
                    connectedLabel = strings.t("explore.connectedTag"),
                    bookmarkLabel = strings.t("explore.addToFavorites"),
                    tabsLabel = strings.t("explore.tabs"),
                    onAccount = { sheet = ExploreSheet.Connection(model.connection) },
                    onTabs = { viewOverride = ExploreView.Tabs },
                    onBack = onPageBack,
                    onForward = onPageForward,
                    onBookmark = { live?.onBookmark() },
                )
            }

            ExploreView.Start -> {
                StartPage(
                    model = model,
                    hidden = hidden,
                    onOpenUrl = onOpenUrl?.let { open -> { text: String -> open(text); viewOverride = ExploreView.Browsing } },
                    onBrowse = { viewOverride = ExploreView.Browsing },
                    onTabs = { viewOverride = ExploreView.Tabs },
                    onManageGroups = { sheet = model.groupManageSheet },
                    live = live,
                    onOpenSite = { url -> live?.onOpenSite(url); viewOverride = ExploreView.Browsing },
                    modifier = Modifier.weight(1f),
                )
                // Device-found on the Xiaomi (2026-09-02): without this the bar
                // sits UNDER the system navigation and the four labels are cut
                // in half. WalletScreen has always done it; Explore inherited
                // the bar without the padding that makes it reachable.
                VelaTabBar(
                    tabs = model.nav,
                    modifier = Modifier
                        .fillMaxWidth()
                        .navigationBarsPadding(),
                    selected = VelaTab.Explore,
                    onSelect = onSelectTab,
                )
            }
        }
    }

    consent?.let { card ->
        ModalBottomSheet(
            onDismissRequest = { live?.onConsent(false) },
            containerColor = colors.bgBase,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        ) {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                ConnectionPanel(
                    connection = card,
                    closeLabel = strings.t("connect.browser.cancel"),
                    onClose = { live?.onConsent(false) },
                    onDisconnect = { live?.onConsent(true) },
                )
            }
        }
    }
    sheet?.let { current ->
        ModalBottomSheet(
            onDismissRequest = { sheet = null },
            containerColor = colors.bgBase,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        ) {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                when (current) {
                    // Live: the rows are the core's and change under an open
                    // sheet (a new group appeared only after reopening —
                    // device-found); the drawn snapshot serves the gallery.
                    is ExploreSheet.GroupManage -> GroupManageSheetContent(
                        sheet = if (live != null) model.groupManageSheet else current,
                        hidden = hidden,
                        closeLabel = strings.t("explore.close"),
                        hideLabel = strings.t("explore.hide"),
                        showLabel = strings.t("explore.show"),
                        deleteLabel = strings.t("explore.delete"),
                        onClose = { sheet = null },
                        onToggle = { id ->
                            val row = current.rows.firstOrNull { it.id == id }
                            if (live != null && row != null) {
                                live.onGroupToggle(id, !row.hidden)
                            } else {
                                hidden = if (hidden.contains(id)) hidden - id else hidden + id
                            }
                        },
                        onNew = { live?.onGroupNew() },
                    )

                    is ExploreSheet.SiteMenu -> SiteMenuSheetContent(
                        sheet = current,
                        closeLabel = strings.t("explore.close"),
                        onClose = { sheet = null },
                        onPick = { id ->
                            sheet = null
                            live?.onSiteMenuPick(id)
                            if (id == "close") { onClosePage(); viewOverride = ExploreView.Start }
                        },
                    )

                    is ExploreSheet.Connection -> ConnectionPanel(
                        connection = if (live != null) model.connection else current.connection,
                        closeLabel = strings.t("explore.close"),
                        onClose = { sheet = null },
                        onDisconnect = { sheet = null; live?.onDisconnect() },
                    )
                }
            }
        }
    }

    if (signingUp && signing != null) {
        SigningSheet(model = signing, onDismiss = { signingUp = false })
    }
}

@Composable
private fun StartPage(
    model: ExploreScreenModel,
    hidden: Set<String>,
    onBrowse: () -> Unit,
    onOpenUrl: ((String) -> Unit)? = null,
    onTabs: () -> Unit,
    onManageGroups: () -> Unit,
    modifier: Modifier = Modifier,
    live: ExploreCallbacks? = null,
    onOpenSite: ((String) -> Unit)? = null,
) {
    val colors = VelaTheme.colors
    val strings = LocalVelaStrings.current
    Column(
        modifier = modifier
            .fillMaxWidth()
            // Same convention as WalletScreen/ContactsScreen: the scrolling
            // body clears the status bar, the bars at the edges clear the
            // navigation bar. Without it the title sat under the clock and the
            // tab labels under the gesture bar (device-found, Xiaomi alioth).
            .statusBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = VelaSizing.screenPaddingX),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = VelaSpacing.xl2, bottom = VelaSpacing.xl),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = model.title,
                color = colors.fgBase,
                fontFamily = VelaFontFamily,
                fontWeight = VelaFontWeight.bold,
                fontSize = VelaTextSize.xl3,
            )
            model.tabCountLabel?.let { count ->
                Box(
                    modifier = Modifier
                        .defaultMinSize(ExploreMetrics.tabCount, ExploreMetrics.tabCount)
                        .border(
                            VelaBorder.emphasis, colors.fgBase,
                            RoundedCornerShape(VelaRadius.sm),
                        )
                        .clickable(onClick = onTabs),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = count,
                        color = colors.fgBase,
                        fontFamily = VelaFontFamily,
                        fontWeight = VelaFontWeight.semibold,
                        fontSize = VelaTextSize.base,
                    )
                }
            }
        }

        ExploreSearchField(
            placeholder = model.searchPlaceholder,
            scanLabel = model.scanLabel,
            onSubmit = { text -> if (onOpenUrl != null && text.isNotBlank()) onOpenUrl(text) else onBrowse() },
        )

        model.empty?.let {
            ExploreEmpty(it, onBrowse = onBrowse)
        }

        model.favorites?.let { favorites ->
            SectionHeader(
                title = favorites.title,
                action = favorites.action,
                onAction = onManageGroups,
                modifier = Modifier.padding(top = VelaSpacing.xl),
            )
            // A fixed four-column grid, not a lazy one: this sits inside a
            // scrolling column, and nesting a lazy grid in one is what makes
            // Compose throw about infinite height constraints.
            favorites.tiles.chunked(4).forEach { row ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = VelaSpacing.md),
                    horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                ) {
                    row.forEach { tile ->
                        SiteTile(
                            tile = tile,
                            onOpen = { id -> if (onOpenSite != null && id != "add") onOpenSite(id) else onBrowse() },
                            modifier = Modifier.weight(1f),
                        )
                    }
                    repeat(4 - row.size) { Box(Modifier.weight(1f)) }
                }
            }
        }

        model.groups.filterNot { hidden.contains(it.id) }.forEach { group ->
            SectionHeader(
                title = group.title,
                action = if (group.action == GroupAction.Clear) {
                    strings.t("explore.clear")
                } else {
                    "⋯"
                },
                onAction = { if (group.action == GroupAction.Clear && live != null) live.onRecentClear() else onManageGroups() },
            )
            group.sites.forEach { site ->
                SiteRow(site = site, onOpen = { id -> if (onOpenSite != null) onOpenSite(id) else onBrowse() })
            }
        }

        Box(Modifier.padding(bottom = VelaSpacing.xl3))
    }
}
