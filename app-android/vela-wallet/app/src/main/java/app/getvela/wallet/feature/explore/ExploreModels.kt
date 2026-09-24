package app.getvela.wallet.feature.explore

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import app.getvela.wallet.feature.wallet.TabsModel

/**
 * Explore view models (spec 022, data-model.md §2 — the Android port of the
 * web's `src/lib/explore/model.ts`).
 *
 * Components consume ONLY these display-ready shapes: no fetching, no URL
 * parsing, no business state. A real WebView and a dApp registry replace the
 * fixture layer that builds them and nothing else.
 */

enum class ExploreScreenState { E1, E2, E3, E4, E5, E6, E7 }

/** A site as the browser home draws it — a lettermark, never a fetched icon. */
@Immutable
data class SiteModel(
    val id: String,
    val name: String,
    val host: String,
    /** Single grapheme drawn in the avatar. */
    val letter: String,
    /** Brand colour behind the letter; the tile tints it down itself. */
    val tint: Color,
    /** Row-only second line (a group's blurb), absent in the tile grid. */
    val subtitle: String? = null,
    /** Row-only trailing text — "刚刚", "昨天". Fixture content. */
    val meta: String? = null,
)

/** The favourites grid mixes sites with the trailing "add" affordance. */
@Immutable
sealed interface TileModel {
    data class Site(val site: SiteModel) : TileModel
    data class Add(val label: String) : TileModel
}

/** `Favorites` and `Recent` are system groups: hideable, never deletable. */
enum class GroupKind { Favorites, Recent, Custom }

/** The trailing affordance on a group's header row. */
enum class GroupAction { Edit, Clear, Menu }

@Immutable
data class GroupModel(
    val id: String,
    val title: String,
    val kind: GroupKind,
    val action: GroupAction?,
    val sites: List<SiteModel>,
    val hidden: Boolean = false,
)

@Immutable
data class TabModel(
    val id: String,
    val title: String,
    val site: SiteModel?,
    val selected: Boolean,
    /** The start page's own tab — drawn with the sail, not a favicon. */
    val startPage: Boolean,
)

/**
 * The page inside the browser. FIXTURE CONTENT, not chrome: it stands in for
 * whatever site is open, so its words are the mock's and are never translated
 * (the rule spec 015 set for 大表哥). A real WebView replaces it wholesale.
 */
@Immutable
data class DemoPageModel(
    val title: String,
    val fields: List<Field>,
    val cta: String,
    /** The SITE's accent, not ours. */
    val ctaTint: Color,
) {
    @Immutable
    data class Field(val value: String, val symbol: String)
}

@Immutable
data class BrowserModel(
    val url: String,
    val host: String,
    val secure: Boolean,
    val connected: Boolean,
    val canBack: Boolean,
    val canForward: Boolean,
    val bookmarked: Boolean,
    val accountSeed: String,
    val tabCount: Int,
    val page: DemoPageModel,
    /** Spec 070: a live page's load, 0–100; drawn as a hairline under the address bar while `loading`. */
    val loading: Boolean = false,
    val progress: Int = 100,
    /** The main frame could not load (network, certificate): the retry panel stands where the page is. */
    val failed: Boolean = false,
    /** The page's renderer died: the tab shows the reload panel until the person asks. */
    val crashed: Boolean = false,
)

@Immutable
data class SiteMenuItem(
    val id: String,
    val icon: ImageVector,
    val label: String,
    val danger: Boolean = false,
)

@Immutable
data class GroupManageRow(
    val id: String,
    val title: String,
    /** "8 个网站" / "2 · 已隐藏" — resolved by the fixture layer. */
    val meta: String?,
    val system: Boolean,
    val hidden: Boolean = false,
)

@Immutable
data class ConnectionModel(
    val title: String,
    val site: SiteModel,
    val statusLine: String,
    val accountName: String,
    val accountAddress: String,
    val accountSeed: String,
    val switchLabel: String,
    val networkLabel: String,
    val networkName: String,
    val networkDot: Color,
    val explainer: String,
    val disconnect: String,
    val footnote: String,
    /** Spec 070: the lock tells the truth — `false` draws the warning, never a green padlock. */
    val secure: Boolean = true,
)

@Immutable
sealed interface ExploreSheet {
    data class GroupManage(
        val title: String,
        val rows: List<GroupManageRow>,
        val newGroup: String,
    ) : ExploreSheet

    data class SiteMenu(
        val site: SiteModel,
        val statusLine: String,
        val items: List<SiteMenuItem>,
        val secure: Boolean = true,
    ) : ExploreSheet

    data class Connection(val connection: ConnectionModel) : ExploreSheet
}

/** Which surface the screen is showing (SPEC 动效 · 探索 手机). */
enum class ExploreView { Start, Browsing, Tabs }

@Immutable
data class TabsScreenCopy(
    val title: String,
    val done: String,
    val newTab: String,
    val closeAll: String,
    val close: String,
)

@Immutable
data class ExploreEmptyCopy(val title: String, val caption: String, val cta: String)

@Immutable
data class FavoritesSection(val title: String, val action: String, val tiles: List<TileModel>)

@Immutable
data class ExploreScreenModel(
    val state: ExploreScreenState,
    val view: ExploreView,
    val title: String,
    val tabCountLabel: String?,
    val searchPlaceholder: String,
    val scanLabel: String,
    val empty: ExploreEmptyCopy?,
    val favorites: FavoritesSection?,
    val groups: List<GroupModel>,
    val browser: BrowserModel,
    val tabs: List<TabModel>,
    val tabsScreen: TabsScreenCopy,
    /** Which sheet the state opens with, if any (E3/E6/E7). */
    val sheet: ExploreSheet?,
    /**
     * The sheets browsing can raise on demand. Part of the model rather than
     * built at the tap, so a screen never invents copy at interaction time.
     */
    val groupManageSheet: ExploreSheet.GroupManage,
    val siteMenuSheet: ExploreSheet.SiteMenu,
    val connection: ConnectionModel,
    val nav: TabsModel,
)
