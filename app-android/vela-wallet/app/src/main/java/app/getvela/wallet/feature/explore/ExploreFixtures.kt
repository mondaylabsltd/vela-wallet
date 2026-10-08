package app.getvela.wallet.feature.explore

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.designsystem.components.VelaIcons
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.TabsModel
import app.getvela.wallet.feature.wallet.WalletFixtures

/**
 * Canonical explore fixtures (spec 022, data-model.md §2 — the single canon all
 * four platforms port; web reference: `src/lib/explore/fixtures.ts`).
 *
 * Site names, hosts and the demo page are verbatim mock content
 * and are never translated; every label resolves through the corpus. Brand hex
 * values are FIXTURE DATA, exempt from the tokens-only rule exactly as the
 * wallet's chain dots are.
 */
object ExploreFixtures {

    // --- Canon ----------------------------------------------------------------

    object Brand {
        val uniswap = Color(0xFFFF007A)
        val aave = Color(0xFF8B6DFF)
        val pancake = Color(0xFF1FC7D4)
        val polymarket = Color(0xFF4267F4)
        val opensea = Color(0xFF2081E2)
        val lido = Color(0xFFF0616D)
        val ens = Color(0xFF5284FF)
        val hyperliquid = Color(0xFF50D2C1)

        /** The stand-in web page's own palette (spec 022 §2). */
        object DemoPage {
            val surface = Color(0xFFF0EFEC)
            val card = Color(0xFFFFFFFF)
            val field = Color(0xFFF5F3EF)
            val ink = Color(0xFF1A1A18)
            val inkMuted = Color(0xFF8C887E)
        }
    }

    val uniswap = SiteModel("uniswap", "Uniswap", "app.uniswap.org", "U", Brand.uniswap)
    val aave = SiteModel("aave", "Aave", "app.aave.com", "A", Brand.aave)
    val pancake = SiteModel("pancake", "PancakeSwap", "pancakeswap.finance", "P", Brand.pancake)
    val polymarket = SiteModel("polymarket", "Polymarket", "polymarket.com", "P", Brand.polymarket)
    val opensea = SiteModel("opensea", "OpenSea", "opensea.io", "O", Brand.opensea)
    val lido = SiteModel("lido", "Lido", "stake.lido.fi", "L", Brand.lido)
    val ens = SiteModel("ens", "ENS", "app.ens.domains", "E", Brand.ens)
    val hyperliquid =
        SiteModel("hyperliquid", "Hyperliquid", "app.hyperliquid.xyz", "H", Brand.hyperliquid)

    /** The favourites grid, in mock order (E2/DE2). */
    val favorites: List<SiteModel> =
        listOf(uniswap, aave, pancake, polymarket, opensea, lido, ens)

    val networkName = "Ethereum"
    val networkDot = WalletFixtures.ChainColors.ethereum

    /** The page the browser shows. Fixture content: the site's words, not ours. */
    val demoPage = DemoPageModel(
        title = "兑换",
        fields = listOf(
            DemoPageModel.Field("0.5", "ETH"),
            DemoPageModel.Field("1,280.42", "USDC"),
        ),
        cta = "兑换",
        ctaTint = Brand.uniswap,
    )

    // --- Assembly -------------------------------------------------------------

    private fun groups(s: VelaStrings): List<GroupModel> = listOf(
        GroupModel(
            id = "recent",
            title = s.t("explore.recent"),
            kind = GroupKind.Recent,
            action = GroupAction.Clear,
            sites = listOf(hyperliquid.copy(meta = "刚刚")),
        ),
    )

    /**
     * The sites open in tabs, in strip order (the web board's phone strip):
     * three, so the home's resume section shows full — three rows under a
     * header that counts four tabs, because the start page is one of them.
     * The order is also the recency the fixture assumes: Uniswap was left last.
     */
    val tabSites: List<SiteModel> = listOf(uniswap, polymarket, aave)

    /**
     * The most rows the resume section draws — the core's
     * `explore_sites::RESUME_SHOWN`, which decides it live; mirrored here only
     * so the fixture is the shape the core hands over.
     */
    const val RESUME_SHOWN = 3

