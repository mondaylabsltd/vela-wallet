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
    /** Spec 079: the site's own icon, best first (https only); the letter shows until one lands, and when none does. */
    val iconUrls: List<String> = emptyList(),
)

/** The favourites grid mixes sites with the trailing "add" affordance. */
@Immutable
sealed interface TileModel {
    data class Site(val site: SiteModel) : TileModel
    data class Add(val label: String) : TileModel
}

/**
 * `Favorites` and `Recent` are the start page's only groups (issue #465):
 * hideable, never deletable.
 */
enum class GroupKind { Favorites, Recent }

/** The trailing affordance on a group's header row. */
enum class GroupAction { Edit, Clear }

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
    /** Spec 079: the page as it last left the screen; the drawn stand-in when there is none yet. */
    val snapshot: androidx.compose.ui.graphics.ImageBitmap? = null,
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
    /** The page has history behind it. The bar's ‹ is never greyed: without history it goes to the home. */
    val canBack: Boolean,
    /** Drives the site menu's Forward row, greyed (never hidden) when there is nothing ahead. */
    val canForward: Boolean,
    val bookmarked: Boolean,
    val accountSeed: String,
    /** Every open tab — the bar's box, the switcher's number, the resume header's n. */
    val tabCount: Int,
    val page: DemoPageModel,
    /** Spec 070: a live page's load, 0–100; drawn as a hairline under the address bar while `loading`. */
    val loading: Boolean = false,
    val progress: Int = 100,
    /** The main frame could not load (network, certificate): the retry panel stands where the page is. */
    val failed: Boolean = false,
    /** Spec 079: why, in the core's words for its class; `null` when it did not fail. */
    val failureReason: String? = null,
    /** Spec 079: a retry is running — the panel stays and says "正在重试…". */
    val retrying: Boolean = false,
    /** Spec 079: the page's chain could not be reached — one line under the address bar; `null` when it can. */
    val chainNotice: String? = null,
    /** Spec 079: the chain notice's retry is in flight. */
    val chainAsking: Boolean = false,
    /** The page's renderer died: the tab shows the reload panel until the person asks. */
    val crashed: Boolean = false,
    /**
     * Spec 082 RE1: whether the bar shows a lock at all — none for a failure
     * panel, a load still pending in an empty tab, or an empty tab.
     */
    val lockShown: Boolean = true,
    /** Spec 099 FR-014: the shown tab's status line; `null` when it has nothing to say (or it was put away). */
    val status: BrowserStatusModel? = null,
)

/**
 * Spec 099 FR-014: one quiet line under the address bar about the shown tab —
 * it was reloaded to save memory, the wallet was not offered to its page, or
 * the latest request that ended in trouble, in the core's words for its
 * layer. [seen] is what its ✕ puts away: the line stays gone until it would
 * say something else.
 */
@Immutable
data class BrowserStatusModel(
    val seen: String,
    val text: String,
    /** A warning (the wallet not offered, a failure) rather than a note (reloaded). */
    val warning: Boolean,
    /** The Details action's label — it opens the tab's status panel. */
    val details: String,
)

/** Spec 099 FR-014: the shown tab's status panel — its page, the wallet on it, every request (newest first), and the record to copy. */
@Immutable
data class BrowserInspectorModel(
    val title: String,
    val origin: String,
    val page: String,
    val provider: String,
    val requestsTitle: String,
    val rows: List<Row>,
    /** "No requests yet", when there are none. */
    val empty: String?,
    val copyLabel: String,
    val copiedLabel: String,
    /** The core's copyable record — the same text on every client and in a bug report. */
    val report: String,
) {
    /** One request: its method (monospace) and how it ended — "✓ 120 ms", the reason's words, or "…" while open. */
    @Immutable
    data class Row(val method: String, val outcome: String)
}

@Immutable
data class SiteMenuItem(
    val id: String,
    val icon: ImageVector,
    val label: String,
    val danger: Boolean = false,
    /** Drawn dimmed and inert when `false` — a tab's "close tabs to the right" on the last tab (spec 099). */
    val enabled: Boolean = true,
)

