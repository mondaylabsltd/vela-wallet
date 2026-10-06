package app.getvela.wallet.feature.browser.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/*
 * The `explore_sites` machine's wire (spec 044) — favourites, groups and
 * open tabs. `added_ms` / `created_ms` are `f64` in the core.
 */

@Serializable
enum class ExploreSystemGroup {
    @SerialName("favorites") Favorites,

    @SerialName("recent") Recent,
}

@Serializable
data class ExploreSite(
    val origin: String,
    val url: String,
    val host: String,
    val name: String,
    val renamed: Boolean = false,
    val added_ms: Double,
)

@Serializable
data class ExploreGroup(
    val id: String,
    val name: String,
    val members: List<String> = emptyList(),
    val hidden: Boolean = false,
    val created_ms: Double,
)

@Serializable
data class ExploreTab(val id: String, val url: String? = null, val title: String = "", val host: String = "")

@Serializable
data class ExploreDoc(
    val favorites: List<ExploreSite> = emptyList(),
    val groups: List<ExploreGroup> = emptyList(),
    val tabs: List<ExploreTab> = emptyList(),
    val selected_tab: String? = null,
    val hidden_system: List<ExploreSystemGroup> = emptyList(),
    /**
     * Issue #425: the rule the favourites' names were made under (the core's
     * `NAME_RULE`). This document is stored THROUGH this class, so a field
     * missing here is lost on every write — and a document that comes back
     * without it reads as one from before the rule, its names reset to hosts
     * on every launch.
     */
    val name_rule: Int = 0,
)

@Serializable
data class ExploreGroupView(val id: String, val name: String, val hidden: Boolean = false, val sites: List<ExploreSite> = emptyList())

@Serializable
data class ExploreView(
    val favorites: List<ExploreSite> = emptyList(),
    val groups: List<ExploreGroupView> = emptyList(),
    val tabs: List<ExploreTab> = emptyList(),
    val selected_tab: String? = null,
    val favorites_hidden: Boolean = false,
    val recent_hidden: Boolean = false,
    val favorites_full: Boolean = false,
    val tabs_full: Boolean = false,
    val ready: Boolean = false,
    /** Spec 099 R2: every tab id, most recently used first — what the engine plan keeps by. */
    val recent_tabs: List<String> = emptyList(),
)

@Serializable
sealed class ExploreOperation {
    @Serializable
    @SerialName("read_explore")
    data object ReadExplore : ExploreOperation()

    @Serializable
    @SerialName("write_explore")
    data class WriteExplore(val doc: ExploreDoc) : ExploreOperation()
}

@Serializable
sealed class ExploreShellResult {
    @Serializable
    @SerialName("loaded")
    data class Loaded(val doc: ExploreDoc? = null) : ExploreShellResult()

    @Serializable
    @SerialName("written")
    data object Written : ExploreShellResult()
}

@Serializable
sealed class ExploreEvent {
    @Serializable
    @SerialName("start")
    data object Start : ExploreEvent()

    @Serializable
    @SerialName("favorite_added")
    data class FavoriteAdded(val url: String, val title: String? = null, val now_ms: Double) : ExploreEvent()

    /**
     * Issue #425: a page loaded without failing — the core's visit
     * (`browserLoadVisit`), never the engine's error page. A favourite of that
     * site still named by its host takes its title.
     */
    @Serializable
    @SerialName("page_loaded")
    data class PageLoaded(val url: String, val title: String? = null) : ExploreEvent()

    @Serializable
    @SerialName("favorite_removed")
    data class FavoriteRemoved(val origin: String) : ExploreEvent()

    @Serializable
    @SerialName("favorite_renamed")
    data class FavoriteRenamed(val origin: String, val name: String) : ExploreEvent()

    @Serializable
    @SerialName("group_created")
    data class GroupCreated(val name: String, val now_ms: Double) : ExploreEvent()

    @Serializable
    @SerialName("group_renamed")
    data class GroupRenamed(val id: String, val name: String) : ExploreEvent()

    @Serializable
    @SerialName("group_deleted")
    data class GroupDeleted(val id: String) : ExploreEvent()

    @Serializable
    @SerialName("group_hidden_set")
    data class GroupHiddenSet(val id: String, val hidden: Boolean) : ExploreEvent()

    @Serializable
    @SerialName("system_group_hidden_set")
    data class SystemGroupHiddenSet(val group: ExploreSystemGroup, val hidden: Boolean) : ExploreEvent()

    @Serializable
    @SerialName("group_member_added")
    data class GroupMemberAdded(val id: String, val origin: String) : ExploreEvent()

    @Serializable
    @SerialName("group_member_removed")
    data class GroupMemberRemoved(val id: String, val origin: String) : ExploreEvent()

    @Serializable
    @SerialName("tab_opened")
    data class TabOpened(val url: String? = null, val title: String? = null, val now_ms: Double) : ExploreEvent()

    @Serializable
    @SerialName("tab_navigated")
    data class TabNavigated(val id: String, val url: String, val title: String? = null) : ExploreEvent()

    @Serializable
    @SerialName("tab_selected")
    data class TabSelected(val id: String) : ExploreEvent()

    @Serializable
    @SerialName("tab_closed")
    data class TabClosed(val id: String) : ExploreEvent()

    /**
     * Spec 099: close several at once — the ids [BrowserTabs.closedBy] names
     * for a [TabCloseScope]. One write; the selection follows the core's rule
     * (a survivor stays; a closed one moves to the nearest surviving tab on
     * its right, else its left; none left is the start page).
     */
    @Serializable
    @SerialName("tabs_closed")
    data class TabsClosed(val ids: List<String>) : ExploreEvent()
}

/** Spec 099: what a batch close takes — the core's `explore_sites::TabCloseScope`. */
@Serializable
sealed class TabCloseScope {
    /** Every tab but this one ("close other tabs"). */
    @Serializable
    @SerialName("others")
    data class Others(val keep: String) : TabCloseScope()

    /** Every tab to the right of this one in the strip. */
    @Serializable
    @SerialName("right")
    data class Right(val of: String) : TabCloseScope()

    /** Every tab. */
    @Serializable
    @SerialName("all")
    data object All : TabCloseScope()
}
