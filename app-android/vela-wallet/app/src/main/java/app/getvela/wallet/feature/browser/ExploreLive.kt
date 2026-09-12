package app.getvela.wallet.feature.browser

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.core.BhistEntry
import app.getvela.wallet.feature.browser.core.BhistView
import app.getvela.wallet.feature.browser.core.EngineState
import app.getvela.wallet.feature.browser.core.ExploreSite
import app.getvela.wallet.feature.browser.core.ExploreView
import app.getvela.wallet.feature.explore.ExploreEmptyCopy
import app.getvela.wallet.feature.explore.ExploreScreenModel
import app.getvela.wallet.feature.explore.ExploreSheet
import app.getvela.wallet.feature.explore.FavoritesSection
import app.getvela.wallet.feature.explore.GroupAction
import app.getvela.wallet.feature.explore.GroupKind
import app.getvela.wallet.feature.explore.GroupManageRow
import app.getvela.wallet.feature.explore.GroupModel
import app.getvela.wallet.feature.explore.SiteModel
import app.getvela.wallet.feature.explore.TabModel
import app.getvela.wallet.feature.explore.TileModel

/**
 * The explore screen from the core's views (spec 044 T019; the desktop's
 * `explore/live.rs`). The drawn model keeps only its words; favourites,
 * groups, recents, tabs and the page's chrome are this device's own.
 *
 * A site's `id` is the URL it opens — what a tap has to hand back.
 */
object ExploreLive {
    fun home(
        fallback: ExploreScreenModel,
        view: ExploreView,
        history: BhistView,
        engine: EngineState?,
        strings: VelaStrings,
    ): ExploreScreenModel {
        val favorites = view.favorites.map { tileOf(it) }
        val populated = favorites.isNotEmpty() || history.entries.isNotEmpty() || view.groups.isNotEmpty()
        val groups = buildList {
            if (!view.recent_hidden) recentGroup(history.entries, strings)?.let { add(it) }
            addAll(customGroups(view))
        }
        val tabs = view.tabs.map { tab ->
            TabModel(
                id = tab.id,
                title = tab.title.ifBlank { strings.t("explore.startPage") },
                site = tab.url?.let { url -> SiteModel(id = url, name = tab.title, host = tab.host, letter = letterOf(tab.host), tint = tintOf(tab.host)) },
                selected = view.selected_tab == tab.id,
                startPage = tab.url == null,
            )
        }
        val bookmarked = engine?.origin != null && view.favorites.any { it.origin == engine.origin }
        return fallback.copy(
            tabCountLabel = tabs.size.takeIf { it > 0 }?.toString(),
            empty = if (populated) null else ExploreEmptyCopy(strings.t("explore.startTitle"), strings.t("explore.startHint"), strings.t("explore.startCta")),
            favorites = if (favorites.isEmpty() || view.favorites_hidden) {
                null
            } else {
                FavoritesSection(
                    title = strings.t("explore.favorites"),
                    action = strings.t("explore.edit"),
                    tiles = favorites.map { TileModel.Site(it) } + TileModel.Add(strings.t("explore.add")),
                )
            },
            groups = groups,
            browser = fallback.browser.copy(
                url = engine?.url ?: fallback.browser.url,
                host = engine?.host?.ifBlank { null } ?: fallback.browser.host,
                secure = engine?.secure ?: fallback.browser.secure,
                canBack = engine?.canBack ?: false,
                canForward = engine?.canForward ?: false,
                bookmarked = bookmarked,
                tabCount = tabs.size,
            ),
            tabs = tabs,
            groupManageSheet = ExploreSheet.GroupManage(
                title = strings.t("explore.manageGroups"),
                newGroup = strings.t("explore.newGroup"),
                rows = buildList {
                    add(GroupManageRow("favorites", strings.t("explore.favorites"), strings.t("explore.siteCount", mapOf("n" to view.favorites.size.toString())), system = true, hidden = view.favorites_hidden))
                    add(GroupManageRow("recent", strings.t("explore.recent"), strings.t("explore.systemGroup"), system = true, hidden = view.recent_hidden))
                    view.groups.forEach { group ->
                        add(GroupManageRow(group.id, group.name, strings.t("explore.siteCount", mapOf("n" to group.sites.size.toString())), system = false, hidden = group.hidden))
                    }
                },
            ),
            siteMenuSheet = engine?.let { e ->
                fallback.siteMenuSheet.copy(
                    site = SiteModel(id = e.url, name = e.title.ifBlank { e.host }, host = e.host, letter = letterOf(e.host), tint = tintOf(e.host)),
                    statusLine = if (e.secure) strings.t("explore.secureSite") else e.host,
                )
            } ?: fallback.siteMenuSheet,
        )
    }

    /** A recent row shows the title over the host it actually is. */
    fun siteOf(entry: BhistEntry): SiteModel = SiteModel(
        id = entry.url,
        name = entry.title.trim().ifEmpty { entry.host },
        host = entry.host,
        letter = letterOf(entry.host),
        tint = tintOf(entry.host),
        subtitle = entry.host,
    )

    fun tileOf(site: ExploreSite): SiteModel = SiteModel(
        id = site.url,
        name = site.name,
        host = site.host,
        letter = letterOf(site.host),
        tint = tintOf(site.host),
        subtitle = site.host,
    )

    fun recentGroup(entries: List<BhistEntry>, strings: VelaStrings): GroupModel? {
        if (entries.isEmpty()) return null
        return GroupModel(
            id = "recent",
            title = strings.t("explore.recent"),
            kind = GroupKind.Recent,
            action = GroupAction.Clear,
            sites = entries.map { siteOf(it) },
        )
    }

    fun customGroups(view: ExploreView): List<GroupModel> = view.groups
        .filter { !it.hidden }
        .map { group ->
            GroupModel(
                id = group.id,
                title = group.name,
                kind = GroupKind.Custom,
                action = GroupAction.Menu,
                sites = group.sites.map { tileOf(it) },
            )
        }

    /** The first letter or digit of the host, upper-cased; `?` for none. */
    fun letterOf(host: String): String =
        host.firstOrNull { it.isLetterOrDigit() && it.code < 128 }?.uppercaseChar()?.toString() ?: "?"

    /** A stable hue per host (FNV-1a, the desktop's rule): the same site is the same colour everywhere. */
    fun tintOf(host: String): Color {
        var hash = 2_166_136_261L.toInt()
        for (byte in host.toByteArray(Charsets.UTF_8)) {
            hash = hash xor (byte.toInt() and 0xff)
            hash *= 16_777_619
        }
        val hue = ((hash.toLong() and 0xffff_ffffL) % 360L).toFloat()
        return Color.hsl(hue, 0.72f, 0.55f)
    }
}
