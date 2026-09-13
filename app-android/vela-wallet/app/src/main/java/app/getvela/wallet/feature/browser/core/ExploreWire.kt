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
}
