package app.getvela.wallet.feature.explore

import app.getvela.wallet.core.designsystem.components.VelaModalSheet
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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
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
import app.getvela.wallet.feature.explore.components.BrowserNotice
import app.getvela.wallet.feature.explore.components.BrowserStatusLine
import app.getvela.wallet.feature.explore.components.BrowserStatusSheetContent
import app.getvela.wallet.feature.explore.components.ChainNotice
import app.getvela.wallet.feature.explore.components.GroupManageSheetContent
import app.getvela.wallet.feature.explore.components.PickerOption
import app.getvela.wallet.feature.explore.components.PickerSheetContent
import app.getvela.wallet.feature.explore.components.SiteMenuSheetContent
import app.getvela.wallet.feature.explore.components.TabMenuSheetContent
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
    /** Manage groups' eye: Favorites or Recent dApps hidden (`true`) or shown again. */
    val onGroupToggle: (String, Boolean) -> Unit,
    val onSiteMenuPick: (String) -> Unit,
    val onBookmark: () -> Unit,
    val onRecentClear: () -> Unit,
    /** The connection sheet's disconnect: the permissions machine's revoke (spec 044). */
    val onDisconnect: () -> Unit = {},
    /** The consent card's answer (spec 044). */
    val onConsent: (approved: Boolean) -> Unit = {},
    /** Spec 079: the chain notice's retry — one read of the page's chain. */
    val onChainRetry: () -> Unit = {},
    /** Spec 099 FR-014: the status line put away (its ✕ or its Details), by what it said. */
    val onStatusSeen: (String) -> Unit = {},
    /** Spec 099 FR-014: the shown tab's status panel opened (`true`) or closed — the core carries its record meanwhile. */
    val onInspector: (Boolean) -> Unit = {},
    /** Spec 099: a tab's menu — close every other tab (the core names which). */
    val onTabsCloseOthers: (String) -> Unit = {},
    /** Spec 099: a tab's menu — close every tab to its right in the strip (the core names which). */
    val onTabsCloseRight: (String) -> Unit = {},
    /** Spec 099: which of a tab's batch closes would take any tab — the core's answer, asked as its menu opens. */
    val tabCloses: (String) -> TabCloses = { TabCloses() },
    /** Spec 100: the add-network sheet's answers — the core decides what each means. */
    val onAddNetwork: (AddNetworkAction) -> Unit = {},
    /** Spec 100: the chain-setup tool, for a chain this wallet refuses. */
    val onChainSetupTool: () -> Unit = {},
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
    /** Back inside the page; `false` when it has no history left (spec 070: system Back then leaves the page). */
    onPageBack: () -> Boolean = { false },
    onPageForward: () -> Unit = {},
    /** Reload the page — or bring back a tab whose renderer died (spec 070). */
    onPageReload: () -> Unit = {},
    /** Spec 070: the networks the page in front may be put on, and the accounts the wallet has. */
    networkOptions: List<PickerOption> = emptyList(),
    onPickNetwork: (String) -> Unit = {},
    accountOptions: List<PickerOption> = emptyList(),
    onPickAccount: (String) -> Unit = {},
    live: ExploreCallbacks? = null,
    /** Spec 044: the core is asking whether this origin may connect; drawn as the connection sheet's not-yet-connected form. */
    consent: ConnectionModel? = null,
    /** Spec 100: a page asks to add a network — drawn after a consent, in the consent's place. */
    addNetwork: AddNetworkModel? = null,
    /**
     * Issue #273: the live scanner, drawn in place of the tab while open. It
     * hands back a web address to open (`onUrl`) or asks to be closed.
     */
    scanner: (@Composable (onUrl: (String) -> Unit, onClose: () -> Unit) -> Unit)? = null,
    /** The connection sheet's "Switch account" — the host's own switcher. */
    onSwitchAccount: (() -> Unit)? = null,
    /**
     * Spec 079: a page's signing request is up. The browser's own sheets give
     * way to it rather than stack behind it (device-found: the connection
     * panel sat under the signing sheet after an account switch).
     */
    signingOpen: Boolean = false,
    /** Spec 099 FR-014: the shown tab's status panel, from the core's record while it is open. */
    inspector: BrowserInspectorModel? = null,
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
    // …except a tab whose renderer died: it has no page, and shows the reload
    // panel where the page was (spec 070).
    val view = (viewOverride ?: model.view).let { if (live != null && it == ExploreView.Browsing && page == null && !model.browser.crashed) ExploreView.Start else it }
    var scanning by rememberSaveable { mutableStateOf(false) }
    /** Which pick-one sheet is up over the connection panel: `"network"` or `"account"`. */
    var picker by remember { mutableStateOf<String?>(null) }
    /** Spec 099 FR-014: the shown tab's status panel is up. */
    var statusOpen by remember { mutableStateOf(false) }
    /** Spec 099: the tab whose long-press menu is up in the switcher. */
    var tabMenu by remember { mutableStateOf<String?>(null) }
    val searchFocus = remember { androidx.compose.ui.focus.FocusRequester() }
    androidx.compose.runtime.LaunchedEffect(signingOpen) {
        if (signingOpen) {
            sheet = null
            picker = null
            statusOpen = false
            tabMenu = null
        }
    }
    // Spec 070: system Back walks the page's own history first, then leaves
    // the page for the start page — it used to leave 探索 altogether. The
    // switcher goes back to where it came from. (Registered before the
    // scanner's, which wins while scanning.)
    BackHandler(enabled = !scanning && live != null && view == ExploreView.Browsing) {
        if (!onPageBack()) viewOverride = ExploreView.Start
    }
    BackHandler(enabled = !scanning && live != null && view == ExploreView.Tabs) {
        viewOverride = if (page != null) ExploreView.Browsing else ExploreView.Start
    }
    BackHandler(enabled = scanning) { scanning = false }

    if (scanning && scanner != null) {
        scanner(
            { url -> scanning = false; onOpenUrl?.invoke(url); viewOverride = ExploreView.Browsing },
            { scanning = false },
        )
    } else Column(
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
                onMenu = { id -> tabMenu = id },
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
                    url = model.browser.url,
                    insecureLabel = strings.t("connect.browser.a11yInsecure"),
                    loading = model.browser.loading,
                    progress = model.browser.progress,
                    onSubmitUrl = onOpenUrl,
                    lockShown = model.browser.lockShown,
                )
                // Spec 079: the page loaded but its chain cannot be reached — said
                // once, under the address bar, while the page stays usable.
                model.browser.chainNotice?.takeIf { page != null && !model.browser.failed && !model.browser.crashed }?.let { notice ->
                    ChainNotice(
                        text = notice,
                        action = strings.t("connect.browser.retry"),
                        busy = model.browser.chainAsking,
                        busyLabel = strings.t("explore.loadRetrying"),
                        onAction = { live?.onChainRetry?.invoke() },
                    )
                }
                // Spec 099 FR-014: what the tab's layers last said, and a way into its record.
                model.browser.status?.takeIf { live != null }?.let { status ->
                    BrowserStatusLine(
                        status = status,
                        dismissLabel = strings.t("explore.close"),
                        onDetails = {
                            live?.onStatusSeen?.invoke(status.seen)
                            statusOpen = true
                        },
                        onDismiss = { live?.onStatusSeen?.invoke(status.seen) },
                    )
                }
                Box(Modifier.weight(1f)) {
                    when {
                        // The renderer died: the app is fine, the page is gone,
                        // and one tap brings it back (spec 070).
                        live != null && model.browser.crashed -> BrowserNotice(
                            title = strings.t("explore.pageCrashedTitle"),
                            body = strings.t("explore.pageCrashedBody"),
                            action = strings.t("explore.reload"),
                            onAction = onPageReload,
                        )
                        page != null -> {
                            page()
                            if (model.browser.failed) {
                                // Spec 079: why it failed (the core's words for the
                                // class), the host, and a retry that keeps this panel
                                // up — the engine's own error page is never shown.
                                val title = strings.t("connect.browser.loadFailed")
                                BrowserNotice(
                                    title = title,
                                    body = model.browser.failureReason?.takeIf { it != title }.orEmpty(),
                                    detail = model.browser.host,
                                    action = strings.t("connect.browser.retry"),
                                    onAction = onPageReload,
                                    busy = model.browser.retrying,
                                    busyLabel = strings.t("explore.loadRetrying"),
                                    modifier = Modifier.fillMaxSize(),
                                )
                            }
                        }
                        else -> DemoPage(model.browser.page, onAction = { if (signing != null) signingUp = true })
                    }
                }
                BrowserToolbar(
                    modifier = Modifier.navigationBarsPadding(),
                    browser = model.browser,
                    backLabel = strings.t("explore.back"),
                    forwardLabel = strings.t("explore.forward"),
                    accountLabel = strings.t("explore.account"),
                    connectedLabel = strings.t("explore.connectedTag"),
                    bookmarkLabel = strings.t(if (model.browser.bookmarked) "explore.removeFromFavorites" else "explore.addToFavorites"),
                    tabsLabel = strings.t("explore.tabs"),
                    onAccount = { sheet = ExploreSheet.Connection(model.connection) },
                    onTabs = { viewOverride = ExploreView.Tabs },
                    onBack = { onPageBack() },
                    onForward = onPageForward,
                    onBookmark = { live?.onBookmark() },
                )
            }

            ExploreView.Start -> {
                StartPage(
                    model = model,
                    hidden = hidden,
                    onOpenUrl = onOpenUrl?.let { open -> { text: String -> open(text); viewOverride = ExploreView.Browsing } },
                    onScan = scanner?.let { { scanning = true } },
                    // Live: nothing to "browse" but what a person types — the
                    // "+" tile and an empty Go put the cursor in the field.
                    onBrowse = { if (live != null) runCatching { searchFocus.requestFocus() } else viewOverride = ExploreView.Browsing },
                    searchFocus = searchFocus.takeIf { live != null },
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
        // Spec 079: like the signing sheet, the consent closes only on its ✕ or
        // 取消 — a stray swipe must not refuse a connection the person was reading.
        VelaModalSheet(
            onDismissRequest = { live?.onConsent(false) },
            containerColor = colors.bgBase,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            dismissible = false,
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
    if (consent == null) addNetwork?.let { card ->
        // Like the consent: it closes only on its ✕ or its buttons, and any
        // close is `dapp_add_declined` — the core decides what that answers.
        VelaModalSheet(
            onDismissRequest = { live?.onAddNetwork?.invoke(AddNetworkAction.Decline) },
            containerColor = colors.bgBase,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            dismissible = false,
        ) {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                app.getvela.wallet.feature.explore.components.AddNetworkPanel(
                    model = card,
                    onAction = { action -> live?.onAddNetwork?.invoke(action) },
                    onSetupTool = { live?.onChainSetupTool?.invoke() },
                )
            }
        }
    }
    sheet?.let { current ->
        VelaModalSheet(
            onDismissRequest = { sheet = null; picker = null },
            containerColor = colors.bgBase,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        ) {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                when (current) {
                    // Live: the rows are the core's and change under an open
                    // sheet (a hide showed only after reopening — device-found);
                    // the drawn snapshot serves the gallery.
                    is ExploreSheet.GroupManage -> {
                        val shown = if (live != null) model.groupManageSheet else current
                        GroupManageSheetContent(
                            sheet = shown,
                            hidden = hidden,
                            closeLabel = strings.t("explore.close"),
                            hideLabel = strings.t("explore.hide"),
                            showLabel = strings.t("explore.show"),
                            onClose = { sheet = null },
                            onToggle = { id ->
                                // The row as drawn, not as the sheet opened: the
                                // snapshot still said "shown" after a hide, so a
                                // second tap hid it again (#410).
                                val row = shown.rows.firstOrNull { it.id == id }
                                if (live != null && row != null) {
                                    live.onGroupToggle(id, !row.hidden)
                                } else {
                                    hidden = if (hidden.contains(id)) hidden - id else hidden + id
                                }
                            },
                        )
                    }

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

                    is ExploreSheet.Connection -> when {
                        picker == "network" && networkOptions.isNotEmpty() -> PickerSheetContent(
                            title = strings.t("explore.network"),
                            options = networkOptions,
                            closeLabel = strings.t("explore.close"),
                            onClose = { picker = null },
                            onPick = { id -> picker = null; onPickNetwork(id) },
                        )
                        picker == "account" && accountOptions.isNotEmpty() -> PickerSheetContent(
                            title = strings.t("explore.switchAccount"),
                            options = accountOptions,
                            closeLabel = strings.t("explore.close"),
                            onClose = { picker = null },
                            onPick = { id -> picker = null; onPickAccount(id) },
                        )
                        else -> ConnectionPanel(
                            connection = if (live != null) model.connection else current.connection,
                            closeLabel = strings.t("explore.close"),
                            onClose = { sheet = null },
                            onDisconnect = { sheet = null; live?.onDisconnect() },
                            onSwitchAccount = { picker = "account" }.takeIf { accountOptions.size > 1 },
                            onNetwork = { picker = "network" }.takeIf { networkOptions.isNotEmpty() },
                        )
                    }
                }
            }
        }
    }

    // Spec 099: a tab's long-press menu. Closing all goes to the start page, as
    // the switcher's own 关闭全部标签页 does; the other closes leave the
    // switcher up with what is left.
    tabMenu?.takeIf { view == ExploreView.Tabs }?.let { id -> model.tabs.firstOrNull { it.id == id } }?.let { tab ->
        val closes = remember(tab.id, model.tabs) { live?.tabCloses?.invoke(tab.id) ?: TabCloses() }
        VelaModalSheet(
            onDismissRequest = { tabMenu = null },
            containerColor = colors.bgBase,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        ) {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                TabMenuSheetContent(
                    tab = tab,
                    items = ExploreFixtures.tabMenu(model.tabsScreen, closes),
                    closeLabel = strings.t("explore.close"),
                    onClose = { tabMenu = null },
                    onPick = { pick ->
                        tabMenu = null
                        when (pick) {
                            ExploreFixtures.TAB_MENU_CLOSE -> live?.onTabClose(tab.id) ?: run { viewOverride = ExploreView.Start }
                            ExploreFixtures.TAB_MENU_OTHERS -> live?.onTabsCloseOthers(tab.id)
                            ExploreFixtures.TAB_MENU_RIGHT -> live?.onTabsCloseRight(tab.id)
                            ExploreFixtures.TAB_MENU_ALL -> { live?.onTabsCloseAll(); viewOverride = ExploreView.Start }
                        }
                    },
                )
            }
        }
    }

    if (statusOpen && live != null && view == ExploreView.Browsing) {
        // The core carries the tab's whole record only while this is up.
        androidx.compose.runtime.DisposableEffect(Unit) {
            live.onInspector(true)
            onDispose { live.onInspector(false) }
        }
        val context = androidx.compose.ui.platform.LocalContext.current
        VelaModalSheet(
            onDismissRequest = { statusOpen = false },
            containerColor = colors.bgBase,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        ) {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                inspector?.let { record ->
                    BrowserStatusSheetContent(
                        model = record,
                        closeLabel = strings.t("explore.close"),
                        onClose = { statusOpen = false },
                        onCopy = { report ->
                            (context.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as? android.content.ClipboardManager)
                                ?.setPrimaryClip(android.content.ClipData.newPlainText("tab status", report))
                            android.widget.Toast.makeText(context, record.copiedLabel, android.widget.Toast.LENGTH_SHORT).show()
                        },
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
    onScan: (() -> Unit)? = null,
    onTabs: () -> Unit,
    onManageGroups: () -> Unit,
    modifier: Modifier = Modifier,
    live: ExploreCallbacks? = null,
    onOpenSite: ((String) -> Unit)? = null,
    searchFocus: androidx.compose.ui.focus.FocusRequester? = null,
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
            onScan = onScan,
            focusRequester = searchFocus,
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

        // Issue #465: Recent dApps is the only group under Favorites — its one
        // action is Clear; hiding it lives in Manage groups, behind Edit.
        model.groups.filterNot { hidden.contains(it.id) }.forEach { group ->
            SectionHeader(
                title = group.title,
                action = strings.t("explore.clear"),
                onAction = { live?.onRecentClear() },
            )
            group.sites.forEach { site ->
                SiteRow(site = site, onOpen = { id -> if (onOpenSite != null) onOpenSite(id) else onBrowse() })
            }
        }

        Box(Modifier.padding(bottom = VelaSpacing.xl3))
    }
}