    /** The strip: one tab per site, then the start page's own tab. */
    private fun tabs(s: VelaStrings, selected: String, sites: List<SiteModel>): List<TabModel> =
        sites.map { site -> TabModel(site.id, site.name, site, selected == site.id, startPage = false) } +
            TabModel("start", s.t("explore.startPage"), null, selected == "start", startPage = true)

    /**
     * The home's resume section, built the way the core builds `resumable`:
     * the tabs with a page (never a start page), most recent first, at most
     * [RESUME_SHOWN]; the header counts every tab. No page anywhere, no section.
     */
    private fun resume(s: VelaStrings, strip: List<TabModel>): ResumeSection? {
        val withPage = strip.mapNotNull { tab -> tab.site?.takeUnless { tab.startPage }?.let { it.copy(id = tab.id, name = tab.title, subtitle = it.host) } }
        if (withPage.isEmpty()) return null
        return ResumeSection(
            title = s.t("explore.openTabs", mapOf("n" to strip.size.toString())),
            action = s.t("explore.tabs"),
            tabs = withPage.take(RESUME_SHOWN),
        )
    }

    /**
     * E6's site menu, in the board's order. Spec 099 navigation moved Forward
     * here when the app's tab bar took the browser toolbar's place: it leads,
     * greyed when there is nothing ahead, so every row under it keeps its
     * place from page to page. The star moved here from the toolbar too.
     */
    fun siteMenu(s: VelaStrings, canForward: Boolean = false) = ExploreSheet.SiteMenu(
        site = uniswap,
        statusLine = s.t("explore.secureSite"),
        items = listOf(
            SiteMenuItem("forward", VelaIcons.ArrowRight, s.t("explore.forward"), enabled = canForward),
            SiteMenuItem("refresh", VelaIcons.RefreshCw, s.t("explore.refresh")),
            SiteMenuItem("share", VelaIcons.Share2, s.t("explore.share")),
            SiteMenuItem("copy", VelaIcons.Copy, s.t("explore.copyLink")),
            SiteMenuItem("favorite", VelaIcons.Star, s.t("explore.addToFavorites")),
            SiteMenuItem("system", VelaIcons.ExternalLink, s.t("explore.openInSystemBrowser")),
            SiteMenuItem("disconnect", VelaIcons.Power, s.t("explore.disconnect")),
            SiteMenuItem("close", VelaIcons.Close, s.t("explore.closePage")),
        ),
    )

    /**
     * Spec 099: a tab's long-press menu — close it, then Chrome's three batch
     * closes, in the desktop's order. Which tabs each takes is the core's; a
     * close that would take none is shown greyed out.
     */
    fun tabMenu(copy: TabsScreenCopy, closes: TabCloses) = listOf(
        SiteMenuItem(TAB_MENU_CLOSE, VelaIcons.Close, copy.close),
        SiteMenuItem(TAB_MENU_OTHERS, VelaIcons.Close, copy.closeOthers, enabled = closes.others),
        SiteMenuItem(TAB_MENU_RIGHT, VelaIcons.ChevronRight, copy.closeRight, enabled = closes.right),
        SiteMenuItem(TAB_MENU_ALL, VelaIcons.Trash2, copy.closeAll),
    )

    const val TAB_MENU_CLOSE = "close"
    const val TAB_MENU_OTHERS = "others"
    const val TAB_MENU_RIGHT = "right"
    const val TAB_MENU_ALL = "all"

    fun connection(s: VelaStrings) = ConnectionModel(
        title = s.t("explore.connectionTitle"),
        site = uniswap,
        statusLine = "${s.t("explore.secureSite")} · ${s.t("explore.connectedTag")}",
        accountName = WalletFixtures.NAME,
        accountAddress = WalletFixtures.ADDRESS_DISPLAY,
        accountSeed = WalletFixtures.ADDRESS_FULL,
        switchLabel = s.t("explore.switchAccount"),
        networkLabel = s.t("explore.network"),
        networkName = networkName,
        networkDot = networkDot,
        explainer = s.t("explore.connectionExplainer"),
        disconnect = s.t("explore.disconnect"),
        footnote = s.t("explore.autoRequestHint"),
    )