@Immutable
data class GroupManageRow(
    val id: String,
    val title: String,
    /** "8 个网站" — resolved by the fixture layer; Recent dApps has none. */
    val meta: String?,
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
    /** Spec 079: the network's logo; the dot shows until it lands. */
    val networkLogoUrl: String? = null,
    /** Spec 079: the consent card — its action is the primary one. */
    val primaryAction: Boolean = false,
    /**
     * Spec 097 E: the core holds a grant for this site. Not connected (and not
     * asking), the panel offers nothing that implies access — no account it
     * sees, no explainer, no Disconnect, no "requests appear here".
     */
    val connected: Boolean = true,
)

/**
 * Spec 100: the add-network sheet a page opened (`NetView.dapp_add`), in words.
 * Every judgement is the core's — which chain, whose name and coin, the
 * verdict, whether Add acts; [app.getvela.wallet.feature.browser.ExploreLive.addNetwork]
 * picks Settings' own line for each part.
 */
@Immutable
data class AddNetworkModel(
    val title: String,
    /** "{{host}} asks to add a network" — who asks, from the transport. */
    val lead: String,
    val site: SiteModel,
    /** Label and value, in order; a value the core does not know yet is left out. */
    val rows: List<Pair<String, String>>,
    /** The name and coin are the site's, not Vela's catalog's. */
    val fromSite: String?,
    val pill: app.getvela.wallet.feature.settings.StatusPillModel?,
    val checksTitle: String?,
    val checks: List<app.getvela.wallet.feature.settings.CheckItemModel>,
    /** The sentence under the verdict. */
    val note: String?,
    /** "Add Network" — only where the core says it can act. */
    val add: String?,
    val retry: String?,
    /** The chain-setup tool, for a chain this wallet refuses. */
    val setupTool: String?,
    /** Cancel while a decision is open, Done after a verdict — either way `dapp_add_declined`. */
    val dismiss: String,
) {
    companion object {
        /** Where a chain this wallet refuses can be made ready (iOS and the web link the same page). */
        const val CHAIN_SETUP_URL = "https://getvela.app/chain-setup"
    }
}

/** Spec 100: what the add-network sheet's buttons say to the core. */
enum class AddNetworkAction { Approve, Decline, Retry }

@Immutable
sealed interface ExploreSheet {
    /** Issue #465: exactly two rows, Favorites and Recent dApps, each with an eye. */
    data class GroupManage(
        val title: String,
        val rows: List<GroupManageRow>,
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
    /** Spec 099: a tab's long-press menu — Chrome's batch closes. */
    val closeOthers: String,
    val closeRight: String,
)

/**
 * Spec 099: which of a tab's batch closes would close anything now — the
 * core's `explore_tabs_closed_by` asked for that tab, non-empty. The menu
 * greys out the ones that would not.
 */
@Immutable
data class TabCloses(val others: Boolean = true, val right: Boolean = true)

@Immutable
data class ExploreEmptyCopy(val title: String, val caption: String, val cta: String)

@Immutable
data class FavoritesSection(val title: String, val action: String, val tiles: List<TileModel>)

/**
 * The home's resume section (spec 099 navigation): the tabs that have a page,
 * in the core's order and cap (`ExploreView.resumable`), under a header that
 * counts every open tab and opens the switcher. Each row's `id` is its TAB's
 * id — a tap resumes that tab, live, with no reload.
 */
@Immutable
data class ResumeSection(
    /** "已打开 {{n}} 个标签页" — n is every tab, start pages included: the switcher's number. */
    val title: String,
    /** "标签页 ›" — opens the switcher. */
    val action: String,
    val tabs: List<SiteModel>,
)

@Immutable
data class ExploreScreenModel(
    val state: ExploreScreenState,
    val view: ExploreView,
    val title: String,
    val searchPlaceholder: String,
    val scanLabel: String,
    val empty: ExploreEmptyCopy?,
    /** Drawn only while some tab has a page — there is no empty or loading form of it. */
    val resume: ResumeSection?,
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
