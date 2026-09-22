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
 * Site names, hosts, group titles and the demo page are verbatim mock content
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
        val curve = Color(0xFF7B7BE8)
        val limitless = Color(0xFF8B6DFF)

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
    val curve = SiteModel("curve", "Curve", "curve.fi", "C", Brand.curve)
    val limitless = SiteModel("limitless", "Limitless", "limitless.exchange", "L", Brand.limitless)

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
        // Custom group titles and blurbs are what the person typed — mock
        // content, verbatim, never translated (the spec-015 rule).
        GroupModel(
            id = "trading",
            title = "交易",
            kind = GroupKind.Custom,
            action = GroupAction.Menu,
            sites = listOf(
                curve.copy(subtitle = "稳定币兑换"),
                hyperliquid.copy(subtitle = "永续合约交易"),
            ),
        ),
        GroupModel(
            id = "prediction",
            title = "预测市场",
            kind = GroupKind.Custom,
            action = GroupAction.Menu,
            sites = listOf(
                polymarket.copy(subtitle = "事件预测市场"),
                limitless.copy(subtitle = "预测市场"),
            ),
        ),
    )

    private fun tabs(s: VelaStrings, selected: String): List<TabModel> = listOf(
        TabModel("uniswap", uniswap.name, uniswap, selected == "uniswap", startPage = false),
        TabModel(
            "polymarket", polymarket.name, polymarket, selected == "polymarket", startPage = false,
        ),
        TabModel("start", s.t("explore.startPage"), null, selected == "start", startPage = true),
    )

    /** E6's site menu, in mock order. */
    fun siteMenu(s: VelaStrings) = ExploreSheet.SiteMenu(
        site = uniswap,
        statusLine = s.t("explore.secureSite"),
        items = listOf(
            SiteMenuItem("refresh", VelaIcons.RefreshCw, s.t("explore.refresh")),
            SiteMenuItem("share", VelaIcons.Share2, s.t("explore.share")),
            SiteMenuItem("copy", VelaIcons.Copy, s.t("explore.copyLink")),
            SiteMenuItem("favorite", VelaIcons.Star, s.t("explore.addToFavorites")),
            SiteMenuItem("system", VelaIcons.ExternalLink, s.t("explore.openInSystemBrowser")),
            SiteMenuItem("disconnect", VelaIcons.Power, s.t("explore.disconnect")),
            SiteMenuItem("close", VelaIcons.Close, s.t("explore.closePage")),
        ),
    )

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

    fun groupManage(s: VelaStrings) = ExploreSheet.GroupManage(
        title = s.t("explore.manageGroups"),
        newGroup = s.t("explore.newGroup"),
        rows = listOf(
            GroupManageRow(
                "favorites", s.t("explore.favorites"),
                s.t("explore.siteCount", mapOf("n" to "8")), system = true,
            ),
            GroupManageRow("recent", s.t("explore.recent"), s.t("explore.systemGroup"), true),
            GroupManageRow("trading", "交易", s.t("explore.siteCount", mapOf("n" to "4")), false),
            GroupManageRow(
                "prediction", "预测市场", s.t("explore.siteCount", mapOf("n" to "2")), false,
            ),
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
        val groupManage = groupManage(s)
        val siteMenu = siteMenu(s)
        val connection = connection(s)

        return ExploreScreenModel(
            state = state,
            view = view,
            title = s.t("explore.title"),
            tabCountLabel = if (populated) "2" else null,
            searchPlaceholder = s.t("explore.searchPlaceholder"),
            scanLabel = s.t("explore.scan"),
            empty = if (populated) {
                null
            } else {
                ExploreEmptyCopy(
                    s.t("explore.startTitle"), s.t("explore.startHint"), s.t("explore.startCta"),
                )
            },
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
                url = uniswap.host,
                host = uniswap.host,
                secure = true,
                connected = true,
                canBack = true,
                canForward = false,
                bookmarked = false,
                accountSeed = WalletFixtures.ADDRESS_FULL,
                tabCount = 2,
                page = demoPage,
            ),
            // E5 opens the switcher FROM a page, so the page's tab is the
            // selected one — the mock's accent border is on Uniswap, not on
            // 起始页 (device-found against E5, 2026-09-02).
            tabs = tabs(
                s,
                if (browsing || state == ExploreScreenState.E5) "uniswap" else "start",
            ),
            tabsScreen = TabsScreenCopy(
                title = s.t("explore.tabs"),
                done = s.t("explore.done"),
                newTab = s.t("explore.newTab"),
                closeAll = s.t("explore.closeAllTabs"),
                close = s.t("explore.closeTab"),
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