    /** E3: the start page's two sections (issue #465), each with its eye. */
    fun groupManage(s: VelaStrings) = ExploreSheet.GroupManage(
        title = s.t("explore.manageGroups"),
        rows = listOf(
            GroupManageRow("favorites", s.t("explore.favorites"), s.t("explore.siteCount", mapOf("n" to "8"))),
            GroupManageRow("recent", s.t("explore.recent"), meta = null),
        ),
    )

    /** Every phone state (E1–E7). */
    fun buildState(state: ExploreScreenState, s: VelaStrings): ExploreScreenModel {
        val populated = state != ExploreScreenState.E1
        val browsing = state == ExploreScreenState.E4 ||
            state == ExploreScreenState.E6 ||
            state == ExploreScreenState.E7
        val view = when {
            browsing -> ExploreView.Browsing
            state == ExploreScreenState.E5 -> ExploreView.Tabs
            else -> ExploreView.Start
        }
        // E5 opens the switcher FROM a page, so the page's tab is the
        // selected one — the mock's accent border is on Uniswap, not on
        // 起始页 (device-found against E5, 2026-09-02). E1 is the first visit:
        // the start page is the only tab there is.
        val strip = tabs(
            s,
            if (browsing || state == ExploreScreenState.E5) "uniswap" else "start",
            if (populated) tabSites else emptyList(),
        )
        val canForward = false
        val groupManage = groupManage(s)
        val siteMenu = siteMenu(s, canForward)
        val connection = connection(s)

        return ExploreScreenModel(
            state = state,
            view = view,
            title = s.t("explore.title"),
            searchPlaceholder = s.t("explore.searchPlaceholder"),
            scanLabel = s.t("explore.scan"),
            empty = if (populated) {
                null
            } else {
                ExploreEmptyCopy(
                    s.t("explore.startTitle"), s.t("explore.startHint"), s.t("explore.startCta"),
                )
            },
            resume = resume(s, strip),
            favorites = if (populated) {
                FavoritesSection(
                    title = s.t("explore.favorites"),
                    action = s.t("explore.edit"),
                    tiles = favorites.map { TileModel.Site(it) } +
                        TileModel.Add(s.t("explore.add")),
                )
            } else {
                null
            },
            groups = if (populated) groups(s) else emptyList(),
            browser = BrowserModel(
                url = "https://${uniswap.host}/swap",
                host = uniswap.host,
                secure = true,
                connected = true,
                canBack = true,
                canForward = canForward,
                bookmarked = false,
                accountSeed = WalletFixtures.ADDRESS_FULL,
                tabCount = strip.size,
                page = demoPage,
            ),
            tabs = strip,
            tabsScreen = TabsScreenCopy(
                title = s.t("explore.tabs"),
                done = s.t("explore.done"),
                newTab = s.t("explore.newTab"),
                closeAll = s.t("explore.closeAllTabs"),
                close = s.t("explore.closeTab"),
                closeOthers = s.t("explore.closeOtherTabs"),
                closeRight = s.t("explore.closeTabsToRight"),
            ),
            sheet = when (state) {
                ExploreScreenState.E3 -> groupManage
                ExploreScreenState.E6 -> siteMenu
                ExploreScreenState.E7 -> ExploreSheet.Connection(connection)
                else -> null
            },
            groupManageSheet = groupManage,
            siteMenuSheet = siteMenu,
            connection = connection,
            nav = TabsModel(
                wallet = s.t("componentsUi.mainNav.wallet"),
                contacts = s.t("componentsUi.mainNav.contacts"),
                explore = s.t("componentsUi.mainNav.explore"),
                settings = s.t("componentsUi.mainNav.settings"),
            ),
        )
    }
}

/**
 * The signed-in wallet's identity over the fixture's (spec 019's swap). A
 * connection panel naming a stranger's account would be the wallet lying about
 * what it just granted.
 */
fun ExploreScreenModel.withIdentity(name: String, address: String): ExploreScreenModel = copy(
    browser = browser.copy(accountSeed = address),
    connection = connection.copy(
        accountName = name,
        accountAddress = shortenAddress(address),
        accountSeed = address,
    ),
)

/** `0x14fB1f…D1eA5c` — this client's short form (spec 015). */
private fun shortenAddress(address: String): String =
    if (address.length <= 14) address else "${address.take(6)}…${address.takeLast(4)}"
